use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use crate::bookmarks::ApiErr;
use crate::AppState;

const SETTABLE: &[&str] = &[
    "base_dir",
    "archive_backend",
    "browser_path",
    "singlefile_cmd",
    "singlefile_browser_path",
    "monolith_cmd",
    "browser_cmd",
    "device_id",
];

#[utoipa::path(
    get,
    path = "/api/v2/settings",
    responses(
        (status = 200, description = "Settable config values", body = Object),
    )
)]
pub async fn get_settings() -> Result<Json<serde_json::Value>, ApiErr> {
    let (cfg, _) = liber_core::config::load_config().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok(Json(serde_json::json!({
        "base_dir": cfg.base_dir.to_string_lossy(),
        "device_id": cfg.device_id,
        "archive_backend": cfg.archive_backend,
        "browser_path": cfg.browser_path,
        "singlefile_cmd": cfg.singlefile_cmd,
        "singlefile_browser_path": cfg.singlefile_browser_path,
        "monolith_cmd": cfg.monolith_cmd,
        "browser_cmd": cfg.browser_cmd,
        "active_profile": cfg.active_profile,
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
        (status = 400, description = "Unknown key or bad backend", body = Object),
    )
)]
pub async fn set_setting(
    State(state): State<AppState>,
    Json(input): Json<SetSettingsBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    if !SETTABLE.contains(&input.key.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown key {:?}", input.key)})),
        ));
    }
    if input.key == "archive_backend" {
        liber_core::archive::parse_backend(&input.value).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
    }
    let _guard = state.write_mu.lock().await;
    let (mut cfg, path) = liber_core::config::load_config().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    match input.key.as_str() {
        "base_dir" => cfg.base_dir = input.value.into(),
        "archive_backend" => cfg.archive_backend = input.value,
        "browser_path" => cfg.browser_path = input.value,
        "singlefile_cmd" => cfg.singlefile_cmd = input.value,
        "singlefile_browser_path" => cfg.singlefile_browser_path = input.value,
        "monolith_cmd" => cfg.monolith_cmd = input.value,
        "browser_cmd" => cfg.browser_cmd = input.value,
        "device_id" => cfg.device_id = input.value,
        _ => unreachable!(),
    }
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

    #[tokio::test]
    async fn settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("LIBER_CONFIG", dir.path().join("config.json"));
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let app = build_router(AppState::new(cfg, String::new()));
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v2/settings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        "{\"key\":\"archive_backend\",\"value\":\"builtin\"}",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ok"], true);
        std::env::remove_var("LIBER_CONFIG");
    }
}
