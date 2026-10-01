use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::dedupe::normalize_for_dedupe;
use crate::model::{AutoRule, Bookmark};
use crate::slug::dedupe_strings;
use crate::store::Store;
use crate::CoreError;

pub const OPLOG_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Default)]
pub struct MergeReport {
    pub merged: usize,
    pub inserted: usize,
    pub deduped: usize,
    pub deleted: usize,
    pub rules: usize,
}

fn entry_key(
    device: &str,
    ts: &DateTime<Utc>,
    op: &str,
    uuid: &Option<Uuid>,
) -> (String, String, String, String) {
    (
        device.to_string(),
        ts.to_rfc3339(),
        op.to_string(),
        uuid.map(|u| u.to_string()).unwrap_or_default(),
    )
}

fn merge_fields(dst: &mut Bookmark, src: &Bookmark) {
    if src.updated_at > dst.updated_at {
        dst.title = src.title.clone();
        dst.description = src.description.clone();
        dst.url = src.url.clone();
        dst.folder = src.folder.clone();
        dst.updated_at = src.updated_at;
    }
    if src.created_at < dst.created_at {
        dst.created_at = src.created_at;
    }
    dst.tags = dedupe_strings([dst.tags.clone(), src.tags.clone()].concat());
    if dst.html_file.is_empty() {
        dst.html_file = src.html_file.clone();
    }
    if dst.markdown_file.is_none() {
        dst.markdown_file = src.markdown_file.clone();
    }
    if dst.archive_file.is_none() {
        dst.archive_file = src.archive_file.clone();
    }
    let mut seen: std::collections::HashSet<String> =
        dst.attachments.iter().map(|a| a.path.clone()).collect();
    for at in &src.attachments {
        if seen.insert(at.path.clone()) {
            dst.attachments.push(at.clone());
        }
    }
    for a in &src.applied_rules {
        if !dst.applied_rules.iter().any(|x| x.rule_id == a.rule_id) {
            dst.applied_rules.push(a.clone());
        }
    }
    if src.open_count > dst.open_count {
        dst.open_count = src.open_count;
        dst.last_opened_at = src.last_opened_at;
    }
    if src.last_checked_at > dst.last_checked_at {
        dst.last_checked_at = src.last_checked_at;
        dst.check_status = src.check_status.clone();
    }
}

fn move_to_unindexed(store: &Store, b: &Bookmark) {
    let kinds = [
        (!b.html_file.is_empty()).then(|| ("html", b.html_file.clone())),
        b.markdown_file.clone().map(|f| ("markdown", f)),
        b.archive_file.clone().map(|f| ("archive", f)),
    ];
    let dirs = [
        store.cfg.html_dir(),
        store.cfg.markdown_dir(),
        store.cfg.archive_dir(),
    ];
    for (opt, dir) in kinds.into_iter().zip(dirs) {
        let Some((kind, rel)) = opt else {
            continue;
        };
        let src = dir.join(&rel);
        if !src.exists() {
            continue;
        }
        let name = rel.rsplit('/').next().unwrap_or(&rel);
        let dst = store
            .cfg
            .profile_dir()
            .join("unindexed")
            .join(kind)
            .join(name);
        if std::fs::create_dir_all(dst.parent().unwrap()).is_ok() {
            let _ = std::fs::rename(&src, &dst);
        }
    }
    for at in &b.attachments {
        let src = store.cfg.attachment_dir().join(&at.path);
        if !src.exists() {
            continue;
        }
        let name = at.path.rsplit('/').next().unwrap_or(&at.path);
        let dst = store
            .cfg
            .profile_dir()
            .join("unindexed")
            .join("attachments")
            .join(name);
        if std::fs::create_dir_all(dst.parent().unwrap()).is_ok() {
            let _ = std::fs::rename(&src, &dst);
        }
    }
}

pub fn replay_entries(
    store: &mut Store,
    entries: &[crate::model::OpLogEntry],
) -> Result<MergeReport, CoreError> {
    let mut rep = MergeReport::default();
    let mut ordered: Vec<&crate::model::OpLogEntry> = entries.iter().collect();
    ordered.sort_by(|a, b| {
        a.ts.cmp(&b.ts)
            .then_with(|| a.device_id.cmp(&b.device_id))
            .then_with(|| a.seq.cmp(&b.seq))
    });
    for e in ordered {
        let key = entry_key(&e.device_id, &e.ts, &e.op, &e.uuid);
        if store.oplog_applied(&key)? {
            continue;
        }
        match e.op.as_str() {
            "upsert" => {
                let incoming: Bookmark = serde_json::from_value(e.payload.clone())
                    .map_err(|err| CoreError::Storage(err.to_string()))?;
                replay_upsert(store, &incoming, &mut rep)?;
            }
            "delete" => {
                replay_delete(store, &e.payload, &mut rep)?;
            }
            "rule_put" => {
                let rule: AutoRule = serde_json::from_value(e.payload.clone())
                    .map_err(|err| CoreError::Storage(err.to_string()))?;
                if store
                    .list_rules()?
                    .iter()
                    .any(|r| r.pattern.eq_ignore_ascii_case(&rule.pattern))
                {
                    rep.deduped += 1;
                } else if store.find_rule(&rule.id)?.is_some() {
                    store.update_rule(&rule)?;
                    rep.merged += 1;
                } else {
                    store.add_rule_with_id(&rule)?;
                    rep.rules += 1;
                }
            }
            "rule_del" => {
                let id = e.payload.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let pattern = e
                    .payload
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mut gone = false;
                if !id.is_empty() && store.delete_rule_quiet(id)? {
                    gone = true;
                }
                if !gone && !pattern.is_empty() {
                    if let Some(r) = store
                        .list_rules()?
                        .into_iter()
                        .find(|r| r.pattern.eq_ignore_ascii_case(pattern))
                    {
                        store.delete_rule_quiet(&r.id)?;
                        gone = true;
                    }
                }
                if gone {
                    rep.rules += 1;
                }
            }
            _ => {}
        }
        store.oplog_mark_applied(&key)?;
    }
    Ok(rep)
}

