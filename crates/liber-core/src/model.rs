use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub uuid: Uuid,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub folder: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub html_file: String,
    pub markdown_file: Option<String>,
    pub archive_file: Option<String>,
    pub attachments: Vec<Attachment>,
    pub open_count: u64,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub check_status: Option<String>,
    pub applied_rules: Vec<String>,
}
