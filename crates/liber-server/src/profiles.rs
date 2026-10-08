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

#[utoipa::path(
    get,
    path = "/api/v2/profiles",
    responses(
        (status = 200, description = "Profiles with the active one marked", body = Object),
    )
)]
pub async fn list_profiles(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let cfg = crate::live_config(&state);
    Ok(Json(serde_json::json!({
        "active": cfg.active_profile.clone().unwrap_or("default".to_string()),
        "profiles": liber_core::profile::list_profiles(&cfg),
    })))
}

#[derive(Deserialize)]
pub struct SwitchBody {
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/api/v2/profiles/switch",
    request_body(content = Object, description = "Profile as {name}"),
    responses(
        (status = 200, description = "Switched profile", body = Object),
        (status = 400, description = "Invalid profile name", body = crate::api::ErrorBody),
    )
)]
pub async fn switch_profile(
    State(state): State<AppState>,
    Json(input): Json<SwitchBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut cfg = crate::live_config(&state);
    let active = liber_core::profile::switch_profile(&mut cfg, &input.name).map_err(core_err)?;
    let path = liber_core::config::load_config()
        .map(|(_, path)| path)
        .map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("resolving config path: {e}"),
            )
        })?;
    liber_core::config::save_config_to(&path, &cfg).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("saving config: {e}"),
        )
    })?;
    *state.cfg.write().unwrap() = cfg;
    Ok(Json(
        serde_json::json!({"result": "switched", "active": active}),
    ))
}

#[derive(Deserialize)]
pub struct DeleteBody {
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/api/v2/profiles/delete",
    request_body(content = Object, description = "Profile as {name}"),
    responses(
        (status = 200, description = "Untracked profile", body = Object),
        (status = 400, description = "Active, default, or bad name", body = crate::api::ErrorBody),
        (status = 404, description = "No such profile", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_profile(
    State(state): State<AppState>,
    Json(input): Json<DeleteBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut cfg = crate::live_config(&state);
    let deleted = liber_core::profile::delete_profile(&mut cfg, &input.name).map_err(core_err)?;
    let path = liber_core::config::load_config()
        .map(|(_, path)| path)
        .map_err(|e| {
            err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("resolving config path: {e}"),
            )
        })?;
    liber_core::config::save_config_to(&path, &cfg).map_err(|e| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("saving config: {e}"),
        )
    })?;
    *state.cfg.write().unwrap() = cfg;
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
    use liber_core::store::{Config, Store};

    fn test_app() -> (axum::Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let _ = Store::open(cfg.clone()).unwrap();
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
    async fn profiles_switch_delete_flow() {
        let (app, dir) = test_app();
        std::env::set_var("LIBER_CONFIG", dir.path().join("config.json"));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/profiles")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["active"], "default");
        assert_eq!(v["profiles"].as_array().unwrap().len(), 1);

        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/profiles/switch",
                    serde_json::json!({"name": "work"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["active"], "work");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/profiles")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (_, v) = body_json(res).await;
        assert_eq!(v["active"], "work");

        let (status, _) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/profiles/delete",
                    serde_json::json!({"name": "work"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/profiles/switch",
                    serde_json::json!({"name": "default"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["active"], "default");

        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/profiles/delete",
                    serde_json::json!({"name": "work"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["name"], "work");

        let (status, _) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/profiles/delete",
                    serde_json::json!({"name": "work"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = body_json(
            app.oneshot(post_json(
                "/api/v2/profiles/switch",
                serde_json::json!({"name": "a/b"}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
