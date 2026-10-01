use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::check::{quarantine_bookmark, resolve_check_targets, scan_targets, CheckStatus};
use liber_core::edit::{edit_bookmark, EditOptions};
use liber_core::store::Store;

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
pub struct RunBody {
    pub spec: Option<String>,
    pub workers: Option<usize>,
    pub stale_hours: Option<u64>,
}

pub async fn run_check(
    State(state): State<AppState>,
    Json(input): Json<RunBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = store_of(&state)?;
    let tokens = match &input.spec {
        Some(s) if !s.trim().is_empty() => {
            Some(liber_core::idspec::parse_id_spec(s).map_err(core_err)?)
        }
        _ => None,
    };
    let stale = input
        .stale_hours
        .map(|h| Duration::from_secs(h.saturating_mul(3600)));
    let (targets, fresh) =
        resolve_check_targets(&store, tokens.as_deref(), stale).map_err(core_err)?;
    let total = targets.len();
    let workers = input.workers.unwrap_or(12).max(1);
    let outcomes = tokio::task::spawn_blocking(move || {
        let client = liber_core::check::check_client()?;
        Ok::<_, liber_core::CoreError>(scan_targets(&client, targets, workers, |_, _| {}))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?
    .map_err(core_err)?;
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let mut rows = Vec::new();
    let mut ok = 0;
    for o in &outcomes {
        store
            .stamp_check(&o.bookmark.uuid, Some(o.result.status.as_str()))
            .map_err(core_err)?;
        match o.result.status {
            CheckStatus::Ok => ok += 1,
            _ => rows.push(serde_json::json!({
                "uuid": o.bookmark.uuid.to_string(),
                "title": o.bookmark.title,
                "url": o.bookmark.url,
                "status": o.result.status.as_str(),
                "detail": o.result.detail,
                "target": o.result.target,
            })),
        }
    }
    Ok(Json(serde_json::json!({
        "checked": total,
        "ok": ok,
        "fresh_skipped": fresh,
        "rows": rows,
    })))
}

#[derive(Deserialize)]
pub struct ApplyUpdate {
    pub uuid: String,
    pub url: String,
}

#[derive(Deserialize, Default)]
pub struct ApplyBody {
    #[serde(default)]
    pub updates: Vec<ApplyUpdate>,
    #[serde(default)]
    pub quarantine: Vec<String>,
}

pub async fn apply_check(
    State(state): State<AppState>,
    Json(input): Json<ApplyBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let mut updated = 0;
    let mut quarantined = 0;
    let mut skipped = Vec::new();
    for u in &input.updates {
        let tokens = liber_core::idspec::parse_id_spec(&u.uuid).map_err(core_err)?;
        let mut hits = store.resolve_spec(&tokens).map_err(core_err)?;
        if hits.is_empty() {
            skipped.push(serde_json::json!({"uuid": u.uuid, "reason": "not found"}));
            continue;
        }
        let uuid = hits.remove(0).uuid;
        match edit_bookmark(
            &mut store,
            &uuid,
            EditOptions {
                url: Some(u.url.clone()),
                ..Default::default()
            },
        ) {
            Ok(_) => updated += 1,
            Err(liber_core::CoreError::Duplicate(_)) => {
                skipped.push(
                    serde_json::json!({"uuid": u.uuid, "reason": "target already bookmarked"}),
                );
            }
            Err(e) => return Err(core_err(e)),
        }
    }
    for q in &input.quarantine {
        let tokens = liber_core::idspec::parse_id_spec(q).map_err(core_err)?;
        let mut hits = store.resolve_spec(&tokens).map_err(core_err)?;
        if hits.is_empty() {
            skipped.push(serde_json::json!({"uuid": q, "reason": "not found"}));
            continue;
        }
        let uuid = hits.remove(0).uuid;
        if quarantine_bookmark(&mut store, &uuid).map_err(core_err)? {
            quarantined += 1;
        }
    }
    Ok(Json(serde_json::json!({
        "updated": updated,
        "quarantined": quarantined,
        "skipped": skipped,
    })))
}

