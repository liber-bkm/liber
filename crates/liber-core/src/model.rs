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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRule {
    pub id: String,
    pub pattern: String,
    pub action_tag: Option<String>,
    pub action_folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpLogEntry {
    pub seq: i64,
    pub uuid: Option<Uuid>,
    pub device_id: String,
    pub ts: DateTime<Utc>,
    pub op: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct NewBookmark {
    pub uuid: uuid::Uuid,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub folder: String,
    pub html_file: String,
    pub markdown_file: Option<String>,
    pub archive_file: Option<String>,
}
