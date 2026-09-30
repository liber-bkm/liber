use axum::routing::get;
use axum::Router;

pub fn build_router() -> Router {
    Router::new()
        .route("/api/v2/health", get(health))
        .route("/api/v2/bookmarks", get(list_stub).post(add_stub))
}

async fn health() -> &'static str {
    "ok"
}

async fn list_stub() -> String {
    "{\"bookmarks\":[]}".to_string()
}

async fn add_stub() -> String {
    "{\"error\":\"not implemented\"}".to_string()
}
