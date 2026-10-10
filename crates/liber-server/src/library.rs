use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Json;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use liber_core::backup::{create_backup, restore_backup};
use liber_core::export::{export_site, write_netscape_export};
use liber_core::import::import_data;
use liber_core::reindex::{reindex, ReindexFlags};
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
pub struct ImportBody {
    pub content: String,
    #[serde(default)]
    pub markdown: bool,
    #[serde(default)]
    pub archive: bool,
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

#[utoipa::path(
    get,
    path = "/api/v2/library/backup",
    responses(
        (status = 200, description = "Full library tarball download", content_type = "application/gzip"),
    )
)]
pub async fn backup_library(State(state): State<AppState>) -> Result<Response, ApiErr> {
    let cfg = crate::live_config(&state);
    let err = |e: liber_core::CoreError| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    };
    let tmp = std::env::temp_dir().join(format!(
        "liber-backup-{}.tar.gz",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    create_backup(&cfg.profile_dir(), &tmp, &cfg.effective_device_id()).map_err(err)?;
    let data = std::fs::read(&tmp).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;
    let _ = std::fs::remove_file(&tmp);
    let name = format!(
        "liber-backup-{}.tar.gz",
        chrono::Utc::now().format("%Y-%m-%d")
    );
    let disposition = format!("attachment; filename=\"{name}\"");
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/gzip"),
            (header::CONTENT_DISPOSITION, disposition.as_str()),
        ],
        data,
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/v2/favicons/{host}",
    params(("host" = String, Path, description = "Hostname to fetch the icon for")),
    responses(
        (status = 200, description = "Site icon bytes", content_type = "image/png"),
        (status = 404, description = "No usable icon", body = Object),
    )
)]
pub async fn get_favicon(State(state): State<AppState>, Path(host): Path<String>) -> Response {
    let cfg = crate::live_config(&state);
    let dir = cfg.profile_dir();
    if let Some(hit) = liber_core::favicon::cached_icon(&dir, &host.to_lowercase()) {
        return file_response(&hit).await;
    }
    let fetched =
        tokio::task::spawn_blocking(move || liber_core::favicon::fetch_host_icon(&dir, &host))
            .await;
    match fetched {
        Ok(Ok(path)) => file_response(&path).await,
        _ => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "no favicon",
        )
            .into_response(),
    }
}

async fn file_response(path: &std::path::Path) -> Response {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_string();
    match tokio::fs::read(path).await {
        Ok(data) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, liber_core::favicon::icon_mime(&ext)),
                (header::CACHE_CONTROL, "public, max-age=86400"),
            ],
            data,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain")],
            "no favicon",
        )
            .into_response(),
    }
}
#[derive(Deserialize, Default)]
pub struct RestoreBody {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub confirm: bool,
}

pub const RESTORE_MAX_BYTES: u64 = 256 * 1024 * 1024;

