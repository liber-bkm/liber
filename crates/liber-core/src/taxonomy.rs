use crate::edit::{edit_bookmark, EditOptions};
use crate::model::Bookmark;
use crate::slug::{path_in_subtree, path_rest, sanitize_folder, sanitize_tag, tag_in_subtree};
use crate::store::Store;
use crate::CoreError;

pub fn tag_counts(store: &Store) -> Result<Vec<(String, usize)>, CoreError> {
    store.tag_counts_sql()
}

pub fn folder_counts(store: &Store) -> Result<Vec<(String, usize)>, CoreError> {
    store.folder_counts_sql()
}

fn descendant_rest<'a>(value: &'a str, target: &str, ignore_ascii_case: bool) -> Option<&'a str> {
    path_rest(value, target, ignore_ascii_case).filter(|rest| !rest.is_empty())
}

pub fn rename_tag(store: &mut Store, old: &str, new_tag: &str) -> Result<Vec<Bookmark>, CoreError> {
    let old = sanitize_tag(old);
    let new_tag = sanitize_tag(new_tag);
    if old.is_empty() || new_tag.is_empty() {
        return Err(CoreError::Invalid(
            "old and new tag are required".to_string(),
        ));
    }
    if old == new_tag {
        return Err(CoreError::Invalid(format!(
            "{old:?} and {new_tag:?} are the same tag"
        )));
    }
    if descendant_rest(&new_tag, &old, true).is_some() {
        return Err(CoreError::Invalid(format!(
            "new tag {new_tag:?} can't be inside the old tag {old:?}"
        )));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        let mut tags = Vec::with_capacity(b.tags.len());
        let mut touched = false;
        for tag in &b.tags {
            if let Some(rest) = path_rest(tag, &old, true) {
                tags.push(format!("{new_tag}{rest}"));
                touched = true;
            } else {
                tags.push(tag.clone());
            }
        }
        if !touched {
            continue;
        }
        let out = edit_bookmark(
            store,
            &b.uuid,
            EditOptions {
                tags: Some(tags),
                ..Default::default()
            },
        )?;
        changed.push(out);
    }
    Ok(changed)
}

pub fn delete_tag(store: &mut Store, tag: &str) -> Result<Vec<Bookmark>, CoreError> {
    let tag = sanitize_tag(tag);
    if tag.is_empty() {
        return Err(CoreError::Invalid("tag is required".to_string()));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        let tags: Vec<String> = b
            .tags
            .iter()
            .filter(|t| !tag_in_subtree(t, &tag))
            .cloned()
            .collect();
        if tags.len() == b.tags.len() {
            continue;
        }
        let out = edit_bookmark(
            store,
            &b.uuid,
            EditOptions {
                tags: Some(tags),
                ..Default::default()
            },
        )?;
        changed.push(out);
    }
    Ok(changed)
}

pub fn rename_folder(
    store: &mut Store,
    old: &str,
    new_folder: &str,
) -> Result<Vec<Bookmark>, CoreError> {
    let old = sanitize_folder(old);
    let new_folder = sanitize_folder(new_folder);
    if old.is_empty() {
        return Err(CoreError::Invalid(
            "old folder can't be root, name a specific subfolder".to_string(),
        ));
    }
    if old == new_folder {
        return Err(CoreError::Invalid(format!(
            "{old:?} and {new_folder:?} are the same folder"
        )));
    }
    if descendant_rest(&new_folder, &old, false).is_some() {
        return Err(CoreError::Invalid(format!(
            "new folder {new_folder:?} can't be inside the old folder {old:?}"
        )));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        let Some(rest) = path_rest(&b.folder, &old, false) else {
            continue;
        };
        let next = format!("{new_folder}{rest}");
        let next = next.trim_start_matches('/').to_string();
        let out = edit_bookmark(
            store,
            &b.uuid,
            EditOptions {
                folder: Some(next),
                ..Default::default()
            },
        )?;
        changed.push(out);
    }
    Ok(changed)
}

