use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;

use liber_core::store::Store;
use liber_core::sync::{export_bundle, git_snapshot, prune_oplog, replay_entries};

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
pub struct ExportBody {
    pub since: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v2/sync/export",
    request_body(content = Object, description = "Cursor as {since?}"),
    responses(
        (status = 200, description = "Oplog entries", body = Object),
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/v2/sync/import",
    request_body(content = Object, description = "Bundle as {entries[]}"),
    responses(
        (status = 200, description = "Merge report counts", body = Object),
        (status = 400, description = "Bad bundle", body = Object),
    )
)]
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
        "renumbered": rep.renumbered,
        "short_ids_assigned": rep.short_ids_assigned,
        "short_ids_compacted": rep.short_ids_compacted,
        "artifacts": rep.artifacts,
        "skipped": rep.skipped,
    })))
}

#[derive(Deserialize, Default)]
pub struct CommitBody {
    #[serde(default)]
    pub push: bool,
}

#[utoipa::path(
    post,
    path = "/api/v2/sync/commit",
    request_body(content = Object, description = "Snapshot options as {push?}, empty body means no push"),
    responses(
        (status = 200, description = "Snapshot output, errors carried in body", body = Object),
    )
)]
pub async fn commit_snapshot(
    State(state): State<AppState>,
    body: Option<Json<CommitBody>>,
) -> Json<serde_json::Value> {
    let push = body.map(|Json(b)| b.push).unwrap_or(false);
    let _guard = state.write_mu.lock().await;
    let dir = crate::live_config(&state).profile_dir();
    let message = chrono::Utc::now()
        .format("liber sync: %Y-%m-%d %H:%M:%S")
        .to_string();
    let result = tokio::task::spawn_blocking(move || git_snapshot(&dir, &message, push)).await;
    match result {
        Err(e) => Json(serde_json::json!({"output": "", "error": e.to_string()})),
        Ok(Err(e)) => Json(serde_json::json!({"output": "", "error": e.to_string()})),
        Ok(Ok(false)) => Json(serde_json::json!({
            "output": "Not a git repository, nothing committed (repos are never initialized automatically).",
        })),
        Ok(Ok(true)) => Json(serde_json::json!({"output": "Committed sync snapshot."})),
    }
}

#[derive(Deserialize)]
pub struct PickQuery {
    pub q: Option<String>,
}

fn plain(status: StatusCode, text: String) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        text,
    )
        .into_response()
}

