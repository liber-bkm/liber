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
}

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
    let rep = reindex(&mut store, ReindexFlags { prune: input.prune }).map_err(|e| {
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
    })))
}
