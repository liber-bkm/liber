use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use lol_html::{
    doc_text, element, end_tag, html_content::ContentType, text, HtmlRewriter, Settings,
};
use uuid::Uuid;

use crate::store::{Config, Store};
use crate::CoreError;

pub const MAX_PAGE_BYTES: u64 = 20 << 20;
pub const MAX_ASSET_BYTES: u64 = 10 << 20;
pub const MAX_TOTAL_BYTES: u64 = 50 << 20;
pub const ASSET_WORKERS: usize = 6;
pub const ARCHIVE_TIMEOUT_SECS: u64 = 45;
const ARCHIVE_UA: &str = "Mozilla/5.0 (compatible; liber-bookmark-manager/1.0)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Builtin,
    Browser,
    SingleFile,
    Monolith,
}

pub fn parse_backend(s: &str) -> Result<Option<Backend>, CoreError> {
    match s.to_lowercase().trim() {
        "" | "auto" => Ok(None),
        "builtin" | "native" => Ok(Some(Backend::Builtin)),
        "browser" => Ok(Some(Backend::Browser)),
        "single-file" | "singlefile" => Ok(Some(Backend::SingleFile)),
        "monolith" => Ok(Some(Backend::Monolith)),
        other => Err(CoreError::Invalid(format!(
            "unknown archive_backend {other:?} (expected auto, builtin, browser, single-file, or monolith)"
        ))),
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let p = PathBuf::from(name);
        if p.is_file() {
            return Some(p);
        }
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn find_browser(cfg: &Config) -> Option<PathBuf> {
    if !cfg.browser_path.trim().is_empty() {
        return find_on_path(cfg.browser_path.trim());
    }
    ["chromium", "chromium-browser", "google-chrome"]
        .into_iter()
        .find_map(find_on_path)
}

fn cmd_or_default(cfg_value: &str, fallback: &str) -> String {
    if cfg_value.trim().is_empty() {
        fallback.to_string()
    } else {
        cfg_value.trim().to_string()
    }
}

pub fn resolve_backend(
    cfg: &Config,
    override_backend: Option<&str>,
) -> Result<(Backend, Vec<String>), CoreError> {
    let mut warnings = Vec::new();
    let wanted = match override_backend {
        Some(s) => parse_backend(s)?,
        None => parse_backend(&cfg.archive_backend)?,
    };
    if let Some(b) = wanted {
        check_available(cfg, b)?;
        return Ok((b, warnings));
    }
    if cfg!(target_os = "android") {
        return Ok((Backend::Builtin, warnings));
    }
    if find_browser(cfg).is_some() {
        return Ok((Backend::Browser, warnings));
    }
    if !cfg.browser_path.trim().is_empty() {
        warnings.push(format!(
            "browser {:?} not found, trying external archivers",
            cfg.browser_path.trim()
        ));
    }
    let sf = cmd_or_default(&cfg.singlefile_cmd, "single-file");
    if find_on_path(&sf).is_some() {
        return Ok((Backend::SingleFile, warnings));
    }
    let mo = cmd_or_default(&cfg.monolith_cmd, "monolith");
    if find_on_path(&mo).is_some() {
        return Ok((Backend::Monolith, warnings));
    }
    warnings.push("no browser or external archiver found, using builtin snapshot".to_string());
    Ok((Backend::Builtin, warnings))
}

fn check_available(cfg: &Config, backend: Backend) -> Result<(), CoreError> {
    match backend {
        Backend::Builtin => Ok(()),
        Backend::Browser => match find_browser(cfg) {
            Some(_) => Ok(()),
            None => Err(CoreError::Invalid(
                "no browser found (set browser_path or install chromium)".to_string(),
            )),
        },
        Backend::SingleFile => {
            let sf = cmd_or_default(&cfg.singlefile_cmd, "single-file");
            match find_on_path(&sf) {
                Some(_) => Ok(()),
                None => Err(CoreError::Invalid(format!(
                    "{sf:?} not found in PATH (install single-file-cli or set singlefile_cmd)"
                ))),
            }
        }
        Backend::Monolith => {
            let mo = cmd_or_default(&cfg.monolith_cmd, "monolith");
            match find_on_path(&mo) {
                Some(_) => Ok(()),
                None => Err(CoreError::Invalid(format!(
                    "{mo:?} not found in PATH (install monolith or set monolith_cmd)"
                ))),
            }
        }
    }
}

#[derive(Clone)]
struct Fetched {
    data: Vec<u8>,
    mime: String,
}

pub fn mime_for(url: &str, header: Option<&str>) -> String {
    if let Some(h) = header {
        let mime = h.split(';').next().unwrap_or("").trim();
        if !mime.is_empty() {
            return mime.to_string();
        }
    }
    let path = url.rsplit(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "mp4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn capped_read(resp: reqwest::blocking::Response, cap: u64) -> Result<Vec<u8>, CoreError> {
    use std::io::Read;
    let mut data = Vec::new();
    let mut reader = resp;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            break;
        }
        if (data.len() + n) as u64 > cap {
            return Err(CoreError::Storage("response exceeds size cap".to_string()));
        }
        data.extend_from_slice(&buf[..n]);
    }
    Ok(data)
}

fn fetch_bytes(
    client: &reqwest::blocking::Client,
    url: &str,
    cap: u64,
) -> Result<(Vec<u8>, String), CoreError> {
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
    let mime = mime_for(
        url,
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
    );
    Ok((capped_read(resp, cap)?, mime))
}

fn resolve_asset(raw: &str, base: &url::Url) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty()
        || raw.starts_with("data:")
        || raw.starts_with("javascript:")
        || raw.starts_with('#')
    {
        return None;
    }
    let abs = base.join(raw).ok()?;
    if abs.scheme() != "http" && abs.scheme() != "https" {
        return None;
    }
    Some(abs.to_string())
}

