use liber_core::create::{create_bookmark, CreateOptions};
use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
use liber_core::search::{order_results, resolve_sort_mode, SearchFields};
use liber_core::store::{BookmarkFilter, Store};
use liber_tauri::{open_store, AddResult, AppState, ListResponse, TauriBookmark};
use tauri::State;

fn resolve_one(store: &Store, id: &str) -> Result<liber_core::model::Bookmark, String> {
    let tokens = liber_core::idspec::parse_id_spec(id).map_err(|e| e.to_string())?;
    let hits = store.resolve_spec(&tokens).map_err(|e| e.to_string())?;
    hits.into_iter()
        .next()
        .ok_or_else(|| "not found".to_string())
}

#[tauri::command]
fn list_bookmarks(
    state: State<'_, AppState>,
    q: Option<String>,
    sort: Option<String>,
    tag: Option<String>,
    folder: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
) -> Result<ListResponse, String> {
    let store = open_store(&state)?;
    let mode = resolve_sort_mode(sort.as_deref(), q.as_deref()).map_err(|e| e.to_string())?;
    let filter = BookmarkFilter {
        folder,
        tag,
        query: q.clone(),
        opened_only: false,
    };
    let per_page = per_page.unwrap_or(50).clamp(1, 500);
    let page_num = page.unwrap_or(1).max(1);
    let start = (page_num - 1) * per_page;
    let (found, total) = store
        .query_bookmarks(&filter, mode, per_page, start)
        .map_err(|e| e.to_string())?;
    let fields = SearchFields::all();
    let ordered = order_results(found, q.as_deref().unwrap_or(""), &fields, mode);
    Ok(ListResponse {
        total,
        page: page_num,
        per_page,
        bookmarks: ordered.iter().map(TauriBookmark::from).collect(),
    })
}

#[tauri::command]
fn get_bookmark(state: State<'_, AppState>, id: String) -> Result<TauriBookmark, String> {
    let store = open_store(&state)?;
    Ok(TauriBookmark::from(&resolve_one(&store, &id)?))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn add_bookmark(
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    markdown: Option<bool>,
    confirm_dup: Option<bool>,
) -> Result<AddResult, String> {
    if url.trim().is_empty() {
        return Err("url is required".to_string());
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let opts = CreateOptions {
        title,
        description: description.unwrap_or_default(),
        tags: tags.unwrap_or_default(),
        folder: folder.unwrap_or_default(),
        markdown: markdown.unwrap_or(false),
    };
    match create_bookmark(&mut store, &url, opts) {
        Ok(b) => Ok(AddResult {
            status: "created".to_string(),
            bookmark: TauriBookmark::from(&b),
        }),
        Err(liber_core::CoreError::Duplicate(_)) => {
            let dup = store
                .find_by_url(&liber_core::slug::normalize_url(&url))
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "duplicate".to_string())?;
            if confirm_dup.unwrap_or(false) {
                return Ok(AddResult {
                    status: "duplicate_accepted".to_string(),
                    bookmark: TauriBookmark::from(&dup),
                });
            }
            Err(serde_json::json!({
                "error": "duplicate",
                "existing": TauriBookmark::from(&dup),
                "hint": "repeat with confirm_dup true to accept",
            })
            .to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn update_bookmark(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    url: Option<String>,
) -> Result<TauriBookmark, String> {
    if let Some(t) = &title {
        if t.trim().is_empty() {
            return Err("title must not be empty".to_string());
        }
    }
    if let Some(u) = &url {
        if u.trim().is_empty() {
            return Err("url must not be empty".to_string());
        }
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let out = edit_bookmark(
        &mut store,
        &target.uuid,
        EditOptions {
            title,
            description,
            tags,
            folder,
            url,
            add_markdown: false,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(TauriBookmark::from(&out))
}

#[tauri::command]
fn delete_bookmark(
    state: State<'_, AppState>,
    id: String,
    confirm: Option<bool>,
) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    if !confirm.unwrap_or(false) {
        return Ok(serde_json::json!({
            "confirm_required": true,
            "bookmark": TauriBookmark::from(&target),
            "hint": "repeat with confirm true to delete",
        }));
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    delete_bookmark_with_files(&mut store, &target.uuid).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"deleted": target.uuid.to_string()}))
}

#[tauri::command]
fn open_bookmark(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    store.record_open(&target.uuid).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"url": target.url}))
}

#[tauri::command]
fn list_tags(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let counts = liber_core::taxonomy::tag_counts(&store).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "tags": counts.iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect::<Vec<_>>(),
    }))
}

#[tauri::command]
fn list_folders(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let counts = liber_core::taxonomy::folder_counts(&store).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "folders": counts.iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect::<Vec<_>>(),
    }))
}

#[tauri::command]
fn rename_tag(
    state: State<'_, AppState>,
    old: String,
    new_tag: String,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let changed =
        liber_core::taxonomy::rename_tag(&mut store, &old, &new_tag).map_err(|e| e.to_string())?;
    if changed.is_empty() {
        return Err("no bookmarks have that tag".to_string());
    }
    Ok(serde_json::json!({"renamed": changed.len()}))
}

#[tauri::command]
fn delete_tag(
    state: State<'_, AppState>,
    tag: String,
    confirm: Option<bool>,
) -> Result<serde_json::Value, String> {
    if tag.trim().is_empty() {
        return Err("tag is required".to_string());
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    if !confirm.unwrap_or(false) {
        let counts = liber_core::taxonomy::tag_counts(&store).map_err(|e| e.to_string())?;
        let found = counts
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(tag.trim()));
        let Some((_, count)) = found else {
            return Err("no bookmarks have that tag".to_string());
        };
        return Ok(serde_json::json!({
            "confirm_required": true,
            "count": count,
            "hint": "repeat with confirm true to delete",
        }));
    }
    let changed = liber_core::taxonomy::delete_tag(&mut store, &tag).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"deleted": changed.len()}))
}

#[tauri::command]
fn rename_folder(
    state: State<'_, AppState>,
    old: String,
    new_folder: String,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let changed = liber_core::taxonomy::rename_folder(&mut store, &old, &new_folder)
        .map_err(|e| e.to_string())?;
    if changed.is_empty() {
        return Err("no bookmarks in that folder".to_string());
    }
    Ok(serde_json::json!({"renamed": changed.len()}))
}

#[tauri::command]
fn delete_folder(state: State<'_, AppState>, folder: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let changed =
        liber_core::taxonomy::delete_folder(&mut store, &folder).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"moved_to_root": changed.len()}))
}

fn main() {
    let (cfg, _) = liber_core::config::load_config().expect("loading config");
    let state = liber_tauri::AppState::new(cfg);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            list_bookmarks,
            get_bookmark,
            add_bookmark,
            update_bookmark,
            delete_bookmark,
            open_bookmark,
            list_tags,
            list_folders,
            rename_tag,
            delete_tag,
            rename_folder,
            delete_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error running liber");
}
