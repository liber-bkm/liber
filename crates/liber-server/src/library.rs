use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::Json;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use liber_core::export::{export_site, write_netscape_export};
use liber_core::import::import_data;
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
pub struct ImportBody {
    pub content: String,
    #[serde(default)]
    pub markdown: bool,
    #[serde(default)]
    pub archive: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/library/import",
    request_body(content = Object, description = "Netscape file as {content, markdown?, archive?}"),
    responses(
        (status = 200, description = "Added, skipped, and warning counts", body = Object),
    )
)]
pub async fn import_library(
    State(state): State<AppState>,
    Json(input): Json<ImportBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let report =
        import_data(&mut store, &input.content, input.markdown, input.archive).map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "added": report.added,
        "skipped_dup": report.skipped_dup,
        "skipped_bad": report.skipped_bad,
        "warnings": report.warnings,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::{build_router, AppState};
    use liber_core::store::Config;

    const SAMPLE: &str = "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<DL><p>\n<DT><H3>tech</H3>\n<DL><p>\n<DT><A HREF=\"https://example.com/a\" TAGS=\"rust\">A</A>\n</DL><p>\n</DL><p>\n";

    fn test_app() -> (axum::Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        (build_router(AppState::new(cfg, String::new())), dir)
    }

    async fn body(res: axum::response::Response) -> (StatusCode, Vec<u8>) {
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec();
        (status, bytes)
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
    async fn import_then_export() {
        let (app, _dir) = test_app();
        let (status, bytes) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/library/import",
                    serde_json::json!({"content": SAMPLE}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["added"], 1);

        let (status, bytes) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/library/export-bookmarks")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let doc = String::from_utf8(bytes).unwrap();
        assert!(doc.contains("https://example.com/a"));
        assert!(doc.contains("TAGS=\"rust\""));

        let (status, bytes) = body(
            app.oneshot(post("/api/v2/library/export-site", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["index"].as_str().unwrap().ends_with("index.html"));
    }

    #[tokio::test]
    async fn import_with_archive_reports_failure_as_warning() {
        let (app, _dir) = test_app();
        let sample = "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<DL><p>\n<DT><A HREF=\"http://127.0.0.1:1/unreachable\">Down</A>\n</DL><p>\n";
        let (status, bytes) = body(
            app.oneshot(post(
                "/api/v2/library/import",
                serde_json::json!({"content": sample, "archive": true}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["added"], 1);
        let warnings = v["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].as_str().unwrap().contains("archive failed"));
    }
}

#[utoipa::path(
    get,
    path = "/api/v2/library/export-bookmarks",
    responses(
        (status = 200, description = "Netscape bookmark file download", content_type = "text/html"),
    )
)]
pub async fn export_bookmarks(State(state): State<AppState>) -> Result<Response, ApiErr> {
    let store = store_of(&state)?;
    let doc = write_netscape_export(&store).map_err(core_err)?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"liber-bookmarks.html\"",
            ),
        ],
        doc,
    )
        .into_response())
}

#[derive(Deserialize, Default)]
pub struct ExportSiteBody {
    pub dir: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v2/library/export-site",
    request_body(content = Object, description = "Target as {dir?}"),
    responses(
        (status = 200, description = "Written index path", body = Object),
    )
)]
pub async fn export_site_ep(
    State(state): State<AppState>,
    Json(input): Json<ExportSiteBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let out_dir = match &input.dir {
        Some(d) if !d.trim().is_empty() => std::path::PathBuf::from(d.trim()),
        _ => state.cfg.profile_dir().join("site"),
    };
    let store = store_of(&state)?;
    let index = export_site(&store, &out_dir).map_err(core_err)?;
    Ok(Json(serde_json::json!({"index": index.to_string_lossy()})))
}