fn css_url_refs(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = css;
    while let Some(i) = rest.find("url(") {
        rest = &rest[i + 4..];
        let end = rest.find(')').unwrap_or(rest.len());
        let raw = rest[..end]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim();
        if !raw.is_empty() {
            out.push(raw.to_string());
        }
        rest = &rest[end.min(rest.len())..];
        if rest.starts_with(')') {
            rest = &rest[1..];
        }
    }
    out
}

fn css_imports(css: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = css;
    while let Some(i) = rest.find("@import") {
        rest = &rest[i + 7..];
        let end = rest.find(';').map(|e| e + 1).unwrap_or(rest.len());
        let stmt = format!("@import{}", &rest[..end]);
        let inner = stmt
            .trim_start_matches("@import")
            .trim()
            .trim_end_matches(';')
            .trim();
        let raw = if let Some(t) = inner
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .or_else(|| inner.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')))
        {
            t.to_string()
        } else if let Some(u) = inner.strip_prefix("url(") {
            let u = u.trim_end_matches(')').trim();
            u.trim_matches(|c| c == '"' || c == '\'').trim().to_string()
        } else {
            String::new()
        };
        if !raw.is_empty() {
            out.push((stmt, raw));
        }
        rest = &rest[end.min(rest.len())..];
    }
    out
}

fn data_url(f: &Fetched) -> String {
    use base64::Engine;
    format!(
        "data:{};base64,{}",
        f.mime,
        base64::engine::general_purpose::STANDARD.encode(&f.data)
    )
}

fn replace_url_refs(css: &str, urls: &[String], map: &HashMap<String, Fetched>) -> String {
    let mut out = css.to_string();
    let mut sorted: Vec<&String> = urls.iter().collect();
    sorted.sort();
    sorted.dedup();
    for raw in sorted {
        let Some(f) = map.get(raw.as_str()) else {
            continue;
        };
        let replacement = format!("url(\"{}\")", data_url(f));
        out = out.replace(&format!("url({raw})"), &replacement);
        out = out.replace(&format!("url('{raw}')"), &replacement);
        out = out.replace(&format!("url(\"{raw}\")"), &replacement);
    }
    out
}

fn srcset_rewrite(value: &str, page_url: &url::Url, map: &HashMap<String, Fetched>) -> String {
    let mut parts = Vec::new();
    for cand in value.split(',') {
        let fields: Vec<&str> = cand.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        let mut first = fields[0].to_string();
        if let Some(abs) = resolve_asset(fields[0], page_url) {
            if let Some(f) = map.get(&abs) {
                first = data_url(f);
            }
        }
        let mut rebuilt = vec![first];
        rebuilt.extend(fields[1..].iter().map(|s| s.to_string()));
        parts.push(rebuilt.join(" "));
    }
    parts.join(", ")
}

struct Collected {
    urls: HashSet<String>,
    stylesheets: Vec<String>,
    iframes: Vec<String>,
}

fn collect_assets(html: &str, page_url: &url::Url) -> Result<Collected, CoreError> {
    let urls = Mutex::new(HashSet::new());
    let sheets = Mutex::new(Vec::new());
    let frames = Mutex::new(Vec::new());
    let remember = |raw: &str| {
        if let Some(abs) = resolve_asset(raw, page_url) {
            urls.lock().unwrap().insert(abs);
        }
    };
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!(
                    "img[src], source[src], video[src], audio[src], video[poster]",
                    |el| {
                        for attr in ["src", "poster"] {
                            if let Some(v) = el.get_attribute(attr) {
                                if let Some(abs) = resolve_asset(&v, page_url) {
                                    urls.lock().unwrap().insert(abs);
                                }
                            }
                        }
                        Ok(())
                    }
                ),
                element!("source[srcset], img[srcset]", |el| {
                    if let Some(v) = el.get_attribute("srcset") {
                        for cand in v.split(',') {
                            if let Some(first) = cand.split_whitespace().next() {
                                remember(first);
                            }
                        }
                    }
                    Ok(())
                }),
                element!("link[rel=stylesheet][href]", |el| {
                    if let Some(v) = el.get_attribute("href") {
                        if let Some(abs) = resolve_asset(&v, page_url) {
                            urls.lock().unwrap().insert(abs.clone());
                            sheets.lock().unwrap().push(abs);
                        }
                    }
                    Ok(())
                }),
                element!("link[rel~=icon][href]", |el| {
                    if let Some(v) = el.get_attribute("href") {
                        remember(&v);
                    }
                    Ok(())
                }),
                element!("[style]", |el| {
                    if let Some(v) = el.get_attribute("style") {
                        for raw in css_url_refs(&v) {
                            remember(&raw);
                        }
                    }
                    Ok(())
                }),
                element!("iframe[src]", |el| {
                    if let Some(v) = el.get_attribute("src") {
                        if let Some(abs) = resolve_asset(&v, page_url) {
                            frames.lock().unwrap().push(abs);
                        }
                    }
                    Ok(())
                }),
                text!("style", |t| {
                    for raw in css_url_refs(t.as_str()) {
                        remember(&raw);
                    }
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |_: &[u8]| {},
    );
    rewriter
        .write(html.as_bytes())
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    rewriter
        .end()
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(Collected {
        urls: urls.into_inner().unwrap(),
        stylesheets: sheets.into_inner().unwrap(),
        iframes: frames.into_inner().unwrap(),
    })
}

fn fetch_all(
    client: &reqwest::blocking::Client,
    mut urls: Vec<String>,
) -> HashMap<String, Fetched> {
    use std::sync::atomic::{AtomicU64, Ordering};
    urls.sort();
    urls.dedup();
    let total = AtomicU64::new(0);
    let out = Mutex::new(HashMap::new());
    for chunk in urls.chunks(ASSET_WORKERS.max(1)) {
        std::thread::scope(|s| {
            for url in chunk {
                let client = client.clone();
                let total = &total;
                let out = &out;
                s.spawn(move || {
                    if total.load(Ordering::SeqCst) >= MAX_TOTAL_BYTES {
                        return;
                    }
                    let Ok((data, mime)) = fetch_bytes(&client, url, MAX_ASSET_BYTES) else {
                        return;
                    };
                    let prev = total.fetch_add(data.len() as u64, Ordering::SeqCst);
                    if prev + data.len() as u64 > MAX_TOTAL_BYTES {
                        total.fetch_sub(data.len() as u64, Ordering::SeqCst);
                        return;
                    }
                    out.lock()
                        .unwrap()
                        .insert(url.clone(), Fetched { data, mime });
                });
            }
        });
    }
    out.into_inner().unwrap()
}

fn process_stylesheet(
    client: &reqwest::blocking::Client,
    sheet_url: &str,
    fetched: &mut HashMap<String, Fetched>,
) {
    let Ok((data, _)) = fetch_bytes(client, sheet_url, MAX_ASSET_BYTES) else {
        return;
    };
    let Ok(mut text) = String::from_utf8(data) else {
        return;
    };
    let Ok(sheet_base) = url::Url::parse(sheet_url) else {
        return;
    };
    for (stmt, raw) in css_imports(&text) {
        let Some(abs) = resolve_asset(&raw, &sheet_base) else {
            continue;
        };
        if let Ok((idata, _)) = fetch_bytes(client, &abs, MAX_ASSET_BYTES) {
            if let Ok(itext) = String::from_utf8(idata) {
                text = text.replacen(&stmt, &itext, 1);
            }
        }
    }
    let mut refs: Vec<String> = css_url_refs(&text);
    refs.sort();
    refs.dedup();
    let mut nested: HashMap<String, Fetched> = HashMap::new();
    for raw in &refs {
        if let Some(abs) = resolve_asset(raw, &sheet_base) {
            if let Some(f) = fetched.get(&abs) {
                nested.insert(raw.clone(), f.clone());
            }
        }
    }
    let mut missing: Vec<String> = Vec::new();
    for raw in &refs {
        if !nested.contains_key(raw) {
            if let Some(abs) = resolve_asset(raw, &sheet_base) {
                if !fetched.contains_key(&abs) {
                    missing.push(abs);
                }
            }
        }
    }
    for (url, f) in fetch_all(client, missing) {
        fetched.insert(url.clone(), f.clone());
        for raw in &refs {
            if resolve_asset(raw, &sheet_base).as_deref() == Some(url.as_str()) {
                nested.insert(raw.clone(), f.clone());
            }
        }
    }
    let rewritten = replace_url_refs(&text, &refs, &nested);
    fetched.insert(
        sheet_url.to_string(),
        Fetched {
            data: rewritten.into_bytes(),
            mime: "text/css".to_string(),
        },
    );
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn builtin_snapshot(page_url: &str, html: &str) -> Result<String, CoreError> {
    let base = url::Url::parse(page_url).map_err(|e| CoreError::Storage(e.to_string()))?;
    let collected = collect_assets(html, &base)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(ARCHIVE_TIMEOUT_SECS))
        .user_agent(ARCHIVE_UA)
        .build()
        .map_err(|e| CoreError::Storage(e.to_string()))?;

    let mut css_jobs: Vec<String> = collected.stylesheets.clone();
    css_jobs.sort();
    css_jobs.dedup();
    let page_urls: Vec<String> = collected.urls.into_iter().collect();
    let mut fetched = fetch_all(&client, page_urls);
    for sheet in &css_jobs {
        process_stylesheet(&client, sheet, &mut fetched);
    }

    for frame in &collected.iframes {
        if !fetched.contains_key(frame) {
            if let Ok((data, mime)) = fetch_bytes(&client, frame, 1 << 20) {
                if mime == "text/html" {
                    fetched.insert(frame.clone(), Fetched { data, mime });
                }
            }
        }
    }

    let mut output = Vec::new();
    let fetched_ref = &fetched;
    let style_buf = Mutex::new(String::new());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!("script", |el| {
                    el.remove();
                    Ok(())
                }),
                element!("noscript", |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }),
                element!(
                    "img[src], source[src], video[src], audio[src], video[poster]",
                    |el| {
                        for attr in ["src", "poster"] {
                            if let Some(v) = el.get_attribute(attr) {
                                if let Some(abs) = resolve_asset(&v, &base) {
                                    if let Some(f) = fetched_ref.get(&abs) {
                                        el.set_attribute(attr, &data_url(f))?;
                                    }
                                }
                            }
                        }
                        Ok(())
                    }
                ),
                element!("source[srcset], img[srcset]", |el| {
                    if let Some(v) = el.get_attribute("srcset") {
                        el.set_attribute("srcset", &srcset_rewrite(&v, &base, fetched_ref))?;
                    }
                    Ok(())
                }),
                element!("link[rel=stylesheet][href]", |el| {
                    if let Some(v) = el.get_attribute("href") {
                        if let Some(abs) = resolve_asset(&v, &base) {
                            let css = fetched_ref
                                .get(&abs)
                                .map(|f| String::from_utf8_lossy(&f.data).into_owned())
                                .unwrap_or_default();
                            el.replace(&format!("<style>{css}</style>"), ContentType::Html);
                        }
                    }
                    Ok(())
                }),
                element!("[style]", |el| {
                    if let Some(v) = el.get_attribute("style") {
                        let raws = css_url_refs(&v);
                        let mut by_raw = HashMap::new();
                        for raw in raws {
                            if let Some(abs) = resolve_asset(&raw, &base) {
                                if let Some(f) = fetched_ref.get(&abs) {
                                    by_raw.insert(raw, f.clone());
                                }
                            }
                        }
                        let refs: Vec<String> = by_raw.keys().cloned().collect();
                        el.set_attribute("style", &replace_url_refs(&v, &refs, &by_raw))?;
                    }
                    Ok(())
                }),
                element!("iframe[src]", |el| {
                    if let Some(v) = el.get_attribute("src") {
                        if let Some(abs) = resolve_asset(&v, &base) {
                            if let Some(f) = fetched_ref.get(&abs) {
                                let escaped = escape_attr(&String::from_utf8_lossy(&f.data));
                                el.set_attribute("srcdoc", &escaped)?;
                                el.remove_attribute("src");
                                el.set_attribute("sandbox", "")?;
                            }
                        }
                    }
                    Ok(())
                }),
                element!("*", |el| {
                    let names: Vec<String> = el.attributes().iter().map(|a| a.name()).collect();
                    for name in names {
                        if name.starts_with("on") {
                            el.remove_attribute(&name);
                        }
                    }
                    Ok(())
                }),
                text!("style", |t| {
                    style_buf.lock().unwrap().push_str(t.as_str());
                    if t.last_in_text_node() {
                        let full = std::mem::take(&mut *style_buf.lock().unwrap());
                        let mut by_raw = HashMap::new();
                        for raw in css_url_refs(&full) {
                            if let Some(abs) = resolve_asset(&raw, &base) {
                                if let Some(f) = fetched_ref.get(&abs) {
                                    by_raw.insert(raw, f.clone());
                                }
                            }
                        }
                        let refs: Vec<String> = by_raw.keys().cloned().collect();
                        t.replace(&replace_url_refs(&full, &refs, &by_raw), ContentType::Text);
                    }
                    Ok(())
                }),
            ],
            ..Settings::default()
        },
        |chunk: &[u8]| output.extend_from_slice(chunk),
    );
    rewriter
        .write(html.as_bytes())
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    rewriter
        .end()
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    let mut doc = String::from_utf8(output).map_err(|e| CoreError::Storage(e.to_string()))?;
    doc.insert_str(
        0,
        &format!("<!-- saved by liber-rs (builtin snapshot) from {page_url} -->\n"),
    );
    Ok(doc)
}

