use crate::model::OpLogEntry;
use crate::sync::MergeReport;
use crate::CoreError;

fn base(server: &str) -> String {
    server.trim_end_matches('/').to_string()
}

fn client() -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| CoreError::Storage(format!("starting http client: {e}")))
}

fn post_json(
    url: &str,
    token: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, CoreError> {
    let payload = serde_json::to_string(&body).map_err(|e| CoreError::Storage(e.to_string()))?;
    let mut req = client()?
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload);
    let token = token.trim();
    if !token.is_empty() {
        req = req.header(
            "Authorization",
            format!("Bearer {}", crate::auth::auth_mac(token, "liber-bearer-v1")),
        );
    }
    let resp = req
        .send()
        .map_err(|e| CoreError::Storage(format!("request to {url} failed: {e}")))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(CoreError::Invalid(
            "server requires a token (see --token or LIBER_AUTH_TOKEN)".to_string(),
        ));
    }
    if !status.is_success() {
        let detail: String = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(CoreError::Storage(format!(
            "server answered {status}: {detail}"
        )));
    }
    serde_json::from_str(&text).map_err(|e| CoreError::Storage(format!("bad server reply: {e}")))
}

pub fn pull_bundle(
    server: &str,
    token: &str,
    since: Option<i64>,
) -> Result<Vec<OpLogEntry>, CoreError> {
    let v = post_json(
        &format!("{}/api/v2/sync/export", base(server)),
        token,
        serde_json::json!({"since": since}),
    )?;
    serde_json::from_value(v.get("entries").cloned().unwrap_or_default())
        .map_err(|e| CoreError::Storage(format!("bad bundle reply: {e}")))
}

pub fn push_bundle(
    server: &str,
    token: &str,
    entries: &[OpLogEntry],
) -> Result<MergeReport, CoreError> {
    let v = post_json(
        &format!("{}/api/v2/sync/import", base(server)),
        token,
        serde_json::json!({"entries": entries}),
    )?;
    serde_json::from_value(v).map_err(|e| CoreError::Storage(format!("bad merge reply: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn mock_once(
        handle: impl FnOnce(&str, &HashMap<String, String>, &str) -> (u16, String) + Send + 'static,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = HashMap::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim();
                if line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    headers.insert(k.trim().to_lowercase(), v.trim().to_string());
                }
            }
            let len: usize = headers
                .get("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).unwrap();
            let (status, reply) = handle(
                request_line.trim(),
                &headers,
                &String::from_utf8_lossy(&body),
            );
            let reason = if status == 200 {
                "OK"
            } else if status == 401 {
                "Unauthorized"
            } else {
                "Error"
            };
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
        });
        format!("http://{addr}")
    }

    #[test]
    fn pull_posts_export_with_bearer() {
        let base = mock_once(|request, headers, body| {
            assert!(
                request.starts_with("POST /api/v2/sync/export "),
                "{request}"
            );
            let expected = format!("Bearer {}", crate::auth::auth_mac("tok", "liber-bearer-v1"));
            assert_eq!(
                headers.get("authorization").map(String::as_str),
                Some(expected.as_str())
            );
            let v: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(v["since"], serde_json::Value::Null);
            (200, r#"{"entries": []}"#.to_string())
        });
        let entries = pull_bundle(&format!("{base}/"), "tok", None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn push_posts_entries_and_reads_report() {
        let base = mock_once(|request, headers, body| {
            assert!(
                request.starts_with("POST /api/v2/sync/import "),
                "{request}"
            );
            assert!(headers.get("authorization").is_none());
            let v: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(v["entries"].as_array().unwrap().len(), 0);
            (200, r#"{"merged": 0, "inserted": 0, "deduped": 0, "deleted": 0, "rules": 0, "renumbered": 0, "short_ids_assigned": 0, "short_ids_compacted": 0}"#.to_string())
        });
        let rep = push_bundle(&base, "", &[]).unwrap();
        assert_eq!(rep.inserted, 0);
        assert_eq!(rep.short_ids_compacted, 0);
    }

    #[test]
    fn unauthorized_suggests_token() {
        let base = mock_once(|_, _, _| (401, r#"{"error": "auth"}"#.to_string()));
        let err = pull_bundle(&base, "", None).unwrap_err();
        assert!(matches!(err, CoreError::Invalid(_)));
        assert!(err.to_string().contains("--token"));
    }
}
