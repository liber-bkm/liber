use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use crate::bookmarks::ApiErr;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/v2/settings",
    responses(
        (status = 200, description = "Settable config values plus maintenance status", body = Object),
    )
)]
pub async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let cfg = crate::live_config(&state);
    let err = |e: liber_core::CoreError| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    };
    let status = liber_core::store::Store::open(cfg.clone())
        .map_err(err)?
        .maintenance_status()
        .map_err(err)?;
    Ok(Json(serde_json::json!({
        "base_dir": cfg.base_dir.to_string_lossy(),
        "html_dir": cfg.html_dir,
        "markdown_dir": cfg.markdown_dir,
        "archive_dir": cfg.archive_dir,
        "attachment_dir": cfg.attachment_dir,
        "device_id": cfg.device_id,
        "archive_backend": cfg.archive_backend,
        "browser_cmd": cfg.browser_cmd,
        "browser_path": cfg.browser_path,
        "editor_cmd": cfg.editor_cmd,
        "singlefile_cmd": cfg.singlefile_cmd,
        "singlefile_browser_path": cfg.singlefile_browser_path,
        "monolith_cmd": cfg.monolith_cmd,
        "active_profile": cfg.active_profile,
        "maintenance_status": status,
    })))
}

#[derive(Deserialize)]
pub struct SetSettingsBody {
    pub key: String,
    pub value: String,
}

#[utoipa::path(
    put,
    path = "/api/v2/settings",
    request_body(content = Object, description = "Setting as {key, value}"),
    responses(
        (status = 200, description = "Saved", body = Object),
        (status = 400, description = "Unknown key or bad value", body = Object),
    )
)]
pub async fn set_setting(
    State(state): State<AppState>,
    Json(input): Json<SetSettingsBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    if !liber_core::config::API_SETTABLE.contains(&input.key.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown key {:?}", input.key)})),
        ));
    }
    let _guard = state.write_mu.lock().await;
    let (mut cfg, path) = liber_core::config::load_config().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    liber_core::config::apply_setting(&mut cfg, &input.key, &input.value).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    liber_core::config::save_config_to(&path, &cfg).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    *state.cfg.write().unwrap() = cfg;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request};
    use tower::ServiceExt;

    use crate::{build_router, AppState};
    use liber_core::store::Config;

    fn test_app() -> (axum::Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::env::set_var("LIBER_CONFIG", &path);
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        liber_core::config::save_config_to(&path, &cfg).unwrap();
        (build_router(AppState::new(cfg, String::new())), dir)
    }

    fn put(key: &str, value: &str) -> Request<Body> {
        Request::builder()
            .method("PUT")
            .uri("/api/v2/settings")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"key": key, "value": value}).to_string(),
            ))
            .unwrap()
    }

    async fn get_json(app: axum::Router) -> (StatusCode, serde_json::Value) {
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn put_json(
        app: axum::Router,
        key: &str,
        value: &str,
    ) -> (StatusCode, serde_json::Value) {
        let res = app.oneshot(put(key, value)).await.unwrap();
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or_default())
    }

    #[tokio::test]
    async fn settings_roundtrip() {
        let (app, _dir) = test_app();
        let (status, _) = get_json(app.clone()).await;
        assert_eq!(status, StatusCode::OK);

        let (status, v) = put_json(app.clone(), "archive_backend", "builtin").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["ok"], true);

        let (status, v) = get_json(app.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["archive_backend"], "builtin");
        assert_eq!(v["maintenance_status"]["bookmarks"], 0);
        assert_eq!(v["maintenance_status"]["oplog_entries"], 0);
        std::env::remove_var("LIBER_CONFIG");
    }

    #[tokio::test]
    async fn settings_widened_keys_validate() {
        let (app, _dir) = test_app();
        let (status, v) = put_json(app.clone(), "editor_cmd", "hx").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["ok"], true);

        let (status, v) = put_json(app.clone(), "html_dir", "/tmp/html").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["ok"], true);

        let (status, v) = get_json(app.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["editor_cmd"], "hx");
        assert_eq!(v["html_dir"], "/tmp/html");

        for (key, value) in [
            ("archive_backend", "nope"),
            ("html_dir", "   "),
            ("auth_token", "secret"),
            ("bogus", "x"),
        ] {
            let (status, v) = put_json(app.clone(), key, value).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "accepts {key:?}");
            assert!(v["error"].is_string(), "missing error for {key:?}");
        }
        std::env::remove_var("LIBER_CONFIG");
    }
}
