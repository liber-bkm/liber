use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};

use liber_core::automation::{apply_rules, create_rule, describe_rule, edit_rule, suggest_rules};
use liber_core::model::AutoRule;
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

#[derive(Serialize)]
struct RuleShape {
    id: String,
    pattern: String,
    tags: Vec<String>,
    folder: Option<String>,
    description: String,
    applied_count: usize,
}

fn shape(rule: &AutoRule, applied_count: usize) -> RuleShape {
    RuleShape {
        id: rule.id.clone(),
        pattern: rule.pattern.clone(),
        tags: rule.action_tags.clone(),
        folder: rule.action_folder.clone(),
        description: describe_rule(rule),
        applied_count,
    }
}

fn applied_counts(store: &Store) -> Result<std::collections::HashMap<String, usize>, ApiErr> {
    let mut counts = std::collections::HashMap::new();
    for b in store.list().map_err(core_err)? {
        for a in &b.applied_rules {
            *counts.entry(a.rule_id.clone()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

pub async fn list_rules(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = store_of(&state)?;
    let counts = applied_counts(&store)?;
    let rules: Vec<RuleShape> = store
        .list_rules()
        .map_err(core_err)?
        .iter()
        .map(|r| shape(r, counts.get(&r.id).copied().unwrap_or(0)))
        .collect();
    Ok(Json(serde_json::json!({"rules": rules})))
}

#[derive(Deserialize)]
pub struct AddRuleBody {
    pub pattern: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub folder: Option<String>,
}

pub async fn add_rule_ep(
    State(state): State<AppState>,
    Json(input): Json<AddRuleBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let (rule, changed) =
        create_rule(&mut store, input.pattern, input.tags, input.folder).map_err(core_err)?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"rule": shape(&rule, changed.len())})),
    ))
}

#[derive(Deserialize)]
pub struct EditRuleBody {
    pub pattern: Option<String>,
    pub tags: Option<Vec<String>>,
    pub folder: Option<String>,
    #[serde(default)]
    pub reapply: bool,
}

pub async fn edit_rule_ep(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EditRuleBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let (rule, changed) = edit_rule(
        &mut store,
        &id,
        input.pattern,
        input.tags,
        input.folder,
        input.reapply,
    )
    .map_err(core_err)?;
    Ok(Json(
        serde_json::json!({"rule": shape(&rule, 0), "reapplied": changed.len()}),
    ))
}

pub async fn delete_rule_ep(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    if !store.delete_rule(&id).map_err(core_err)? {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no such rule"})),
        ));
    }
    Ok(Json(serde_json::json!({"deleted": id})))
}

#[derive(Deserialize)]
pub struct ApplyBody {
    pub id: Option<String>,
}

pub async fn apply_rules_ep(
    State(state): State<AppState>,
    Json(input): Json<ApplyBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let changed = apply_rules(&mut store, input.id.as_deref()).map_err(core_err)?;
    Ok(Json(serde_json::json!({"applied": changed.len()})))
}

#[derive(Deserialize)]
pub struct LearnParams {
    pub min: Option<usize>,
}

pub async fn learn_rules(
    State(state): State<AppState>,
    Query(p): Query<LearnParams>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let store = store_of(&state)?;
    let min = p.min.unwrap_or(3).max(2);
    let out = suggest_rules(&store, min).map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "suggestions": out.iter().map(|s| serde_json::json!({
            "host": s.host, "folder": s.folder, "count": s.count,
        })).collect::<Vec<_>>(),
    })))
}

pub async fn learn_create(
    State(state): State<AppState>,
    Json(p): Json<LearnParams>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _guard = state.write_mu.lock().await;
    let mut store = store_of(&state)?;
    let min = p.min.unwrap_or(3).max(2);
    let suggestions = suggest_rules(&store, min).map_err(core_err)?;
    let mut created = 0;
    let mut applied = 0;
    for s in suggestions {
        let (_, changed) = create_rule(
            &mut store,
            format!("host:{}", s.host),
            vec![],
            Some(s.folder),
        )
        .map_err(core_err)?;
        created += 1;
        applied += changed.len();
    }
    Ok(Json(
        serde_json::json!({"created": created, "applied": applied}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
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
        (build_router(AppState::new(cfg, String::new())), dir)
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

    #[tokio::test]
    async fn rules_flow() {
        let (app, _dir) = test_app();
        let (status, _) = body(
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

        let (status, v) = body(
            app.clone()
                .oneshot(post(
                    "/api/v2/rules",
                    serde_json::json!({"pattern": "host:example.com", "tags": ["news"], "folder": "tech"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = v["rule"]["id"].as_str().unwrap().to_string();
        assert_eq!(v["rule"]["applied_count"], 1);

        let (_, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/rules")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(v["rules"][0]["applied_count"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri(format!("/api/v2/rules/{id}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from("{\"tags\":[\"t2\"],\"reapply\":true}"))
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["reapplied"], 1);

        let (status, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(format!("/api/v2/rules/{id}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["deleted"], id);
    }

    #[tokio::test]
    async fn learn_flow() {
        let (app, _dir) = test_app();
        for i in 0..3 {
            let (status, _) = body(
                app.clone()
                    .oneshot(post(
                        "/api/v2/bookmarks",
                        serde_json::json!({"url": format!("https://example.com/{i}"), "folder": "tech"}),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }
        let (_, v) = body(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v2/rules/learn?min=2")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(v["suggestions"][0]["host"], "example.com");

        let (status, v) = body(
            app.oneshot(post("/api/v2/rules/learn", serde_json::json!({"min": 2})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["created"], 1);
        assert_eq!(v["applied"], 3);
    }
}
