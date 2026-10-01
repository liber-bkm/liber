use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use uuid::Uuid;

use crate::automation::host_of;
use crate::edit::{edit_bookmark, EditOptions};
use crate::model::Bookmark;
use crate::store::Store;
use crate::CoreError;

pub const QUARANTINE_FOLDER: &str = "quarantine";
const CHECK_UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Moved,
    Dead,
    Uncertain,
}

impl CheckStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Moved => "moved",
            CheckStatus::Dead => "dead",
            CheckStatus::Uncertain => "uncertain",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub detail: String,
    pub target: Option<String>,
}

impl CheckResult {
    fn ok() -> Self {
        Self {
            status: CheckStatus::Ok,
            detail: String::new(),
            target: None,
        }
    }
}

pub fn check_client() -> Result<Client, CoreError> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(Policy::none())
        .user_agent(CHECK_UA)
        .build()
        .map_err(|e| CoreError::Storage(e.to_string()))
}

fn short_err(e: &reqwest::Error) -> String {
    let s = e.to_string();
    let s = match s.rsplit_once(": ") {
        Some((_, tail)) => tail,
        None => s.as_str(),
    };
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.chars().take(120).collect()
}

fn resolve_ref(base: &str, loc: &str) -> String {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return loc.to_string();
    }
    if let Some(path) = loc.strip_prefix('/') {
        let (scheme, rest) = match base.find("://") {
            Some(i) => (&base[..i], &base[i + 3..]),
            None => ("https", base),
        };
        let host = match rest.find(['/', '?', '#']) {
            Some(i) => &rest[..i],
            None => rest,
        };
        return format!("{scheme}://{host}/{path}");
    }
    loc.to_string()
}

fn same_host(a: &str, b: &str) -> bool {
    let ha = host_of(a);
    !ha.is_empty() && ha == host_of(b)
}

fn status_of(code: u16) -> &'static str {
    match code {
        200 => "200 OK",
        404 => "404 Not Found",
        410 => "410 Gone",
        301 => "301 Moved Permanently",
        308 => "308 Permanent Redirect",
        405 => "405 Method Not Allowed",
        _ => "other",
    }
}

pub fn classify_url(client: &Client, raw_url: &str) -> CheckResult {
    let mut url = raw_url.to_string();
    for _ in 0..6 {
        let resp = match client.head(&url).send() {
            Ok(r) => r,
            Err(e) => {
                return CheckResult {
                    status: CheckStatus::Uncertain,
                    detail: short_err(&e),
                    target: None,
                }
            }
        };
        let code = resp.status().as_u16();
        if code == 404 || code == 410 {
            return CheckResult {
                status: CheckStatus::Dead,
                detail: status_of(code).to_string(),
                target: None,
            };
        }
        if code == 301 || code == 308 {
            let loc = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if loc.is_empty() {
                return CheckResult {
                    status: CheckStatus::Uncertain,
                    detail: format!("{} (no location)", status_of(code)),
                    target: None,
                };
            }
            return CheckResult {
                status: CheckStatus::Moved,
                detail: status_of(code).to_string(),
                target: Some(resolve_ref(&url, &loc)),
            };
        }
        if (300..400).contains(&code) {
            let loc = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if loc.is_empty() {
                return CheckResult {
                    status: CheckStatus::Uncertain,
                    detail: format!("{} (no location)", status_of(code)),
                    target: None,
                };
            }
            let next = resolve_ref(&url, &loc);
            if !same_host(raw_url, &next) {
                return CheckResult {
                    status: CheckStatus::Uncertain,
                    detail: format!("{} to different host", status_of(code)),
                    target: None,
                };
            }
            url = next;
            continue;
        }
        if code == 405 || code == 501 {
            break;
        }
        if (200..300).contains(&code) {
            return CheckResult::ok();
        }
        return CheckResult {
            status: CheckStatus::Uncertain,
            detail: status_of(code).to_string(),
            target: None,
        };
    }
    let resp = match client.get(&url).send() {
        Ok(r) => r,
        Err(e) => {
            return CheckResult {
                status: CheckStatus::Uncertain,
                detail: short_err(&e),
                target: None,
            }
        }
    };
    let code = resp.status().as_u16();
    if code == 404 || code == 410 {
        return CheckResult {
            status: CheckStatus::Dead,
            detail: status_of(code).to_string(),
            target: None,
        };
    }
    if code == 301 || code == 308 {
        let loc = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if loc.is_empty() {
            return CheckResult {
                status: CheckStatus::Uncertain,
                detail: format!("{} (no location)", status_of(code)),
                target: None,
            };
        }
        return CheckResult {
            status: CheckStatus::Moved,
            detail: status_of(code).to_string(),
            target: Some(resolve_ref(&url, &loc)),
        };
    }
    if (200..300).contains(&code) {
        return CheckResult::ok();
    }
    CheckResult {
        status: CheckStatus::Uncertain,
        detail: status_of(code).to_string(),
        target: None,
    }
}

