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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MarkdownAction {
    #[default]
    Keep,
    Add,
    Remove,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ArchiveAction {
    #[default]
    Keep,
    Add {
        backend: Option<String>,
    },
    Remove,
}

#[derive(Debug, Clone, Default)]
pub struct EditDraft {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub folder: Option<String>,
    pub url: Option<String>,
    pub markdown: MarkdownAction,
    pub archive: ArchiveAction,
    pub attach_paths: Vec<std::path::PathBuf>,
    pub detach: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EditApplied {
    pub bookmark: Bookmark,
    pub warnings: Vec<String>,
}

pub fn apply_edit(
    store: &mut Store,
    uuid: &Uuid,
    draft: EditDraft,
) -> Result<EditApplied, CoreError> {
    let mut warnings = Vec::new();
    let mut b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;

    if let Some(url) = draft.url {
        let url = normalize_url(&url);
        if let Some(other) = store.find_by_url(&url)? {
            if other.uuid != *uuid {
                return Err(CoreError::Duplicate(other.uuid.to_string()));
            }
        }
        b.url = url;
    }
    if let Some(title) = draft.title {
        if !title.trim().is_empty() {
            b.title = title.trim().to_string();
        }
    }
    if let Some(description) = draft.description {
        b.description = description;
    }
    if let Some(tags) = draft.tags {
        b.tags = dedupe_strings(tags);
    }

    let mut folder_changed = false;
    if let Some(folder) = draft.folder {
        let folder = sanitize_folder(&folder);
        folder_changed = folder != b.folder;
        b.folder = folder;
    }

    if folder_changed {
        move_bookmark_files(store, &mut b)?;
    }

    match draft.markdown {
        MarkdownAction::Keep => {
            rewrite_bookmark_files(store, &mut b, false)?;
        }
        MarkdownAction::Add => {
            rewrite_bookmark_files(store, &mut b, true)?;
        }
        MarkdownAction::Remove => {
            if let Some(rel) = b.markdown_file.take() {
                let _ = std::fs::remove_file(store.cfg.markdown_dir().join(&rel));
                let payload = serde_json::json!({"uuid": uuid.to_string()});
                store.append_oplog(Some(*uuid), "notes_del", payload)?;
            }
            if !b.html_file.is_empty() {
                write_html_bookmark(&store.cfg.html_dir().join(&b.html_file), &b)?;
            }
        }
    }

    match draft.archive {
        ArchiveAction::Keep => {
            store.update_bookmark(&b)?;
        }
        ArchiveAction::Add { backend } => {
            store.update_bookmark(&b)?;
            match crate::archive::archive_bookmark(store, &b.uuid, backend.as_deref()) {
                Ok(warns) => warnings.extend(warns),
                Err(e) => warnings.push(format!("archive failed: {e}")),
            }
            b = store
                .get(uuid)?
                .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
        }
        ArchiveAction::Remove => {
            if let Some(rel) = b.archive_file.take() {
                let _ = std::fs::remove_file(store.cfg.archive_dir().join(&rel));
                let payload = serde_json::json!({"uuid": uuid.to_string()});
                store.append_oplog(Some(*uuid), "archive_del", payload)?;
            }
            store.update_bookmark(&b)?;
            b = store
                .get(uuid)?
                .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
        }
    }

    for path in &draft.attach_paths {
        match crate::attach::attach_file(store, &b.uuid, path) {
            Ok(at) => warnings.push(format!("attached {}", at.name)),
            Err(e) => warnings.push(format!("could not attach {}: {e}", path.display())),
        }
    }
    for which in &draft.detach {
        match crate::attach::detach_attachment(store, &b.uuid, which) {
            Ok(name) => warnings.push(format!("detached {name}")),
            Err(e) => warnings.push(format!("detach failed: {e}")),
        }
    }

    b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    store.update_bookmark(&b)?;
    let b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    let _ = crate::search::reindex_one(store, &b.uuid);
    Ok(EditApplied {
        bookmark: b,
        warnings,
    })
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
    Ok(apply_edit(
        store,
        uuid,
        EditDraft {
            title: opts.title,
            description: opts.description,
            tags: opts.tags,
            folder: opts.folder,
            url: opts.url,
            markdown: if opts.add_markdown {
                MarkdownAction::Add
            } else {
                MarkdownAction::Keep
            },
            archive: ArchiveAction::Keep,
            attach_paths: Vec::new(),
            detach: Vec::new(),
        },
    )?
    .bookmark)
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
    let gone = store.delete_bookmark(uuid)?;
    if gone {
        let _ = crate::search::drop_one(store, uuid);
    }
    Ok(gone)
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
    store.update_bookmark(&b)?;
    let payload = serde_json::json!({"uuid": uuid.to_string(), "body": body});
    store.append_oplog(Some(*uuid), "notes_save", payload)?;
    let _ = crate::search::reindex_one(store, uuid);
    Ok(())
}

pub fn remove_note_body(store: &mut Store, uuid: &Uuid) -> Result<bool, CoreError> {
    let Some(mut b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let Some(rel) = b.markdown_file.take() else {
        return Ok(false);
    };
    let _ = std::fs::remove_file(store.cfg.markdown_dir().join(&rel));
    store.update_bookmark(&b)?;
    let payload = serde_json::json!({"uuid": uuid.to_string()});
    store.append_oplog(Some(*uuid), "notes_del", payload)?;
    let _ = crate::search::reindex_one(store, uuid);
    Ok(true)
}

pub fn remove_archive_file(store: &mut Store, uuid: &Uuid) -> Result<bool, CoreError> {
    let Some(mut b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let Some(rel) = b.archive_file.take() else {
        return Ok(false);
    };
    let _ = std::fs::remove_file(store.cfg.archive_dir().join(&rel));
    store.update_bookmark(&b)?;
    let payload = serde_json::json!({"uuid": uuid.to_string()});
    store.append_oplog(Some(*uuid), "archive_del", payload)?;
    let _ = crate::search::reindex_one(store, uuid);
    Ok(true)
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

    #[test]
    fn draft_applies_fields_and_markdown_remove() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = created(&mut s, "https://example.com/a", "tech");
        let md_rel = b.markdown_file.clone().unwrap();
        assert!(s.cfg.markdown_dir().join(&md_rel).exists());
        let applied = apply_edit(
            &mut s,
            &b.uuid,
            EditDraft {
                title: Some("Renamed".to_string()),
                markdown: MarkdownAction::Remove,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(applied.bookmark.title, "Renamed");
        assert!(applied.bookmark.markdown_file.is_none());
        assert!(!s.cfg.markdown_dir().join(&md_rel).exists());
        let stored = s.get(&b.uuid).unwrap().unwrap();
        assert_eq!(stored.title, "Renamed");
        assert!(stored.markdown_file.is_none());
    }

    #[test]
    fn draft_archive_remove_clears_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = created(&mut s, "https://example.com/a", "tech");
        let rel = "tech/manual-archive.html";
        std::fs::create_dir_all(s.cfg.archive_dir().join("tech")).unwrap();
        std::fs::write(s.cfg.archive_dir().join(rel), "<html></html>").unwrap();
        let mut with_arch = s.get(&b.uuid).unwrap().unwrap();
        with_arch.archive_file = Some(rel.to_string());
        s.update_bookmark(&with_arch).unwrap();
        let applied = apply_edit(
            &mut s,
            &b.uuid,
            EditDraft {
                archive: ArchiveAction::Remove,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(applied.bookmark.archive_file.is_none());
        assert!(!s.cfg.archive_dir().join(rel).exists());
    }
}
