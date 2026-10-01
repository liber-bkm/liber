use std::sync::Arc;

use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;

#[cfg(embed_frontend)]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../frontend/dist"]
struct EmbeddedUi;

#[cfg(embed_frontend)]
fn embedded_file(path: &str) -> Option<(Vec<u8>, String)> {
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let data = EmbeddedUi::get(rel).or_else(|| EmbeddedUi::get("index.html"))?;
    let mime = liber_core::archive::mime_for(rel, None);
    Some((data.data.into_owned(), mime))
}

pub mod api;
pub mod auth;
pub mod bookmarks;
pub mod bulk;
pub mod check;
pub mod library;
pub mod reindex;
pub mod rules;
pub mod sync;
pub mod taxonomy;

#[derive(Clone)]
pub struct AppState {
    pub cfg: liber_core::store::Config,
    pub token: String,
    pub write_mu: Arc<tokio::sync::Mutex<()>>,
    pub static_dir: Option<std::path::PathBuf>,
}

impl AppState {
    pub fn new(cfg: liber_core::store::Config, token: String) -> Self {
        Self {
            cfg,
            token,
            write_mu: Arc::new(tokio::sync::Mutex::new(())),
            static_dir: None,
        }
    }

    pub fn with_static_dir(mut self, dir: Option<std::path::PathBuf>) -> Self {
        self.static_dir = dir;
        self
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v2/health", get(health))
        .route(
            "/api/v2/bookmarks",
            get(bookmarks::list_bookmarks).post(bookmarks::add_bookmark),
        )
        .route(
            "/api/v2/bookmarks/:id",
            get(bookmarks::get_bookmark)
                .put(bookmarks::update_bookmark)
                .delete(bookmarks::delete_bookmark),
        )
        .route("/api/v2/bookmarks/:id/open", post(bookmarks::open_bookmark))
        .route(
            "/api/v2/bookmarks/:id/attachments/:name",
            get(bookmarks::download_attachment),
        )
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route("/logout", get(auth::logout).post(auth::logout))
        .route("/api/v2/tags", get(taxonomy::list_tags))
        .route("/api/v2/tags/rename", post(taxonomy::rename_tag_ep))
        .route("/api/v2/tags/delete", post(taxonomy::delete_tag_ep))
        .route("/api/v2/folders", get(taxonomy::list_folders))
        .route("/api/v2/folders/rename", post(taxonomy::rename_folder_ep))
        .route("/api/v2/folders/delete", post(taxonomy::delete_folder_ep))
        .route(
            "/api/v2/rules",
            get(rules::list_rules).post(rules::add_rule_ep),
        )
        .route(
            "/api/v2/rules/:id",
            axum::routing::put(rules::edit_rule_ep).delete(rules::delete_rule_ep),
        )
        .route("/api/v2/rules/apply", post(rules::apply_rules_ep))
        .route(
            "/api/v2/rules/learn",
            get(rules::learn_rules).post(rules::learn_create),
        )
        .route("/api/v2/check/run", post(check::run_check))
        .route("/api/v2/check/apply", post(check::apply_check))
        .route("/api/v2/bulk", post(bulk::bulk))
        .route("/api/v2/library/import", post(library::import_library))
        .route(
            "/api/v2/library/export-bookmarks",
            get(library::export_bookmarks),
        )
        .route("/api/v2/library/export-site", post(library::export_site_ep))
        .route("/api/v2/sync/export", post(sync::export_oplog))
        .route("/api/v2/sync/import", post(sync::import_oplog))
        .route("/api/v2/sync/prune", post(sync::prune_oplog_ep))
        .route("/api/v2/reindex", post(reindex::reindex_ep))
        .fallback(frontend_fallback)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

fn fallback_embedded(path: &str) -> axum::response::Response {
    let _ = path;
    #[cfg(embed_frontend)]
    if let Some((data, mime)) = embedded_file(path) {
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, mime)],
            data,
        )
            .into_response();
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(axum::http::header::CONTENT_TYPE, "text/plain")],
        "liber API is running, but no web UI is bundled with this build. \
         Serve a frontend with --static-dir, or rebuild with EMBED_UI=1 \
         after running pnpm build in frontend/.\n",
    )
        .into_response()
}

async fn frontend_fallback(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use axum::http::StatusCode;
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            "{\"error\":\"not found\"}\n",
        )
            .into_response();
    }
    let Some(dir) = &state.static_dir else {
        return fallback_embedded(&path);
    };
    let rel = path.trim_start_matches('/');
    let candidate = dir.join(if rel.is_empty() { "index.html" } else { rel });
    let file = if candidate.is_file() {
        candidate
    } else {
        dir.join("index.html")
    };
    if !file.is_file() {
        return fallback_embedded(&path);
    }
    let mime = liber_core::archive::mime_for(file.to_string_lossy().as_ref(), None);
    match tokio::fs::read(&file).await {
        Ok(data) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, mime)],
            data,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

pub async fn serve(
    cfg: liber_core::store::Config,
    token: String,
    addr: &str,
    static_dir: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let state = AppState::new(cfg, token).with_static_dir(static_dir);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
