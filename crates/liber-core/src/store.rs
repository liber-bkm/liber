use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::dedupe::normalize_for_dedupe;
use crate::model::{Attachment, AutoRule, Bookmark, NewBookmark, OpLogEntry};
use crate::CoreError;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS bookmarks (
    uuid TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    url_norm TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    folder TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    html_file TEXT NOT NULL DEFAULT '',
    markdown_file TEXT,
    archive_file TEXT,
    open_count INTEGER NOT NULL DEFAULT 0,
    last_opened_at TEXT,
    last_checked_at TEXT,
    check_status TEXT,
    applied_rules TEXT NOT NULL DEFAULT '[]'
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_url_norm ON bookmarks(url_norm);
CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bookmark_uuid TEXT NOT NULL REFERENCES bookmarks(uuid) ON DELETE CASCADE,
    name TEXT NOT NULL,
    path TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    pattern TEXT NOT NULL UNIQUE,
    action_tag TEXT,
    action_folder TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS oplog (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT,
    device_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    op TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL DEFAULT ''
);
";

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub base_dir: PathBuf,
    pub device_id: String,
    pub active_profile: Option<String>,
    pub auth_token: String,
    pub archive_backend: String,
}

#[derive(Debug, Default)]
pub struct Store {
    pub cfg: Config,
}

impl Store {
    pub fn open(cfg: Config) -> Result<Self, crate::CoreError> {
        Ok(Self { cfg })
    }
}
