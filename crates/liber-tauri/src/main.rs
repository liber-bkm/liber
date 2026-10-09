use liber_core::create::{create_bookmark, CreateOptions};
use liber_core::edit::{delete_bookmark_with_files, edit_bookmark, EditOptions};
use liber_core::search::{order_results, resolve_sort_mode, SearchFields};
use liber_core::store::{BookmarkFilter, Store};
use liber_tauri::{open_store, AddResult, AppState, ListResponse, TauriBookmark};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

fn resolve_one(store: &Store, id: &str) -> Result<liber_core::model::Bookmark, String> {
    let tokens = liber_core::idspec::parse_id_spec(id).map_err(|e| e.to_string())?;
    let hits = store.resolve_spec(&tokens).map_err(|e| e.to_string())?;
    hits.into_iter()
        .next()
        .ok_or_else(|| "not found".to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn list_bookmarks(
    state: State<'_, AppState>,
    q: Option<String>,
    sort: Option<String>,
    tag: Option<String>,
    folder: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
    scope: Option<String>,
) -> Result<ListResponse, String> {
    let store = open_store(&state)?;
    let mode = resolve_sort_mode(sort.as_deref(), q.as_deref()).map_err(|e| e.to_string())?;
    let fields = match &scope {
        Some(s) => liber_core::search::parse_field_list(s).map_err(|e| e.to_string())?,
        None => SearchFields::all(),
    };
    let filter = BookmarkFilter {
        folder,
        tag,
        query: q.clone(),
        scope: fields,
        opened_only: false,
    };
    let per_page = per_page.unwrap_or(50).clamp(1, 500);
    let page_num = page.unwrap_or(1).max(1);
    let start = (page_num - 1) * per_page;
    let (found, total) = store
        .query_bookmarks(&filter, mode, per_page, start)
        .map_err(|e| e.to_string())?;
    let ordered = order_results(found, q.as_deref().unwrap_or(""), mode);
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
async fn add_bookmark(
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    markdown: Option<bool>,
    archive: Option<bool>,
    confirm_dup: Option<bool>,
) -> Result<AddResult, String> {
    if url.trim().is_empty() {
        return Err("url is required".to_string());
    }
    let cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let mu = state.write_mu.clone();
    tauri::async_runtime::spawn_blocking(move || {
        add_bookmark_blocking(AddJob {
            cfg,
            mu,
            url,
            title,
            description,
            tags,
            folder,
            markdown,
            archive,
            confirm_dup,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

struct AddJob {
    cfg: liber_core::store::Config,
    mu: std::sync::Arc<std::sync::Mutex<()>>,
    url: String,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    markdown: Option<bool>,
    archive: Option<bool>,
    confirm_dup: Option<bool>,
}

fn add_bookmark_blocking(job: AddJob) -> Result<AddResult, String> {
    let AddJob {
        cfg,
        mu,
        url,
        title,
        description,
        tags,
        folder,
        markdown,
        archive,
        confirm_dup,
    } = job;
    let _guard = mu.lock().map_err(|e| e.to_string())?;
    let mut store = Store::open(cfg).map_err(|e| e.to_string())?;
    let opts = CreateOptions {
        title,
        description: description.unwrap_or_default(),
        tags: tags.unwrap_or_default(),
        folder: folder.unwrap_or_default(),
        markdown: markdown.unwrap_or(false),
    };
    match create_bookmark(&mut store, &url, opts) {
        Ok(b) => {
            let mut warnings = Vec::new();
            let b = if archive.unwrap_or(false) {
                match liber_core::archive::archive_bookmark(&mut store, &b.uuid, None) {
                    Ok(w) => {
                        warnings.extend(w);
                        store.get(&b.uuid).map_err(|e| e.to_string())?.unwrap_or(b)
                    }
                    Err(e) => {
                        warnings.push(format!("archive failed: {e}"));
                        b
                    }
                }
            } else {
                b
            };
            Ok(AddResult {
                status: "created".to_string(),
                bookmark: TauriBookmark::from(&b),
                warnings,
            })
        }
        Err(liber_core::CoreError::Duplicate(_)) => {
            let dup = store
                .find_by_url(&liber_core::slug::normalize_url(&url))
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "duplicate".to_string())?;
            if confirm_dup.unwrap_or(false) {
                return Ok(AddResult {
                    status: "duplicate_accepted".to_string(),
                    bookmark: TauriBookmark::from(&dup),
                    warnings: Vec::new(),
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

fn rule_shape(rule: &liber_core::model::AutoRule, applied_count: usize) -> serde_json::Value {
    serde_json::json!({
        "id": rule.id,
        "pattern": rule.pattern,
        "tags": rule.action_tags,
        "folder": rule.action_folder,
        "description": liber_core::automation::describe_rule(rule),
        "applied_count": applied_count,
    })
}

fn applied_counts(store: &Store) -> Result<std::collections::HashMap<String, usize>, String> {
    let mut counts = std::collections::HashMap::new();
    let list = store.list().map_err(|e| e.to_string())?;
    for b in &list {
        for a in &b.applied_rules {
            *counts.entry(a.rule_id.clone()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

#[tauri::command]
fn list_rules(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let counts = applied_counts(&store)?;
    let rules: Vec<serde_json::Value> = store
        .list_rules()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| rule_shape(r, counts.get(&r.id).copied().unwrap_or(0)))
        .collect();
    Ok(serde_json::json!({"rules": rules}))
}

#[tauri::command]
fn add_rule(
    state: State<'_, AppState>,
    pattern: String,
    tags: Option<Vec<String>>,
    folder: Option<String>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let (rule, changed) =
        liber_core::automation::create_rule(&mut store, pattern, tags.unwrap_or_default(), folder)
            .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"rule": rule_shape(&rule, changed.len())}))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn edit_rule(
    state: State<'_, AppState>,
    id: String,
    pattern: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    reapply: Option<bool>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let (rule, changed) = liber_core::automation::edit_rule(
        &mut store,
        &id,
        pattern,
        tags,
        folder,
        reapply.unwrap_or(false),
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"rule": rule_shape(&rule, 0), "reapplied": changed.len()}))
}

#[tauri::command]
fn delete_rule(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    if !store.delete_rule(&id).map_err(|e| e.to_string())? {
        return Err("no such rule".to_string());
    }
    Ok(serde_json::json!({"deleted": id}))
}

#[tauri::command]
fn apply_rules(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let changed = liber_core::automation::apply_rules(&mut store, id.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"applied": changed.len()}))
}

#[tauri::command]
fn learn_suggestions(
    state: State<'_, AppState>,
    min: Option<usize>,
) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let out = liber_core::automation::suggest_rules(&store, min.unwrap_or(3).max(2))
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "suggestions": out.iter().map(|s| serde_json::json!({
            "host": s.host, "folder": s.folder, "count": s.count,
        })).collect::<Vec<_>>(),
    }))
}

#[tauri::command]
fn learn_create(
    state: State<'_, AppState>,
    min: Option<usize>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let suggestions = liber_core::automation::suggest_rules(&store, min.unwrap_or(3).max(2))
        .map_err(|e| e.to_string())?;
    let mut created = 0;
    let mut applied = 0;
    for s in suggestions {
        let (_, changed) = liber_core::automation::create_rule(
            &mut store,
            format!("host:{}", s.host),
            vec![],
            Some(s.folder),
        )
        .map_err(|e| e.to_string())?;
        created += 1;
        applied += changed.len();
    }
    Ok(serde_json::json!({"created": created, "applied": applied}))
}

#[tauri::command]
fn fetch_history(state: State<'_, AppState>) -> Result<ListResponse, String> {
    use liber_core::search::SortMode;
    let store = open_store(&state)?;
    let filter = BookmarkFilter {
        opened_only: true,
        ..Default::default()
    };
    let (found, total) = store
        .query_bookmarks(&filter, SortMode::Visited, 100, 0)
        .map_err(|e| e.to_string())?;
    Ok(ListResponse {
        total,
        page: 1,
        per_page: 100,
        bookmarks: found.iter().map(TauriBookmark::from).collect(),
    })
}

#[tauri::command]
fn fetch_settings(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let status = open_store(&state)?
        .maintenance_status()
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "base_dir": cfg.base_dir.to_string_lossy(),
        "html_dir": cfg.html_dir,
        "markdown_dir": cfg.markdown_dir,
        "archive_dir": cfg.archive_dir,
        "attachment_dir": cfg.attachment_dir,
        "device_id": cfg.device_id,
        "archive_backend": cfg.archive_backend,
        "browser_cmd": cfg.browser_cmd,
        "browser_path": cfg.browser_path,
        "editor_cmd": cfg.editor_cmd,
        "singlefile_cmd": cfg.singlefile_cmd,
        "singlefile_browser_path": cfg.singlefile_browser_path,
        "monolith_cmd": cfg.monolith_cmd,
        "active_profile": cfg.active_profile,
        "maintenance_status": status,
    }))
}

#[tauri::command]
fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<serde_json::Value, String> {
    if !liber_core::config::API_SETTABLE.contains(&key.as_str()) {
        return Err(format!("unknown key {key:?}"));
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    liber_core::config::apply_setting(&mut cfg, &key, &value).map_err(|e| e.to_string())?;
    liber_core::config::save_config_to(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    *state.cfg.lock().map_err(|e| e.to_string())? = cfg;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    Ok(serde_json::json!({
        "active": cfg.active_profile.clone().unwrap_or("default".to_string()),
        "profiles": liber_core::profile::list_profiles(&cfg),
    }))
}

#[tauri::command]
fn switch_profile(state: State<'_, AppState>, name: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let active = liber_core::profile::switch_profile(&mut cfg, &name).map_err(|e| e.to_string())?;
    liber_core::config::save_config_to(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    *state.cfg.lock().map_err(|e| e.to_string())? = cfg;
    Ok(serde_json::json!({"result": "switched", "active": active}))
}

#[tauri::command]
fn delete_profile(state: State<'_, AppState>, name: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let deleted =
        liber_core::profile::delete_profile(&mut cfg, &name).map_err(|e| e.to_string())?;
    liber_core::config::save_config_to(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    *state.cfg.lock().map_err(|e| e.to_string())? = cfg;
    Ok(serde_json::json!({"result": "deleted", "name": deleted}))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn bulk_op(
    state: State<'_, AppState>,
    ids: Vec<String>,
    op: String,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    confirm: Option<bool>,
) -> Result<serde_json::Value, String> {
    if ids.is_empty() {
        return Err("no bookmarks selected".to_string());
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let mut targets = Vec::new();
    for id in &ids {
        let target = resolve_one(&store, id).map_err(|_| format!("not found: {id}"))?;
        targets.push(target);
    }
    match op.as_str() {
        "delete" => {
            if !confirm.unwrap_or(false) {
                return Ok(serde_json::json!({
                    "confirm_required": true,
                    "count": targets.len(),
                    "hint": "repeat with confirm true to delete",
                }));
            }
            for b in &targets {
                liber_core::edit::delete_bookmark_with_files(&mut store, &b.uuid)
                    .map_err(|e| e.to_string())?;
            }
            Ok(serde_json::json!({"deleted": targets.len()}))
        }
        "set_tags" => {
            let tags = tags.unwrap_or_default();
            for b in &targets {
                edit_bookmark(
                    &mut store,
                    &b.uuid,
                    EditOptions {
                        tags: Some(tags.clone()),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(serde_json::json!({"updated": targets.len()}))
        }
        "move_folder" => {
            let folder = folder.unwrap_or_default();
            for b in &targets {
                edit_bookmark(
                    &mut store,
                    &b.uuid,
                    EditOptions {
                        folder: Some(folder.clone()),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(serde_json::json!({"updated": targets.len()}))
        }
        other => Err(format!(
            "unknown bulk op {other:?} (delete, set_tags, move_folder)"
        )),
    }
}

#[tauri::command]
fn run_reindex(
    state: State<'_, AppState>,
    prune: Option<bool>,
    compact_ids: Option<bool>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let rep = liber_core::reindex::reindex(
        &mut store,
        liber_core::reindex::ReindexFlags {
            prune: prune.unwrap_or(false),
            compact_ids: compact_ids.unwrap_or(false),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "adopted": rep.adopted,
        "relinked_markdown": rep.relinked_markdown,
        "relinked_archive": rep.relinked_archive,
        "swept_conflicts": rep.swept_conflicts,
        "quarantined_attachments": rep.quarantined_attachments,
        "pending": rep.pending,
        "pruned": rep.pruned,
        "indexed": rep.indexed,
        "short_ids_compacted": rep.short_ids_compacted,
    }))
}

#[tauri::command]
async fn check_run(
    state: State<'_, AppState>,
    spec: Option<String>,
    workers: Option<usize>,
    stale_hours: Option<u64>,
    stale: Option<String>,
) -> Result<serde_json::Value, String> {
    let (targets, fresh) = {
        let store = open_store(&state)?;
        let tokens = match &spec {
            Some(s) if !s.trim().is_empty() => {
                Some(liber_core::idspec::parse_id_spec(s).map_err(|e| e.to_string())?)
            }
            _ => None,
        };
        let stale = match (&stale, stale_hours) {
            (Some(s), _) if !s.trim().is_empty() => {
                Some(liber_core::check::parse_stale_duration(s).map_err(|e| e.to_string())?)
            }
            (_, Some(h)) => Some(std::time::Duration::from_secs(h.saturating_mul(3600))),
            _ => None,
        };
        liber_core::check::resolve_check_targets(&store, tokens.as_deref(), stale)
            .map_err(|e| e.to_string())?
    };
    let total = targets.len();
    let workers = workers.unwrap_or(12).max(1);
    let outcomes = tauri::async_runtime::spawn_blocking(move || {
        let client = liber_core::check::check_client()?;
        Ok::<_, liber_core::CoreError>(liber_core::check::scan_targets(
            &client,
            targets,
            workers,
            |_, _| {},
        ))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let mut rows = Vec::new();
    let mut ok = 0;
    for o in &outcomes {
        store
            .stamp_check(&o.bookmark.uuid, Some(o.result.status.as_str()))
            .map_err(|e| e.to_string())?;
        match o.result.status {
            liber_core::check::CheckStatus::Ok => ok += 1,
            _ => rows.push(serde_json::json!({
                "uuid": o.bookmark.uuid.to_string(),
                "title": o.bookmark.title,
                "url": o.bookmark.url,
                "status": o.result.status.as_str(),
                "detail": o.result.detail,
                "target": o.result.target,
            })),
        }
    }
    Ok(serde_json::json!({
        "checked": total,
        "ok": ok,
        "fresh_skipped": fresh,
        "rows": rows,
    }))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn check_apply(
    state: State<'_, AppState>,
    updates: Option<Vec<CheckUpdate>>,
    quarantine: Option<Vec<String>>,
    delete: Option<Vec<String>>,
    confirm: Option<bool>,
) -> Result<serde_json::Value, String> {
    let delete = delete.unwrap_or_default();
    if !delete.is_empty() && !confirm.unwrap_or(false) {
        return Ok(serde_json::json!({
            "confirm_required": true,
            "count": delete.len(),
            "hint": "repeat with confirm true to delete",
        }));
    }
    let updates = updates.unwrap_or_default();
    let mut titles = std::collections::HashMap::new();
    for u in &updates {
        if u.retitle.unwrap_or(false) {
            titles.insert(u.uuid.clone(), liber_core::check::refetch_title(&u.url));
        }
    }
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let mut updated = 0;
    let mut retitled = 0;
    let mut quarantined = 0;
    let mut deleted = 0;
    let mut skipped = Vec::new();
    for u in &updates {
        let target = match resolve_one(&store, &u.uuid) {
            Ok(b) => b,
            Err(_) => {
                skipped.push(serde_json::json!({"uuid": u.uuid, "reason": "not found"}));
                continue;
            }
        };
        match edit_bookmark(
            &mut store,
            &target.uuid,
            EditOptions {
                url: Some(u.url.clone()),
                ..Default::default()
            },
        ) {
            Ok(_) => updated += 1,
            Err(liber_core::CoreError::Duplicate(_)) => {
                skipped.push(
                    serde_json::json!({"uuid": u.uuid, "reason": "target already bookmarked"}),
                );
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
        if let Some(title) = titles.remove(&u.uuid) {
            let current = store
                .get(&target.uuid)
                .map_err(|e| e.to_string())?
                .map(|b| b.title)
                .unwrap_or_default();
            if !title.is_empty() && title != current {
                edit_bookmark(
                    &mut store,
                    &target.uuid,
                    EditOptions {
                        title: Some(title),
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
                retitled += 1;
            }
        }
    }
    for q in quarantine.unwrap_or_default() {
        let target = match resolve_one(&store, &q) {
            Ok(b) => b,
            Err(_) => {
                skipped.push(serde_json::json!({"uuid": q, "reason": "not found"}));
                continue;
            }
        };
        if liber_core::check::quarantine_bookmark(&mut store, &target.uuid)
            .map_err(|e| e.to_string())?
        {
            quarantined += 1;
        }
    }
    for d in &delete {
        let target = match resolve_one(&store, d) {
            Ok(b) => b,
            Err(_) => {
                skipped.push(serde_json::json!({"uuid": d, "reason": "already gone"}));
                continue;
            }
        };
        if liber_core::edit::delete_bookmark_with_files(&mut store, &target.uuid)
            .map_err(|e| e.to_string())?
        {
            deleted += 1;
        } else {
            skipped.push(serde_json::json!({"uuid": d, "reason": "already gone"}));
        }
    }
    Ok(serde_json::json!({
        "updated": updated,
        "retitled": retitled,
        "quarantined": quarantined,
        "deleted": deleted,
        "skipped": skipped,
    }))
}

#[derive(Debug, serde::Deserialize)]
struct CheckUpdate {
    uuid: String,
    url: String,
    retitle: Option<bool>,
}

#[tauri::command]
fn fetch_notes(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let body = liber_core::edit::read_note_body(&store, &target.uuid).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"body": body}))
}

#[tauri::command]
fn save_notes(
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    liber_core::edit::save_note_body(&mut store, &target.uuid, &body).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
fn delete_notes(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    liber_core::edit::apply_edit(
        &mut store,
        &target.uuid,
        liber_core::edit::EditDraft {
            markdown: liber_core::edit::MarkdownAction::Remove,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
fn fetch_archive(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let Some(rel) = &target.archive_file else {
        return Err("no archive for this bookmark".to_string());
    };
    let html = std::fs::read_to_string(store.cfg.archive_dir().join(rel))
        .map_err(|_| "archive file missing".to_string())?;
    Ok(serde_json::json!({"html": html}))
}

fn temp_open_path(file_name: &str) -> std::path::PathBuf {
    let safe: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("liber-{safe}"))
}

#[tauri::command]
fn open_archive_external(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let Some(rel) = &target.archive_file else {
        return Err("no archive for this bookmark".to_string());
    };
    let html = std::fs::read_to_string(store.cfg.archive_dir().join(rel))
        .map_err(|_| "archive file missing".to_string())?;
    let path = temp_open_path(&format!("archive-{}.html", target.uuid));
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<String>)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"opened": true}))
}

#[tauri::command]
fn open_attachment_external(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
    name: String,
) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let Some(at) = target
        .attachments
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&name))
    else {
        return Err("no such attachment".to_string());
    };
    let data = std::fs::read(store.cfg.attachment_dir().join(&at.path))
        .map_err(|_| "attachment file missing".to_string())?;
    let path = temp_open_path(&format!("{}-{}_{}", target.uuid, "attachment", at.name));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<String>)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"opened": true}))
}

#[tauri::command]
async fn create_archive(
    state: State<'_, AppState>,
    id: String,
    backend: Option<String>,
) -> Result<serde_json::Value, String> {
    let cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let mu = state.write_mu.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = mu.lock().map_err(|e| e.to_string())?;
        let mut store = Store::open(cfg).map_err(|e| e.to_string())?;
        let target = resolve_one(&store, &id)?;
        let applied = liber_core::edit::apply_edit(
            &mut store,
            &target.uuid,
            liber_core::edit::EditDraft {
                archive: liber_core::edit::ArchiveAction::Add { backend },
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"ok": true, "warnings": applied.warnings}))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn delete_archive(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    liber_core::edit::apply_edit(
        &mut store,
        &target.uuid,
        liber_core::edit::EditDraft {
            archive: liber_core::edit::ArchiveAction::Remove,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
fn upload_attachment(
    state: State<'_, AppState>,
    id: String,
    name: String,
    content: String,
) -> Result<serde_json::Value, String> {
    use base64::Engine;
    if name.trim().is_empty() {
        return Err("name is required".to_string());
    }
    if content.len() > 32 << 20 {
        return Err("attachment too large".to_string());
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .map_err(|_| "content is not valid base64".to_string())?;
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let at = liber_core::attach::attach_bytes(&mut store, &target.uuid, name.trim(), &data)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"name": at.name}))
}

#[tauri::command]
fn delete_attachment(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let detached = liber_core::attach::detach_attachment(&mut store, &target.uuid, &name)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"detached": detached}))
}

#[tauri::command]
fn download_attachment(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<serde_json::Value, String> {
    use base64::Engine;
    let store = open_store(&state)?;
    let target = resolve_one(&store, &id)?;
    let Some(at) = target
        .attachments
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&name))
    else {
        return Err("no such attachment".to_string());
    };
    let data = std::fs::read(store.cfg.attachment_dir().join(&at.path))
        .map_err(|_| "attachment file missing".to_string())?;
    let mime = liber_core::archive::mime_for(&at.name, None);
    Ok(serde_json::json!({
        "name": at.name,
        "mime": mime,
        "content": base64::engine::general_purpose::STANDARD.encode(&data),
    }))
}

#[tauri::command]
fn import_library(
    state: State<'_, AppState>,
    content: String,
    markdown: Option<bool>,
    archive: Option<bool>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let mut store = open_store(&state)?;
    let report = liber_core::import::import_data(
        &mut store,
        &content,
        markdown.unwrap_or(false),
        archive.unwrap_or(false),
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "added": report.added,
        "skipped_dup": report.skipped_dup,
        "skipped_bad": report.skipped_bad,
        "warnings": report.warnings,
    }))
}

#[tauri::command]
fn export_bookmarks(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let doc = liber_core::export::write_netscape_export(&store).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"content": doc}))
}

#[tauri::command]
fn export_site(
    state: State<'_, AppState>,
    dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = open_store(&state)?;
    let out_dir = match &dir {
        Some(d) if !d.trim().is_empty() => std::path::PathBuf::from(d.trim()),
        _ => store.cfg.profile_dir().join("site"),
    };
    let index = liber_core::export::export_site(&store, &out_dir).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"index": index.to_string_lossy()}))
}

