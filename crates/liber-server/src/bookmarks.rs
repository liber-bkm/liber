use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

use liber_core::create::{create_bookmark, CreateOptions};
use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
use liber_core::search::{order_results, parse_sort_mode, SearchFields};
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
    Store::open(state.cfg.clone()).map_err(core_err)
}

pub async fn list_bookmarks(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<ListResponse>, (StatusCode, Json<serde_json::Value>)> {
    use liber_core::store::BookmarkFilter;
    let store = open_store(&state)?;
    let sort = parse_sort_mode(p.sort.as_deref().unwrap_or("")).map_err(core_err)?;
    let filter = BookmarkFilter {
        folder: p.folder.clone(),
        tag: p.tag.clone(),
        query: p.q.clone(),
        opened_only: false,
    };
    let per_page = p.per_page.unwrap_or(50).clamp(1, 500);
    let page = p.page.unwrap_or(1).max(1);
    let start = (page - 1) * per_page;
    let (found, total) = store
        .query_bookmarks(&filter, sort, per_page, start)
        .map_err(core_err)?;
    let fields = SearchFields::all();
    let ordered = order_results(found, p.q.as_deref().unwrap_or(""), &fields, sort);
    let slice: Vec<ApiBookmark> = ordered.iter().map(ApiBookmark::from).collect();
    Ok(Json(ListResponse {
        total,
        page,
        per_page,
        bookmarks: slice,
    }))
}

pub async fn add_bookmark(
    State(state): State<AppState>,
    Json(input): Json<AddRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if input.url.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "url is required"));
    }
    let _guard = state.write_mu.lock().await;
    let mut store = open_store(&state)?;
    let opts = CreateOptions {
        title: input.title,
        description: input.description.unwrap_or_default(),
        tags: input.tags,
        folder: input.folder.unwrap_or_default(),
        markdown: input.markdown,
    };
    match create_bookmark(&mut store, &input.url, opts) {
        Ok(b) => Ok((
            StatusCode::CREATED,
            Json(serde_json::to_value(ApiBookmark::from(&b)).unwrap_or_default()),
        )),
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

pub async fn get_bookmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiBookmark>, (StatusCode, Json<serde_json::Value>)> {
    Ok(Json(ApiBookmark::from(&resolve_one(&state, &id).await?)))
}

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
}
