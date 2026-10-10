use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use liber_core::create::{create_bookmark, CreateOptions};
use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
use liber_core::search::order_results;
use liber_core::store::Store;

use crate::api::{AddRequest, ApiBookmark, DeleteParams, ListParams, ListResponse, UpdateRequest};
use crate::AppState;

pub(crate) type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({"error": msg.into()})))
}

fn core_err(e: liber_core::CoreError) -> ApiErr {
    match &e {
        liber_core::CoreError::NotFound(m) => err(StatusCode::NOT_FOUND, m.clone()),
        liber_core::CoreError::Duplicate(m) => err(StatusCode::CONFLICT, m.clone()),
        liber_core::CoreError::Invalid(m) => err(StatusCode::BAD_REQUEST, m.clone()),
        liber_core::CoreError::Storage(m) => err(StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
        liber_core::CoreError::Other(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

fn open_store(state: &AppState) -> Result<Store, ApiErr> {
    Store::open(crate::live_config(state)).map_err(core_err)
}

#[utoipa::path(
    get,
    path = "/api/v2/bookmarks",
    params(ListParams),
    responses(
        (status = 200, description = "Paged bookmark list", body = crate::api::ListResponse),
        (status = 400, description = "Bad sort mode", body = crate::api::ErrorBody),
    )
)]
pub async fn list_bookmarks(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<ListResponse>, (StatusCode, Json<serde_json::Value>)> {
    use liber_core::store::BookmarkFilter;
    let store = open_store(&state)?;
    let sort = liber_core::search::resolve_sort_mode(p.sort.as_deref(), p.q.as_deref())
        .map_err(core_err)?;
    let scope = match &p.scope {
        Some(s) => liber_core::search::parse_field_list(s).map_err(core_err)?,
        None => liber_core::search::SearchFields::all(),
    };
    let filter = BookmarkFilter {
        folder: p.folder.clone(),
        tag: p.tag.clone(),
        query: p.q.clone(),
        scope,
        opened_only: false,
    };
    let per_page = p.per_page.unwrap_or(50).clamp(1, 500);
    let page = p.page.unwrap_or(1).max(1);
    let start = (page - 1) * per_page;
    let (mut found, mut total) = store
        .query_bookmarks(&filter, sort, per_page, start)
        .map_err(core_err)?;
    let mut snippets: std::collections::HashMap<uuid::Uuid, String> =
        std::collections::HashMap::new();
    if p.deep.unwrap_or(false) {
        if let Some(q) = &p.q {
            if !q.trim().is_empty() {
                let mut seen: std::collections::HashSet<uuid::Uuid> =
                    found.iter().map(|b| b.uuid).collect();
                let deep = liber_core::search::deep_search_with_snippets(&store, q, 200, &scope)
                    .map_err(core_err)?;
                for (uuid, _score, fragment) in deep {
                    if seen.insert(uuid) {
                        if let Some(b) = store.get(&uuid).map_err(core_err)? {
                            if !fragment.trim().is_empty() {
                                snippets.insert(uuid, fragment);
                            }
                            found.push(b);
                            total += 1;
                        }
                    }
                }
            }
        }
    }
    let ordered = order_results(found, p.q.as_deref().unwrap_or(""), sort);
    let mut slice: Vec<ApiBookmark> = ordered.iter().map(ApiBookmark::from).collect();
    for b in &mut slice {
        if b.snippet.is_none() {
            if let Ok(uuid) = b.uuid.parse::<uuid::Uuid>() {
                if let Some(fragment) = snippets.remove(&uuid) {
                    b.snippet = Some(fragment);
                }
            }
        }
    }
    Ok(Json(ListResponse {
        total,
        page,
        per_page,
        bookmarks: slice,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v2/bookmarks",
    request_body(content = crate::api::AddRequest, description = "Bookmark to add"),
    responses(
        (status = 201, description = "Bookmark created", body = crate::api::ApiBookmark),
        (status = 200, description = "Duplicate accepted idempotently", body = crate::api::ApiBookmark),
        (status = 400, description = "Missing url", body = crate::api::ErrorBody),
        (status = 409, description = "Duplicate url", body = Object),
    )
)]
pub async fn add_bookmark(
    State(state): State<AppState>,
    Json(input): Json<AddRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if input.url.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "url is required"));
    }
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let title = match input.title {
        Some(t) if !t.trim().is_empty() => Some(t),
        _ => {
            let url = liber_core::slug::normalize_url(&input.url);
            let fetched =
                tokio::task::spawn_blocking(move || liber_core::create::fetch_title(&url))
                    .await
                    .unwrap_or_default();
            if fetched.trim().is_empty() {
                None
            } else {
                Some(fetched)
            }
        }
    };
    let opts = CreateOptions {
        title,
        description: input.description.unwrap_or_default(),
        tags: input.tags,
        folder: input.folder.unwrap_or_default(),
        markdown: input.markdown,
    };
    match create_bookmark(&mut store, &input.url, opts) {
        Ok(b) => {
            let mut warnings = Vec::new();
            let b = if input.archive {
                match liber_core::archive::archive_bookmark(&mut store, &b.uuid, None) {
                    Ok(w) => {
                        warnings.extend(w);
                        store.get(&b.uuid).map_err(core_err)?.unwrap_or(b)
                    }
                    Err(e) => {
                        warnings.push(format!("archive failed: {e}"));
                        b
                    }
                }
            } else {
                b
            };
            for at in &input.attachments {
                use base64::Engine;
                if at.name.trim().is_empty() {
                    warnings.push("attachment skipped: name is required".to_string());
                    continue;
                }
                if at.content.len() > 32 << 20 {
                    warnings.push(format!("attachment skipped: {} too large", at.name));
                    continue;
                }
                match base64::engine::general_purpose::STANDARD.decode(at.content.as_bytes()) {
                    Err(_) => warnings.push(format!(
                        "attachment skipped: {} is not valid base64",
                        at.name
                    )),
                    Ok(data) => match liber_core::attach::attach_bytes(
                        &mut store,
                        &b.uuid,
                        at.name.trim(),
                        &data,
                    ) {
                        Ok(_) => {}
                        Err(e) => warnings.push(format!(
                            "attachment skipped: could not attach {}: {e}",
                            at.name
                        )),
                    },
                }
            }
            let b = store.get(&b.uuid).map_err(core_err)?.unwrap_or(b);
            let mut v = serde_json::to_value(ApiBookmark::from(&b)).unwrap_or_default();
            if !warnings.is_empty() {
                v["warnings"] = serde_json::to_value(warnings).unwrap_or_default();
            }
            Ok((StatusCode::CREATED, Json(v)))
        }
        Err(liber_core::CoreError::Duplicate(_)) => {
            let dup = store
                .find_by_url(&liber_core::slug::normalize_url(&input.url))
                .map_err(core_err)?
                .ok_or_else(|| err(StatusCode::CONFLICT, "duplicate"))?;
            if input.confirm_dup {
                return Ok((
                    StatusCode::OK,
                    Json(serde_json::to_value(ApiBookmark::from(&dup)).unwrap_or_default()),
                ));
            }
            Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "duplicate",
                    "existing": ApiBookmark::from(&dup),
                    "hint": "repeat with confirm_dup true to accept",
                })),
            ))
        }
        Err(e) => Err(core_err(e)),
    }
}

