use std::path::{Path, PathBuf};

use crate::model::Bookmark;
use crate::store::Store;
use crate::CoreError;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn flatten_description(desc: &str) -> String {
    desc.replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn write_netscape_export(store: &Store) -> Result<String, CoreError> {
    let mut out = String::new();
    out.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
    out.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n");
    let all: Vec<Bookmark> = store.list()?;
    let mut groups: Vec<(String, Vec<Bookmark>)> = Vec::new();
    for b in all {
        match groups.iter_mut().find(|(f, _)| *f == b.folder) {
            Some((_, v)) => v.push(b),
            None => groups.push((b.folder.clone(), vec![b])),
        }
    }
    for (folder, members) in &groups {
        let parts: Vec<&str> = if folder.is_empty() {
            vec![]
        } else {
            folder.split('/').collect()
        };
        for p in &parts {
            out.push_str(&format!("<DT><H3>{}</H3>\n<DL><p>\n", esc(p)));
        }
        for b in members {
            let title = if b.title.trim().is_empty() {
                b.url.clone()
            } else {
                b.title.clone()
            };
            out.push_str(&format!("    <DT><A HREF=\"{}\"", esc(&b.url)));
            if !b.tags.is_empty() {
                out.push_str(&format!(" TAGS=\"{}\"", esc(&b.tags.join(","))));
            }
            out.push_str(&format!(">{}</A>\n", esc(&title)));
            let desc = flatten_description(&b.description);
            if !desc.is_empty() {
                out.push_str(&format!("    <DD>{desc}\n"));
            }
        }
        for _ in &parts {
            out.push_str("</DL>\n");
        }
    }
    out.push_str("</DL>\n");
    Ok(out)
}

fn rel_link(out_dir: &Path, target: &Path) -> String {
    use std::path::Component;
    let mut o: Vec<Component> = out_dir.components().collect();
    let mut t: Vec<Component> = target.components().collect();
    while !o.is_empty() && !t.is_empty() && o[0] == t[0] {
        o.remove(0);
        t.remove(0);
    }
    if o.is_empty() && t.is_empty() {
        return ".".to_string();
    }
    let mut rel = PathBuf::new();
    for _ in &o {
        rel.push("..");
    }
    for c in &t {
        rel.push(c);
    }
    rel.to_string_lossy().to_string()
}

pub fn export_site(store: &Store, out_dir: &Path) -> Result<PathBuf, CoreError> {
    std::fs::create_dir_all(out_dir).map_err(|e| CoreError::Storage(e.to_string()))?;
    let mut doc = String::new();
    doc.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>liber-rs export</title>\n<style>\n  body { font-family: system-ui, sans-serif; max-width: 760px; margin: 3rem auto; padding: 0 1rem; }\n</style>\n</head>\n<body>\n");
    let all = store.list()?;
    doc.push_str(&format!(
        "<h1>liber-rs export &middot; {} bookmark(s)</h1>\n",
        all.len()
    ));
    let mut folders: Vec<(String, Vec<&Bookmark>)> = Vec::new();
    for b in &all {
        match folders.iter_mut().find(|(f, _)| *f == b.folder) {
            Some((_, v)) => v.push(b),
            None => folders.push((b.folder.clone(), vec![b])),
        }
    }
    for (folder, members) in &folders {
        let label = if folder.is_empty() { "/" } else { folder };
        doc.push_str(&format!("<section><h2>{}</h2>\n<ul>\n", esc(label)));
        for b in members {
            let html = if b.html_file.is_empty() {
                String::new()
            } else {
                rel_link(out_dir, &store.cfg.html_dir().join(&b.html_file))
            };
            doc.push_str(&format!(
                "  <li><a href=\"{}\">{}</a>\n",
                esc(&html),
                esc(&b.title)
            ));
            if let Some(rel) = &b.markdown_file {
                doc.push_str(&format!(
                    "    <a href=\"{}\">md</a>\n",
                    esc(&rel_link(out_dir, &store.cfg.markdown_dir().join(rel)))
                ));
            }
            if let Some(rel) = &b.archive_file {
                doc.push_str(&format!(
                    "    <a href=\"{}\">arc</a>\n",
                    esc(&rel_link(out_dir, &store.cfg.archive_dir().join(rel)))
                ));
            }
            doc.push_str(&format!(
                "    <div><a href=\"{}\">{}</a>",
                esc(&b.url),
                esc(&b.url)
            ));
            for t in &b.tags {
                doc.push_str(&format!(" <span>#{}</span>", esc(t)));
            }
            doc.push_str(&format!(" &middot; {}</div>\n", b.uuid));
            if !b.description.is_empty() {
                doc.push_str(&format!("    <div>{}</div>\n", esc(&b.description)));
            }
            doc.push_str("  </li>\n");
        }
        doc.push_str("</ul></section>\n");
    }
    doc.push_str("</body>\n</html>\n");
    let out = out_dir.join("index.html");
    std::fs::write(&out, doc).map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config;

    fn seeded() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap();
        create_bookmark(
            &mut store,
            "https://example.com/a",
            CreateOptions {
                title: Some("A & B".to_string()),
                description: "line one\nline two".to_string(),
                tags: vec!["x".to_string()],
                folder: "tech".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        (store, dir)
    }

    #[test]
    fn netscape_roundtrip() {
        let (store, _dir) = seeded();
        let doc = write_netscape_export(&store).unwrap();
        assert!(doc.contains("<DT><H3>tech</H3>"));
        assert!(doc.contains("TAGS=\"x\""));
        assert!(doc.contains("A &amp; B"));
        assert!(doc.contains("<DD>line one line two"));
        let entries = crate::import::parse_netscape(&doc);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].folder, "tech");
        assert_eq!(entries[0].tags, vec!["x".to_string()]);
        assert_eq!(entries[0].desc, "line one line two");
        assert_eq!(entries[0].title, "A & B");
    }

    #[test]
    fn site_export_links() {
        let (store, _dir) = seeded();
        let out_dir = tempfile::tempdir().unwrap();
        let index = export_site(&store, out_dir.path()).unwrap();
        let html = std::fs::read_to_string(&index).unwrap();
        assert!(html.contains("<h2>tech</h2>"));
        assert!(html.contains("https://example.com/a"));
        assert!(html.contains(".html"));
    }
}