#[utoipa::path(
    get,
    path = "/api/v2/pick",
    params(("q" = Option<String>, Query, description = "Search query, required")),
    responses(
        (status = 200, description = "Single URL or up to 30 matches as plain text", content_type = "text/plain"),
        (status = 400, description = "Missing q parameter", content_type = "text/plain"),
        (status = 404, description = "No bookmarks match", content_type = "text/plain"),
    )
)]
pub async fn pick_bookmark(State(state): State<AppState>, Query(p): Query<PickQuery>) -> Response {
    let q = p.q.unwrap_or_default();
    if q.trim().is_empty() {
        return plain(
            StatusCode::BAD_REQUEST,
            "missing q parameter, e.g. /api/v2/pick?q=example".to_string(),
        );
    }
    let store = match store_of(&state) {
        Ok(s) => s,
        Err((status, Json(v))) => {
            return plain(status, v.to_string());
        }
    };
    let targets = match liber_core::picker::search_targets(&store, &q) {
        Ok(t) => t,
        Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    if targets.is_empty() {
        return plain(
            StatusCode::NOT_FOUND,
            format!("no bookmarks matching {q:?}"),
        );
    }
    if targets.len() == 1 {
        return plain(StatusCode::OK, format!("{}\n", targets[0].url));
    }
    let mut out = String::new();
    for b in targets.iter().take(30) {
        let id = match b.short_id {
            Some(n) => n.to_string(),
            None => b.uuid.to_string()[..8].to_string(),
        };
        out.push_str(&format!("[{id}] {}\n{}\n", b.title, b.url));
    }
    plain(StatusCode::OK, out)
}

#[derive(Deserialize, Default)]
pub struct PruneBody {
    pub days: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v2/sync/prune",
    request_body(content = Object, description = "Retention as {days?, default 90}"),
    responses(
        (status = 200, description = "Pruned entry count", body = Object),
    )
)]
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

    async fn text(res: axum::response::Response) -> (StatusCode, String, String) {
        let status = res.status();
        let ctype = res
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, ctype, String::from_utf8_lossy(&bytes).to_string())
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn export_import_roundtrip() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let app_a = test_app(&dir_a);
        let app_b = test_app(&dir_b);
        let (status, _) = body(
            app_a
                .clone()
                .oneshot(post(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, v) = body(
            app_a
                .oneshot(post("/api/v2/sync/export", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let entries = v["entries"].clone();

        let (status, v) = body(
            app_b
                .clone()
                .oneshot(post(
                    "/api/v2/sync/import",
                    serde_json::json!({"entries": entries}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["inserted"], 1);

        let (status, v) = body(
            app_b
                .oneshot(post("/api/v2/sync/prune", serde_json::json!({"days": 0})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["pruned"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn commit_outside_repo_reports_in_body() {
        let dir = tempfile::tempdir().unwrap();
        let app = test_app(&dir);
        let (status, v) = body(
            app.clone()
                .oneshot(post("/api/v2/sync/commit", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["output"]
            .as_str()
            .unwrap()
            .contains("Not a git repository"));
        assert!(v.get("error").is_none());

        let (status, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v2/sync/commit")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["output"]
            .as_str()
            .unwrap()
            .contains("Not a git repository"));
    }

    #[tokio::test]
    async fn commit_inside_repo_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let app = test_app(&dir);
        let run = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap()
                .status
                .success());
        };
        run(&["init"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "test"]);
        run(&["config", "commit.gpgsign", "false"]);
        {
            let cfg = Config {
                base_dir: dir.path().to_path_buf(),
                device_id: "test-device".to_string(),
                ..Default::default()
            };
            let mut store = Store::open(cfg).unwrap();
            liber_core::create::create_bookmark(
                &mut store,
                "https://example.com/sync-seed",
                liber_core::create::CreateOptions {
                    title: Some("Sync seed".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let (status, v) = body(
            app.clone()
                .oneshot(post("/api/v2/sync/commit", serde_json::json!({})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["output"].as_str().unwrap().contains("Committed"));
        let head = std::process::Command::new("git")
            .args(["log", "--format=%s", "-1"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&head.stdout).starts_with("liber sync: "));
    }

    #[tokio::test]
    async fn pick_shapes() {
        use liber_core::create::{create_bookmark, CreateOptions};
        let dir = tempfile::tempdir().unwrap();
        let app = test_app(&dir);
        let (status, ctype, t) =
            text(app.clone().oneshot(get("/api/v2/pick")).await.unwrap()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(ctype.starts_with("text/plain"));
        assert!(t.contains("missing q parameter"));

        let (status, _, _) = text(
            app.clone()
                .oneshot(get("/api/v2/pick?q=%20"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _, _) = text(
            app.clone()
                .oneshot(get("/api/v2/pick?q=nomatch-xyz"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let mut store = Store::open(cfg).unwrap();
        create_bookmark(
            &mut store,
            "https://example.com/solo",
            CreateOptions {
                title: Some("Solo entry".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let (status, ctype, t) = text(
            app.clone()
                .oneshot(get("/api/v2/pick?q=solo"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(ctype.starts_with("text/plain"));
        assert_eq!(t, "https://example.com/solo\n");

        for i in 0..35 {
            create_bookmark(
                &mut store,
                &format!("https://example.com/batch-{i}"),
                CreateOptions {
                    title: Some(format!("Batch item {i}")),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let (status, _, t) = text(
            app.clone()
                .oneshot(get("/api/v2/pick?q=batch"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(t.matches('[').count(), 30);
        assert!(t.contains("https://example.com/batch-0"));
    }
}