async fn resolve_one(
    state: &AppState,
    id: &str,
) -> Result<liber_core::model::Bookmark, (StatusCode, Json<serde_json::Value>)> {
    let store = open_store(state)?;
    let tokens = liber_core::idspec::parse_id_spec(id).map_err(core_err)?;
    let mut hits = store.resolve_spec(&tokens).map_err(core_err)?;
    if hits.is_empty() {
        return Err(err(StatusCode::NOT_FOUND, "not found"));
    }
    Ok(hits.remove(0))
}

#[utoipa::path(
    get,
    path = "/api/v2/bookmarks/{id}",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Bookmark", body = crate::api::ApiBookmark),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn get_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiBookmark>, (StatusCode, Json<serde_json::Value>)> {
    Ok(Json(ApiBookmark::from(&resolve_one(&state, &id).await?)))
}

#[utoipa::path(
    put,
    path = "/api/v2/bookmarks/{id}",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    request_body(content = crate::api::UpdateRequest, description = "Partial fields to update"),
    responses(
        (status = 200, description = "Updated bookmark", body = crate::api::ApiBookmark),
        (status = 400, description = "Empty title or url", body = crate::api::ErrorBody),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn update_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateRequest>,
) -> Result<Json<ApiBookmark>, (StatusCode, Json<serde_json::Value>)> {
    if let Some(t) = &input.title {
        if t.trim().is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "title must not be empty"));
        }
    }
    if let Some(u) = &input.url {
        if u.trim().is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "url must not be empty"));
        }
    }
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let tokens = liber_core::idspec::parse_id_spec(&id).map_err(core_err)?;
    let mut hits = store.resolve_spec(&tokens).map_err(core_err)?;
    if hits.is_empty() {
        return Err(err(StatusCode::NOT_FOUND, "not found"));
    }
    let opts = EditOptions {
        title: input.title,
        description: input.description,
        tags: input.tags,
        folder: input.folder,
        url: input.url,
        add_markdown: false,
    };
    let out = edit_bookmark(&mut store, &hits.remove(0).uuid, opts).map_err(core_err)?;
    Ok(Json(ApiBookmark::from(&out)))
}