#[utoipa::path(
    post,
    path = "/api/v2/library/restore",
    request_body(content = Object, description = "Backup as {content (base64 tarball), confirm?}"),
    responses(
        (status = 200, description = "Restored manifest plus reindex counts, or a confirm gate", body = Object),
        (status = 400, description = "Bad base64 or bad backup", body = Object),
        (status = 413, description = "Backup larger than 256 MiB", body = Object),
    )
)]
pub async fn restore_library(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiErr> {
    use base64::Engine;
    let len: u64 = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if len > RESTORE_MAX_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({"error": "backup larger than 256 MiB"})),
        ));
    }
    let input: RestoreBody = serde_json::from_slice(&body).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "body must be JSON {content, confirm?}"})),
        )
    })?;
    if !input.confirm {
        return Ok(Json(serde_json::json!({
            "confirm_required": true,
            "hint": "repeat with confirm true to replace the library",
        })));
    }
    let _guard = state.write_mu.lock().await;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(input.content.trim())
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "content is not valid base64"})),
            )
        })?;
    let tmp = std::env::temp_dir().join(format!(
        "liber-restore-{}.tar.gz",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let result = (|| -> Result<_, ApiErr> {
        std::fs::write(&tmp, &raw).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;
        let cfg = crate::live_config(&state);
        let manifest = restore_backup(&tmp, &cfg.profile_dir()).map_err(core_err)?;
        let mut store = store_of(&state)?;
        let rep = reindex(
            &mut store,
            ReindexFlags {
                prune: false,
                compact_ids: false,
            },
        )
        .map_err(core_err)?;
        Ok((manifest, rep))
    })();
    let _ = std::fs::remove_file(&tmp);
    let (manifest, rep) = result?;
    Ok(Json(serde_json::json!({
        "restored_from": manifest.device_id,
        "created_at": manifest.created_at,
        "adopted": rep.adopted,
        "indexed": rep.indexed,
        "pending": rep.pending,
    })))
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
    let cfg = crate::live_config(&state);
    let report = tokio::task::spawn_blocking(move || {
        let mut store = Store::open(cfg)?;
        import_data(&mut store, &input.content, input.markdown, input.archive)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?
    .map_err(core_err)?;
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

    fn put(uri: &str, v: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("PUT")
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

    async fn seed_bookmark(app: axum::Router) -> String {
        let (status, bytes) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["uuid"].as_str().unwrap().to_string()
    }

    async fn download_backup(app: axum::Router) -> Vec<u8> {
        let (status, bytes) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/library/backup")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        bytes
    }

    #[tokio::test]
    async fn backup_download_is_gzip_tarball() {
        let (app, dir) = test_app();
        seed_bookmark(app.clone()).await;
        let bytes = download_backup(app.clone()).await;
        assert!(bytes.starts_with(&[0x1f, 0x8b]));
        let file = dir.path().join("api-backup.tar.gz");
        std::fs::write(&file, &bytes).unwrap();
        let manifest = liber_core::backup::read_manifest(&file).unwrap();
        assert_eq!(manifest.device_id, "test-device");
    }

    #[tokio::test]
    async fn restore_is_confirm_gated_and_validates() {
        let (app, _dir) = test_app();
        let (status, bytes) = body(
            app.clone()
                .oneshot(post("/api/v2/library/restore", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["confirm_required"], true);

        let (status, _) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/library/restore",
                    serde_json::json!({"content": "!!!", "confirm": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn restore_roundtrip_replaces_library() {
        use base64::Engine;
        let (app, _dir) = test_app();
        let uuid = seed_bookmark(app.clone()).await;
        let bytes = download_backup(app.clone()).await;
        let content = base64::engine::general_purpose::STANDARD.encode(&bytes);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}?confirm=true"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let (status, bytes) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/library/restore",
                    serde_json::json!({"content": content, "confirm": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["restored_from"], "test-device");

        let (status, bytes) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/bookmarks")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["total"], 1);
    }

    #[tokio::test]
    async fn favicon_rejects_bad_host_and_misses_cleanly() {
        let (app, _dir) = test_app();
        for uri in [
            "/api/v2/favicons/",
            "/api/v2/favicons/bad%20host",
            "/api/v2/favicons/127.0.0.1",
        ] {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert!(
                res.status() == StatusCode::NOT_FOUND || res.status() == StatusCode::BAD_REQUEST,
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn deep_search_returns_snippets() {
        let (app, _dir) = test_app();
        let (status, bytes) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let uuid = v["uuid"].as_str().unwrap().to_string();
        assert!(v.get("snippet").is_none());

        let (status, _) = body(
            app.clone()
                .oneshot(put(
                    format!("/api/v2/bookmarks/{uuid}/notes").as_str(),
                    serde_json::json!({"body": "wombat husbandry notes"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = body(
            app.clone()
                .oneshot(post("/api/v2/reindex", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, bytes) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/bookmarks?q=wombat&deep=true")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let rows = v["bookmarks"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let snippet = rows[0]["snippet"].as_str().unwrap();
        assert!(snippet.contains("<b>wombat</b>"), "{snippet}");
        assert!(!snippet.contains("uuid:"), "frontmatter leaked: {snippet}");
    }
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
        _ => crate::live_config(&state).profile_dir().join("site"),
    };
    let store = store_of(&state)?;
    let index = export_site(&store, &out_dir).map_err(core_err)?;
    Ok(Json(serde_json::json!({"index": index.to_string_lossy()})))
}
