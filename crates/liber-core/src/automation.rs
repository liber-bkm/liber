use crate::edit::{move_bookmark_files, rewrite_bookmark_files};
use crate::model::{AppliedRule, AutoRule, Bookmark};
use crate::slug::{dedupe_strings, sanitize_folder};
use crate::store::Store;
use crate::CoreError;

pub fn rule_matches(pattern: &str, url: &str, title: &str) -> bool {
    let lower = pattern.to_lowercase();
    let (needle, target) = match lower.strip_prefix("title:") {
        Some(rest) => (rest, title.to_lowercase()),
        None => match lower.strip_prefix("host:") {
            Some(rest) => (rest, host_of(url)),
            None => (lower.as_str(), url.to_lowercase()),
        },
    };
    !needle.is_empty() && target.contains(needle)
}

pub fn host_of(url: &str) -> String {
    let mut rest = url;
    if let Some(i) = rest.find("://") {
        rest = &rest[i + 3..];
    }
    if let Some(i) = rest.find(['/', '?', '#']) {
        rest = &rest[..i];
    }
    if let Some(i) = rest.rfind(':') {
        rest = &rest[..i];
    }
    rest.to_lowercase()
}

pub fn describe_rule(rule: &AutoRule) -> String {
    let lower = rule.pattern.to_lowercase();
    let kind = if lower.starts_with("title:") {
        "title"
    } else if lower.starts_with("host:") {
        "host"
    } else {
        "url"
    };
    let mut out = format!("[{}] {kind} contains {:?}", rule.id, rule.pattern);
    if let Some(folder) = &rule.action_folder {
        out.push_str(&format!(" -> folder {folder:?}"));
    }
    if !rule.action_tags.is_empty() {
        out.push_str(&format!(" -> tag(s) {}", rule.action_tags.join(", ")));
    }
    out
}

pub fn resolve_for_new(
    rules: &[AutoRule],
    url: &str,
    title: &str,
    folder: String,
    tags: Vec<String>,
) -> (String, Vec<String>, Vec<AppliedRule>) {
    let mut folder = folder;
    let mut tags = tags;
    let mut applied = Vec::new();
    for r in rules {
        if !rule_matches(&r.pattern, url, title) {
            continue;
        }
        let mut used = false;
        let mut set_folder = None;
        if let Some(f) = &r.action_folder {
            if folder.is_empty() {
                folder = f.clone();
                set_folder = Some(f.clone());
            }
            used = true;
        }
        if !r.action_tags.is_empty() {
            tags = dedupe_strings([tags, r.action_tags.clone()].concat());
            used = true;
        }
        if used {
            applied.push(AppliedRule {
                rule_id: r.id.clone(),
                folder: set_folder,
            });
        }
    }
    (folder, tags, applied)
}

fn ledger_has(applied: &[AppliedRule], rule_id: &str) -> bool {
    applied.iter().any(|a| a.rule_id == rule_id)
}

fn apply_one(
    store: &mut Store,
    b: &mut Bookmark,
    rule: &AutoRule,
    reapply: bool,
) -> Result<bool, CoreError> {
    let had_prev = ledger_has(&b.applied_rules, &rule.id);
    if !reapply && had_prev {
        return Ok(false);
    }
    if !rule_matches(&rule.pattern, &b.url, &b.title) {
        if reapply && had_prev {
            b.applied_rules.retain(|a| a.rule_id != rule.id);
            store.update_bookmark(b)?;
            return Ok(true);
        }
        return Ok(false);
    }
    let mut changed = false;
    let mut folder_changed = false;
    if let Some(f) = &rule.action_folder {
        let safe = if reapply {
            let prev_folder = b
                .applied_rules
                .iter()
                .find(|a| a.rule_id == rule.id)
                .and_then(|a| a.folder.clone());
            b.folder.is_empty() || (had_prev && prev_folder.as_deref() == Some(b.folder.as_str()))
        } else {
            b.folder.is_empty()
        };
        if safe && b.folder != *f {
            b.folder = f.clone();
            folder_changed = true;
            changed = true;
        }
    }
    if !rule.action_tags.is_empty() {
        let before = b.tags.len();
        b.tags = dedupe_strings([b.tags.clone(), rule.action_tags.clone()].concat());
        if b.tags.len() != before {
            changed = true;
        }
    }
    let set_folder = match &rule.action_folder {
        Some(f) if b.folder == *f => Some(f.clone()),
        _ => None,
    };
    b.applied_rules.retain(|a| a.rule_id != rule.id);
    b.applied_rules.push(AppliedRule {
        rule_id: rule.id.clone(),
        folder: set_folder,
    });
    if folder_changed {
        move_bookmark_files(store, b)?;
    }
    if changed {
        rewrite_bookmark_files(store, b, false)?;
    }
    store.update_bookmark(b)?;
    Ok(true)
}

