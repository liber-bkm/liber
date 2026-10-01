use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
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

#[derive(Deserialize)]
pub struct BulkBody {
    pub ids: Vec<String>,
    pub op: String,
    pub tags: Option<Vec<String>>,
    pub folder: Option<String>,
    #[serde(default)]
    pub confirm: bool,
}

pub async fn bulk(
    State(state): State<AppState>,
    Json(input): Json<BulkBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    if input.ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "no bookmarks selected"})),
        ));
    }
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let mut targets = Vec::new();
    for id in &input.ids {
        let tokens = liber_core::idspec::parse_id_spec(id).map_err(core_err)?;
        let mut hits = store.resolve_spec(&tokens).map_err(core_err)?;
        if hits.is_empty() {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("not found: {id}")})),
            ));
        }
        targets.push(hits.remove(0));
    }
    match input.op.as_str() {
        "delete" => {
            if !input.confirm {
                return Ok(Json(serde_json::json!({
                    "confirm_required": true,
                    "count": targets.len(),
                    "hint": "repeat with confirm true to delete",
                })));
            }
            for b in &targets {
                delete_bookmark_with_files(&mut store, &b.uuid).map_err(core_err)?;
            }
            Ok(Json(serde_json::json!({"deleted": targets.len()})))
        }
        "set_tags" => {
            let tags = input.tags.unwrap_or_default();
            for b in &targets {
                edit_bookmark(
                    &mut store,
                    &b.uuid,
                    EditOptions {
                        tags: Some(tags.clone()),
                        ..Default::default()
                    },
                )
                .map_err(core_err)?;
            }
            Ok(Json(serde_json::json!({"updated": targets.len()})))
        }
        "move_folder" => {
            let folder = input.folder.unwrap_or_default();
            for b in &targets {
                edit_bookmark(
                    &mut store,
                    &b.uuid,
                    EditOptions {
                        folder: Some(folder.clone()),
                        ..Default::default()
                    },
                )
                .map_err(core_err)?;
            }
            Ok(Json(serde_json::json!({"updated": targets.len()})))
        }
        other => Err((
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"error": format!("unknown bulk op {other:?} (delete, set_tags, move_folder)")}),
            ),
        )),
    }
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

    async fn add(app: &axum::Router, url: &str) -> String {
        let (status, v) = body(
            app.clone()
                .oneshot(post("/api/v2/bookmarks", serde_json::json!({"url": url})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        v["uuid"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn bulk_set_tags_and_move() {
        let (app, _dir) = test_app();
        let a = add(&app, "https://example.com/a").await;
        let b = add(&app, "https://example.com/b").await;
        let ids = vec![a.clone(), b.clone()];

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bulk",
                    serde_json::json!({"ids": ids, "op": "set_tags", "tags": ["x"]}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["updated"], 2);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bulk",
                    serde_json::json!({"ids": [a], "op": "move_folder", "folder": "tech"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["updated"], 1);
    }

    #[tokio::test]
    async fn bulk_delete_confirms() {
        let (app, _dir) = test_app();
        let a = add(&app, "https://example.com/a").await;
        let b = add(&app, "https://example.com/b").await;
        let ids = vec![a, b];

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bulk",
                    serde_json::json!({"ids": ids, "op": "delete"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["confirm_required"], true);

        let (status, v) = body(
            app.oneshot(post(
                "/api/v2/bulk",
                serde_json::json!({"ids": ids, "op": "delete", "confirm": true}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["deleted"], 2);
    }
}
