use std::path::{Path, PathBuf};

use crate::CoreError;

pub const MAX_ICON_BYTES: usize = 64 * 1024;
const PAGE_HEAD_BYTES: usize = 512 * 1024;

fn icon_client() -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; liber-bookmark-manager/1.0)")
        .build()
        .map_err(|e| CoreError::Storage(format!("starting http client: {e}")))
}

fn capped_get(
    client: &reqwest::blocking::Client,
    url: &str,
    cap: usize,
) -> Result<Vec<u8>, CoreError> {
    let resp = client
        .get(url)
        .send()
        .map_err(|e| CoreError::Storage(format!("fetching {url}: {e}")))?;
    if resp.status().as_u16() >= 400 {
        return Err(CoreError::Storage(format!(
            "fetching {url}: HTTP {}",
            resp.status()
        )));
    }
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut reader = resp;
    loop {
        use std::io::Read;
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&chunk[..n]);
                if data.len() > cap {
                    return Err(CoreError::Storage(format!("{url} exceeds size cap")));
                }
            }
            Err(e) => return Err(CoreError::Storage(format!("reading {url}: {e}"))),
        }
    }
    Ok(data)
}

pub fn host_of(page_url: &str) -> Result<String, CoreError> {
    let url = url::Url::parse(page_url)
        .map_err(|e| CoreError::Invalid(format!("bad url {page_url:?}: {e}")))?;
    url.host_str()
        .map(|h| h.to_lowercase())
        .ok_or_else(|| CoreError::Invalid(format!("url has no host: {page_url:?}")))
}

