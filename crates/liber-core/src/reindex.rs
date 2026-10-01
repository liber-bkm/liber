use std::collections::HashSet;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::model::NewBookmark;
use crate::search::SearchIndex;
use crate::slug::sanitize_folder;
use crate::store::Store;
use crate::CoreError;

#[derive(Debug, Default)]
pub struct ReindexReport {
    pub adopted: usize,
    pub relinked_markdown: usize,
    pub relinked_archive: usize,
    pub swept_conflicts: usize,
    pub quarantined_attachments: usize,
    pub pending: Vec<String>,
    pub pruned: usize,
    pub indexed: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReindexFlags {
    pub prune: bool,
}

fn is_conflict_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("sync-conflict") || lower.contains("conflicted")
}

fn list_files_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(list_files_recursive(&path));
        } else if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn rel_path(base: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(base)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
}

fn move_to_unindexed(profile_dir: &Path, kind: &str, rel: &str) -> Result<(), CoreError> {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let dst = profile_dir.join("unindexed").join(kind).join(name);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let srcs = [
        profile_dir.join("html").join(rel),
        profile_dir.join("markdown").join(rel),
        profile_dir.join("archive").join(rel),
        profile_dir.join("attachments").join(rel),
    ];
    for src in srcs {
        if src.exists() {
            let _ = std::fs::rename(&src, &dst);
            return Ok(());
        }
    }
    Ok(())
}

fn meta_content(html: &str, name: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let mut search = lower.as_str();
    let mut offset = 0;
    while let Some(i) = search.find("<meta") {
        let abs = offset + i;
        let tag_end = lower[abs..].find('>')? + abs;
        let tag = &html[abs..tag_end];
        let tag_lower = tag.to_lowercase();
        if tag_lower.contains(&format!("name=\"liber:{name}\""))
            || tag_lower.contains(&format!("name='liber:{name}'"))
        {
            if let Some(ci) = tag_lower.find("content=") {
                let rest = tag[ci + 8..].trim_start();
                if let Some(q) = rest.strip_prefix('"') {
                    return Some(q.split('"').next().unwrap_or("").to_string());
                }
                if let Some(q) = rest.strip_prefix('\'') {
                    return Some(q.split('\'').next().unwrap_or("").to_string());
                }
                let end = rest
                    .find([' ', '\t', '\n', '\r', '>'])
                    .unwrap_or(rest.len());
                return Some(rest[..end].to_string());
            }
        }
        search = &lower[tag_end + 1..];
        offset = tag_end + 1;
    }
    None
}