#[utoipa::path(
    delete,
    path = "/api/v2/bookmarks/{id}",
    params(
        ("id" = String, Path, description = "Short id or UUID prefix"),
        DeleteParams,
    ),
    responses(
        (status = 200, description = "Deleted id, or confirm_required union without confirm", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(p): Query<DeleteParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let target = resolve_one(&state, &id).await?;
    if !p.confirm.unwrap_or(false) {
        return Ok(Json(serde_json::json!({
            "confirm_required": true,
            "bookmark": ApiBookmark::from(&target),
            "hint": "repeat with ?confirm=true to delete",
        })));
    }
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    delete_bookmark_with_files(&mut store, &target.uuid).map_err(core_err)?;
    Ok(Json(
        serde_json::json!({"deleted": target.uuid.to_string()}),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v2/bookmarks/{id}/open",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Opened url", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn open_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    store.record_open(&target.uuid).map_err(core_err)?;
    Ok(Json(serde_json::json!({"url": target.url})))
}

#[utoipa::path(
    get,
    path = "/api/v2/history",
    params(
        ("per_page" = Option<usize>, Query, description = "Page size, clamped 1-500"),
        ("page" = Option<usize>, Query, description = "Page number, from 1"),
    ),
    responses(
        (status = 200, description = "Recently opened bookmarks", body = crate::api::ListResponse),
    )
)]
pub async fn history(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<ListResponse>, ApiErr> {
    use liber_core::search::SortMode;
    use liber_core::store::BookmarkFilter;
    let store = open_store(&state)?;
    let filter = BookmarkFilter {
        opened_only: true,
        ..Default::default()
    };
    let per_page = p.per_page.unwrap_or(50).clamp(1, 500);
    let page = p.page.unwrap_or(1).max(1);
    let start = (page - 1) * per_page;
    let (found, total) = store
        .query_bookmarks(&filter, SortMode::Visited, per_page, start)
        .map_err(core_err)?;
    let slice: Vec<ApiBookmark> = found.iter().map(ApiBookmark::from).collect();
    Ok(Json(ListResponse {
        total,
        page,
        per_page,
        bookmarks: slice,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v2/bookmarks/{id}/attachments/{name}",
    params(
        ("id" = String, Path, description = "Short id or UUID prefix"),
        ("name" = String, Path, description = "Attachment file name"),
    ),
    responses(
        (status = 200, description = "Attachment bytes", content_type = "application/octet-stream"),
        (status = 404, description = "No such attachment", body = crate::api::ErrorBody),
    )
)]
pub async fn download_attachment(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<axum::response::Response, ApiErr> {
    use axum::body::Body;
    use axum::http::header;
    let target = resolve_one(&state, &id).await?;
    let Some(at) = target
        .attachments
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&name))
    else {
        return Err(err(StatusCode::NOT_FOUND, "no such attachment"));
    };
    let store = open_store(&state)?;
    let data = std::fs::read(store.cfg.attachment_dir().join(&at.path))
        .map_err(|_| err(StatusCode::NOT_FOUND, "attachment file missing"))?;
    let mime = liber_core::archive::mime_for(&at.name, None);
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", at.name),
            ),
        ],
        Body::from(data),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v2/bookmarks/{id}/attachments",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    request_body(content = Object, description = "Base64 attachment as {name, content}"),
    responses(
        (status = 200, description = "Attached file name", body = Object),
        (status = 400, description = "Bad name, payload, or oversize", body = crate::api::ErrorBody),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn upload_attachment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UploadBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    use base64::Engine;
    if input.name.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "name is required"));
    }
    if input.content.len() > 32 << 20 {
        return Err(err(StatusCode::PAYLOAD_TOO_LARGE, "attachment too large"));
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(input.content.as_bytes())
        .map_err(|_| err(StatusCode::BAD_REQUEST, "content is not valid base64"))?;
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let at = liber_core::attach::attach_bytes(&mut store, &target.uuid, input.name.trim(), &data)
        .map_err(core_err)?;
    Ok(Json(serde_json::json!({"name": at.name})))
}

