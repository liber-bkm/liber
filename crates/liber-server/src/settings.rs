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
    Ok(Json(serde_json::json!({"ok": true})))
}

