use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use liber_core::store::Store;
use liber_core::taxonomy::{
    delete_folder, delete_tag, folder_counts, rename_folder, rename_tag, tag_counts,
};

use crate::bookmarks::ApiErr;
use crate::AppState;

fn open_store(state: &AppState) -> Result<Store, ApiErr> {
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

pub async fn list_tags(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = open_store(&state)?;
    let counts = tag_counts(&store).map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "tags": counts.iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect::<Vec<_>>(),
    })))
}

pub async fn list_folders(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = open_store(&state)?;
    let counts = folder_counts(&store).map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "folders": counts.iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct RenameBody {
    pub old: String,
    pub new: String,
}

pub async fn rename_tag_ep(
    State(state): State<AppState>,
    Json(input): Json<RenameBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let changed = rename_tag(&mut store, &input.old, &input.new).map_err(core_err)?;
    if changed.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no bookmarks have that tag"})),
        ));
    }
    Ok(Json(serde_json::json!({"renamed": changed.len()})))
}

#[derive(Deserialize)]
pub struct DeleteTagBody {
    pub tag: String,
    #[serde(default)]
    pub confirm: bool,
}

pub async fn delete_tag_ep(
    State(state): State<AppState>,
    Json(input): Json<DeleteTagBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    if input.tag.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "tag is required"})),
        ));
    }
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    if !input.confirm {
        let counts = tag_counts(&store).map_err(core_err)?;
        let found = counts
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(input.tag.trim()));
        let Some((_, count)) = found else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "no bookmarks have that tag"})),
            ));
        };
        return Ok(Json(serde_json::json!({
            "confirm_required": true,
            "count": count,
            "hint": "repeat with confirm true to delete",
        })));
    }
    let changed = delete_tag(&mut store, &input.tag).map_err(core_err)?;
    Ok(Json(serde_json::json!({"deleted": changed.len()})))
}

pub async fn rename_folder_ep(
    State(state): State<AppState>,
    Json(input): Json<RenameBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let changed = rename_folder(&mut store, &input.old, &input.new).map_err(core_err)?;
    if changed.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no bookmarks in that folder"})),
        ));
    }
    Ok(Json(serde_json::json!({"renamed": changed.len()})))
}

#[derive(Deserialize)]
pub struct DeleteFolderBody {
    pub folder: String,
}

pub async fn delete_folder_ep(
    State(state): State<AppState>,
    Json(input): Json<DeleteFolderBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let changed = delete_folder(&mut store, &input.folder).map_err(core_err)?;
    Ok(Json(serde_json::json!({"moved_to_root": changed.len()})))
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

    async fn seed(app: &axum::Router) {
        for (url, tags, folder) in [
            ("https://example.com/a", vec!["rust"], "tech"),
            ("https://example.com/b", vec!["rust", "x"], "tech/sub"),
        ] {
            let (status, _) = body(
                app.clone()
                    .oneshot(post(
                        "/api/v2/bookmarks",
                        serde_json::json!({"url": url, "tags": tags, "folder": folder}),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }
    }

    #[tokio::test]
    async fn tags_flow() {
        let (app, _dir) = test_app();
        seed(&app).await;
        let (_, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/tags")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(v["tags"][0]["name"], "rust");

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/tags/delete",
                    serde_json::json!({"tag": "rust"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["confirm_required"], true);
        assert_eq!(v["count"], 2);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/tags/rename",
                    serde_json::json!({"old": "rust", "new": "go"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["renamed"], 2);

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/tags/delete",
                    serde_json::json!({"tag": "x", "confirm": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["deleted"], 1);
    }

    #[tokio::test]
    async fn folders_flow() {
        let (app, _dir) = test_app();
        seed(&app).await;
        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/folders/rename",
                    serde_json::json!({"old": "tech", "new": "dev"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["renamed"], 2);

        let (_, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/folders")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        let names: Vec<&str> = v["folders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"dev"));
        assert!(names.contains(&"dev/sub"));

        let (status, v) = body(
            app.oneshot(post(
                "/api/v2/folders/delete",
                serde_json::json!({"folder": "dev"}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["moved_to_root"], 2);
    }
}
