use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;

use crate::AppState;

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: String) -> ApiErr {
    (status, Json(serde_json::json!({"error": msg})))
}

fn core_err(e: liber_core::CoreError) -> ApiErr {
    let status = match &e {
        liber_core::CoreError::NotFound(_) => StatusCode::NOT_FOUND,
        liber_core::CoreError::Invalid(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    err(status, e.to_string())
}

fn persist(cfg: &liber_core::store::Config, state: &AppState) -> Result<(), ApiErr> {
    let path = liber_core::config::load_config()
        .map(|(_, path)| path)
        .map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("resolving config path: {e}"),
            )
        })?;
    liber_core::config::save_config_to(&path, cfg).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("saving config: {e}"),
        )
    })?;
    *state.cfg.write().unwrap() = cfg.clone();
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v2/searches",
    responses(
        (status = 200, description = "Saved search presets", body = Object),
    )
)]
pub async fn list_searches(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let cfg = crate::live_config(&state);
    Ok(Json(serde_json::json!({"searches": cfg.saved_searches})))
}

#[derive(Deserialize, Default)]
pub struct AddSearchBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub sort: String,
}

#[utoipa::path(
    post,
    path = "/api/v2/searches",
    request_body(content = Object, description = "Preset as {name, query?, scope?, sort?}"),
    responses(
        (status = 200, description = "Saved preset", body = Object),
        (status = 400, description = "Invalid name, scope, or sort", body = crate::api::ErrorBody),
    )
)]
pub async fn add_search(
    State(state): State<AppState>,
    Json(input): Json<AddSearchBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut cfg = crate::live_config(&state);
    let saved = liber_core::config::add_saved_search(
        &mut cfg,
        input.name,
        input.query,
        input.scope,
        input.sort,
    )
    .map_err(core_err)?;
    persist(&cfg, &state)?;
    Ok(Json(
        serde_json::json!({"result": "saved", "search": saved}),
    ))
}

#[derive(Deserialize)]
pub struct DeleteSearchBody {
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/api/v2/searches/delete",
    request_body(content = Object, description = "Preset as {name}"),
    responses(
        (status = 200, description = "Deleted preset", body = Object),
        (status = 404, description = "No such preset", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_search(
    State(state): State<AppState>,
    Json(input): Json<DeleteSearchBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut cfg = crate::live_config(&state);
    let deleted =
        liber_core::config::delete_saved_search(&mut cfg, &input.name).map_err(core_err)?;
    persist(&cfg, &state)?;
    Ok(Json(
        serde_json::json!({"result": "deleted", "name": deleted}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
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
        std::env::set_var("LIBER_CONFIG", dir.path().join("config.json"));
        (build_router(AppState::new(cfg, String::new())), dir)
    }

    fn post_json(uri: &str, v: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap()
    }

    async fn body_json(res: axum::response::Response) -> (StatusCode, serde_json::Value) {
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn searches_add_list_delete_flow() {
        let (app, _dir) = test_app();

        let (status, v) = body_json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/searches")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["searches"], serde_json::json!([]));

        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/searches",
                    serde_json::json!({"name": "rust", "query": "tag:rust", "sort": "newest"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["search"]["name"], "rust");
        assert_eq!(v["search"]["query"], "tag:rust");

        let (status, _) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/searches",
                    serde_json::json!({"name": "  "}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/searches",
                    serde_json::json!({"name": "bad", "scope": "bogus"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, v) = body_json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/searches")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["searches"].as_array().unwrap().len(), 1);

        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/searches/delete",
                    serde_json::json!({"name": "RUST"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["name"], "rust");

        let (status, _) = body_json(
            app.oneshot(post_json(
                "/api/v2/searches/delete",
                serde_json::json!({"name": "rust"}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        std::env::remove_var("LIBER_CONFIG");
    }
}
