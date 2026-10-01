use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub mod api;
pub mod auth;
pub mod bookmarks;
pub mod check;
pub mod library;
pub mod rules;
pub mod sync;
pub mod taxonomy;

#[derive(Clone)]
pub struct AppState {
    pub cfg: liber_core::store::Config,
    pub token: String,
    pub write_mu: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new(cfg: liber_core::store::Config, token: String) -> Self {
        Self {
            cfg,
            token,
            write_mu: Arc::new(tokio::sync::Mutex::new(())),
        }
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
        .route("/api/v2/library/import", post(library::import_library))
        .route(
            "/api/v2/library/export-bookmarks",
            get(library::export_bookmarks),
        )
        .route("/api/v2/library/export-site", post(library::export_site_ep))
        .route("/api/v2/sync/export", post(sync::export_oplog))
        .route("/api/v2/sync/import", post(sync::import_oplog))
        .route("/api/v2/sync/prune", post(sync::prune_oplog_ep))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

pub async fn serve(
    cfg: liber_core::store::Config,
    token: String,
    addr: &str,
) -> anyhow::Result<()> {
    let state = AppState::new(cfg, token);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