fn sanitize_host(host: &str) -> String {
    host.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || !c.is_ascii() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn ext_of_magic(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if data.starts_with(b"GIF8") {
        Some("gif")
    } else if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        Some("ico")
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

pub fn icon_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

const ICON_EXTS: &[&str] = &["png", "jpg", "gif", "ico", "webp"];

pub fn favicon_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join(".liber").join("favicons")
}

pub fn cached_icon(profile_dir: &Path, host: &str) -> Option<PathBuf> {
    let stem = sanitize_host(host);
    ICON_EXTS
        .iter()
        .map(|ext| favicon_dir(profile_dir).join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

fn icon_href(html: &[u8]) -> Option<String> {
    let found = std::sync::Mutex::new(None::<String>);
    let mut rewriter = lol_html::HtmlRewriter::new(
        lol_html::Settings {
            element_content_handlers: vec![lol_html::element!("link", |el| {
                let rel = el.get_attribute("rel").unwrap_or_default().to_lowercase();
                let is_icon = rel
                    .split_whitespace()
                    .any(|t| t == "icon" || t == "shortcut");
                if is_icon && found.lock().unwrap().is_none() {
                    if let Some(href) = el.get_attribute("href") {
                        let href = href.trim().to_string();
                        let lower = href.to_lowercase();
                        if !href.is_empty()
                            && !lower.ends_with(".svg")
                            && !lower.starts_with("data:")
                        {
                            *found.lock().unwrap() = Some(href);
                        }
                    }
                }
                Ok(())
            })],
            ..lol_html::Settings::default()
        },
        |_: &[u8]| {},
    );
    let _ = rewriter.write(html);
    let _ = rewriter.end();
    found.into_inner().unwrap()
}

pub fn fetch_host_icon(profile_dir: &Path, host: &str) -> Result<PathBuf, CoreError> {
    let host = host.trim().to_lowercase();
    if host.is_empty() || host.contains(['/', '\\', ' ', ':', '?', '#', '@']) {
        return Err(CoreError::Invalid(format!("bad hostname: {host:?}")));
    }
    if let Some(hit) = cached_icon(profile_dir, &host) {
        return Ok(hit);
    }
    match fetch_icon(profile_dir, &format!("http://{host}/")) {
        Ok(path) => Ok(path),
        Err(_) => fetch_icon(profile_dir, &format!("https://{host}/")),
    }
}

pub fn fetch_icon(profile_dir: &Path, page_url: &str) -> Result<PathBuf, CoreError> {
    let host = host_of(page_url)?;
    if let Some(hit) = cached_icon(profile_dir, &host) {
        return Ok(hit);
    }
    let client = icon_client()?;
    let mut candidates = Vec::new();
    if let Ok(head) = capped_get(&client, page_url, PAGE_HEAD_BYTES) {
        if let Some(href) = icon_href(&head) {
            if let Ok(abs) = url::Url::parse(page_url).and_then(|u| u.join(&href)) {
                candidates.push(abs.to_string());
            }
        }
    }
    if let Ok(base) = url::Url::parse(page_url) {
        if let Ok(abs) = base.join("/favicon.ico") {
            candidates.push(abs.to_string());
        }
    }
    for url in candidates {
        let data = match capped_get(&client, &url, MAX_ICON_BYTES) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let Some(ext) = ext_of_magic(&data) else {
            continue;
        };
        let dir = favicon_dir(profile_dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| CoreError::Storage(format!("creating favicon dir: {e}")))?;
        let path = dir.join(format!("{}.{ext}", sanitize_host(&host)));
        std::fs::write(&path, &data)
            .map_err(|e| CoreError::Storage(format!("saving favicon: {e}")))?;
        return Ok(path);
    }
    Err(CoreError::Storage(format!(
        "no usable favicon for {page_url}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn test_server() -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0, 0, 0, 0];
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(12) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let parts: Vec<&str> = request_line.split_whitespace().collect();
                let (_m, path) = (parts[0], parts[1]);
                let mut content_len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("content-length") {
                            content_len = v.trim().parse().unwrap_or(0);
                        }
                    }
                }
                if content_len > 0 {
                    let mut discard = vec![0u8; content_len];
                    reader.read_exact(&mut discard).unwrap();
                }
                let (ok, body): (bool, &[u8]) = match path {
                    "/page" => (true, b"<html><head><link rel=\"icon\" href=\"/icon.png\"></head><body>x</body></html>"),
                    "/icon.png" => (true, png.as_slice()),
                    "/svgicon" => (true, b"<svg></svg>"),
                    "/svgpage" => (true, b"<html><head><link rel=\"icon\" href=\"/svgicon\"></head><body>y</body></html>"),
                    "/favicon.ico" => (true, &[0x00, 0x00, 0x01, 0x00, 0, 0, 0, 0]),
                    _ => (false, &[]),
                };
                let status = if ok {
                    "HTTP/1.1 200 OK"
                } else {
                    "HTTP/1.1 404 Not Found"
                };
                let head = format!(
                    "{status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn host_sanitize_vectors() {
        assert_eq!(sanitize_host("Example.COM"), "example.com");
        assert_eq!(sanitize_host("a/b?c"), "a_b_c");
        assert_eq!(
            host_of("https://Example.COM:8080/x").unwrap(),
            "example.com"
        );
        assert!(host_of("not a url").is_err());
    }

    #[test]
    fn link_icon_preferred_over_fallback() {
        let (base, _srv) = test_server();
        let dir = tempfile::tempdir().unwrap();
        let path = fetch_icon(dir.path(), &format!("{base}/page")).unwrap();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        let again = fetch_icon(dir.path(), &format!("{base}/page")).unwrap();
        assert_eq!(path, again);
    }

    #[test]
    fn svg_icon_rejected_falls_back_to_ico() {
        let (base, _srv) = test_server();
        let dir = tempfile::tempdir().unwrap();
        let path = fetch_icon(dir.path(), &format!("{base}/svgpage")).unwrap();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("ico"));
    }

    #[test]
    fn unreachable_host_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fetch_icon(dir.path(), "http://127.0.0.1:1/none").is_err());
        assert!(cached_icon(dir.path(), "127.0.0.1").is_none());
    }
}
