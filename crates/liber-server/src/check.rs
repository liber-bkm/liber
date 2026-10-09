use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::check::{
    parse_stale_duration, quarantine_bookmark, refetch_title, resolve_check_targets, scan_targets,
    CheckStatus,
};
use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
use liber_core::store::Store;

use crate::bookmarks::ApiErr;
use crate::AppState;

fn store_of(state: &AppState) -> Result<Store, ApiErr> {
    Store::open(crate::live_config(state)).map_err(|e| {
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
    pub stale: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v2/check/run",
    request_body(content = Object, description = "Scope as {spec?, workers?, stale?, stale_hours? (deprecated)}"),
    responses(
        (status = 200, description = "Checked, ok, and non-ok rows", body = Object),
        (status = 400, description = "Bad id spec or stale duration", body = Object),
    )
)]
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
    let stale = match (&input.stale, input.stale_hours) {
        (Some(s), _) if !s.trim().is_empty() => Some(parse_stale_duration(s).map_err(core_err)?),
        (_, Some(h)) => Some(Duration::from_secs(h.saturating_mul(3600))),
        _ => None,
    };
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
    #[serde(default)]
    pub retitle: bool,
}

#[derive(Deserialize, Default)]
pub struct ApplyBody {
    #[serde(default)]
    pub updates: Vec<ApplyUpdate>,
    #[serde(default)]
    pub quarantine: Vec<String>,
    #[serde(default)]
    pub delete: Vec<String>,
    #[serde(default)]
    pub confirm: bool,
}

fn resolve_uuid(store: &Store, spec: &str) -> Result<Option<uuid::Uuid>, ApiErr> {
    let tokens = liber_core::idspec::parse_id_spec(spec).map_err(core_err)?;
    match store.resolve_spec(&tokens) {
        Ok(mut hits) => Ok(hits.pop().map(|b| b.uuid)),
        Err(liber_core::CoreError::NotFound(_)) => Ok(None),
        Err(e) => Err(core_err(e)),
    }
}

#[utoipa::path(
    post,
    path = "/api/v2/check/apply",
    request_body(content = Object, description = "Actions as {updates[] ({uuid, url, retitle?}), quarantine[], delete[], confirm?}"),
    responses(
        (status = 200, description = "Updated, retitled, quarantined, deleted, and skipped rows; or a confirm gate", body = Object),
    )
)]
pub async fn apply_check(
    State(state): State<AppState>,
    Json(input): Json<ApplyBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    if !input.delete.is_empty() && !input.confirm {
        return Ok(Json(serde_json::json!({
            "confirm_required": true,
            "count": input.delete.len(),
            "hint": "repeat with confirm true to delete",
        })));
    }
    let retitle_urls: Vec<String> = input
        .updates
        .iter()
        .filter(|u| u.retitle)
        .map(|u| u.url.clone())
        .collect();
    let titles = tokio::task::spawn_blocking(move || {
        retitle_urls
            .iter()
            .map(|u| refetch_title(u))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    let mut title_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (u, t) in input.updates.iter().filter(|u| u.retitle).zip(titles) {
        title_of.insert(u.uuid.clone(), t);
    }
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let mut updated = 0;
    let mut retitled = 0;
    let mut quarantined = 0;
    let mut deleted = 0;
    let mut skipped = Vec::new();
    for u in &input.updates {
        let Some(uuid) = resolve_uuid(&store, &u.uuid)? else {
            skipped.push(serde_json::json!({"uuid": u.uuid, "reason": "not found"}));
            continue;
        };
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
                continue;
            }
            Err(e) => return Err(core_err(e)),
        }
        if u.retitle {
            if let Some(title) = title_of.remove(&u.uuid) {
                let current = store
                    .get(&uuid)
                    .map_err(core_err)?
                    .map(|b| b.title)
                    .unwrap_or_default();
                if !title.is_empty() && title != current {
                    edit_bookmark(
                        &mut store,
                        &uuid,
                        EditOptions {
                            title: Some(title),
                            ..Default::default()
                        },
                    )
                    .map_err(core_err)?;
                    retitled += 1;
                }
            }
        }
    }
    for q in &input.quarantine {
        let Some(uuid) = resolve_uuid(&store, q)? else {
            skipped.push(serde_json::json!({"uuid": q, "reason": "not found"}));
            continue;
        };
        if quarantine_bookmark(&mut store, &uuid).map_err(core_err)? {
            quarantined += 1;
        }
    }
    for d in &input.delete {
        let Some(uuid) = resolve_uuid(&store, d)? else {
            skipped.push(serde_json::json!({"uuid": d, "reason": "already gone"}));
            continue;
        };
        if delete_bookmark_with_files(&mut store, &uuid).map_err(core_err)? {
            deleted += 1;
        } else {
            skipped.push(serde_json::json!({"uuid": d, "reason": "already gone"}));
        }
    }
    Ok(Json(serde_json::json!({
        "updated": updated,
        "retitled": retitled,
        "quarantined": quarantined,
        "deleted": deleted,
        "skipped": skipped,
    })))
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
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        (build_router(AppState::new(cfg, String::new())), dir)
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

    #[tokio::test]
    async fn run_marks_uncertain_and_apply_quarantines() {
        let (app, _dir) = test_app();
        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "http://127.0.0.1:1/unreachable"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let (status, v) = body(
            app.clone()
                .oneshot(post("/api/v2/check/run", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["checked"], 1);
        assert_eq!(v["rows"][0]["status"], "uncertain");

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/apply",
                    serde_json::json!({"quarantine": [uuid]}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["quarantined"], 1);

        let res = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (_, v) = body(res).await;
        assert_eq!(v["folder"], "quarantine");
    }

    #[tokio::test]
    async fn run_accepts_stale_string_and_rejects_garbage() {
        let (app, _dir) = test_app();
        let (status, _) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "http://127.0.0.1:1/unreachable"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, v) = body(
            app.clone()
                .oneshot(post("/api/v2/check/run", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["checked"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/run",
                    serde_json::json!({"stale": "30d"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["checked"], 0);
        assert_eq!(v["fresh_skipped"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/run",
                    serde_json::json!({"stale_hours": 720}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["fresh_skipped"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/run",
                    serde_json::json!({"stale": "nope"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(v["error"].as_str().unwrap().contains("stale"));
    }

    #[tokio::test]
    async fn apply_delete_is_confirm_gated() {
        let (app, _dir) = test_app();
        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "http://127.0.0.1:1/unreachable"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/apply",
                    serde_json::json!({"delete": [uuid]}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["confirm_required"], true);
        assert_eq!(v["count"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/apply",
                    serde_json::json!({"delete": [uuid], "confirm": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["deleted"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/check/apply",
                    serde_json::json!({"updates": [{"uuid": uuid, "url": "https://example.com/x"}]}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["skipped"].as_array().unwrap().len(), 1);

        let res = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