#[derive(serde::Deserialize)]
pub struct UploadBody {
    pub name: String,
    #[serde(default)]
    pub content: String,
}

#[utoipa::path(
    get,
    path = "/api/v2/bookmarks/{id}/notes",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Note body or null", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn get_notes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let store = open_store(&state)?;
    let body = liber_core::edit::read_note_body(&store, &target.uuid).map_err(core_err)?;
    Ok(Json(serde_json::json!({ "body": body })))
}

#[derive(serde::Deserialize)]
pub struct NotesBody {
    #[serde(default)]
    pub body: String,
}

#[utoipa::path(
    put,
    path = "/api/v2/bookmarks/{id}/notes",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    request_body(content = Object, description = "Note body as {body}"),
    responses(
        (status = 200, description = "Saved", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn put_notes(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NotesBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    liber_core::edit::save_note_body(&mut store, &target.uuid, &input.body).map_err(core_err)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[utoipa::path(
    get,
    path = "/api/v2/bookmarks/{id}/archive",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Archived HTML page", content_type = "text/html"),
        (status = 404, description = "No archive", body = crate::api::ErrorBody),
    )
)]
pub async fn get_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiErr> {
    use axum::body::Body;
    use axum::http::header;
    let target = resolve_one(&state, &id).await?;
    let Some(rel) = &target.archive_file else {
        return Err(err(StatusCode::NOT_FOUND, "no archive for this bookmark"));
    };
    let store = open_store(&state)?;
    let data = std::fs::read(store.cfg.archive_dir().join(rel))
        .map_err(|_| err(StatusCode::NOT_FOUND, "archive file missing"))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "sandbox allow-same-origin".to_string(),
            ),
        ],
        Body::from(data),
    )
        .into_response())
}

#[derive(serde::Deserialize)]
pub struct ArchiveBody {
    pub backend: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v2/bookmarks/{id}/archive",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    request_body(content = Object, description = "Backend as {backend?}"),
    responses(
        (status = 200, description = "Archived with warnings", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn post_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ArchiveBody>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let applied = liber_core::edit::apply_edit(
        &mut store,
        &target.uuid,
        liber_core::edit::EditDraft {
            archive: liber_core::edit::ArchiveAction::Add {
                backend: input.backend,
            },
            ..Default::default()
        },
    )
    .map_err(core_err)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "warnings": applied.warnings,
    })))
}

