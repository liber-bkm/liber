use liber_core::model::NewBookmark;
use liber_core::search::SortMode;
use liber_core::store::{BookmarkFilter, Config, Store};
use uuid::Uuid;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

const WORDS: &[&str] = &[
    "guide",
    "reference",
    "tutorial",
    "notes",
    "docs",
    "manual",
    "linux",
    "network",
    "server",
    "rust",
    "database",
    "search",
    "backup",
    "config",
    "recipe",
    "travel",
];
const TAGS: &[&str] = &["reading", "work", "study", "later", "code", "media"];
const FOLDERS: &[&str] = &["", "work", "work/urgent", "personal", "study", "archive"];

fn fixture(n: usize) -> Store {
    let mut rng = Lcg(42);
    let mut store = Store::open_in_memory(Config {
        base_dir: std::path::PathBuf::from("/tmp/liber-perf"),
        device_id: "perf".to_string(),
        ..Default::default()
    })
    .unwrap();
    for i in 0..n {
        let title = format!(
            "{} {} {i}",
            WORDS[rng.next(WORDS.len())],
            WORDS[rng.next(WORDS.len())]
        );
        store
            .add_bookmark(NewBookmark {
                uuid: Uuid::new_v4(),
                url: format!("https://site{}.example.com/page/{i}", i % 500),
                title,
                description: format!("notes on {}", WORDS[rng.next(WORDS.len())]),
                tags: vec![TAGS[rng.next(TAGS.len())].to_string()],
                folder: FOLDERS[rng.next(FOLDERS.len())].to_string(),
                html_file: format!("{i:06}-page.html"),
                markdown_file: None,
                archive_file: None,
                applied_rules: vec![],
            })
            .unwrap();
    }
    store
}

fn timed(name: &str, budget_ms: u128, f: impl FnOnce()) {
    let start = std::time::Instant::now();
    f();
    let ms = start.elapsed().as_millis();
    println!("{name}: {ms}ms (budget {budget_ms}ms)");
    assert!(ms <= budget_ms, "{name} took {ms}ms, budget {budget_ms}ms");
}

#[test]
fn perf_10k_budgets() {
    let store = fixture(10_000);
    timed("page query", 500, || {
        let filter = BookmarkFilter {
            query: Some("guide".to_string()),
            ..Default::default()
        };
        let (page, total) = store
            .query_bookmarks(&filter, SortMode::Newest, 50, 0)
            .unwrap();
        assert!(!page.is_empty());
        assert!(total > 0);
    });
    timed("tag counts", 500, || {
        assert!(!store.tag_counts_sql().unwrap().is_empty());
    });
    timed("folder counts", 500, || {
        assert!(!store.folder_counts_sql().unwrap().is_empty());
    });
    timed("full scan", 5000, || {
        assert_eq!(store.list().unwrap().len(), 10_000);
    });
}