pub fn extract_archive_text(html: &str) -> String {
    let mut stripped = Vec::new();
    let mut stripper = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("script, style", |el| {
                el.remove();
                Ok(())
            })],
            ..Settings::default()
        },
        |chunk: &[u8]| stripped.extend_from_slice(chunk),
    );
    let _ = stripper.write(html.as_bytes());
    let _ = stripper.end();
    let text = Mutex::new(String::new());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            document_content_handlers: vec![doc_text!(|t| {
                text.lock().unwrap().push_str(t.as_str());
                text.lock().unwrap().push(' ');
                Ok(())
            })],
            ..Settings::default()
        },
        |_: &[u8]| {},
    );
    let _ = rewriter.write(&stripped);
    let _ = rewriter.end();
    let text = text.into_inner().unwrap();
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(1 << 20).collect()
}

pub fn extract_readable_text(html: &str) -> String {
    let mut stripped = Vec::new();
    let mut stripper = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!(
                "script, style, nav, aside, footer, header, form, noscript, iframe, select, button, input, dialog, menu",
                |el| {
                    el.remove();
                    Ok(())
                }
            )],
            ..Settings::default()
        },
        |chunk: &[u8]| stripped.extend_from_slice(chunk),
    );
    let _ = stripper.write(html.as_bytes());
    let _ = stripper.end();
    let mut marked = Vec::new();
    let mut marker = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!(
                "p, li, blockquote, pre, h1, h2, h3, h4, h5, h6",
                |el| {
                    el.prepend("\n\x02", ContentType::Text);
                    Ok(())
                }
            )],
            ..Settings::default()
        },
        |chunk: &[u8]| marked.extend_from_slice(chunk),
    );
    let _ = marker.write(&stripped);
    let _ = marker.end();
    let mut closed = Vec::new();
    let mut closer = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!(
                "p, li, blockquote, pre, h1, h2, h3, h4, h5, h6",
                |el| {
                    el.on_end_tag(end_tag!(move |end| {
                        end.after("\x03\n", ContentType::Text);
                        Ok(())
                    }))?;
                    Ok(())
                }
            )],
            ..Settings::default()
        },
        |chunk: &[u8]| closed.extend_from_slice(chunk),
    );
    let _ = closer.write(&marked);
    let _ = closer.end();
    let text = Mutex::new(String::new());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            document_content_handlers: vec![doc_text!(|t| {
                text.lock().unwrap().push_str(t.as_str());
                Ok(())
            })],
            ..Settings::default()
        },
        |_: &[u8]| {},
    );
    let _ = rewriter.write(&closed);
    let _ = rewriter.end();
    let text = text.into_inner().unwrap();
    let mut blocks = Vec::new();
    for part in text.split('\x02') {
        let seg = part.split('\x03').next().unwrap_or("");
        let line = seg.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.chars().count() >= 20 {
            blocks.push(line);
        }
    }
    let joined = blocks.join("\n\n");
    if blocks.is_empty() {
        return extract_archive_text(html);
    }
    joined.chars().take(1 << 20).collect()
}

