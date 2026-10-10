use crate::archive::archive_bookmark;
use crate::create::{create_bookmark, CreateOptions};
use crate::slug::{normalize_url, sanitize_folder};
use crate::store::Store;
use crate::CoreError;

#[derive(Debug, Clone, Default)]
pub struct ImportedEntry {
    pub title: String,
    pub url: String,
    pub folder: String,
    pub tags: Vec<String>,
    pub desc: String,
}

fn unescape(s: &str) -> String {
    let mut out = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    out = strip_tags(&out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn attr_value(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_lowercase();
    let needle = format!("{name}=");
    let mut search = lower.as_str();
    let mut offset = 0;
    loop {
        let i = search.find(&needle)?;
        let abs = offset + i;
        let before_ok = abs == 0 || {
            let b = lower.as_bytes()[abs - 1];
            b == b' ' || b == b'\t' || b == b'\n' || b == b'\r'
        };
        let val_start = abs + needle.len();
        let rest = &attrs[val_start..];
        if !before_ok {
            search = &lower[val_start..];
            offset = val_start;
            continue;
        }
        let rest_trim = rest.trim_start();
        if let Some(q) = rest_trim.strip_prefix('"') {
            let end = q.find('"').unwrap_or(q.len());
            return Some(q[..end].to_string());
        }
        if let Some(q) = rest_trim.strip_prefix('\'') {
            let end = q.find('\'').unwrap_or(q.len());
            return Some(q[..end].to_string());
        }
        let end = rest_trim
            .find([' ', '\t', '\n', '\r', '>'])
            .unwrap_or(rest_trim.len());
        return Some(rest_trim[..end].to_string());
    }
}

fn tag_name_at(s: &str) -> Option<(String, String, bool)> {
    let s = s.trim_start();
    if !s.starts_with('<') {
        return None;
    }
    let end = s.find('>')?;
    let inner = s[1..end].trim();
    let closing = inner.starts_with('/');
    let inner = inner.trim_start_matches('/').trim();
    let name_end = inner
        .find([' ', '\t', '\n', '\r', '/'])
        .unwrap_or(inner.len());
    let name = inner[..name_end].to_lowercase();
    let attrs = inner[name_end..].trim().to_string();
    Some((name, attrs, closing))
}

pub fn parse_netscape(content: &str) -> Vec<ImportedEntry> {
    let mut entries = Vec::new();
    let mut folder_stack: Vec<String> = Vec::new();
    let mut pending_folder = String::new();
    let mut pos = 0;
    let bytes = content.as_bytes();
    while pos < bytes.len() {
        let rest = &content[pos..];
        let Some((name, attrs, closing)) = tag_name_at(rest) else {
            pos += 1;
            continue;
        };
        let tag_end = rest.find('>').unwrap() + 1;
        pos += tag_end;
        match name.as_str() {
            "h3" if !closing => {
                let after = &content[pos..];
                let end = after.to_lowercase().find("</h3>").unwrap_or(after.len());
                pending_folder = unescape(after[..end].trim());
                pos += end + "</h3>".len().min(after.len().saturating_sub(end));
            }
            "dl" if !closing => {
                folder_stack.push(std::mem::take(&mut pending_folder));
            }
            "dl" => {
                folder_stack.pop();
            }
            "a" if !closing => {
                let href = attr_value(&attrs, "href")
                    .map(|h| unescape(&h))
                    .unwrap_or_default();
                let tags_raw = attr_value(&attrs, "tags")
                    .map(|t| unescape(&t))
                    .unwrap_or_default();
                let after = &content[pos..];
                let end = after.to_lowercase().find("</a>").unwrap_or(after.len());
                let title = unescape(after[..end].trim());
                pos += end + "</a>".len().min(after.len().saturating_sub(end));
                let folder = folder_stack
                    .iter()
                    .filter(|f| !f.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("/");
                let tags = tags_raw
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                entries.push(ImportedEntry {
                    title,
                    url: href,
                    folder,
                    tags,
                    desc: String::new(),
                });
            }
            "dd" if !closing => {
                let after = &content[pos..];
                let end = after.find(['<', '\r', '\n']).unwrap_or(after.len());
                if let Some(last) = entries.last_mut() {
                    last.desc = unescape(after[..end].trim());
                }
            }
            _ => {}
        }
    }
    entries
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub added: usize,
    pub skipped_dup: usize,
    pub skipped_bad: usize,
    pub warnings: Vec<String>,
}

pub fn import_data(
    store: &mut Store,
    content: &str,
    markdown: bool,
    archive: bool,
) -> Result<ImportReport, CoreError> {
    let mut report = ImportReport::default();
    let was_indexing = store.auto_index();
    store.set_auto_index(false);
    let mut added: Vec<uuid::Uuid> = Vec::new();
    for e in parse_netscape(content) {
        if e.url.trim().is_empty() {
            report.skipped_bad += 1;
            continue;
        }
        let url = normalize_url(&e.url);
        if store.find_by_url(&url)?.is_some() {
            report.skipped_dup += 1;
            continue;
        }
        let title = if e.title.trim().is_empty() {
            url.clone()
        } else {
            e.title
        };
        match create_bookmark(
            store,
            &url,
            CreateOptions {
                title: Some(title),
                description: e.desc,
                tags: e.tags,
                folder: sanitize_folder(&e.folder),
                markdown,
            },
        ) {
            Ok(b) => {
                if archive {
                    if let Err(err) = archive_bookmark(store, &b.uuid, None) {
                        report
                            .warnings
                            .push(format!("warning: archive failed for {url}: {err}"));
                    }
                }
                added.push(b.uuid);
                report.added += 1;
            }
            Err(err) => report
                .warnings
                .push(format!("warning: could not import {url}: {err}")),
        }
    }
    store.set_auto_index(was_indexing);
    if was_indexing {
        let touched: Vec<(uuid::Uuid, bool)> = added.into_iter().map(|u| (u, false)).collect();
        let _ = crate::search::batch_reindex(store, &touched);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Config;

    const SAMPLE: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
<DT><H3>tech</H3>
<DL><p>
<DT><A HREF="https://example.com/a" TAGS="rust,news">Example A</A>
<DD>First line note
<DT><A HREF="https://example.com/b">Example B</A>
</DL><p>
<DT><A HREF="https://other.com/">Other</A>
<DT><A>Bad entry</A>
</DL><p>
"#;

    #[test]
    fn parse_vectors() {
        let entries = parse_netscape(SAMPLE);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].folder, "tech");
        assert_eq!(
            entries[0].tags,
            vec!["rust".to_string(), "news".to_string()]
        );
        assert_eq!(entries[0].desc, "First line note");
        assert_eq!(entries[1].folder, "tech");
        assert_eq!(entries[2].folder, "");
        assert_eq!(entries[3].url, "");
    }

    #[test]
    fn import_counts() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap();
        let report = import_data(&mut store, SAMPLE, false, false).unwrap();
        assert_eq!(report.added, 3);
        assert_eq!(report.skipped_bad, 1);
        let again = import_data(&mut store, SAMPLE, false, false).unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(again.skipped_dup, 3);
        let all = store.list().unwrap();
        assert!(all
            .iter()
            .any(|b| b.folder == "tech" && b.tags.contains(&"rust".to_string())));
        let a = all
            .iter()
            .find(|b| b.url == "https://example.com/a")
            .unwrap();
        assert_eq!(a.description, "First line note");
    }
}