fn replay_upsert(
    store: &mut Store,
    incoming: &Bookmark,
    rep: &mut MergeReport,
) -> Result<(), CoreError> {
    let norm = normalize_for_dedupe(&incoming.url);
    if let Some(mut local) = store.get(&incoming.uuid)? {
        if normalize_for_dedupe(&local.url) == norm {
            merge_fields(&mut local, incoming);
            store.replace_bookmark_exact(&local)?;
            rep.merged += 1;
            return Ok(());
        }
        if let Some(mut other) = store.find_by_url(&incoming.url)? {
            if other.uuid != local.uuid {
                merge_fields(&mut other, incoming);
                store.replace_bookmark_exact(&other)?;
                rep.deduped += 1;
                return Ok(());
            }
        }
        let mut updated = incoming.clone();
        updated.created_at = updated.created_at.min(local.created_at);
        merge_fields(&mut updated, &local);
        store.replace_bookmark_exact(&updated)?;
        rep.merged += 1;
        return Ok(());
    }
    if let Some(mut other) = store.find_by_url(&incoming.url)? {
        merge_fields(&mut other, incoming);
        store.replace_bookmark_exact(&other)?;
        rep.deduped += 1;
        return Ok(());
    }
    let _ = norm;
    store.replace_bookmark_exact(incoming)?;
    rep.inserted += 1;
    Ok(())
}

fn replay_delete(
    store: &mut Store,
    payload: &serde_json::Value,
    rep: &mut MergeReport,
) -> Result<(), CoreError> {
    let uuid_str = payload.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
    let uuid: Uuid = match uuid_str.parse() {
        Ok(u) => u,
        Err(_) => return Ok(()),
    };
    let time_str = payload.get("time").and_then(|v| v.as_str()).unwrap_or("");
    let tomb_time: DateTime<Utc> = match time_str.parse() {
        Ok(t) => t,
        Err(_) => return Ok(()),
    };
    let Some(local) = store.get(&uuid)? else {
        return Ok(());
    };
    if local.updated_at > tomb_time {
        return Ok(());
    }
    move_to_unindexed(store, &local);
    if store.delete_bookmark(&uuid)? {
        rep.deleted += 1;
    }
    Ok(())
}

pub fn export_bundle(
    store: &Store,
    since_seq: Option<i64>,
) -> Result<Vec<crate::model::OpLogEntry>, CoreError> {
    let all = store.oplog_entries()?;
    Ok(all
        .into_iter()
        .filter(|e| since_seq.map(|s| e.seq > s).unwrap_or(true))
        .collect())
}

pub fn write_bundle(
    path: &std::path::Path,
    entries: &[crate::model::OpLogEntry],
) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let data =
        serde_json::to_string_pretty(entries).map_err(|e| CoreError::Storage(e.to_string()))?;
    std::fs::write(path, data).map_err(|e| CoreError::Storage(e.to_string()))
}

pub fn read_bundle(path: &std::path::Path) -> Result<Vec<crate::model::OpLogEntry>, CoreError> {
    let data = std::fs::read(path).map_err(|e| CoreError::Storage(e.to_string()))?;
    serde_json::from_slice(&data).map_err(|e| CoreError::Storage(e.to_string()))
}

pub fn prune_oplog(store: &mut Store, retention_days: i64) -> Result<usize, CoreError> {
    store.prune_oplog_older_than(retention_days)
}

pub fn git_snapshot(
    base_dir: &std::path::Path,
    message: &str,
    push: bool,
) -> Result<bool, CoreError> {
    let probe = std::process::Command::new("git")
        .args([
            "-C",
            &base_dir.to_string_lossy(),
            "rev-parse",
            "--is-inside-work-tree",
        ])
        .output()
        .map_err(|e| CoreError::Storage(format!("starting git: {e}")))?;
    if !probe.status.success() {
        return Ok(false);
    }
    let add = std::process::Command::new("git")
        .args(["-C", &base_dir.to_string_lossy(), "add", "-A"])
        .output()
        .map_err(|e| CoreError::Storage(format!("git add: {e}")))?;
    if !add.status.success() {
        return Err(CoreError::Storage("git add failed".to_string()));
    }
    let commit = std::process::Command::new("git")
        .args(["-C", &base_dir.to_string_lossy(), "commit", "-m", message])
        .output()
        .map_err(|e| CoreError::Storage(format!("git commit: {e}")))?;
    if !commit.status.success() {
        let err = String::from_utf8_lossy(&commit.stderr);
        if err.contains("nothing to commit") {
            return Ok(true);
        }
        return Err(CoreError::Storage(format!(
            "git commit failed: {}",
            err.trim()
        )));
    }
    if push {
        let push_out = std::process::Command::new("git")
            .args(["-C", &base_dir.to_string_lossy(), "push"])
            .output()
            .map_err(|e| CoreError::Storage(format!("git push: {e}")))?;
        if !push_out.status.success() {
            return Err(CoreError::Storage("git push failed".to_string()));
        }
    }
    Ok(true)
}
}
