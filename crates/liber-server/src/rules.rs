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