#[utoipa::path(
    delete,
    path = "/api/v2/bookmarks/{id}/archive",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Archive removed", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    liber_core::edit::apply_edit(
        &mut store,
        &target.uuid,
        liber_core::edit::EditDraft {
            archive: liber_core::edit::ArchiveAction::Remove,
            ..Default::default()
        },
    )
    .map_err(core_err)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[utoipa::path(
    delete,
    path = "/api/v2/bookmarks/{id}/notes",
    params(("id" = String, Path, description = "Short id or UUID prefix")),
    responses(
        (status = 200, description = "Notes removed", body = Object),
        (status = 404, description = "Not found", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_notes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    liber_core::edit::apply_edit(
        &mut store,
        &target.uuid,
        liber_core::edit::EditDraft {
            markdown: liber_core::edit::MarkdownAction::Remove,
            ..Default::default()
        },
    )
    .map_err(core_err)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[utoipa::path(
    delete,
    path = "/api/v2/bookmarks/{id}/attachments/{name}",
    params(
        ("id" = String, Path, description = "Short id or UUID prefix"),
        ("name" = String, Path, description = "Attachment file name"),
    ),
    responses(
        (status = 200, description = "Detached file name", body = Object),
        (status = 404, description = "No such attachment", body = crate::api::ErrorBody),
    )
)]
pub async fn delete_attachment(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let target = resolve_one(&state, &id).await?;
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let detached =
        liber_core::attach::detach_attachment(&mut store, &target.uuid, &name).map_err(core_err)?;
    Ok(Json(serde_json::json!({"detached": detached})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request};
    use tower::ServiceExt;

    use crate::{build_router, AppState};
    use liber_core::store::Config;

    fn test_state(token: &str) -> (axum::Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let state = AppState::new(cfg, token.to_string());
        (build_router(state), dir)
    }

    async fn body_json(res: axum::response::Response) -> (StatusCode, serde_json::Value) {
        let status = res.status();
        let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        (status, v)
    }

    fn post_json(uri: &str, v: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn history_orders_by_open() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 0);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v2/bookmarks/{uuid}/open"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);
        assert_eq!(v["bookmarks"][0]["uuid"], uuid);
    }

    #[tokio::test]
    async fn notes_archive_upload_flow() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a", "markdown": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}/notes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["body"].as_str().unwrap().contains("Visit original"));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v2/bookmarks/{uuid}/notes"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"body\":\"my notes here\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}/notes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["body"], "my notes here");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}/archive"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        let res = app
            .clone()
            .oneshot(post_json(
                &format!("/api/v2/bookmarks/{uuid}/attachments"),
                serde_json::json!({"name": "n.txt", "content": "aGVsbG8="}),
            ))
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["name"], "n.txt");

        let res = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}/attachments/n.txt"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn artifact_remove_endpoints() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a", "markdown": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}/notes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}/notes"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert!(v["body"].is_null());

        let res = app
            .clone()
            .oneshot(post_json(
                &format!("/api/v2/bookmarks/{uuid}/attachments"),
                serde_json::json!({"name": "n.txt", "content": "aGVsbG8="}),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}/attachments/n.txt"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["detached"], "n.txt");
    }

    #[tokio::test]
    async fn crud_lifecycle() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/a", "title": "A"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["title"], "A");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?q=example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"title\":\"A2\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["title"], "A2");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v2/bookmarks/{uuid}/open"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["url"], "https://example.com/a");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["confirm_required"], true);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}?confirm=true"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, _) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn duplicate_flow() {
        let (app, _dir) = test_state("");
        let add = || {
            post_json(
                "/api/v2/bookmarks",
                serde_json::json!({"url": "https://example.com/dup"}),
            )
        };
        let (status, _) = body_json(app.clone().oneshot(add()).await.unwrap()).await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, v) = body_json(app.clone().oneshot(add()).await.unwrap()).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(v.get("existing").is_some());
        let (status, v) = body_json(
            app.oneshot(post_json(
                "/api/v2/bookmarks",
                serde_json::json!({"url": "https://example.com/dup", "confirm_dup": true}),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["url"], "https://example.com/dup");
    }

    #[tokio::test]
    async fn add_with_archive_reports_failure_as_warning() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "http://127.0.0.1:1/unreachable", "archive": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(v["warnings"].as_array().is_some());
        assert!(v["warnings"][0]
            .as_str()
            .unwrap()
            .contains("archive failed"));
        assert_eq!(v["has_archive"], false);
    }

    #[tokio::test]
    async fn default_list_order_is_newest_first() {
        let (app, _dir) = test_state("");
        for url in ["https://example.com/a", "https://example.com/b"] {
            let (status, _) = body_json(
                app.clone()
                    .oneshot(post_json(
                        "/api/v2/bookmarks",
                        serde_json::json!({"url": url}),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 2);
        assert_eq!(v["bookmarks"][0]["url"], "https://example.com/b");
        assert_eq!(v["bookmarks"][1]["url"], "https://example.com/a");
    }

    #[tokio::test]
    async fn validation_errors() {
        let (app, _dir) = test_state("");
        let (status, _) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "  "}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?sort=bogus")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn deep_search_unions_archive_hits() {
        use liber_core::search::SearchIndex;
        use liber_core::store::{Config, Store};
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let mut store = Store::open(cfg.clone()).unwrap();
        let b = {
            use liber_core::create::{create_bookmark, CreateOptions};
            create_bookmark(
                &mut store,
                "https://example.com/a",
                CreateOptions {
                    title: Some("Plain".to_string()),
                    ..Default::default()
                },
            )
            .unwrap()
        };
        let index = SearchIndex::open_or_create(&store.cfg.tantivy_dir()).unwrap();
        index.index_bookmark(&b, "obscurecontentword").unwrap();
        drop(store);
        let app = build_router(crate::AppState::new(cfg, String::new()));

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?q=obscurecontentword")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, plain) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(plain["total"], 0);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?q=obscurecontentword&deep=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, deep) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(deep["total"], 1);
        assert_eq!(deep["bookmarks"][0]["uuid"], b.uuid.to_string());
    }

    #[tokio::test]
    async fn scoped_search_filters() {
        let (app, _dir) = test_state("");
        for (url, title, tags) in [
            ("https://example.com/rust", "Rust guide", vec!["prog"]),
            ("https://example.com/pasta", "Pasta recipe", vec!["food"]),
        ] {
            let (status, _) = body_json(
                app.clone()
                    .oneshot(post_json(
                        "/api/v2/bookmarks",
                        serde_json::json!({"url": url, "title": title, "tags": tags}),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }
        async fn total(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            body_json(res).await
        }
        let (status, v) = total(&app, "/api/v2/bookmarks?q=tag:prog").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);
        assert_eq!(v["bookmarks"][0]["title"], "Rust guide");
        let (status, v) = total(&app, "/api/v2/bookmarks?q=title:pasta%20tag:food").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);
        let (status, v) = total(&app, "/api/v2/bookmarks?q=rust&scope=title").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);
        let (status, v) = total(&app, "/api/v2/bookmarks?q=prog&scope=title").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 0);
        let (status, v) = total(&app, "/api/v2/bookmarks?q=rust&scope=u").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["total"], 1);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?q=rust&scope=bogus")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn delete_confirm_union() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/del"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["confirm_required"], true);
        assert_eq!(v["bookmark"]["uuid"], uuid);
        assert!(v["hint"].as_str().unwrap().contains("confirm"));

        let res = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v2/bookmarks/{uuid}?confirm=true"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["deleted"], uuid);
    }

    #[tokio::test]
    async fn update_validation_errors() {
        let (app, _dir) = test_state("");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({"url": "https://example.com/u"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let uuid = v["uuid"].as_str().unwrap().to_string();

        for body in ["{\"title\":\"  \"}", "{\"url\":\"  \"}"] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri(format!("/api/v2/bookmarks/{uuid}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            let (status, v) = body_json(res).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(v["error"].is_string());
        }
    }

    #[tokio::test]
    async fn missing_resources_404_json() {
        let (app, _dir) = test_state("");
        let missing = "00000000-0000-0000-0000-000000000000";
        let get = Request::builder()
            .uri(format!("/api/v2/bookmarks/{missing}"))
            .body(Body::empty())
            .unwrap();
        let (status, v) = body_json(app.clone().oneshot(get).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(v["error"].is_string());

        let open = Request::builder()
            .method("POST")
            .uri(format!("/api/v2/bookmarks/{missing}/open"))
            .body(Body::empty())
            .unwrap();
        let (status, v) = body_json(app.clone().oneshot(open).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(v["error"].is_string());

        let notes = Request::builder()
            .uri(format!("/api/v2/bookmarks/{missing}/notes"))
            .body(Body::empty())
            .unwrap();
        let (status, v) = body_json(app.clone().oneshot(notes).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(v["error"].is_string());

        let attach = Request::builder()
            .uri(format!("/api/v2/bookmarks/{missing}/attachments/nope.txt"))
            .body(Body::empty())
            .unwrap();
        let (status, v) = body_json(app.oneshot(attach).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(v["error"].is_string());
    }

    #[tokio::test]
    async fn pagination_envelope_clamps() {
        let (app, _dir) = test_state("");
        for url in [
            "https://example.com/p1",
            "https://example.com/p2",
            "https://example.com/p3",
        ] {
            let (status, _) = body_json(
                app.clone()
                    .oneshot(post_json(
                        "/api/v2/bookmarks",
                        serde_json::json!({"url": url}),
                    ))
                    .await
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?per_page=5000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["per_page"], 500);
        assert_eq!(v["total"], 3);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?per_page=2&page=2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["page"], 2);
        assert_eq!(v["total"], 3);
        assert_eq!(v["bookmarks"].as_array().unwrap().len(), 1);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks?page=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["page"], 1);
    }

    #[tokio::test]
    async fn health_and_unknown_route_shapes() {
        let (app, _dir) = test_state("");
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v2/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/no-such-thing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, v) = body_json(res).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(v["error"], "not found");
    }

    #[tokio::test]
    async fn add_with_attachments_warns_per_file() {
        use base64::Engine;
        let (app, _dir) = test_state("");
        let good = base64::engine::general_purpose::STANDARD.encode(b"hello");
        let (status, v) = body_json(
            app.clone()
                .oneshot(post_json(
                    "/api/v2/bookmarks",
                    serde_json::json!({
                        "url": "https://example.com/a",
                        "attachments": [
                            {"name": "a.txt", "content": good},
                            {"name": "", "content": good},
                            {"name": "b.txt", "content": "!!!"},
                        ],
                    }),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(v["attachments"].as_array().unwrap().len(), 1);
        let warnings = v["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 2);
    }
}
