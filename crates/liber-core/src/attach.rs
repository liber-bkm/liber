use std::path::Path;

use uuid::Uuid;

use crate::model::Attachment;
use crate::slug::sanitize_filename;
use crate::store::Store;
use crate::CoreError;

fn unique_rel(store: &Store, b: &crate::model::Bookmark, name: &str) -> String {
    let base_name = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".to_string());
    let stem = Path::new(&base_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = Path::new(&base_name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let slug = {
        let s = crate::slug::slugify(&stem);
        if s.is_empty() {
            "attachment".to_string()
        } else {
            s
        }
    };
    let short = &b.uuid.to_string()[..8];
    let base = sanitize_filename(&format!("{short}-{slug}"));
    let mut rel = format!("{base}{ext}");
    let mut n = 2;
    while b.attachments.iter().any(|a| a.path == rel)
        || store.cfg.attachment_dir().join(&rel).exists()
    {
        rel = format!("{base}-{n}{ext}");
        n += 1;
    }
    rel
}

pub fn attach_file(store: &mut Store, uuid: &Uuid, src: &Path) -> Result<Attachment, CoreError> {
    if !src.exists() {
        return Err(CoreError::Invalid(format!("cannot read {}", src.display())));
    }
    if src.is_dir() {
        return Err(CoreError::Invalid(format!(
            "{} is a directory, attach files not folders",
            src.display()
        )));
    }
    let Some(b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "attachment".to_string());
    let rel = unique_rel(store, &b, &name);
    let dst = store.cfg.attachment_dir().join(&rel);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    std::fs::copy(src, &dst).map_err(|e| CoreError::Storage(format!("copying attachment: {e}")))?;
    store.add_attachment(uuid, name.clone(), rel.clone())?;
    Ok(Attachment { name, path: rel })
}

pub fn find_attachment_index(b: &crate::model::Bookmark, which: &str) -> Result<usize, CoreError> {
    if b.attachments.is_empty() {
        return Err(CoreError::Invalid(
            "this bookmark has no attachments".to_string(),
        ));
    }
    if let Ok(n) = which.trim().parse::<usize>() {
        if n >= 1 && n <= b.attachments.len() {
            return Ok(n - 1);
        }
        return Err(CoreError::Invalid(format!(
            "no attachment #{n} (has {})",
            b.attachments.len()
        )));
    }
    let hits: Vec<usize> = b
        .attachments
        .iter()
        .enumerate()
        .filter(|(_, a)| a.name.eq_ignore_ascii_case(which.trim()))
        .map(|(i, _)| i)
        .collect();
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(CoreError::Invalid(format!("no attachment named {which:?}"))),
        n => Err(CoreError::Invalid(format!(
            "{which:?} matches {n} attachments, use its number"
        ))),
    }
}

pub fn detach_attachment(store: &mut Store, uuid: &Uuid, which: &str) -> Result<String, CoreError> {
    let Some(b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let idx = find_attachment_index(&b, which)?;
    let at = &b.attachments[idx];
    let _ = std::fs::remove_file(store.cfg.attachment_dir().join(&at.path));
    let name = at.name.clone();
    let path = at.path.clone();
    store.remove_attachment(uuid, &path)?;
    let refreshed = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    store.update_bookmark(&refreshed)?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config;
    use std::path::PathBuf;

    fn setup() -> (Store, tempfile::TempDir, Uuid, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap();
        let b = create_bookmark(
            &mut store,
            "https://example.com/a",
            CreateOptions {
                title: Some("A".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let src = dir.path().join("note.txt");
        std::fs::write(&src, "hello").unwrap();
        (store, dir, b.uuid, src)
    }

    #[test]
    fn attach_detach_roundtrip() {
        let (mut store, _dir, uuid, src) = setup();
        let at = attach_file(&mut store, &uuid, &src).unwrap();
        assert_eq!(at.name, "note.txt");
        assert!(store.cfg.attachment_dir().join(&at.path).exists());
        let b = store.get(&uuid).unwrap().unwrap();
        assert_eq!(b.attachments.len(), 1);

        let at2 = attach_file(&mut store, &uuid, &src).unwrap();
        assert_ne!(at.path, at2.path);

        let name = detach_attachment(&mut store, &uuid, "1").unwrap();
        assert_eq!(name, "note.txt");
        assert!(!store.cfg.attachment_dir().join(&at.path).exists());
        assert_eq!(store.get(&uuid).unwrap().unwrap().attachments.len(), 1);

        assert!(detach_attachment(&mut store, &uuid, "nope").is_err());
        assert!(attach_file(&mut store, &uuid, Path::new("/nonexistent-xyz")).is_err());
    }

    #[test]
    fn attach_rejects_dirs() {
        let (mut store, _dir, uuid, _) = setup();
        let sub = tempfile::tempdir().unwrap();
        assert!(attach_file(&mut store, &uuid, sub.path()).is_err());
    }
}
