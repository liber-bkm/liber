use std::sync::Arc;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;

pub mod api;
pub mod auth;
pub mod bookmarks;

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
