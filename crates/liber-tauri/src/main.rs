use liber_core::create::{create_bookmark, CreateOptions};
use liber_core::search::{order_results, parse_sort_mode, SearchFields};
use liber_core::store::BookmarkFilter;
use liber_tauri::{open_store, AddResult, AppState, ListResponse, TauriBookmark};
use tauri::State;

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
    let mode = parse_sort_mode(sort.as_deref().unwrap_or("")).map_err(|e| e.to_string())?;
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
    let tokens = liber_core::idspec::parse_id_spec(&id).map_err(|e| e.to_string())?;
    let mut hits = store.resolve_spec(&tokens).map_err(|e| e.to_string())?;
    match hits.pop() {
        Some(b) => Ok(TauriBookmark::from(&b)),
        None => Err("not found".to_string()),
    }
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
            Err("duplicate".to_string())
        }
        Err(e) => Err(e.to_string()),
    }
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
        ])
        .run(tauri::generate_context!())
        .expect("error running liber");
}