fn fetch_page(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<(Vec<u8>, String), CoreError> {
    let resp = client
        .get(url)
        .send()
        .map_err(|e| CoreError::Storage(format!("fetching page: {e}")))?;
    if resp.status().as_u16() >= 400 {
        return Err(CoreError::Storage(format!(
            "fetching page: HTTP {}",
            resp.status()
        )));
    }
    let final_url = resp.url().to_string();
    Ok((capped_read(resp, MAX_PAGE_BYTES)?, final_url))
}

fn archive_client() -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(ARCHIVE_TIMEOUT_SECS))
        .user_agent(ARCHIVE_UA)
        .build()
        .map_err(|e| CoreError::Storage(e.to_string()))
}

fn builtin_for_url(client: &reqwest::blocking::Client, url: &str) -> Result<String, CoreError> {
    let (page, final_url) = fetch_page(client, url)?;
    let html = String::from_utf8_lossy(&page);
    builtin_snapshot(&final_url, &html)
}

fn write_archive_file(store: &Store, rel: &str, doc: &str) -> Result<(), CoreError> {
    let out_path = store.cfg.archive_dir().join(rel);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    std::fs::write(&out_path, doc).map_err(|e| CoreError::Storage(e.to_string()))
}

fn run_browser(cfg: &Config, url: &str) -> Result<String, CoreError> {
    let browser = find_browser(cfg).ok_or_else(|| {
        CoreError::Invalid("no browser found (set browser_path or install chromium)".to_string())
    })?;
    let out = std::process::Command::new(&browser)
        .args([
            "--headless",
            "--window-size=1920,1080",
            "--run-all-compositor-stages-before-draw",
            "--virtual-time-budget=9000",
            "--incognito",
            "--dump-dom",
            url,
        ])
        .output()
        .map_err(|e| CoreError::Storage(format!("starting browser: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(CoreError::Storage(format!(
            "browser failed: {}",
            err.trim()
        )));
    }
    let dom = String::from_utf8(out.stdout)
        .map_err(|e| CoreError::Storage(format!("browser output not UTF-8: {e}")))?;
    builtin_snapshot(url, &dom)
}