#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub bookmark: Bookmark,
    pub result: CheckResult,
}

pub fn scan_targets(
    client: &Client,
    targets: Vec<Bookmark>,
    workers: usize,
    progress: impl Fn(usize, usize) + Sync,
) -> Vec<CheckOutcome> {
    let total = targets.len();
    let done = AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::with_capacity(total));
    let workers = workers.max(1);
    for chunk in targets.chunks(workers) {
        std::thread::scope(|s| {
            for b in chunk {
                let client = client.clone();
                let out = &out;
                let done = &done;
                let progress = &progress;
                s.spawn(move || {
                    let result = classify_url(&client, &b.url);
                    out.lock().unwrap().push(CheckOutcome {
                        bookmark: b.clone(),
                        result,
                    });
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    progress(n, total);
                });
            }
        });
    }
    let mut out = out.into_inner().unwrap();
    out.sort_by_key(|a| a.bookmark.uuid);
    out
}

pub fn resolve_check_targets(
    store: &Store,
    tokens: Option<&[String]>,
    stale: Option<Duration>,
) -> Result<(Vec<Bookmark>, usize), CoreError> {
    let mut targets = match tokens {
        Some(t) if !t.is_empty() => store.resolve_spec(t)?,
        _ => store.list()?,
    };
    let mut fresh = 0;
    if let Some(stale) = stale {
        let cutoff = chrono::Utc::now() - stale;
        targets.retain(|b| match b.last_checked_at {
            Some(t) if t > cutoff => {
                fresh += 1;
                false
            }
            _ => true,
        });
    }
    Ok((targets, fresh))
}

pub fn quarantine_bookmark(store: &mut Store, uuid: &Uuid) -> Result<bool, CoreError> {
    let Some(b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    if b.folder == QUARANTINE_FOLDER {
        return Ok(false);
    }
    edit_bookmark(
        store,
        uuid,
        EditOptions {
            folder: Some(QUARANTINE_FOLDER.to_string()),
            ..Default::default()
        },
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn test_server() -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let parts: Vec<&str> = request_line.split_whitespace().collect();
                let (method, path) = (parts[0], parts[1]);
                let body = match (method, path) {
                    ("HEAD", "/ok") => "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ("HEAD", "/gone") => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ("HEAD", "/moved") => "HTTP/1.1 301 Moved Permanently\r\nLocation: /ok\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ("HEAD", "/nohead") => "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ("GET", "/nohead") => "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nhi",
                    ("HEAD", "/foreign") => "HTTP/1.1 302 Found\r\nLocation: https://other.test/x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    _ => "HTTP/1.1 500 Oops\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                };
                stream.write_all(body.as_bytes()).unwrap();
            }
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn classify_vectors() {
        let (base, _srv) = test_server();
        let client = check_client().unwrap();
        let r = classify_url(&client, &format!("{base}/ok"));
        assert_eq!(r.status, CheckStatus::Ok);
        let r = classify_url(&client, &format!("{base}/gone"));
        assert_eq!(r.status, CheckStatus::Dead);
        let r = classify_url(&client, &format!("{base}/moved"));
        assert_eq!(r.status, CheckStatus::Moved);
        assert_eq!(r.target, Some(format!("{base}/ok")));
        let r = classify_url(&client, &format!("{base}/nohead"));
        assert_eq!(r.status, CheckStatus::Ok);
        let r = classify_url(&client, &format!("{base}/foreign"));
        assert_eq!(r.status, CheckStatus::Uncertain);
        let r = classify_url(&client, "http://127.0.0.1:1/unreachable");
        assert_eq!(r.status, CheckStatus::Uncertain);
    }
}
