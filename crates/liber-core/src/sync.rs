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
    pub renumbered: usize,
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
        updated.short_id = local.short_id;
        merge_fields(&mut updated, &local);
        updated.short_id = local.short_id;
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
    let before = incoming.short_id;
    store.replace_bookmark_exact(incoming)?;
    let after = store.get(&incoming.uuid)?.and_then(|b| b.short_id);
    if before.is_some() && before != after {
        rep.renumbered += 1;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config;

    fn device_store(base: &std::path::Path, device: &str) -> Store {
        Store::open_in_memory(Config {
            base_dir: base.to_path_buf(),
            device_id: device.to_string(),
            ..Default::default()
        })
        .unwrap()
    }

    fn add(store: &mut Store, url: &str, title: &str) -> Bookmark {
        create_bookmark(
            store,
            url,
            CreateOptions {
                title: Some(title.to_string()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn converge_new_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        add(&mut a, "https://example.com/1", "One");
        let bundle = export_bundle(&a, None).unwrap();
        let rep = replay_entries(&mut b, &bundle).unwrap();
        assert_eq!(rep.inserted, 1);
        assert_eq!(b.list().unwrap().len(), 1);
        let rep = replay_entries(&mut b, &bundle).unwrap();
        assert_eq!(rep.inserted + rep.merged + rep.deduped, 0);
    }

    #[test]
    fn newer_wins_and_tags_union() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let ba = add(&mut a, "https://example.com/1", "Old");
        let bundle = export_bundle(&a, None).unwrap();
        replay_entries(&mut b, &bundle).unwrap();
        let mut ba2 = a.get(&ba.uuid).unwrap().unwrap();
        ba2.tags = vec!["t1".to_string()];
        a.update_bookmark(&ba2).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let mut bb = b.get(&ba.uuid).unwrap().unwrap();
        bb.title = "New".to_string();
        bb.tags = vec!["t2".to_string()];
        b.update_bookmark(&bb).unwrap();
        let ab = export_bundle(&a, None).unwrap();
        let rep = replay_entries(&mut b, &ab).unwrap();
        assert!(rep.merged > 0);
        let got = b.get(&ba.uuid).unwrap().unwrap();
        assert_eq!(got.title, "New");
        assert!(got.tags.contains(&"t1".to_string()));
        assert!(got.tags.contains(&"t2".to_string()));
    }

    #[test]
    fn tombstone_loses_to_newer_edit() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let ba = add(&mut a, "https://example.com/1", "One");
        replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        a.delete_bookmark(&ba.uuid).unwrap();
        let mut bb = b.get(&ba.uuid).unwrap().unwrap();
        bb.title = "Edited".to_string();
        b.update_bookmark(&bb).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let rep = replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        assert_eq!(rep.deleted, 0);
        assert!(b.get(&ba.uuid).unwrap().is_some());
    }

    #[test]
    fn tombstone_wins_when_newer() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let ba = add(&mut a, "https://example.com/1", "One");
        replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        a.delete_bookmark(&ba.uuid).unwrap();
        let rep = replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        assert_eq!(rep.deleted, 1);
        assert!(b.get(&ba.uuid).unwrap().is_none());
    }

    #[test]
    fn short_id_collision_renumbers_loser() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let ba = add(&mut a, "https://example.com/a", "A");
        let bb = add(&mut b, "https://example.com/b", "B");
        assert_eq!(ba.short_id, Some(1));
        assert_eq!(bb.short_id, Some(1));
        let rep = replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        assert_eq!(rep.inserted, 1);
        assert_eq!(rep.renumbered, 1);
        let got = b.get(&ba.uuid).unwrap().unwrap();
        assert_eq!(got.short_id, Some(2));
        assert_eq!(b.get(&bb.uuid).unwrap().unwrap().short_id, Some(1));
        assert!(b.resolve_spec(&["2".to_string()]).unwrap()[0].uuid == ba.uuid);
    }

    #[test]
    fn replay_preserves_local_short_id_on_update() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let ba = add(&mut a, "https://example.com/a", "A");
        replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        b.compact_short_ids().unwrap();
        let before = b.get(&ba.uuid).unwrap().unwrap().short_id;
        let mut remote = a.get(&ba.uuid).unwrap().unwrap();
        remote.title = "Remote edit".to_string();
        remote.updated_at = chrono::Utc::now() + chrono::Duration::seconds(60);
        remote.short_id = Some(99);
        let entry = crate::model::OpLogEntry {
            seq: 999,
            uuid: Some(remote.uuid),
            device_id: "a".to_string(),
            ts: chrono::Utc::now(),
            op: "upsert".to_string(),
            payload: serde_json::to_value(&remote).unwrap(),
        };
        replay_entries(&mut b, &[entry]).unwrap();
        let got = b.get(&ba.uuid).unwrap().unwrap();
        assert_eq!(got.short_id, before);
        assert_eq!(got.title, "Remote edit");
    }

    #[test]
    fn independent_compaction_converges_on_same_set() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        let x = add(&mut a, "https://example.com/x", "X");
        let y = add(&mut a, "https://example.com/y", "Y");
        let z = add(&mut a, "https://example.com/z", "Z");
        replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        a.delete_bookmark(&y.uuid).unwrap();
        replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        a.compact_short_ids().unwrap();
        b.compact_short_ids().unwrap();
        for uuid in [x.uuid, z.uuid] {
            assert_eq!(
                a.get(&uuid).unwrap().unwrap().short_id,
                b.get(&uuid).unwrap().unwrap().short_id
            );
        }
        let mut ids: Vec<i64> = a
            .list()
            .unwrap()
            .iter()
            .map(|b| b.short_id.unwrap())
            .collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn rules_union_by_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = device_store(dir.path(), "a");
        let mut b = device_store(dir.path(), "b");
        a.add_rule("host:example.com".to_string(), vec!["t".to_string()], None)
            .unwrap();
        b.add_rule("host:example.com".to_string(), vec!["t".to_string()], None)
            .unwrap();
        b.add_rule(
            "host:other.com".to_string(),
            vec![],
            Some("misc".to_string()),
        )
        .unwrap();
        let rep = replay_entries(&mut b, &export_bundle(&a, None).unwrap()).unwrap();
        assert_eq!(rep.deduped, 1);
        assert_eq!(b.list_rules().unwrap().len(), 2);
    }
}
