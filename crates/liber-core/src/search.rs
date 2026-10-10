use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use uuid::Uuid;

use crate::model::Bookmark;
use crate::CoreError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    Relevance,
    Newest,
    Oldest,
    Visited,
    Title,
}

pub fn parse_sort_mode(s: &str) -> Result<SortMode, CoreError> {
    match s.to_lowercase().trim() {
        "" => Ok(SortMode::Relevance),
        "newest" => Ok(SortMode::Newest),
        "oldest" => Ok(SortMode::Oldest),
        "visited" => Ok(SortMode::Visited),
        "title" => Ok(SortMode::Title),
        other => Err(CoreError::Invalid(format!(
            "unknown sort {other:?} (expected newest, oldest, visited, or title)"
        ))),
    }
}

pub fn resolve_sort_mode(sort: Option<&str>, query: Option<&str>) -> Result<SortMode, CoreError> {
    match sort.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => parse_sort_mode(s),
        None if query.map(str::trim).filter(|q| !q.is_empty()).is_some() => Ok(SortMode::Relevance),
        None => Ok(SortMode::Newest),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SearchFields {
    pub title: bool,
    pub url: bool,
    pub tags: bool,
    pub folder: bool,
    pub description: bool,
}

impl SearchFields {
    pub fn all() -> Self {
        Self::default()
    }

    pub fn any(&self) -> bool {
        self.title || self.url || self.tags || self.folder || self.description
    }

    fn scoped(&self) -> Vec<FieldScope> {
        let mut out = Vec::new();
        if self.title {
            out.push(FieldScope::Title);
        }
        if self.url {
            out.push(FieldScope::Url);
        }
        if self.description {
            out.push(FieldScope::Description);
        }
        if self.folder {
            out.push(FieldScope::Folder);
        }
        if self.tags {
            out.push(FieldScope::Tags);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldScope {
    Title,
    Url,
    Tags,
    Folder,
    Description,
}

impl FieldScope {
    pub fn from_prefix(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "title" | "n" => Some(Self::Title),
            "url" | "u" => Some(Self::Url),
            "tag" | "tags" | "t" => Some(Self::Tags),
            "folder" | "f" => Some(Self::Folder),
            "desc" | "description" | "d" => Some(Self::Description),
            _ => None,
        }
    }

    pub fn tantivy_field(&self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Url => "url",
            Self::Tags => "tags",
            Self::Folder => "folder",
            Self::Description => "description",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryTerm {
    pub scope: Option<FieldScope>,
    pub text: String,
}

fn split_terms(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in q.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_token(tok: &str) -> QueryTerm {
    if let Some((pre, val)) = tok.split_once(':') {
        if !pre.is_empty() {
            if let Some(scope) = FieldScope::from_prefix(pre) {
                let text = val.trim().to_string();
                if !text.is_empty() {
                    return QueryTerm {
                        scope: Some(scope),
                        text,
                    };
                }
            }
        }
    }
    QueryTerm {
        scope: None,
        text: tok.to_string(),
    }
}

pub fn parse_query_terms(q: &str) -> Vec<QueryTerm> {
    split_terms(q).iter().map(|t| parse_token(t)).collect()
}

pub fn parse_field_list(s: &str) -> Result<SearchFields, CoreError> {
    let mut fields = SearchFields::default();
    let mut any = false;
    for tok in s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
    {
        match FieldScope::from_prefix(tok) {
            Some(FieldScope::Title) => fields.title = true,
            Some(FieldScope::Url) => fields.url = true,
            Some(FieldScope::Tags) => fields.tags = true,
            Some(FieldScope::Folder) => fields.folder = true,
            Some(FieldScope::Description) => fields.description = true,
            None => {
                return Err(CoreError::Invalid(format!(
                    "unknown search field {tok:?} (expected title, url, tag, folder, desc, or Go letters n, u, t, d, f)"
                )))
            }
        }
        any = true;
    }
    if !any {
        return Err(CoreError::Invalid(
            "empty field list (expected title, url, tag, folder, or desc)".to_string(),
        ));
    }
    Ok(fields)
}

fn field_contains(b: &Bookmark, scope: FieldScope, q: &str) -> bool {
    match scope {
        FieldScope::Title => b.title.to_lowercase().contains(q),
        FieldScope::Url => b.url.to_lowercase().contains(q),
        FieldScope::Folder => b.folder.to_lowercase().contains(q),
        FieldScope::Description => b.description.to_lowercase().contains(q),
        FieldScope::Tags => b.tags.iter().any(|t| t.to_lowercase().contains(q)),
    }
}

fn term_matches(b: &Bookmark, term: &QueryTerm, default: &SearchFields) -> bool {
    let q = term.text.to_lowercase();
    match term.scope {
        Some(scope) => field_contains(b, scope, &q),
        None if !default.any() => {
            field_contains(b, FieldScope::Title, &q)
                || field_contains(b, FieldScope::Url, &q)
                || field_contains(b, FieldScope::Folder, &q)
                || field_contains(b, FieldScope::Description, &q)
                || field_contains(b, FieldScope::Tags, &q)
        }
        None => {
            (default.title && field_contains(b, FieldScope::Title, &q))
                || (default.url && field_contains(b, FieldScope::Url, &q))
                || (default.folder && field_contains(b, FieldScope::Folder, &q))
                || (default.description && field_contains(b, FieldScope::Description, &q))
                || (default.tags && field_contains(b, FieldScope::Tags, &q))
        }
    }
}

pub fn bookmark_matches_query(b: &Bookmark, query: &str, default: &SearchFields) -> bool {
    let terms = parse_query_terms(query);
    if terms.is_empty() {
        return true;
    }
    terms.iter().all(|t| term_matches(b, t, default))
}

fn rank_text(terms: &[QueryTerm]) -> String {
    let free: Vec<&str> = terms
        .iter()
        .filter(|t| t.scope.is_none())
        .map(|t| t.text.as_str())
        .collect();
    let joined = if free.is_empty() {
        terms
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        free.join(" ")
    };
    joined.to_lowercase()
}

fn rank_score(b: &Bookmark, q: &str) -> u8 {
    let title = b.title.to_lowercase();
    if title.starts_with(q) {
        return 0;
    }
    if title.contains(q) {
        return 1;
    }
    2
}

fn escape_tantivy(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub fn scoped_tantivy_query(query: &str, default: &SearchFields) -> String {
    let mut parts = Vec::new();
    for term in parse_query_terms(query) {
        let lit = escape_tantivy(&term.text);
        match term.scope {
            Some(scope) => parts.push(format!("{}:{lit}", scope.tantivy_field())),
            None if !default.any() => parts.push(lit),
            None => {
                let ors: Vec<String> = default
                    .scoped()
                    .iter()
                    .map(|scope| format!("{}:{lit}", scope.tantivy_field()))
                    .collect();
                if ors.len() == 1 {
                    parts.push(ors.into_iter().next().unwrap_or(lit));
                } else {
                    parts.push(format!("({})", ors.join(" OR ")));
                }
            }
        }
    }
    parts.join(" AND ")
}

pub fn order_results(mut list: Vec<Bookmark>, query: &str, sort: SortMode) -> Vec<Bookmark> {
    let q = rank_text(&parse_query_terms(query));
    match sort {
        SortMode::Newest => list.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.uuid.cmp(&b.uuid))
        }),
        SortMode::Oldest => list.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.uuid.cmp(&b.uuid))
        }),
        SortMode::Visited => list.sort_by(|a, b| match (&a.last_opened_at, &b.last_opened_at) {
            (None, None) => a.uuid.cmp(&b.uuid),
            (None, _) => std::cmp::Ordering::Greater,
            (_, None) => std::cmp::Ordering::Less,
            (Some(x), Some(y)) => y.cmp(x).then_with(|| a.uuid.cmp(&b.uuid)),
        }),
        SortMode::Title => list.sort_by(|a, b| {
            a.title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then_with(|| a.uuid.cmp(&b.uuid))
        }),
        SortMode::Relevance => {
            if q.is_empty() {
                list.sort_by_key(|a| a.uuid);
            } else {
                list.sort_by(|a, b| {
                    rank_score(a, &q)
                        .cmp(&rank_score(b, &q))
                        .then_with(|| a.uuid.cmp(&b.uuid))
                });
            }
        }
    }
    list
}

fn build_schema() -> (Schema, Field, Field, Field, Field, Field, Field, Field) {
    let mut builder = Schema::builder();
    let uuid = builder.add_text_field("uuid", STRING | STORED);
    let title = builder.add_text_field("title", TEXT);
    let description = builder.add_text_field("description", TEXT);
    let url = builder.add_text_field("url", TEXT);
    let tags = builder.add_text_field("tags", TEXT);
    let folder = builder.add_text_field("folder", TEXT);
    let content = builder.add_text_field("content", TEXT | STORED);
    (
        builder.build(),
        uuid,
        title,
        description,
        url,
        tags,
        folder,
        content,
    )
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    uuid: Field,
    title: Field,
    description: Field,
    url: Field,
    tags: Field,
    folder: Field,
    content: Field,
}

impl SearchIndex {
    fn wrap(index: Index) -> Result<Self, CoreError> {
        let schema = index.schema();
        let uuid = schema.get_field("uuid").map_err(map_err)?;
        let title = schema.get_field("title").map_err(map_err)?;
        let description = schema.get_field("description").map_err(map_err)?;
        let url = schema.get_field("url").map_err(map_err)?;
        let tags = schema.get_field("tags").map_err(map_err)?;
        let folder = schema.get_field("folder").map_err(map_err)?;
        let content = schema.get_field("content").map_err(map_err)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(map_err)?;
        Ok(Self {
            index,
            reader,
            uuid,
            title,
            description,
            url,
            tags,
            folder,
            content,
        })
    }

    pub fn open_or_create(dir: &Path) -> Result<Self, CoreError> {
        let (schema, _, _, _, _, _, _, _) = build_schema();
        let index = if dir.join("meta.json").exists() {
            let existing = Index::open_in_dir(dir).map_err(map_err)?;
            if existing.schema() != schema {
                drop(existing);
                std::fs::remove_dir_all(dir)
                    .map_err(|e| CoreError::Storage(format!("migrating index: {e}")))?;
                std::fs::create_dir_all(dir)
                    .map_err(|e| CoreError::Storage(format!("migrating index: {e}")))?;
                Index::create_in_dir(dir, schema).map_err(map_err)?
            } else {
                existing
            }
        } else {
            std::fs::create_dir_all(dir).map_err(|e| CoreError::Storage(e.to_string()))?;
            Index::create_in_dir(dir, schema).map_err(map_err)?
        };
        Self::wrap(index)
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        let (schema, _, _, _, _, _, _, _) = build_schema();
        Self::wrap(Index::create_in_ram(schema))
    }

    fn writer(&self) -> Result<IndexWriter, CoreError> {
        self.index.writer(15_000_000).map_err(map_err)
    }

    pub fn index_bookmark(&self, b: &Bookmark, content: &str) -> Result<(), CoreError> {
        let mut writer = self.writer()?;
        let uuid_str = b.uuid.to_string();
        writer.delete_term(tantivy::Term::from_field_text(self.uuid, &uuid_str));
        writer
            .add_document(doc!(
                self.uuid => uuid_str,
                self.title => b.title.clone(),
                self.description => b.description.clone(),
                self.url => b.url.clone(),
                self.tags => b.tags.join(" "),
                self.folder => b.folder.clone(),
                self.content => content.to_string(),
            ))
            .map_err(map_err)?;
        writer.commit().map_err(map_err)?;
        self.reader.reload().map_err(map_err)?;
        Ok(())
    }

    pub fn rebuild_all(&self, docs: &[(Bookmark, String)]) -> Result<(), CoreError> {
        let mut writer = self.writer()?;
        writer.delete_all_documents().map_err(map_err)?;
        for (b, content) in docs {
            let uuid_str = b.uuid.to_string();
            writer
                .add_document(doc!(
                    self.uuid => uuid_str,
                    self.title => b.title.clone(),
                    self.description => b.description.clone(),
                    self.url => b.url.clone(),
                    self.tags => b.tags.join(" "),
                    self.folder => b.folder.clone(),
                    self.content => content.clone(),
                ))
                .map_err(map_err)?;
        }
        writer.commit().map_err(map_err)?;
        self.reader.reload().map_err(map_err)?;
        Ok(())
    }

    pub fn delete_bookmark(&self, uuid: &Uuid) -> Result<(), CoreError> {
        let mut writer = self.writer()?;
        writer.delete_term(tantivy::Term::from_field_text(self.uuid, &uuid.to_string()));
        writer.commit().map_err(map_err)?;
        self.reader.reload().map_err(map_err)?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(Uuid, f32)>, CoreError> {
        self.reader.reload().map_err(map_err)?;
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![
                self.title,
                self.description,
                self.url,
                self.tags,
                self.folder,
                self.content,
            ],
        );
        let q = parser.parse_query(query).map_err(map_err)?;
        let top = searcher
            .search(&q, &TopDocs::with_limit(limit))
            .map_err(map_err)?;
        let mut out = Vec::new();
        for (score, addr) in top {
            let doc: tantivy::TantivyDocument = searcher.doc(addr).map_err(map_err)?;
            let val = doc
                .get_first(self.uuid)
                .and_then(|v| v.as_str())
                .ok_or_else(|| CoreError::Storage("index doc missing uuid".to_string()))?;
            let uuid: Uuid = val
                .parse()
                .map_err(|e| CoreError::Storage(format!("bad uuid in index: {e}")))?;
            out.push((uuid, score));
        }
        Ok(out)
    }

    pub fn search_with_snippets(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(Uuid, f32, String)>, CoreError> {
        self.reader.reload().map_err(map_err)?;
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![
                self.title,
                self.description,
                self.url,
                self.tags,
                self.folder,
                self.content,
            ],
        );
        let q = parser.parse_query(query).map_err(map_err)?;
        let top = searcher
            .search(&q, &TopDocs::with_limit(limit))
            .map_err(map_err)?;
        let mut gen =
            tantivy::SnippetGenerator::create(&searcher, &q, self.content).map_err(map_err)?;
        gen.set_max_num_chars(200);
        let mut out = Vec::new();
        for (score, addr) in top {
            let doc: tantivy::TantivyDocument = searcher.doc(addr).map_err(map_err)?;
            let val = doc
                .get_first(self.uuid)
                .and_then(|v| v.as_str())
                .ok_or_else(|| CoreError::Storage("index doc missing uuid".to_string()))?;
            let uuid: Uuid = val
                .parse()
                .map_err(|e| CoreError::Storage(format!("bad uuid in index: {e}")))?;
            let fragment = gen.snippet_from_doc(&doc).to_html();
            out.push((uuid, score, fragment));
        }
        Ok(out)
    }
}

fn map_err<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Storage(e.to_string())
}