#[tauri::command]
fn sync_commit(
    state: State<'_, AppState>,
    push: Option<bool>,
) -> Result<serde_json::Value, String> {
    let _guard = state.write_mu.lock().map_err(|e| e.to_string())?;
    let cfg = state.cfg.lock().map_err(|e| e.to_string())?.clone();
    let message = chrono::Utc::now()
        .format("liber sync: %Y-%m-%d %H:%M:%S")
        .to_string();
    match liber_core::sync::git_snapshot(&cfg.profile_dir(), &message, push.unwrap_or(false)) {
        Err(e) => Ok(serde_json::json!({"output": "", "error": e.to_string()})),
        Ok(false) => Ok(serde_json::json!({
            "output": "Not a git repository, nothing committed (repos are never initialized automatically).",
        })),
        Ok(true) => Ok(serde_json::json!({"output": "Committed sync snapshot."})),
    }
}

fn resolve_app_config(
    app: &tauri::AppHandle,
) -> Result<(liber_core::store::Config, std::path::PathBuf), String> {
    if liber_core::config::system_dirs_available() {
        return liber_core::config::load_config().map_err(|e| e.to_string());
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    liber_core::config::load_config_from(dir.join("config.json"), dir.join("library"))
        .map_err(|e| e.to_string())
}

pub(crate) fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let (cfg, path) = resolve_app_config(app.handle())
                .map_err(|e| Box::<dyn std::error::Error>::from(format!("loading config: {e}")))?;
            app.manage(liber_tauri::AppState::new(cfg, path));
            Ok(())
        })
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
            list_rules,
            add_rule,
            edit_rule,
            delete_rule,
            apply_rules,
            learn_suggestions,
            learn_create,
            fetch_history,
            fetch_settings,
            set_setting,
            bulk_op,
            run_reindex,
            check_run,
            check_apply,
            fetch_notes,
            save_notes,
            delete_notes,
            fetch_archive,
            create_archive,
            delete_archive,
            upload_attachment,
            delete_attachment,
            download_attachment,
            open_archive_external,
            open_attachment_external,
            import_library,
            export_bookmarks,
            export_site,
            list_profiles,
            switch_profile,
            delete_profile,
            sync_commit,
        ])
        .run(tauri::generate_context!())
        .expect("error running liber");
}
