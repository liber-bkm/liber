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
}

pub fn bookmark_matches(b: &Bookmark, query: &str, fields: &SearchFields) -> bool {
    let q = query.to_lowercase();
    let all = !fields.any();
    if (all || fields.title) && b.title.to_lowercase().contains(&q) {
        return true;
    }
    if (all || fields.url) && b.url.to_lowercase().contains(&q) {
        return true;
    }
    if (all || fields.folder) && b.folder.to_lowercase().contains(&q) {
        return true;
    }
    if (all || fields.description) && b.description.to_lowercase().contains(&q) {
        return true;
    }
    if all || fields.tags {
        for t in &b.tags {
            if t.to_lowercase().contains(&q) {
                return true;
            }
        }
    }
    false
}

fn rank_score(b: &Bookmark, q: &str, fields: &SearchFields) -> u8 {
    let all = !fields.any();
    let title = b.title.to_lowercase();
    if (all || fields.title) && title.starts_with(q) {
        return 0;
    }
    if (all || fields.title) && title.contains(q) {
        return 1;
    }
    2
}

pub fn order_results(
    mut list: Vec<Bookmark>,
    query: &str,
    fields: &SearchFields,
    sort: SortMode,
) -> Vec<Bookmark> {
    let q = query.to_lowercase();
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
                    rank_score(a, &q, fields)
                        .cmp(&rank_score(b, &q, fields))
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
    let content = builder.add_text_field("content", TEXT);
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
            Index::open_in_dir(dir).map_err(map_err)?
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
}

fn map_err<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Storage(e.to_string())
}

pub fn deep_search_uuids(
    store: &crate::store::Store,
    query: &str,
    limit: usize,
) -> Result<Vec<Uuid>, CoreError> {
    let dir = store.cfg.tantivy_dir();
    if !dir.join("meta.json").exists() {
        return Ok(Vec::new());
    }
    let index = SearchIndex::open_or_create(&dir)?;
    Ok(index
        .search(query, limit)?
        .into_iter()
        .map(|(uuid, _)| uuid)
        .collect())
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
        }
    }

    #[test]
    fn sort_modes() {
        assert_eq!(parse_sort_mode("newest").unwrap(), SortMode::Newest);
        assert_eq!(parse_sort_mode("").unwrap(), SortMode::Relevance);
        assert!(parse_sort_mode("bogus").is_err());
    }

    #[test]
    fn field_match_scoping() {
        let mut b = bookmark("Rust guide", "https://example.com/rust");
        b.tags = vec!["prog".to_string()];
        let scoped = SearchFields {
            title: true,
            ..Default::default()
        };
        assert!(bookmark_matches(&b, "rust", &scoped));
        assert!(!bookmark_matches(&b, "prog", &scoped));
        assert!(bookmark_matches(&b, "prog", &SearchFields::all()));
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
}
