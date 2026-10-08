use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::reindex::{reindex, ReindexFlags};
use liber_core::store::Store;

use crate::bookmarks::ApiErr;
use crate::AppState;

#[derive(Deserialize, Default)]
pub struct ReindexBody {
    #[serde(default)]
    pub prune: bool,
    #[serde(default)]
    pub compact_ids: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/reindex",
    request_body(content = Object, description = "Repair flags as {prune?, compact_ids?}"),
    responses(
        (status = 200, description = "Reindex report counts", body = Object),
    )
)]
pub async fn reindex_ep(
    State(state): State<AppState>,
    Json(input): Json<ReindexBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = Store::open(state.cfg.clone()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    let rep = reindex(
        &mut store,
        ReindexFlags {
            prune: input.prune,
            compact_ids: input.compact_ids,
        },
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    Ok(Json(serde_json::json!({
        "adopted": rep.adopted,
        "relinked_markdown": rep.relinked_markdown,
        "relinked_archive": rep.relinked_archive,
        "swept_conflicts": rep.swept_conflicts,
        "quarantined_attachments": rep.quarantined_attachments,
        "pending": rep.pending,
        "pruned": rep.pruned,
        "indexed": rep.indexed,
        "short_ids_compacted": rep.short_ids_compacted,
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

    #[tokio::test]
    async fn reindex_report_shape() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let app = build_router(AppState::new(cfg, String::new()));
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v2/reindex")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for key in [
            "adopted",
            "relinked_markdown",
            "relinked_archive",
            "swept_conflicts",
            "quarantined_attachments",
            "pending",
            "pruned",
            "indexed",
            "short_ids_compacted",
        ] {
            assert!(v.get(key).is_some(), "missing report key {key}");
        }
        assert_eq!(v["adopted"], 0);
        assert_eq!(v["pruned"], 0);
    }
}
