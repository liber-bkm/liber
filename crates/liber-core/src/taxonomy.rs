use crate::edit::{edit_bookmark, EditOptions};
use crate::model::Bookmark;
use crate::slug::sanitize_folder;
use crate::store::Store;
use crate::CoreError;

pub fn tag_counts(store: &Store) -> Result<Vec<(String, usize)>, CoreError> {
    let mut counts = std::collections::HashMap::new();
    for b in store.list()? {
        for t in &b.tags {
            *counts.entry(t.clone()).or_insert(0) += 1;
        }
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(out)
}

pub fn folder_counts(store: &Store) -> Result<Vec<(String, usize)>, CoreError> {
    let mut counts = std::collections::HashMap::new();
    for b in store.list()? {
        let label = if b.folder.is_empty() {
            "/".to_string()
        } else {
            b.folder.clone()
        };
        *counts.entry(label).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(out)
}

fn index_of_fold(tags: &[String], target: &str) -> Option<usize> {
    tags.iter().position(|t| t.eq_ignore_ascii_case(target))
}

pub fn rename_tag(store: &mut Store, old: &str, new_tag: &str) -> Result<Vec<Bookmark>, CoreError> {
    let old = old.trim();
    let new_tag = new_tag.trim();
    if old.is_empty() || new_tag.is_empty() {
        return Err(CoreError::Invalid(
            "old and new tag are required".to_string(),
        ));
    }
    if old.eq_ignore_ascii_case(new_tag) {
        return Err(CoreError::Invalid(format!(
            "{old:?} and {new_tag:?} are the same tag"
        )));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        let Some(idx) = index_of_fold(&b.tags, old) else {
            continue;
        };
        let mut tags: Vec<String> = b
            .tags
            .iter()
            .filter(|t| *t != &b.tags[idx])
            .cloned()
            .collect();
        tags.push(new_tag.to_string());
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
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(CoreError::Invalid("tag is required".to_string()));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        let Some(idx) = index_of_fold(&b.tags, tag) else {
            continue;
        };
        let tags: Vec<String> = b
            .tags
            .iter()
            .filter(|t| *t != &b.tags[idx])
            .cloned()
            .collect();
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

fn folder_in_subtree(folder: &str, target: &str) -> bool {
    folder == target || folder.starts_with(&format!("{target}/"))
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
    let mut changed = Vec::new();
    for b in store.list()? {
        if !folder_in_subtree(&b.folder, &old) {
            continue;
        }
        let next = if b.folder == old {
            new_folder.clone()
        } else {
            format!("{}{}", new_folder, &b.folder[old.len()..])
        };
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
    rename_folder(store, folder, "")
}

}