pub fn apply_to_existing(
    store: &mut Store,
    uuid: &uuid::Uuid,
    rules: &[AutoRule],
) -> Result<bool, CoreError> {
    let Some(mut b) = store.get(uuid)? else {
        return Err(CoreError::NotFound(uuid.to_string()));
    };
    let mut changed = false;
    for r in rules {
        if apply_one(store, &mut b, r, false)? {
            changed = true;
        }
    }
    Ok(changed)
}

pub fn create_rule(
    store: &mut Store,
    pattern: String,
    tags: Vec<String>,
    folder: Option<String>,
) -> Result<(AutoRule, Vec<Bookmark>), CoreError> {
    let pattern = pattern.trim().to_string();
    if pattern.is_empty() {
        return Err(CoreError::Invalid("--match is required".to_string()));
    }
    let folder = folder
        .map(|f| sanitize_folder(&f))
        .filter(|f| !f.is_empty());
    let tags = dedupe_strings(tags);
    if folder.is_none() && tags.is_empty() {
        return Err(CoreError::Invalid(
            "a rule needs a folder and/or tags".to_string(),
        ));
    }
    let rule = store.add_rule(pattern, tags, folder)?;
    let mut changed = Vec::new();
    for b in store.list()? {
        if apply_to_existing(store, &b.uuid, std::slice::from_ref(&rule))? {
            changed.push(store.get(&b.uuid)?.unwrap());
        }
    }
    Ok((rule, changed))
}

pub fn edit_rule(
    store: &mut Store,
    id: &str,
    pattern: Option<String>,
    tags: Option<Vec<String>>,
    folder: Option<String>,
    reapply: bool,
) -> Result<(AutoRule, Vec<Bookmark>), CoreError> {
    let Some(mut rule) = store.find_rule(id)? else {
        return Err(CoreError::NotFound(id.to_string()));
    };
    if let Some(p) = pattern {
        if !p.trim().is_empty() {
            rule.pattern = p.trim().to_string();
        }
    }
    if let Some(f) = folder {
        rule.action_folder = {
            let f = sanitize_folder(&f);
            if f.is_empty() {
                None
            } else {
                Some(f)
            }
        };
    }
    if let Some(t) = tags {
        rule.action_tags = dedupe_strings(t);
    }
    store.update_rule(&rule)?;
    let mut changed = Vec::new();
    if reapply {
        for b in store.list()? {
            let Some(mut current) = store.get(&b.uuid)? else {
                continue;
            };
            if apply_one(store, &mut current, &rule, true)? {
                changed.push(store.get(&b.uuid)?.unwrap());
            }
        }
    }
    Ok((rule, changed))
}

pub fn apply_rules(store: &mut Store, id: Option<&str>) -> Result<Vec<Bookmark>, CoreError> {
    let rules = match id {
        Some(want) => {
            let Some(rule) = store.find_rule(want)? else {
                return Err(CoreError::NotFound(want.to_string()));
            };
            vec![rule]
        }
        None => store.list_rules()?,
    };
    if rules.is_empty() {
        return Err(CoreError::Invalid("no automations to apply".to_string()));
    }
    let mut changed = Vec::new();
    for b in store.list()? {
        if apply_to_existing(store, &b.uuid, &rules)? {
            changed.push(store.get(&b.uuid)?.unwrap());
        }
    }
    Ok(changed)
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub host: String,
    pub folder: String,
    pub count: usize,
}

