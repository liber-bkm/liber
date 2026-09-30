use std::path::Path;

use crate::model::Bookmark;
use crate::CoreError;

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

pub fn render_html(b: &Bookmark) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str(&format!("<title>{}</title>\n", esc(&b.title)));
    out.push_str(&format!(
        "<meta name=\"liber:uuid\" content=\"{}\">\n",
        b.uuid
    ));
    out.push_str(&format!(
        "<meta name=\"liber:url\" content=\"{}\">\n",
        esc(&b.url)
    ));
    out.push_str(&format!(
        "<meta name=\"liber:folder\" content=\"{}\">\n",
        esc(&b.folder)
    ));
    out.push_str(&format!(
        "<meta name=\"liber:tags\" content=\"{}\">\n",
        esc(&b.tags.join(", "))
    ));
    out.push_str(&format!(
        "<meta name=\"liber:created\" content=\"{}\">\n",
        b.created_at.to_rfc3339()
    ));
    out.push_str("<style>\n  body { font-family: system-ui, sans-serif; max-width: 640px; margin: 3rem auto; padding: 0 1rem; }\n</style>\n</head>\n<body>\n");
    out.push_str(&format!(
        "  <h1><a href=\"{}\">{}</a></h1>\n",
        esc(&b.url),
        esc(&b.title)
    ));
    if !b.description.is_empty() {
        out.push_str(&format!(
            "  <p class=\"desc\">{}</p>\n",
            esc(&b.description)
        ));
    }
    out.push_str("  <div>");
    for t in &b.tags {
        out.push_str(&format!("<span class=\"tag\">#{}</span>", esc(t)));
    }
    out.push_str("</div>\n");
    out.push_str(&format!(
        "  <p class=\"meta\">Saved {}{} &middot; {}</p>\n",
        b.created_at.format("%b %-d, %Y"),
        if b.folder.is_empty() {
            String::new()
        } else {
            format!(" &middot; {}", esc(&b.folder))
        },
        b.uuid
    ));
    out.push_str("</body>\n</html>\n");
    out
}

pub fn render_markdown(b: &Bookmark, body: &str) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: {:?}\n", b.title));
    out.push_str(&format!("url: {:?}\n", b.url));
    out.push_str(&format!("uuid: {}\n", b.uuid));
    out.push_str(&format!("date: {}\n", b.created_at.to_rfc3339()));
    if !b.folder.is_empty() {
        out.push_str(&format!("folder: {:?}\n", b.folder));
    }
    if !b.tags.is_empty() {
        out.push_str("tags:\n");
        for t in &b.tags {
            out.push_str(&format!("  - {t}\n"));
        }
    }
    out.push_str("---\n\n");
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn default_markdown_body(b: &Bookmark) -> String {
    let mut body = format!("# {}\n\n", b.title);
    if !b.description.is_empty() {
        body.push_str(&b.description);
        body.push_str("\n\n");
    }
    body.push_str(&format!("[Visit original]({})\n", b.url));
    body
}

fn write_atomic(path: &Path, data: &str) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| CoreError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| CoreError::Storage(e.to_string()))
}

pub fn write_html_bookmark(path: &Path, b: &Bookmark) -> Result<(), CoreError> {
    write_atomic(path, &render_html(b))
}

pub fn write_markdown_bookmark(path: &Path, b: &Bookmark) -> Result<(), CoreError> {
    write_atomic(path, &render_markdown(b, &default_markdown_body(b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn sample() -> Bookmark {
        Bookmark {
            uuid: Uuid::new_v4(),
            url: "https://example.com/?a=1&b=2".to_string(),
            title: "A <b>title</b>".to_string(),
            description: "Fish & chips".to_string(),
            tags: vec!["x".to_string()],
            folder: "tech".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            html_file: String::new(),
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
    fn html_escapes_and_embeds_uuid() {
        let b = sample();
        let html = render_html(&b);
        assert!(html.contains("&lt;b&gt;"));
        assert!(html.contains("Fish &amp; chips"));
        assert!(html.contains(&b.uuid.to_string()));
    }

    #[test]
    fn markdown_roundtrip_write() {
        let dir = tempfile::tempdir().unwrap();
        let b = sample();
        let path = dir.path().join("note.md");
        write_markdown_bookmark(&path, &b).unwrap();
        let data = std::fs::read_to_string(&path).unwrap();
        assert!(data.starts_with("---\n"));
        assert!(data.contains(&b.uuid.to_string()));
    }
}
