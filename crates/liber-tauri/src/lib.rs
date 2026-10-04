use std::sync::{Arc, Mutex};

use liber_core::store::{Config, Store};
use serde::Serialize;

pub struct AppState {
    pub cfg: Config,
    pub write_mu: Arc<Mutex<()>>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            write_mu: Arc::new(Mutex::new(())),
        }
    }
}

pub fn open_store(state: &AppState) -> Result<Store, String> {
    Store::open(state.cfg.clone()).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct TauriBookmark {
    pub uuid: String,
    pub short_id: Option<i64>,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub folder: String,
    pub created_at: String,
    pub updated_at: String,
    pub has_markdown: bool,
    pub has_archive: bool,
    pub open_count: u64,
    pub attachments: Vec<String>,
    pub last_opened_at: Option<String>,
}

impl From<&liber_core::model::Bookmark> for TauriBookmark {
    fn from(b: &liber_core::model::Bookmark) -> Self {
        Self {
            uuid: b.uuid.to_string(),
            short_id: b.short_id,
            url: b.url.clone(),
            title: b.title.clone(),
            description: b.description.clone(),
            tags: b.tags.clone(),
            folder: b.folder.clone(),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
            has_markdown: b.markdown_file.is_some(),
            has_archive: b.archive_file.is_some(),
            open_count: b.open_count,
            attachments: b.attachments.iter().map(|a| a.name.clone()).collect(),
            last_opened_at: b.last_opened_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub bookmarks: Vec<TauriBookmark>,
}

#[derive(Debug, Serialize)]
pub struct AddResult {
    pub status: String,
    pub bookmark: TauriBookmark,
    pub warnings: Vec<String>,
}