pub fn suggest_rules(store: &Store, min: usize) -> Result<Vec<Suggestion>, CoreError> {
    let mut covered = std::collections::HashSet::new();
    for r in store.list_rules()? {
        if let Some(rest) = r.pattern.to_lowercase().strip_prefix("host:") {
            covered.insert(rest.to_string());
        }
    }
    let mut counts: std::collections::HashMap<String, std::collections::HashMap<String, usize>> =
        std::collections::HashMap::new();
    for b in store.list()? {
        let h = host_of(&b.url);
        if h.is_empty() || b.folder.is_empty() || covered.contains(&h) {
            continue;
        }
        *counts
            .entry(h)
            .or_default()
            .entry(b.folder.clone())
            .or_insert(0) += 1;
    }
    let mut out = Vec::new();
    for (host, folders) in counts {
        let mut names: Vec<&String> = folders.keys().collect();
        names.sort();
        let (mut best, mut best_n) = (String::new(), 0);
        for f in names {
            if folders[f] > best_n {
                best = f.clone();
                best_n = folders[f];
            }
        }
        if best_n >= min {
            out.push(Suggestion {
                host,
                folder: best,
                count: best_n,
            });
        }
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.count));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config;

    fn mem_store(base: &std::path::Path) -> Store {
        Store::open_in_memory(Config {
            base_dir: base.to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap()
    }

    fn add(store: &mut Store, url: &str, folder: &str) -> Bookmark {
        create_bookmark(
            store,
            url,
            CreateOptions {
                title: Some(url.to_string()),
                folder: folder.to_string(),
                markdown: true,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn match_kinds() {
        assert!(rule_matches(
            "host:example.com",
            "https://example.com/x",
            "t"
        ));
        assert!(rule_matches(
            "title:rust",
            "https://example.com/",
            "Rust guide"
        ));
        assert!(rule_matches("exam", "https://example.com/", "t"));
        assert!(!rule_matches("", "https://example.com/", "t"));
        assert!(!rule_matches("host:other.com", "https://example.com/", "t"));
        assert_eq!(host_of("https://example.com:8080/a?b=1"), "example.com");
    }

    #[test]
    fn create_backfills_and_ledgers() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        add(&mut s, "https://example.com/a", "");
        add(&mut s, "https://other.com/b", "");
        let (rule, changed) = create_rule(
            &mut s,
            "host:example.com".to_string(),
            vec!["news".to_string()],
            Some("tech".to_string()),
        )
        .unwrap();
        assert_eq!(changed.len(), 1);
        let b = s.get(&changed[0].uuid).unwrap().unwrap();
        assert_eq!(b.folder, "tech");
        assert_eq!(b.tags, vec!["news".to_string()]);
        assert_eq!(b.applied_rules.len(), 1);
        assert_eq!(b.applied_rules[0].rule_id, rule.id);
        assert_eq!(b.applied_rules[0].folder.as_deref(), Some("tech"));
        assert!(s.cfg.html_dir().join(&b.html_file).exists());
    }

    #[test]
    fn manual_moves_stick() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = add(&mut s, "https://example.com/a", "");
        let (rule, _) = create_rule(
            &mut s,
            "host:example.com".to_string(),
            vec![],
            Some("tech".to_string()),
        )
        .unwrap();
        let b = s.get(&b.uuid).unwrap().unwrap();
        assert_eq!(b.folder, "tech");
        crate::edit::edit_bookmark(
            &mut s,
            &b.uuid,
            crate::edit::EditOptions {
                folder: Some("mine".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let changed = apply_rules(&mut s, Some(&rule.id)).unwrap();
        assert!(changed.is_empty());
        assert_eq!(s.get(&b.uuid).unwrap().unwrap().folder, "mine");
    }

    #[test]
    fn reapply_syncs_and_unmatch_clears() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        let b = add(&mut s, "https://example.com/a", "");
        let (rule, _) = create_rule(
            &mut s,
            "host:example.com".to_string(),
            vec!["t".to_string()],
            Some("tech".to_string()),
        )
        .unwrap();
        let (_, changed) = edit_rule(
            &mut s,
            &rule.id,
            None,
            Some(vec!["t2".to_string()]),
            None,
            true,
        )
        .unwrap();
        assert_eq!(changed.len(), 1);
        let b = s.get(&b.uuid).unwrap().unwrap();
        assert!(b.tags.contains(&"t2".to_string()));
        let (_, _) = edit_rule(
            &mut s,
            &rule.id,
            Some("host:gone.com".to_string()),
            None,
            None,
            true,
        )
        .unwrap();
        let b = s.get(&b.uuid).unwrap().unwrap();
        assert!(b.applied_rules.is_empty());
    }

    #[test]
    fn learn_suggests_clusters() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        for i in 0..3 {
            add(&mut s, &format!("https://example.com/{i}"), "tech");
        }
        add(&mut s, "https://other.com/1", "misc");
        let out = suggest_rules(&mut s, 2).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].host, "example.com");
        assert_eq!(out[0].folder, "tech");
        assert!(suggest_rules(&mut s, 9).unwrap().is_empty());
    }

    #[test]
    fn validation() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = mem_store(dir.path());
        assert!(create_rule(&mut s, "  ".to_string(), vec!["t".to_string()], None).is_err());
        assert!(create_rule(&mut s, "x".to_string(), vec![], None).is_err());
        assert!(apply_rules(&mut s, None).is_err());
        assert!(edit_rule(&mut s, "missing", None, None, None, false).is_err());
    }
}
