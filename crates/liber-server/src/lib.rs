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
pub mod openapi;
pub mod profiles;
pub mod reindex;
pub mod rules;
pub mod settings;
pub mod sync;
pub mod taxonomy;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<std::sync::RwLock<liber_core::store::Config>>,
    pub token: String,
    pub write_mu: Arc<tokio::sync::Mutex<()>>,
    pub static_dir: Option<std::path::PathBuf>,
}

impl AppState {
    pub fn new(cfg: liber_core::store::Config, token: String) -> Self {
        Self {
            cfg: Arc::new(std::sync::RwLock::new(cfg)),
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

pub fn live_config(state: &AppState) -> liber_core::store::Config {
    state.cfg.read().unwrap().clone()
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
        .route("/api/v2/history", get(bookmarks::history))
        .route(
            "/api/v2/bookmarks/:id/attachments/:name",
            get(bookmarks::download_attachment).delete(bookmarks::delete_attachment),
        )
        .route(
            "/api/v2/bookmarks/:id/attachments",
            post(bookmarks::upload_attachment),
        )
        .route(
            "/api/v2/bookmarks/:id/notes",
            get(bookmarks::get_notes)
                .put(bookmarks::put_notes)
                .delete(bookmarks::delete_notes),
        )
        .route(
            "/api/v2/bookmarks/:id/archive",
            get(bookmarks::get_archive)
                .post(bookmarks::post_archive)
                .delete(bookmarks::delete_archive),
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
        .route("/api/v2/library/backup", get(library::backup_library))
        .route("/api/v2/favicons/:host", get(library::get_favicon))
        .route(
            "/api/v2/library/restore",
            post(library::restore_library).route_layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/api/v2/sync/export", post(sync::export_oplog))
        .route("/api/v2/sync/import", post(sync::import_oplog))
        .route("/api/v2/sync/prune", post(sync::prune_oplog_ep))
        .route("/api/v2/sync/commit", post(sync::commit_snapshot))
        .route("/api/v2/pick", get(sync::pick_bookmark))
        .route("/api/v2/profiles", get(profiles::list_profiles))
        .route("/api/v2/profiles/switch", post(profiles::switch_profile))
        .route("/api/v2/profiles/delete", post(profiles::delete_profile))
        .route(
            "/api/v2/settings",
            get(settings::get_settings).put(settings::set_setting),
        )
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ServeOptions {
    pub qr: bool,
    pub mdns: bool,
}

pub async fn serve(
    cfg: liber_core::store::Config,
    token: String,
    addr: &str,
    static_dir: Option<std::path::PathBuf>,
    opts: ServeOptions,
) -> anyhow::Result<()> {
    let state = AppState::new(cfg, token).with_static_dir(static_dir);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();
    println!("liber serving on http://127.0.0.1:{port}");
    if let Some(ip) = liber_core::discovery::lan_ip() {
        let url = format!("http://{ip}:{port}");
        println!("on your network: {url}");
        if opts.qr {
            match liber_core::discovery::qr_ascii(&url) {
                Ok(art) => println!("{art}"),
                Err(e) => eprintln!("warning: {e}"),
            }
        }
    } else if opts.qr {
        eprintln!("warning: no LAN address found, skipping QR");
    }
    let _advertiser = if opts.mdns {
        let device = state.cfg.read().unwrap().effective_device_id();
        match liber_core::discovery::advertise(port, &device) {
            Ok(Some(advertiser)) => {
                println!(
                    "advertising as {}",
                    liber_core::discovery::instance_name(&device)
                );
                Some(advertiser)
            }
            Ok(None) => {
                eprintln!("warning: no LAN address found, skipping mDNS");
                None
            }
            Err(e) => {
                eprintln!("warning: {e}");
                None
            }
        }
    } else {
        None
    };
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
