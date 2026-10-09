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

