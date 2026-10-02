use std::path::Path;

use uuid::Uuid;

use crate::model::Bookmark;
use crate::render::{write_html_bookmark, write_markdown_bookmark};
use crate::slug::{dedupe_strings, normalize_url, sanitize_folder};
use crate::store::Store;
use crate::CoreError;

#[derive(Debug, Clone, Default)]
pub struct EditOptions {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub folder: Option<String>,
    pub url: Option<String>,
    pub add_markdown: bool,
}

fn move_file(src: &Path, dst: &Path) -> Result<(), CoreError> {
    if src == dst || !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    std::fs::rename(src, dst).map_err(|e| CoreError::Storage(e.to_string()))
}

fn join_folder(folder: &str, base: &str) -> String {
    if folder.is_empty() {
        base.to_string()
    } else {
        format!("{folder}/{base}")
    }
}

fn file_base(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

fn trim_ext(base: &str) -> String {
    match base.rfind('.') {
        Some(i) => base[..i].to_string(),
        None => base.to_string(),
    }
}

pub fn edit_bookmark(
    store: &mut Store,
    uuid: &Uuid,
    opts: EditOptions,
) -> Result<Bookmark, CoreError> {
    let mut b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;

    if let Some(url) = opts.url {
        let url = normalize_url(&url);
        if let Some(other) = store.find_by_url(&url)? {
            if other.uuid != *uuid {
                return Err(CoreError::Duplicate(other.uuid.to_string()));
            }
        }
        b.url = url;
    }
    if let Some(title) = opts.title {
        if !title.trim().is_empty() {
            b.title = title.trim().to_string();
        }
    }
    if let Some(description) = opts.description {
        b.description = description;
    }
    if let Some(tags) = opts.tags {
        b.tags = dedupe_strings(tags);
    }

    let mut folder_changed = false;
    if let Some(folder) = opts.folder {
        let folder = sanitize_folder(&folder);
        folder_changed = folder != b.folder;
        b.folder = folder;
    }

    if folder_changed {
        move_bookmark_files(store, &mut b)?;
    }

    rewrite_bookmark_files(store, &mut b, opts.add_markdown)?;

    store.update_bookmark(&b)?;
    Ok(b)
}

pub(crate) fn move_bookmark_files(store: &Store, b: &mut Bookmark) -> Result<(), CoreError> {
    if !b.html_file.is_empty() {
        let rel = join_folder(&b.folder, &file_base(&b.html_file));
        move_file(
            &store.cfg.html_dir().join(&b.html_file),
            &store.cfg.html_dir().join(&rel),
        )?;
        b.html_file = rel;
    }
    if let Some(rel) = b.markdown_file.clone() {
        let next = join_folder(&b.folder, &file_base(&rel));
        move_file(
            &store.cfg.markdown_dir().join(&rel),
            &store.cfg.markdown_dir().join(&next),
        )?;
        b.markdown_file = Some(next);
    }
    if let Some(rel) = b.archive_file.clone() {
        let next = join_folder(&b.folder, &file_base(&rel));
        move_file(
            &store.cfg.archive_dir().join(&rel),
            &store.cfg.archive_dir().join(&next),
        )?;
        b.archive_file = Some(next);
    }
    Ok(())
}

pub(crate) fn rewrite_bookmark_files(
    store: &Store,
    b: &mut Bookmark,
    add_markdown: bool,
) -> Result<(), CoreError> {
    if !b.html_file.is_empty() {
        write_html_bookmark(&store.cfg.html_dir().join(&b.html_file), b)?;
    }
    if let Some(rel) = b.markdown_file.clone() {
        write_markdown_bookmark(&store.cfg.markdown_dir().join(&rel), b)?;
    }
    if add_markdown && b.markdown_file.is_none() && !b.html_file.is_empty() {
        let rel = join_folder(
            &b.folder,
            &format!("{}.md", trim_ext(&file_base(&b.html_file))),
        );
        write_markdown_bookmark(&store.cfg.markdown_dir().join(&rel), b)?;
        b.markdown_file = Some(rel);
    }
    Ok(())
}

pub fn delete_bookmark_with_files(store: &mut Store, uuid: &Uuid) -> Result<bool, CoreError> {
    let Some(b) = store.get(uuid)? else {
        return Ok(false);
    };
    if !b.html_file.is_empty() {
        let _ = std::fs::remove_file(store.cfg.html_dir().join(&b.html_file));
    }
    if let Some(rel) = &b.markdown_file {
        let _ = std::fs::remove_file(store.cfg.markdown_dir().join(rel));
    }
    if let Some(rel) = &b.archive_file {
        let _ = std::fs::remove_file(store.cfg.archive_dir().join(rel));
    }
    for at in &b.attachments {
        let _ = std::fs::remove_file(store.cfg.attachment_dir().join(&at.path));
    }
    store.delete_bookmark(uuid)
}

pub fn read_note_body(store: &Store, uuid: &Uuid) -> Result<Option<String>, CoreError> {
    let Some(b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let Some(rel) = &b.markdown_file else {
        return Ok(None);
    };
    let path = store.cfg.markdown_dir().join(rel);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(Some(crate::render::markdown_body(&raw)))
}

pub fn save_note_body(store: &mut Store, uuid: &Uuid, body: &str) -> Result<(), CoreError> {
    let Some(mut b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    if b.markdown_file.is_none() && !b.html_file.is_empty() {
        let rel = join_folder(
            &b.folder,
            &format!("{}.md", trim_ext(&file_base(&b.html_file))),
        );
        b.markdown_file = Some(rel);
    }
    let Some(rel) = b.markdown_file.clone() else {
        return Err(CoreError::Invalid(
            "bookmark has no html file to base notes on".to_string(),
        ));
    };
    let doc = crate::render::render_markdown(&b, body);
    let path = store.cfg.markdown_dir().join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, doc).map_err(|e| CoreError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| CoreError::Storage(e.to_string()))?;
    store.update_bookmark(&b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config;

    fn mem_store(base: &std::path::Path) -> Store {
        Store::open_in_memory(Config {
            base_dir: base.to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap()
    }

    fn created(store: &mut Store, url: &str, folder: &str) -> Bookmark {
        create_bookmark(
            store,
            url,
            CreateOptions {
                title: Some("T".to_string()),
                folder: folder.to_string(),
                markdown: true,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn edit_fields_and_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = created(&mut s, "https://example.com/a", "tech");
        let html_before = std::fs::read_to_string(s.cfg.html_dir().join(&b.html_file)).unwrap();
        assert!(html_before.contains(">T<"));
        let out = edit_bookmark(
            &mut s,
            &b.uuid,
            EditOptions {
                title: Some("New".to_string()),
                tags: Some(vec!["x".to_string(), "X".to_string()]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.title, "New");
        assert_eq!(out.tags, vec!["x".to_string()]);
        let html_after = std::fs::read_to_string(s.cfg.html_dir().join(&out.html_file)).unwrap();
        assert!(html_after.contains(">New<"));
    }

    #[test]
    fn edit_folder_moves_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = created(&mut s, "https://example.com/a", "tech");
        let old_html = s.cfg.html_dir().join(&b.html_file);
        let old_md = s.cfg.markdown_dir().join(b.markdown_file.clone().unwrap());
        let out = edit_bookmark(
            &mut s,
            &b.uuid,
            EditOptions {
                folder: Some("other/sub".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!old_html.exists());
        assert!(!old_md.exists());
        assert!(s.cfg.html_dir().join(&out.html_file).exists());
        assert!(s
            .cfg
            .markdown_dir()
            .join(out.markdown_file.clone().unwrap())
            .exists());
        assert!(out.html_file.starts_with("other/sub/"));
    }

    #[test]
    fn edit_url_duplicate_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        created(&mut s, "https://example.com/a", "");
        let b = created(&mut s, "https://example.com/b", "");
        let err = edit_bookmark(
            &mut s,
            &b.uuid,
            EditOptions {
                url: Some("https://example.com/a".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Duplicate(_)));
    }

    #[test]
    fn delete_removes_files_and_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = created(&mut s, "https://example.com/a", "tech");
        let html = s.cfg.html_dir().join(&b.html_file);
        let md = s.cfg.markdown_dir().join(b.markdown_file.clone().unwrap());
        assert!(delete_bookmark_with_files(&mut s, &b.uuid).unwrap());
        assert!(!html.exists());
        assert!(!md.exists());
        assert!(s.get(&b.uuid).unwrap().is_none());
        assert!(!delete_bookmark_with_files(&mut s, &b.uuid).unwrap());
        let ops = s.oplog_entries().unwrap();
        assert!(ops.iter().any(|e| e.op == "delete"));
    }
}
