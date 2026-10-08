use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};
use uuid::Uuid;

use crate::model::Bookmark;
use crate::search::{bookmark_matches_query, SearchFields};
use crate::store::Store;
use crate::CoreError;

pub fn search_targets(store: &Store, query: &str) -> Result<Vec<Bookmark>, CoreError> {
    let fields = SearchFields::all();
    Ok(store
        .list()?
        .into_iter()
        .filter(|b| bookmark_matches_query(b, query, &fields))
        .collect())
}

pub fn match_text(b: &Bookmark) -> String {
    format!("{} {} {} {}", b.title, b.url, b.tags.join(" "), b.folder)
}

pub fn nucleo_filter(targets: Vec<Bookmark>, query: &str) -> Vec<Bookmark> {
    if query.trim().is_empty() {
        return targets;
    }
    let mut matcher: Nucleo<Bookmark> = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), None, 1);
    let injector = matcher.injector();
    for b in targets {
        injector.push(b, |item, cols| {
            cols[0] = match_text(item).into();
        });
    }
    matcher
        .pattern
        .reparse(0, query, CaseMatching::Ignore, Normalization::Smart, false);
    matcher.tick(100);
    let snap = matcher.snapshot();
    let mut out = Vec::new();
    for i in 0..snap.matched_item_count() {
        if let Some(item) = snap.get_matched_item(i) {
            out.push(item.data.clone());
        }
    }
    out
}

pub fn format_row(n: usize, b: &Bookmark) -> String {
    format!("[{}] {}\n    {}", n, b.title, b.url)
}

pub fn resolve_plain_pick(shown: &[Bookmark], line: &str) -> Option<Uuid> {
    let line = line.trim();
    if let Ok(n) = line.parse::<usize>() {
        if n >= 1 && n <= shown.len() {
            return Some(shown[n - 1].uuid);
        }
        return None;
    }
    let tokens = match crate::idspec::parse_id_spec(line) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let mut hits = Vec::new();
    for t in &tokens {
        let prefix = t.to_lowercase();
        for b in shown {
            if b.uuid.to_string().starts_with(&prefix) {
                hits.push(b.uuid);
            }
        }
    }
    if hits.len() == 1 {
        Some(hits[0])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::{create_bookmark, CreateOptions};
    use crate::store::Config as StoreConfig;

    fn seeded() -> Store {
        let mut store = Store::open_in_memory(StoreConfig {
            base_dir: std::path::PathBuf::from("/tmp/liber-test"),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap();
        for (url, title) in [
            ("https://example.com/rust-guide", "Rust guide"),
            ("https://example.com/rust-book", "Rust book"),
            ("https://example.com/pasta", "Pasta recipe"),
        ] {
            create_bookmark(
                &mut store,
                url,
                CreateOptions {
                    title: Some(title.to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        store
    }

    #[test]
    fn nucleo_fuzzy_matches() {
        let s = seeded();
        let all = s.list().unwrap();
        assert_eq!(nucleo_filter(all.clone(), "").len(), 3);
        let hits = nucleo_filter(all.clone(), "rust");
        assert_eq!(hits.len(), 2);
        let hits = nucleo_filter(all.clone(), "rguid");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Rust guide");
        assert!(nucleo_filter(all, "zzz-no-match").is_empty());
    }

    #[test]
    fn plain_pick_numbers_and_prefixes() {
        let s = seeded();
        let shown = search_targets(&s, "rust").unwrap();
        assert_eq!(shown.len(), 2);
        assert_eq!(resolve_plain_pick(&shown, "1"), Some(shown[0].uuid));
        assert_eq!(resolve_plain_pick(&shown, "0"), None);
        assert_eq!(resolve_plain_pick(&shown, "9"), None);
        let prefix = shown[1].uuid.to_string()[..8].to_string();
        assert_eq!(resolve_plain_pick(&shown, &prefix), Some(shown[1].uuid));
        assert_eq!(resolve_plain_pick(&shown, "xyz"), None);
    }
}
