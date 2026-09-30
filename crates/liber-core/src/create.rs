use uuid::Uuid;

use crate::dedupe::normalize_for_dedupe;
use crate::model::{Bookmark, NewBookmark};
use crate::render::{write_html_bookmark, write_markdown_bookmark};
use crate::slug::{dedupe_strings, normalize_url, sanitize_folder, slug_or_fallback};
use crate::store::Store;
use crate::CoreError;

#[derive(Debug, Clone, Default)]
pub struct CreateOptions {
    pub title: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub folder: String,
    pub markdown: bool,
}

fn rel_path(folder: &str, file: &str) -> String {
    if folder.is_empty() {
        file.to_string()
    } else {
        format!("{folder}/{file}")
    }
}

pub fn create_bookmark(
    store: &mut Store,
    raw_url: &str,
    opts: CreateOptions,
) -> Result<Bookmark, CoreError> {
    let url = normalize_url(raw_url);
    if store.find_by_url(&url)?.is_some() {
        return Err(CoreError::Duplicate(normalize_for_dedupe(&url)));
    }
    let folder = sanitize_folder(&opts.folder);
    let tags = dedupe_strings(opts.tags);
    let title = opts
        .title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.clone());
    let uuid = Uuid::new_v4();
    let base = format!("{uuid}-{}", slug_or_fallback(&title));
    let html_rel = rel_path(&folder, &format!("{base}.html"));
    let md_rel = opts
        .markdown
        .then(|| rel_path(&folder, &format!("{base}.md")));

    let preview = Bookmark {
        uuid,
        url: url.clone(),
        title: title.clone(),
        description: opts.description.clone(),
        tags: tags.clone(),
        folder: folder.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        html_file: html_rel.clone(),
        markdown_file: md_rel.clone(),
        archive_file: None,
        attachments: vec![],
        open_count: 0,
        last_opened_at: None,
        last_checked_at: None,
        check_status: None,
        applied_rules: vec![],
    };
    write_html_bookmark(&store.cfg.html_dir().join(&html_rel), &preview)
        .map_err(|e| CoreError::Storage(format!("writing html bookmark: {e}")))?;
    if let Some(rel) = &md_rel {
        write_markdown_bookmark(&store.cfg.markdown_dir().join(rel), &preview)
            .map_err(|e| CoreError::Storage(format!("writing markdown bookmark: {e}")))?;
    }

    store.add_bookmark(NewBookmark {
        uuid,
        url,
        title,
        description: opts.description,
        tags,
        folder,
        html_file: html_rel,
        markdown_file: md_rel,
        archive_file: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Config;

    fn mem_store(base: &std::path::Path) -> Store {
        Store::open_in_memory(Config {
            base_dir: base.to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn create_writes_files_and_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = create_bookmark(
            &mut s,
            "example.com/page",
            CreateOptions {
                title: Some("My Page".to_string()),
                tags: vec!["A".to_string(), "a".to_string()],
                folder: "tech\\rust".to_string(),
                markdown: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(b.url, "https://example.com/page");
        assert_eq!(b.folder, "tech/rust");
        assert_eq!(b.tags, vec!["A".to_string()]);
        assert!(b.html_file.ends_with("-my-page.html"));
        assert!(s.cfg.html_dir().join(&b.html_file).exists());
        let md = b.markdown_file.clone().unwrap();
        assert!(s.cfg.markdown_dir().join(&md).exists());
        assert!(s.get(&b.uuid).unwrap().is_some());
    }

    #[test]
    fn create_duplicate_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        create_bookmark(&mut s, "https://example.com/", CreateOptions::default()).unwrap();
        let err = create_bookmark(
            &mut s,
            "https://example.com?utm_source=x",
            CreateOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Duplicate(_)));
    }
}