fn title_of(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + 7;
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn shared_base(html_rel: &str) -> String {
    let base = html_rel.rsplit('/').next().unwrap_or(html_rel);
    match base.rfind('.') {
        Some(i) => base[..i].to_string(),
        None => base.to_string(),
    }
}

pub fn reindex(store: &mut Store, flags: ReindexFlags) -> Result<ReindexReport, CoreError> {
    let mut rep = ReindexReport::default();
    let profile = store.cfg.profile_dir();

    for (kind, dir) in [
        ("html", store.cfg.html_dir()),
        ("markdown", store.cfg.markdown_dir()),
        ("archive", store.cfg.archive_dir()),
        ("attachments", store.cfg.attachment_dir()),
    ] {
        for path in list_files_recursive(&dir) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !is_conflict_name(&name) {
                continue;
            }
            let dst = profile.join("unindexed").join(kind).join(&name);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
            }
            let _ = std::fs::rename(&path, &dst);
            rep.swept_conflicts += 1;
        }
    }

    let known_html: HashSet<String> = store.list()?.iter().map(|b| b.html_file.clone()).collect();
    let mut orphans: Vec<(String, String)> = Vec::new();
    for path in list_files_recursive(&store.cfg.html_dir()) {
        let Some(rel) = rel_path(&store.cfg.html_dir(), &path) else {
            continue;
        };
        if known_html.contains(&rel) {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if is_conflict_name(&name) {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        orphans.push((rel, content));
    }
    for (rel, content) in orphans {
        let url = meta_content(&content, "url").unwrap_or_default();
        if url.trim().is_empty() {
            move_to_unindexed(&profile, "html", &rel)?;
            continue;
        }
        if store.find_by_url(&url)?.is_some() {
            move_to_unindexed(&profile, "html", &rel)?;
            continue;
        }
        let folder = Path::new(&rel)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let title = title_of(&content)
            .or_else(|| meta_content(&content, "title"))
            .unwrap_or_else(|| url.clone());
        let tags = meta_content(&content, "tags")
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let uuid: Uuid = meta_content(&content, "uuid")
            .and_then(|s| s.parse().ok())
            .filter(|u: &Uuid| store.get(u).ok().flatten().is_none())
            .unwrap_or_else(Uuid::new_v4);
        let now = chrono::Utc::now();
        let bookmark = crate::model::Bookmark {
            uuid,
            url: url.clone(),
            title,
            description: String::new(),
            tags,
            folder: sanitize_folder(&folder),
            created_at: now,
            updated_at: now,
            html_file: rel,
            markdown_file: None,
            archive_file: None,
            attachments: vec![],
            open_count: 0,
            last_opened_at: None,
            last_checked_at: None,
            check_status: None,
            applied_rules: vec![],
        };
        let nb = NewBookmark {
            uuid: bookmark.uuid,
            url: bookmark.url.clone(),
            title: bookmark.title.clone(),
            description: String::new(),
            tags: bookmark.tags.clone(),
            folder: bookmark.folder.clone(),
            html_file: bookmark.html_file.clone(),
            markdown_file: None,
            archive_file: None,
            applied_rules: vec![],
        };
        let _ = store.add_bookmark(nb).is_ok();
        if store.find_by_url(&url)?.is_some() {
            rep.adopted += 1;
        }
    }

    for mut b in store.list()? {
        let mut touched = false;
        if b.markdown_file.is_none() && !b.html_file.is_empty() {
            let cand = if b.folder.is_empty() {
                format!("{}.md", shared_base(&b.html_file))
            } else {
                format!("{}/{}.md", b.folder, shared_base(&b.html_file))
            };
            if store.cfg.markdown_dir().join(&cand).exists() {
                b.markdown_file = Some(cand);
                touched = true;
                rep.relinked_markdown += 1;
            }
        }
        if b.archive_file.is_none() && !b.html_file.is_empty() {
            let cand = if b.folder.is_empty() {
                format!("{}.html", shared_base(&b.html_file))
            } else {
                format!("{}/{}.html", b.folder, shared_base(&b.html_file))
            };
            if store.cfg.archive_dir().join(&cand).exists() {
                b.archive_file = Some(cand);
                touched = true;
                rep.relinked_archive += 1;
            }
        }
        if touched {
            store.update_bookmark(&b)?;
        }
    }

    let referenced: HashSet<String> = store
        .list()?
        .iter()
        .flat_map(|b| b.attachments.iter().map(|a| a.path.clone()))
        .collect();
    for path in list_files_recursive(&store.cfg.attachment_dir()) {
        let Some(rel) = rel_path(&store.cfg.attachment_dir(), &path) else {
            continue;
        };
        if referenced.contains(&rel) {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let dst = profile.join("unindexed").join("attachments").join(&name);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        let _ = std::fs::rename(&path, &dst);
        rep.quarantined_attachments += 1;
    }

    for b in store.list()? {
        if b.html_file.is_empty() {
            continue;
        }
        if !store.cfg.html_dir().join(&b.html_file).exists() {
            if flags.prune {
                if let Some(rel) = &b.markdown_file {
                    move_to_unindexed(&profile, "markdown", rel)?;
                }
                if let Some(rel) = &b.archive_file {
                    move_to_unindexed(&profile, "archive", rel)?;
                }
                for at in &b.attachments {
                    move_to_unindexed(&profile, "attachments", &at.path)?;
                }
                store.delete_bookmark(&b.uuid)?;
                rep.pruned += 1;
            } else {
                rep.pending.push(b.uuid.to_string());
            }
        }
    }
    if let Ok(index) = SearchIndex::open_or_create(&store.cfg.tantivy_dir()) {
        let mut docs = Vec::new();
        for b in store.list()? {
            let mut content = String::new();
            if let Some(rel) = &b.archive_file {
                if let Ok(html) = std::fs::read_to_string(store.cfg.archive_dir().join(rel)) {
                    content.push_str(&crate::archive::extract_archive_text(&html));
                }
            }
            if let Some(rel) = &b.markdown_file {
                if let Ok(md) = std::fs::read_to_string(store.cfg.markdown_dir().join(rel)) {
                    content.push('\n');
                    content.push_str(&md);
                }
            }
            docs.push((b, content));
        }
        if index.rebuild_all(&docs).is_ok() {
            rep.indexed = docs.len();
        }
    }

    Ok(rep)
}