fn run_external(argv: Vec<String>, out_path: &std::path::Path) -> Result<(), CoreError> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let (cmd, args) = argv
        .split_first()
        .ok_or_else(|| CoreError::Invalid("empty command".to_string()))?;
    let out = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| CoreError::Storage(format!("starting {cmd}: {e}")))?;
    if !out.status.success() {
        let detail = String::from_utf8_lossy(&out.stderr);
        let detail = detail.trim();
        if detail.is_empty() {
            return Err(CoreError::Storage(format!(
                "{cmd} failed with status {}",
                out.status
            )));
        }
        return Err(CoreError::Storage(format!(
            "{cmd} failed with status {}: {detail}",
            out.status
        )));
    }
    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    if size < 100 {
        return Err(CoreError::Storage(format!(
            "{cmd} produced an empty archive ({size} bytes at {})",
            out_path.display()
        )));
    }
    Ok(())
}

pub fn archive_bookmark(
    store: &mut Store,
    uuid: &Uuid,
    backend_override: Option<&str>,
) -> Result<Vec<String>, CoreError> {
    let b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    let (backend, mut warnings) = resolve_backend(&store.cfg, backend_override)?;
    let file_base = b.html_file.rsplit('/').next().unwrap_or(&b.html_file);
    let rel = if b.folder.is_empty() {
        file_base.to_string()
    } else {
        format!("{}/{file_base}", b.folder)
    };
    let out_path = store.cfg.archive_dir().join(&rel);
    match backend {
        Backend::Builtin => {
            let client = archive_client()?;
            let doc = builtin_for_url(&client, &b.url)?;
            write_archive_file(store, &rel, &doc)?;
        }
        Backend::Browser => {
            let doc = match run_browser(&store.cfg, &b.url) {
                Ok(doc) => doc,
                Err(e) => {
                    warnings.push(format!(
                        "browser backend failed ({e}), using builtin snapshot"
                    ));
                    builtin_for_url(&archive_client()?, &b.url)?
                }
            };
            write_archive_file(store, &rel, &doc)?;
        }
        Backend::SingleFile => {
            let cmd = cmd_or_default(&store.cfg.singlefile_cmd, "single-file");
            let mut argv = vec![cmd];
            if !store.cfg.singlefile_browser_path.trim().is_empty() {
                argv.push(format!(
                    "--browser-executable-path={}",
                    store.cfg.singlefile_browser_path.trim()
                ));
            }
            argv.push(b.url.clone());
            argv.push(out_path.to_string_lossy().to_string());
            run_external(argv, &out_path)?;
        }
        Backend::Monolith => {
            let cmd = cmd_or_default(&store.cfg.monolith_cmd, "monolith");
            run_external(
                vec![
                    cmd,
                    b.url.clone(),
                    "-o".to_string(),
                    out_path.to_string_lossy().to_string(),
                ],
                &out_path,
            )?;
        }
    }
    let mut updated = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    updated.archive_file = Some(rel.clone());
    store.update_bookmark(&updated)?;
    let payload = serde_json::json!({"uuid": uuid.to_string(), "path": rel});
    store.append_oplog(Some(*uuid), "archive_add", payload)?;
    let _ = crate::search::reindex_one(store, uuid);
    Ok(warnings)
}