pub fn delete_folder(store: &mut Store, folder: &str) -> Result<Vec<Bookmark>, CoreError> {
    let old = sanitize_folder(folder);
    if old.is_empty() {
        return Err(CoreError::Invalid(
            "folder can't be root, name a specific subfolder".to_string(),
        ));
    }
    let parent = old.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let mut changed = Vec::new();
    for b in store.list()? {
        if !path_in_subtree(&b.folder, &old) {
            continue;
        }
        let rest = path_rest(&b.folder, &old, false).unwrap_or("");
        let next = format!("{parent}{rest}");
        let next = next.trim_start_matches('/').to_string();
        let out = edit_bookmark(
            store,
            &b.uuid,
            EditOptions {
                folder: Some(next),
                ..Default::default()
            },
        )?;
        changed.push(out);
    }
    Ok(changed)
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

    fn add(store: &mut Store, url: &str, tags: Vec<&str>, folder: &str) {
        create_bookmark(
            store,
            url,
            CreateOptions {
                title: Some(url.to_string()),
                tags: tags.into_iter().map(str::to_string).collect(),
                folder: folder.to_string(),
                ..Default::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn tag_rename_merges_and_counts() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec!["Rust", "x"], "");
        add(&mut s, "https://example.com/b", vec!["rust"], "");
        let changed = rename_tag(&mut s, "RUST", "go").unwrap();
        assert_eq!(changed.len(), 2);
        let counts = tag_counts(&s).unwrap();
        assert_eq!(counts, vec![("go".to_string(), 2), ("x".to_string(), 1)]);
        assert!(rename_tag(&mut s, "go", "go").is_err());
        assert!(rename_tag(&mut s, "missing", "go").unwrap().is_empty());
    }

    #[test]
    fn tag_rename_moves_subtree_and_allows_case_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec!["Dev", "dev/rust"], "");
        add(
            &mut s,
            "https://example.com/b",
            vec!["dev/rust/async", "other"],
            "",
        );

        let changed = rename_tag(&mut s, "dev", "Work").unwrap();
        assert_eq!(changed.len(), 2);
        let a = s
            .list()
            .unwrap()
            .into_iter()
            .find(|b| b.url.ends_with("/a"))
            .unwrap();
        assert_eq!(a.tags, vec!["Work".to_string(), "Work/rust".to_string()]);
        let b = s
            .list()
            .unwrap()
            .into_iter()
            .find(|b| b.url.ends_with("/b"))
            .unwrap();
        assert_eq!(
            b.tags,
            vec!["Work/rust/async".to_string(), "other".to_string()]
        );

        let changed = rename_tag(&mut s, "Work", "work").unwrap();
        assert_eq!(changed.len(), 2);
        let a = s
            .list()
            .unwrap()
            .into_iter()
            .find(|b| b.url.ends_with("/a"))
            .unwrap();
        assert_eq!(a.tags, vec!["work".to_string(), "work/rust".to_string()]);

        assert!(rename_tag(&mut s, "work", "work/nested").is_err());
    }

    #[test]
    fn tag_delete_removes_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(
            &mut s,
            "https://example.com/a",
            vec!["dev", "dev/rust", "x"],
            "",
        );
        add(
            &mut s,
            "https://example.com/b",
            vec!["dev/rust/async", "other"],
            "",
        );
        let changed = delete_tag(&mut s, "DEV").unwrap();
        assert_eq!(changed.len(), 2);
        let a = s
            .list()
            .unwrap()
            .into_iter()
            .find(|b| b.url.ends_with("/a"))
            .unwrap();
        assert_eq!(a.tags, vec!["x".to_string()]);
        let b = s
            .list()
            .unwrap()
            .into_iter()
            .find(|b| b.url.ends_with("/b"))
            .unwrap();
        assert_eq!(b.tags, vec!["other".to_string()]);
    }

    #[test]
    fn tag_delete_clears() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec!["x", "y"], "");
        let changed = delete_tag(&mut s, "X").unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].tags, vec!["y".to_string()]);
        assert!(delete_tag(&mut s, " ").is_err());
    }

    #[test]
    fn folder_rename_moves_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec![], "tech");
        add(&mut s, "https://example.com/b", vec![], "tech/rust");
        add(&mut s, "https://example.com/c", vec![], "other");
        let changed = rename_folder(&mut s, "tech", "dev").unwrap();
        assert_eq!(changed.len(), 2);
        let counts = folder_counts(&s).unwrap();
        assert!(counts.contains(&("dev".to_string(), 1)));
        assert!(counts.contains(&("dev/rust".to_string(), 1)));
        assert!(s.cfg.html_dir().join(&changed[0].html_file).exists());
        assert!(rename_folder(&mut s, "", "x").is_err());
        assert!(rename_folder(&mut s, "dev", "dev").is_err());
        assert!(rename_folder(&mut s, "dev", "dev/nested").is_err());
    }

    #[test]
    fn folder_delete_lifts_children_to_parent() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec![], "tech");
        add(&mut s, "https://example.com/b", vec![], "tech/rust");
        add(&mut s, "https://example.com/c", vec![], "tech/ops/db");
        add(&mut s, "https://example.com/d", vec![], "other");

        let changed = delete_folder(&mut s, "tech").unwrap();
        assert_eq!(changed.len(), 3);
        let folders: Vec<String> = s.list().unwrap().into_iter().map(|b| b.folder).collect();
        assert!(folders.contains(&String::new()));
        assert!(folders.contains(&"rust".to_string()));
        assert!(folders.contains(&"ops/db".to_string()));
        assert!(!folders.iter().any(|f| f.starts_with('/')));
    }

    #[test]
    fn folder_delete_nested_keeps_parent() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", vec![], "tech/ops");
        add(&mut s, "https://example.com/b", vec![], "tech/ops/db");
        let changed = delete_folder(&mut s, "tech/ops").unwrap();
        assert_eq!(changed.len(), 2);
        let mut folders: Vec<String> = s.list().unwrap().into_iter().map(|b| b.folder).collect();
        folders.sort();
        assert_eq!(folders, vec!["tech".to_string(), "tech/db".to_string()]);
        assert!(delete_folder(&mut s, "").is_err());
    }
}
