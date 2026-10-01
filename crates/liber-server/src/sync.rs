use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::store::Store;
use liber_core::sync::{export_bundle, prune_oplog, replay_entries};

use crate::bookmarks::ApiErr;
use crate::AppState;

fn store_of(state: &AppState) -> Result<Store, ApiErr> {
    Store::open(state.cfg.clone()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })
}

fn core_err(e: liber_core::CoreError) -> ApiErr {
    let status = match &e {
        liber_core::CoreError::NotFound(_) => StatusCode::NOT_FOUND,
        liber_core::CoreError::Duplicate(_) => StatusCode::CONFLICT,
        liber_core::CoreError::Invalid(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({"error": e.to_string()})))
}

#[derive(Deserialize, Default)]
pub struct ExportBody {
    pub since: Option<i64>,
}

pub async fn export_oplog(
    State(state): State<AppState>,
    Json(input): Json<ExportBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = store_of(&state)?;
    let entries = export_bundle(&store, input.since).map_err(core_err)?;
    Ok(Json(serde_json::json!({"entries": entries})))
}

#[derive(Deserialize)]
pub struct ImportBody {
    pub entries: Vec<liber_core::model::OpLogEntry>,
}

pub async fn import_oplog(
    State(state): State<AppState>,
    Json(input): Json<ImportBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let rep = replay_entries(&mut store, &input.entries).map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "merged": rep.merged,
        "inserted": rep.inserted,
        "deduped": rep.deduped,
        "deleted": rep.deleted,
        "rules": rep.rules,
    })))
}

#[derive(Deserialize, Default)]
pub struct PruneBody {
    pub days: Option<i64>,
}

pub async fn prune_oplog_ep(
    State(state): State<AppState>,
    Json(input): Json<PruneBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let n = prune_oplog(&mut store, input.days.unwrap_or(90)).map_err(core_err)?;
    Ok(Json(serde_json::json!({"pruned": n})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request};
    use tower::ServiceExt;

    use crate::{build_router, AppState};
    use liber_core::store::Config;

    fn test_app(dir: &tempfile::TempDir) -> axum::Router {
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        build_router(AppState::new(cfg, String::new()))
    }

    async fn body(res: axum::response::Response) -> (StatusCode, serde_json::Value) {
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or_default())
    }

    fn post(uri: &str, v: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap()
    }
        assert_eq!(status, StatusCode::CREATED);