pub fn adopt_archive_path(store: &mut Store, uuid: &Uuid, path: &str) -> Result<(), CoreError> {
    let mut b = store
        .get(uuid)?
        .ok_or_else(|| CoreError::NotFound(uuid.to_string()))?;
    b.archive_file = Some(path.to_string());
    store.update_bookmark(&b)?;
    let payload = serde_json::json!({"uuid": uuid.to_string(), "path": path});
    store.append_oplog(Some(*uuid), "archive_add", payload)?;
    let _ = crate::search::reindex_one(store, uuid);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_PORT_OFFSET: AtomicUsize = AtomicUsize::new(0);

    fn test_site() -> (String, std::thread::JoinHandle<()>) {
        let offset = NEXT_PORT_OFFSET.fetch_add(1, Ordering::SeqCst);
        let _ = offset;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let parts: Vec<&str> = request_line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                let path = parts[1];
                let (mime, body): (&str, String) = match path {
                    "/page" => (
                        "text/html",
                        "<!doctype html><html><head><title>T</title><link rel=\"stylesheet\" href=\"/s.css\"></head><body><h1>Hi</h1><img src=\"/i.png\" srcset=\"/i.png 1x, /i2.png 2x\"><script>alert(1)</script><noscript><p>NS</p></noscript><a href=\"#\" onclick=\"evil()\">x</a><div style=\"background:url(/bg.png)\">s</div><iframe src=\"/frame\"></iframe></body></html>".to_string(),
                    ),
                    "/s.css" => (
                        "text/css",
                        "@import \"/more.css\";\nh1 { color: red; background: url(/bg.png); }".to_string(),
                    ),
                    "/more.css" => ("text/css", "p { font: url(/f.woff2); }".to_string()),
                    "/i.png" | "/i2.png" | "/bg.png" => ("image/png", "PNGDATA".to_string()),
                    "/f.woff2" => ("font/woff2", "FONTDATA".to_string()),
                    "/frame" => ("text/html", "<p>framed</p>".to_string()),
                    _ => ("text/plain", "nope".to_string()),
                };
                let status = if body == "nope" {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (format!("http://{addr}"), handle)
    }

    fn get(path: &str, base: &str) -> (u16, String, String) {
        use std::io::Read;
        let addr = base.strip_prefix("http://").unwrap();
        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: x\r\n\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let code: u16 = raw
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        let mime = raw
            .lines()
            .find(|l| l.to_lowercase().starts_with("content-type:"))
            .map(|l| l[13..].trim().to_string())
            .unwrap_or_default();
        (code, mime, body)
    }

    #[test]
    fn snapshot_inlines_everything() {
        let (base, _srv) = test_site();
        let (_, _, html) = get("/page", &base);
        let doc = builtin_snapshot(&format!("{base}/page"), &html).unwrap();
        assert!(doc.contains("liber-rs (builtin snapshot)"));
        for needle in [
            "src=\"http://127.0.0.1",
            "href=\"http://127.0.0.1",
            "url(http://127.0.0.1",
            "url(\"http://127.0.0.1",
        ] {
            assert!(!doc.contains(needle), "external ref remains: {needle}");
        }
        assert!(!doc.contains("<script"), "scripts stripped");
        assert!(doc.contains("<p>NS</p>"), "noscript unwrapped");
        assert!(!doc.contains("onclick"), "handlers dropped");
        assert!(doc.contains("data:image/png;base64,"), "images inlined");
        assert!(doc.contains("data:font/woff2;base64,"), "fonts inlined");
        assert!(doc.contains("color: red"), "stylesheets inlined");
        assert!(doc.contains("srcdoc="), "iframe embedded");
        assert!(doc.contains("sandbox"), "iframe sandboxed");
    }

    #[test]
    fn extract_text_skips_code() {
        let text = extract_archive_text(
            "<p>Hello</p><script>var x=1</script><style>p{}</style><p>World</p>",
        );
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn readable_drops_chrome_keeps_article() {
        let html = "<html><body><nav><ul><li>Home</li><li>About</li></ul></nav><article><h1>Real Title Here</h1><p>This is the actual article body with more than enough words to pass every length threshold in the extractor.</p><p>A second paragraph continues the story with further details and context for the reader.</p></article><footer>copyright 2026</footer></body></html>";
        let text = extract_readable_text(html);
        assert!(text.contains("actual article body"), "article kept: {text}");
        assert!(!text.contains("Home"), "nav dropped: {text}");
        assert!(!text.contains("copyright"), "footer dropped: {text}");
    }

    #[test]
    fn readable_falls_back_on_thin_pages() {
        let html = "<html><body><div>Just a short line of text here</div></body></html>";
        assert_eq!(extract_readable_text(html), extract_archive_text(html));
    }

    #[test]
    fn external_backend_uses_fake_binary() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let fake = bin.join("fake-single-file");
        std::fs::write(
            &fake,
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nprintf '<!DOCTYPE html><html><head><title>Fake</title></head><body><p>ARCHIVED snapshot body with enough bytes to pass the non-empty output gate.</p></body></html>' > \"$last\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            singlefile_cmd: fake.to_string_lossy().to_string(),
            ..Default::default()
        })
        .unwrap();
        let b = create_bookmark(
            &mut store,
            "https://example.com/x",
            CreateOptions {
                title: Some("X".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let warnings = archive_bookmark(&mut store, &b.uuid, Some("single-file")).unwrap();
        assert!(warnings.is_empty());
        let b = store.get(&b.uuid).unwrap().unwrap();
        let rel = b.archive_file.clone().unwrap();
        let content = std::fs::read_to_string(store.cfg.archive_dir().join(&rel)).unwrap();
        assert!(content.contains("ARCHIVED snapshot body"));
    }

    fn fake_tool(dir: &std::path::Path, name: &str, script: &str) -> String {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let fake = bin.join(name);
        std::fs::write(&fake, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        fake.to_string_lossy().to_string()
    }

    #[test]
    fn external_empty_output_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = fake_tool(
            dir.path(),
            "fake-empty",
            "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\n: > \"$last\"\n",
        );
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            singlefile_cmd: cmd,
            ..Default::default()
        })
        .unwrap();
        let b = create_bookmark(
            &mut store,
            "https://example.com/x",
            CreateOptions {
                title: Some("X".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let err = archive_bookmark(&mut store, &b.uuid, Some("single-file"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty archive"), "unexpected: {err}");
        assert!(store.get(&b.uuid).unwrap().unwrap().archive_file.is_none());
    }

    #[test]
    fn external_failure_reports_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = fake_tool(
            dir.path(),
            "fake-fail",
            "#!/bin/sh\necho 'boom: no such browser' >&2\nexit 3\n",
        );
        let mut store = Store::open_in_memory(Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            monolith_cmd: cmd,
            ..Default::default()
        })
        .unwrap();
        let b = create_bookmark(
            &mut store,
            "https://example.com/x",
            CreateOptions {
                title: Some("X".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let err = archive_bookmark(&mut store, &b.uuid, Some("monolith"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("boom: no such browser"), "unexpected: {err}");
    }

    #[test]
    fn backend_selection() {
        let cfg = Config {
            archive_backend: "builtin".to_string(),
            ..Default::default()
        };
        assert_eq!(resolve_backend(&cfg, None).unwrap().0, Backend::Builtin);
        assert!(resolve_backend(&cfg, Some("bogus")).is_err());
        let cfg = Config {
            singlefile_cmd: "definitely-not-a-real-binary-xyz".to_string(),
            ..Default::default()
        };
        let err = resolve_backend(&cfg, Some("single-file"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not found in PATH"), "unexpected: {err}");
    }
}