pub fn deep_search_uuids(
    store: &crate::store::Store,
    query: &str,
    limit: usize,
    default: &SearchFields,
) -> Result<Vec<Uuid>, CoreError> {
    let dir = store.cfg.tantivy_dir();
    if !dir.join("meta.json").exists() {
        return Ok(Vec::new());
    }
    let index = SearchIndex::open_or_create(&dir)?;
    Ok(index
        .search(&scoped_tantivy_query(query, default), limit)?
        .into_iter()
        .map(|(uuid, _)| uuid)
        .collect())
}

pub fn deep_search_with_snippets(
    store: &crate::store::Store,
    query: &str,
    limit: usize,
    default: &SearchFields,
) -> Result<Vec<(Uuid, f32, String)>, CoreError> {
    let dir = store.cfg.tantivy_dir();
    if !dir.join("meta.json").exists() {
        return Ok(Vec::new());
    }
    let index = SearchIndex::open_or_create(&dir)?;
    index.search_with_snippets(&scoped_tantivy_query(query, default), limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    fn bookmark(title: &str, url: &str) -> Bookmark {
        let now = Utc::now();
        Bookmark {
            uuid: Uuid::new_v4(),
            url: url.to_string(),
            title: title.to_string(),
            description: String::new(),
            tags: vec![],
            folder: String::new(),
            created_at: now,
            updated_at: now,
            html_file: "x.html".to_string(),
            markdown_file: None,
            archive_file: None,
            attachments: vec![],
            open_count: 0,
            last_opened_at: None,
            last_checked_at: None,
            check_status: None,
            applied_rules: vec![],
            short_id: None,
        }
    }

    #[test]
    fn sort_modes() {
        assert_eq!(parse_sort_mode("newest").unwrap(), SortMode::Newest);
        assert_eq!(parse_sort_mode("").unwrap(), SortMode::Relevance);
        assert!(parse_sort_mode("bogus").is_err());
    }

    #[test]
    fn resolve_sort_mode_matrix() {
        assert_eq!(resolve_sort_mode(None, None).unwrap(), SortMode::Newest);
        assert_eq!(resolve_sort_mode(None, Some("")).unwrap(), SortMode::Newest);
        assert_eq!(
            resolve_sort_mode(None, Some("   ")).unwrap(),
            SortMode::Newest
        );
        assert_eq!(
            resolve_sort_mode(None, Some("rust")).unwrap(),
            SortMode::Relevance
        );
        assert_eq!(
            resolve_sort_mode(Some("oldest"), Some("rust")).unwrap(),
            SortMode::Oldest
        );
        assert_eq!(
            resolve_sort_mode(Some("  "), None).unwrap(),
            SortMode::Newest
        );
        assert!(resolve_sort_mode(Some("bogus"), None).is_err());
    }

    #[test]
    fn field_match_scoping() {
        let mut b = bookmark("Rust guide", "https://example.com/rust");
        b.tags = vec!["prog".to_string()];
        let scoped = SearchFields {
            title: true,
            ..Default::default()
        };
        assert!(bookmark_matches_query(&b, "rust", &scoped));
        assert!(!bookmark_matches_query(&b, "prog", &scoped));
        assert!(bookmark_matches_query(&b, "prog", &SearchFields::all()));
    }

    #[test]
    fn query_prefix_parsing() {
        use FieldScope::*;
        let terms = parse_query_terms("title:rust url:example.com plain");
        assert_eq!(
            terms,
            vec![
                QueryTerm {
                    scope: Some(Title),
                    text: "rust".to_string()
                },
                QueryTerm {
                    scope: Some(Url),
                    text: "example.com".to_string()
                },
                QueryTerm {
                    scope: None,
                    text: "plain".to_string()
                },
            ]
        );
        assert_eq!(
            parse_query_terms("t:x f:y d:z n:w u:v"),
            vec![
                QueryTerm {
                    scope: Some(Tags),
                    text: "x".to_string()
                },
                QueryTerm {
                    scope: Some(Folder),
                    text: "y".to_string()
                },
                QueryTerm {
                    scope: Some(Description),
                    text: "z".to_string()
                },
                QueryTerm {
                    scope: Some(Title),
                    text: "w".to_string()
                },
                QueryTerm {
                    scope: Some(Url),
                    text: "v".to_string()
                },
            ]
        );
        assert_eq!(
            parse_query_terms("tags:rust desc:\"systems language\""),
            vec![
                QueryTerm {
                    scope: Some(Tags),
                    text: "rust".to_string()
                },
                QueryTerm {
                    scope: Some(Description),
                    text: "systems language".to_string()
                },
            ]
        );
        assert_eq!(
            parse_query_terms("TAG:Rust"),
            vec![QueryTerm {
                scope: Some(Tags),
                text: "Rust".to_string()
            }]
        );
        assert_eq!(
            parse_query_terms("bogus:x title:"),
            vec![
                QueryTerm {
                    scope: None,
                    text: "bogus:x".to_string()
                },
                QueryTerm {
                    scope: None,
                    text: "title:".to_string()
                },
            ]
        );
        assert!(parse_query_terms("   ").is_empty());
        assert!(parse_query_terms("").is_empty());
    }

    #[test]
    fn scoped_matching_is_conjunctive() {
        let mut b = bookmark("Rust guide", "https://other.com/rust");
        b.tags = vec!["prog".to_string()];
        b.folder = "tech".to_string();
        let all = SearchFields::all();
        assert!(bookmark_matches_query(&b, "tag:prog folder:tech", &all));
        assert!(!bookmark_matches_query(&b, "tag:prog folder:other", &all));
        assert!(!bookmark_matches_query(&b, "title:pasta", &all));
        assert!(bookmark_matches_query(&b, "url:other.com rust", &all));
        assert!(!bookmark_matches_query(&b, "url:example.com rust", &all));
    }

    #[test]
    fn field_list_parsing() {
        let f = parse_field_list("title,url").unwrap();
        assert!(f.title && f.url && !f.tags && !f.folder && !f.description);
        let g = parse_field_list("n u t d f").unwrap();
        assert!(g.title && g.url && g.tags && g.description && g.folder);
        let h = parse_field_list("tags,desc").unwrap();
        assert!(h.tags && h.description);
        assert!(parse_field_list("bogus").is_err());
        assert!(parse_field_list("").is_err());
    }

    #[test]
    fn tantivy_scoped_translation() {
        let all = SearchFields::all();
        assert_eq!(scoped_tantivy_query("rust", &all), "\"rust\"");
        assert_eq!(
            scoped_tantivy_query("title:rust tag:prog", &all),
            "title:\"rust\" AND tags:\"prog\""
        );
        let title_only = SearchFields {
            title: true,
            ..Default::default()
        };
        assert_eq!(scoped_tantivy_query("rust", &title_only), "title:\"rust\"");
        let two = parse_field_list("title,url").unwrap();
        assert_eq!(
            scoped_tantivy_query("rust", &two),
            "(title:\"rust\" OR url:\"rust\")"
        );
    }

    #[test]
    fn tantivy_snippets_highlight_matches() {
        let idx = SearchIndex::open_in_memory().unwrap();
        let a = bookmark("Rust programming guide", "https://example.com/rust");
        idx.index_bookmark(&a, "systems language borrow checker triumphs here")
            .unwrap();
        let hits = idx.search_with_snippets("borrow", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, a.uuid);
        assert!(
            hits[0].2.contains("<b>borrow</b>"),
            "fragment: {}",
            hits[0].2
        );
        let plain = hits[0].2.replace("<b>", "").replace("</b>", "");
        assert!(
            !plain.contains('<') && !plain.contains('>'),
            "unescaped html: {plain}"
        );
    }

    #[test]
    fn tantivy_roundtrip() {
        let idx = SearchIndex::open_in_memory().unwrap();
        let a = bookmark("Rust programming guide", "https://example.com/rust");
        let b = bookmark("Cooking recipes", "https://example.com/food");
        idx.index_bookmark(&a, "systems language borrow checker")
            .unwrap();
        idx.index_bookmark(&b, "pasta sauce").unwrap();
        let hits = idx.search("borrow", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, a.uuid);
        idx.delete_bookmark(&a.uuid).unwrap();
        assert!(idx.search("borrow", 10).unwrap().is_empty());
    }

    #[test]
    fn tantivy_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let idx = SearchIndex::open_or_create(dir.path()).unwrap();
        let a = bookmark("Hello world", "https://example.com/hi");
        idx.index_bookmark(&a, "").unwrap();
        drop(idx);
        let idx2 = SearchIndex::open_or_create(dir.path()).unwrap();
        let hits = idx2.search("hello", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, a.uuid);
    }

    #[test]
    fn deep_search_respects_index_presence() {
        use crate::create::{create_bookmark, CreateOptions};
        use crate::store::{Config, Store};
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap();
        assert!(
            deep_search_uuids(&store, "anything", 10, &SearchFields::all())
                .unwrap()
                .is_empty()
        );
        let b = create_bookmark(
            &mut store,
            "https://example.com/a",
            CreateOptions {
                title: Some("Plain title".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let index = SearchIndex::open_or_create(&store.cfg.tantivy_dir()).unwrap();
        index.index_bookmark(&b, "obscurecontentword").unwrap();
        let hits =
            deep_search_uuids(&store, "obscurecontentword", 10, &SearchFields::all()).unwrap();
        assert_eq!(hits, vec![b.uuid]);
        assert!(
            deep_search_uuids(&store, "nomatchword", 10, &SearchFields::all())
                .unwrap()
                .is_empty()
        );
        let scoped =
            deep_search_uuids(&store, "title:obscurecontentword", 10, &SearchFields::all())
                .unwrap();
        assert!(scoped.is_empty());
    }
}
