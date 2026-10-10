use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
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
    applied_rules TEXT NOT NULL DEFAULT '[]',
    short_id INTEGER UNIQUE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_url_norm ON bookmarks(url_norm);
CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder);
CREATE INDEX IF NOT EXISTS idx_bookmarks_created ON bookmarks(created_at);
CREATE INDEX IF NOT EXISTS idx_bookmarks_opened ON bookmarks(last_opened_at);
CREATE TABLE IF NOT EXISTS bookmark_tags (
    bookmark_uuid TEXT NOT NULL REFERENCES bookmarks(uuid) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (bookmark_uuid, tag)
);
CREATE INDEX IF NOT EXISTS idx_bookmark_tags_tag ON bookmark_tags(tag);
CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bookmark_uuid TEXT NOT NULL REFERENCES bookmarks(uuid) ON DELETE CASCADE,
    name TEXT NOT NULL,
    path TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    pattern TEXT NOT NULL UNIQUE,
    action_tags TEXT NOT NULL DEFAULT '[]',
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
CREATE TABLE IF NOT EXISTS applied_oplog (
    device TEXT NOT NULL,
    ts TEXT NOT NULL,
    op TEXT NOT NULL,
    uuid TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (device, ts, op, uuid)
);
";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub base_dir: PathBuf,
    #[serde(default)]
    pub html_dir: Option<PathBuf>,
    #[serde(default)]
    pub markdown_dir: Option<PathBuf>,
    #[serde(default)]
    pub archive_dir: Option<PathBuf>,
    #[serde(default)]
    pub attachment_dir: Option<PathBuf>,
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub auth_token: String,
    #[serde(default)]
    pub archive_backend: String,
    #[serde(default)]
    pub browser_cmd: String,
    #[serde(default)]
    pub editor_cmd: String,
    #[serde(default)]
    pub browser_path: String,
    #[serde(default)]
    pub singlefile_cmd: String,
    #[serde(default)]
    pub singlefile_browser_path: String,
    #[serde(default)]
    pub monolith_cmd: String,
}

impl Config {
    pub fn profile_dir(&self) -> PathBuf {
        match &self.active_profile {
            Some(p) => self.base_dir.join(p),
            None => self.base_dir.clone(),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.profile_dir().join(".liber").join("store.db")
    }

    pub fn effective_device_id(&self) -> String {
        if self.device_id.is_empty() {
            "local".to_string()
        } else {
            self.device_id.clone()
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct MaintenanceStatus {
    pub bookmarks: usize,
    pub oplog_entries: usize,
    pub quarantine: usize,
    pub short_id_gaps: usize,
}

pub struct Store {
    conn: Connection,
    pub cfg: Config,
    auto_index: bool,
}

fn row_bookmark(row: &Row) -> rusqlite::Result<Bookmark> {
    let tags_raw: String = row.get("tags")?;
    let rules_raw: String = row.get("applied_rules")?;
    let uuid_raw: String = row.get("uuid")?;
    Ok(Bookmark {
        uuid: uuid_raw.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        url: row.get("url")?,
        title: row.get("title")?,
        description: row.get("description")?,
        tags: serde_json::from_str(&tags_raw).unwrap_or_default(),
        folder: row.get("folder")?,
        created_at: row.get::<_, String>("created_at")?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        updated_at: row.get::<_, String>("updated_at")?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        html_file: row.get("html_file")?,
        markdown_file: row.get("markdown_file")?,
        archive_file: row.get("archive_file")?,
        attachments: vec![],
        open_count: row.get::<_, i64>("open_count")? as u64,
        last_opened_at: row
            .get::<_, Option<String>>("last_opened_at")?
            .map(|s| s.parse())
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        last_checked_at: row
            .get::<_, Option<String>>("last_checked_at")?
            .map(|s| s.parse())
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        check_status: row.get("check_status")?,
        applied_rules: serde_json::from_str(&rules_raw).unwrap_or_default(),
        short_id: row.get("short_id")?,
    })
}

fn next_short_id(conn: &Connection) -> Result<i64, CoreError> {
    let cur: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'next_short_id'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    match cur {
        Some(v) => {
            let n: i64 = v
                .parse()
                .map_err(|e| CoreError::Storage(format!("bad short id counter: {e}")))?;
            conn.execute(
                "UPDATE meta SET value = ?1 WHERE key = 'next_short_id'",
                params![(n + 1).to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
            Ok(n)
        }
        None => {
            let max: Option<i64> = conn
                .query_row("SELECT MAX(short_id) FROM bookmarks", [], |row| row.get(0))
                .map_err(|e| CoreError::Storage(e.to_string()))?;
            let n = max.unwrap_or(0) + 1;
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('next_short_id', ?1)",
                params![(n + 1).to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
            Ok(n)
        }
    }
}

fn bump_counter_above(conn: &Connection, n: i64) -> Result<(), CoreError> {
    let cur: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'next_short_id'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    let cur: i64 = cur.and_then(|v| v.parse().ok()).unwrap_or(1);
    if cur <= n {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('next_short_id', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![(n + 1).to_string()],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    Ok(())
}

fn sync_tags(conn: &Connection, uuid: &Uuid, tags: &[String]) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM bookmark_tags WHERE bookmark_uuid = ?1",
        params![uuid.to_string()],
    )
    .map_err(|e| CoreError::Storage(e.to_string()))?;
    for t in tags {
        conn.execute(
            "INSERT OR IGNORE INTO bookmark_tags (bookmark_uuid, tag) VALUES (?1, ?2)",
            params![uuid.to_string(), t],
        )
        .map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    Ok(())
}

const BOOKMARK_COLS: &str = "uuid, url, title, description, tags, folder,
    created_at, updated_at, html_file, markdown_file, archive_file,
    open_count, last_opened_at, last_checked_at, check_status, applied_rules,
    short_id";

impl Store {
    fn connect(path: Option<&std::path::Path>) -> Result<Connection, CoreError> {
        let conn = match path {
            Some(p) => Connection::open(p),
            None => Connection::open_in_memory(),
        }
        .map_err(|e| CoreError::Storage(e.to_string()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(conn)
    }

    fn migrate(conn: &Connection) -> Result<(), CoreError> {
        conn.execute_batch(SCHEMA)
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn open(cfg: Config) -> Result<Self, CoreError> {
        let db = cfg.db_path();
        if let Some(parent) = db.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        let conn = Self::connect(Some(&db))?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn,
            cfg,
            auto_index: true,
        })
    }

    pub fn open_in_memory(cfg: Config) -> Result<Self, CoreError> {
        let conn = Self::connect(None)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn,
            cfg,
            auto_index: true,
        })
    }
    pub fn set_auto_index(&mut self, on: bool) {
        self.auto_index = on;
    }

    pub fn auto_index(&self) -> bool {
        self.auto_index
    }
    fn attachments_of(&self, uuid: &Uuid) -> Result<Vec<Attachment>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, path FROM attachments WHERE bookmark_uuid = ?1 ORDER BY id")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(params![uuid.to_string()], |row| {
                Ok(Attachment {
                    name: row.get(0)?,
                    path: row.get(1)?,
                })
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    fn with_attachments(&self, mut b: Bookmark) -> Result<Bookmark, CoreError> {
        b.attachments = self.attachments_of(&b.uuid)?;
        Ok(b)
    }

    pub fn add_bookmark(&mut self, new: NewBookmark) -> Result<Bookmark, CoreError> {
        let norm = normalize_for_dedupe(&new.url);
        if let Some(existing) = self.find_by_url(&new.url)? {
            return Err(CoreError::Duplicate(existing.uuid.to_string()));
        }
        let now = Utc::now();
        let b = Bookmark {
            uuid: new.uuid,
            url: new.url,
            title: new.title,
            description: new.description,
            tags: new.tags,
            folder: new.folder,
            created_at: now,
            updated_at: now,
            html_file: new.html_file,
            markdown_file: new.markdown_file,
            archive_file: new.archive_file,
            attachments: vec![],
            open_count: 0,
            last_opened_at: None,
            last_checked_at: None,
            check_status: None,
            applied_rules: new.applied_rules,
            short_id: Some(next_short_id(&self.conn)?),
        };
        self.conn
            .execute(
                "INSERT INTO bookmarks (uuid, url, url_norm, title, description, tags,
                 folder, created_at, updated_at, html_file, markdown_file, archive_file,
                 open_count, applied_rules, short_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14)",
                params![
                    b.uuid.to_string(),
                    b.url,
                    norm,
                    b.title,
                    b.description,
                    serde_json::to_string(&b.tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.folder,
                    b.created_at.to_rfc3339(),
                    b.updated_at.to_rfc3339(),
                    b.html_file,
                    b.markdown_file,
                    b.archive_file,
                    serde_json::to_string(&b.applied_rules)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.short_id,
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        sync_tags(&self.conn, &b.uuid, &b.tags)?;
        let payload = serde_json::to_value(&b).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(Some(b.uuid), "upsert", payload)?;
        Ok(b)
    }

    pub fn get(&self, uuid: &Uuid) -> Result<Option<Bookmark>, CoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {BOOKMARK_COLS} FROM bookmarks WHERE uuid = ?1"
            ))
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let found: Option<Bookmark> = stmt
            .query_row(params![uuid.to_string()], row_bookmark)
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        found.map(|b| self.with_attachments(b)).transpose()
    }

    pub fn find_by_url(&self, url: &str) -> Result<Option<Bookmark>, CoreError> {
        let norm = normalize_for_dedupe(url);
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {BOOKMARK_COLS} FROM bookmarks WHERE url_norm = ?1"
            ))
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let found: Option<Bookmark> = stmt
            .query_row(params![norm], row_bookmark)
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        found.map(|b| self.with_attachments(b)).transpose()
    }

    pub fn update_bookmark(&mut self, b: &Bookmark) -> Result<(), CoreError> {
        let mut stamped = b.clone();
        stamped.updated_at = Utc::now();
        let b = &stamped;
        let n = self
            .conn
            .execute(
                "UPDATE bookmarks SET url = ?1, url_norm = ?2, title = ?3,
                 description = ?4, tags = ?5, folder = ?6, updated_at = ?7,
                 html_file = ?8, markdown_file = ?9, archive_file = ?10,
                 last_checked_at = ?11, check_status = ?12, applied_rules = ?13
                 WHERE uuid = ?14",
                params![
                    b.url,
                    normalize_for_dedupe(&b.url),
                    b.title,
                    b.description,
                    serde_json::to_string(&b.tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.folder,
                    b.updated_at.to_rfc3339(),
                    b.html_file,
                    b.markdown_file,
                    b.archive_file,
                    b.last_checked_at.map(|t| t.to_rfc3339()),
                    b.check_status,
                    serde_json::to_string(&b.applied_rules)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.uuid.to_string(),
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            return Err(CoreError::NotFound(b.uuid.to_string()));
        }
        sync_tags(&self.conn, &b.uuid, &b.tags)?;
        let payload = serde_json::to_value(b).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(Some(b.uuid), "upsert", payload)
    }

    pub fn replace_bookmark_exact(&mut self, b: &Bookmark) -> Result<(), CoreError> {
        let mut owned = b.clone();
        let local: Option<Option<i64>> = self
            .conn
            .query_row(
                "SELECT short_id FROM bookmarks WHERE uuid = ?1",
                params![owned.uuid.to_string()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        match local {
            Some(Some(n)) => {
                owned.short_id = Some(n);
            }
            Some(None) => {
                owned.short_id = Some(next_short_id(&self.conn)?);
            }
            None => {
                let taken: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT uuid FROM bookmarks WHERE short_id = ?1",
                        params![owned.short_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| CoreError::Storage(e.to_string()))?;
                if let Some(other) = taken {
                    if other != owned.uuid.to_string() {
                        owned.short_id = Some(next_short_id(&self.conn)?);
                    }
                }
                if owned.short_id.is_none() {
                    owned.short_id = Some(next_short_id(&self.conn)?);
                } else if let Some(n) = owned.short_id {
                    bump_counter_above(&self.conn, n)?;
                }
            }
        }
        let b = &owned;
        self.conn
            .execute(
                "INSERT INTO bookmarks (uuid, url, url_norm, title, description, tags,
                 folder, created_at, updated_at, html_file, markdown_file, archive_file,
                 open_count, last_opened_at, last_checked_at, check_status, applied_rules,
                 short_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                 ON CONFLICT(uuid) DO UPDATE SET url = excluded.url, url_norm = excluded.url_norm,
                 title = excluded.title, description = excluded.description, tags = excluded.tags,
                 folder = excluded.folder, created_at = excluded.created_at, updated_at = excluded.updated_at,
                 html_file = excluded.html_file, markdown_file = excluded.markdown_file,
                 archive_file = excluded.archive_file, open_count = excluded.open_count,
                 last_opened_at = excluded.last_opened_at, last_checked_at = excluded.last_checked_at,
                 check_status = excluded.check_status, applied_rules = excluded.applied_rules,
                 short_id = excluded.short_id",
                params![
                    b.uuid.to_string(),
                    b.url,
                    normalize_for_dedupe(&b.url),
                    b.title,
                    b.description,
                    serde_json::to_string(&b.tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.folder,
                    b.created_at.to_rfc3339(),
                    b.updated_at.to_rfc3339(),
                    b.html_file,
                    b.markdown_file,
                    b.archive_file,
                    b.open_count as i64,
                    b.last_opened_at.map(|t| t.to_rfc3339()),
                    b.last_checked_at.map(|t| t.to_rfc3339()),
                    b.check_status,
                    serde_json::to_string(&b.applied_rules)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    b.short_id,
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        self.conn
            .execute(
                "DELETE FROM attachments WHERE bookmark_uuid = ?1",
                params![b.uuid.to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        for at in &b.attachments {
            self.conn
                .execute(
                    "INSERT INTO attachments (bookmark_uuid, name, path) VALUES (?1, ?2, ?3)",
                    params![b.uuid.to_string(), at.name, at.path],
                )
                .map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        sync_tags(&self.conn, &b.uuid, &b.tags)?;
        let payload = serde_json::to_value(b).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(Some(b.uuid), "upsert", payload)
    }

    pub fn backfill_short_ids(&mut self) -> Result<usize, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT uuid FROM bookmarks WHERE short_id IS NULL ORDER BY created_at, uuid")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let uuids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| CoreError::Storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        drop(stmt);
        let first = next_short_id(&self.conn)?;
        for (i, uuid) in uuids.iter().enumerate() {
            self.conn
                .execute(
                    "UPDATE bookmarks SET short_id = ?1 WHERE uuid = ?2 AND short_id IS NULL",
                    params![first + i as i64, uuid],
                )
                .map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        bump_counter_above(&self.conn, first + uuids.len() as i64)?;
        Ok(uuids.len())
    }

    pub fn compact_short_ids(&mut self) -> Result<Vec<(Uuid, Option<i64>, i64)>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT uuid, short_id FROM bookmarks ORDER BY created_at ASC, uuid ASC")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows: Vec<(String, Option<i64>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| CoreError::Storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        drop(stmt);
        let total = rows.len() as i64;
        let mut changed: Vec<(String, Option<i64>, i64)> = Vec::new();
        for (i, (uuid, old)) in rows.iter().enumerate() {
            let want = i as i64 + 1;
            if *old != Some(want) {
                changed.push((uuid.clone(), *old, want));
            }
        }
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES ('next_short_id', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![(total + 1).to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if changed.is_empty() {
            return Ok(Vec::new());
        }
        for (uuid, _, _) in &changed {
            self.conn
                .execute(
                    "UPDATE bookmarks SET short_id = NULL WHERE uuid = ?1",
                    params![uuid],
                )
                .map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        for (uuid, _, want) in &changed {
            self.conn
                .execute(
                    "UPDATE bookmarks SET short_id = ?1 WHERE uuid = ?2",
                    params![want, uuid],
                )
                .map_err(|e| CoreError::Storage(e.to_string()))?;
        }
        changed
            .into_iter()
            .map(|(uuid, old, want)| {
                uuid.parse::<Uuid>()
                    .map(|u| (u, old, want))
                    .map_err(|e| CoreError::Storage(e.to_string()))
            })
            .collect()
    }

    pub fn maintenance_status(&self) -> Result<MaintenanceStatus, CoreError> {
        let count = |sql: &str| {
            self.conn
                .query_row(sql, [], |row| row.get::<_, i64>(0))
                .map_err(|e| CoreError::Storage(e.to_string()))
        };
        let bookmarks = count("SELECT COUNT(*) FROM bookmarks")?;
        let oplog_entries = count("SELECT COUNT(*) FROM oplog")?;
        let quarantine = count("SELECT COUNT(*) FROM bookmarks WHERE folder = 'quarantine'")?;
        let assigned = count("SELECT COUNT(short_id) FROM bookmarks")?;
        let max_id: Option<i64> = self
            .conn
            .query_row("SELECT MAX(short_id) FROM bookmarks", [], |row| row.get(0))
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let short_id_gaps = (bookmarks - assigned) + max_id.unwrap_or(0).saturating_sub(assigned);
        let short_id_gaps = short_id_gaps as usize;
        Ok(MaintenanceStatus {
            bookmarks: bookmarks as usize,
            oplog_entries: oplog_entries as usize,
            quarantine: quarantine as usize,
            short_id_gaps,
        })
    }

    pub fn delete_bookmark(&mut self, uuid: &Uuid) -> Result<bool, CoreError> {
        let existing = self.get(uuid)?;
        let Some(b) = existing else {
            return Ok(false);
        };
        self.conn
            .execute(
                "DELETE FROM bookmarks WHERE uuid = ?1",
                params![uuid.to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let payload = serde_json::json!({
            "uuid": uuid.to_string(),
            "url": b.url,
            "time": Utc::now().to_rfc3339(),
        });
        self.append_oplog(Some(*uuid), "delete", payload)?;
        Ok(true)
    }

    pub fn list(&self) -> Result<Vec<Bookmark>, CoreError> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {BOOKMARK_COLS} FROM bookmarks ORDER BY created_at, uuid"
            ))
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], row_bookmark)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for b in rows {
            let b = b.map_err(|e| CoreError::Storage(e.to_string()))?;
            out.push(b);
        }
        self.with_attachments_batch(&mut out)
    }

    fn with_attachments_batch(
        &self,
        items: &mut Vec<Bookmark>,
    ) -> Result<Vec<Bookmark>, CoreError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = items.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT bookmark_uuid, name, path FROM attachments WHERE bookmark_uuid IN ({}) ORDER BY id",
            placeholders.join(",")
        );
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let uuids: Vec<String> = items.iter().map(|b| b.uuid.to_string()).collect();
        let refs: Vec<&dyn rusqlite::ToSql> =
            uuids.iter().map(|u| u as &dyn rusqlite::ToSql).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Attachment {
                        name: row.get(1)?,
                        path: row.get(2)?,
                    },
                ))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut by_uuid: std::collections::HashMap<String, Vec<Attachment>> =
            std::collections::HashMap::new();
        for r in rows {
            let (uuid, at) = r.map_err(|e| CoreError::Storage(e.to_string()))?;
            by_uuid.entry(uuid).or_default().push(at);
        }
        let mut out = Vec::with_capacity(items.len());
        for mut b in items.drain(..) {
            b.attachments = by_uuid.remove(&b.uuid.to_string()).unwrap_or_default();
            out.push(b);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BookmarkFilter {
    pub folder: Option<String>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub scope: crate::search::SearchFields,
    pub opened_only: bool,
}

impl Store {
    fn like_pattern(q: &str) -> String {
        let escaped: String = q
            .chars()
            .flat_map(|c| match c {
                '%' | '_' | '\\' => vec!['\\', c],
                c => vec![c],
            })
            .collect();
        format!("%{escaped}%")
    }

    pub fn query_bookmarks(
        &self,
        filter: &BookmarkFilter,
        sort: crate::search::SortMode,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Bookmark>, usize), CoreError> {
        use crate::search::SortMode;
        let mut joins = String::new();
        let mut conds: Vec<String> = Vec::new();
        let mut args: Vec<String> = Vec::new();
        if filter.tag.is_some() {
            joins.push_str(" JOIN bookmark_tags t ON t.bookmark_uuid = b.uuid");
        }
        if let Some(folder) = &filter.folder {
            conds.push("b.folder = ?".to_string());
            args.push(folder.clone());
        }
        if let Some(tag) = &filter.tag {
            conds.push("t.tag = ? COLLATE NOCASE".to_string());
            args.push(tag.clone());
        }
        if let Some(q) = &filter.query {
            for term in crate::search::parse_query_terms(q) {
                let pat = Self::like_pattern(&term.text);
                match term.scope {
                    Some(crate::search::FieldScope::Title) => {
                        conds.push("b.title LIKE ? ESCAPE '\\'".to_string());
                        args.push(pat);
                    }
                    Some(crate::search::FieldScope::Url) => {
                        conds.push("b.url LIKE ? ESCAPE '\\'".to_string());
                        args.push(pat);
                    }
                    Some(crate::search::FieldScope::Description) => {
                        conds.push("b.description LIKE ? ESCAPE '\\'".to_string());
                        args.push(pat);
                    }
                    Some(crate::search::FieldScope::Folder) => {
                        conds.push("b.folder LIKE ? ESCAPE '\\'".to_string());
                        args.push(pat);
                    }
                    Some(crate::search::FieldScope::Tags) => {
                        conds.push("EXISTS (SELECT 1 FROM bookmark_tags t2 WHERE t2.bookmark_uuid = b.uuid AND t2.tag LIKE ? ESCAPE '\\')".to_string());
                        args.push(pat);
                    }
                    None if !filter.scope.any() => {
                        conds.push("(b.title LIKE ? ESCAPE '\\' OR b.url LIKE ? ESCAPE '\\' OR b.description LIKE ? ESCAPE '\\' OR b.folder LIKE ? ESCAPE '\\' OR EXISTS (SELECT 1 FROM bookmark_tags t2 WHERE t2.bookmark_uuid = b.uuid AND t2.tag LIKE ? ESCAPE '\\'))".to_string());
                        for _ in 0..5 {
                            args.push(pat.clone());
                        }
                    }
                    None => {
                        let mut ors = Vec::new();
                        if filter.scope.title {
                            ors.push("b.title LIKE ? ESCAPE '\\'".to_string());
                            args.push(pat.clone());
                        }
                        if filter.scope.url {
                            ors.push("b.url LIKE ? ESCAPE '\\'".to_string());
                            args.push(pat.clone());
                        }
                        if filter.scope.description {
                            ors.push("b.description LIKE ? ESCAPE '\\'".to_string());
                            args.push(pat.clone());
                        }
                        if filter.scope.folder {
                            ors.push("b.folder LIKE ? ESCAPE '\\'".to_string());
                            args.push(pat.clone());
                        }
                        if filter.scope.tags {
                            ors.push("EXISTS (SELECT 1 FROM bookmark_tags t2 WHERE t2.bookmark_uuid = b.uuid AND t2.tag LIKE ? ESCAPE '\\')".to_string());
                            args.push(pat.clone());
                        }
                        conds.push(format!("({})", ors.join(" OR ")));
                    }
                }
            }
        }
        if filter.opened_only {
            conds.push("b.last_opened_at IS NOT NULL".to_string());
        }
        let where_clause = if conds.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conds.join(" AND "))
        };
        let order = match sort {
            SortMode::Newest => "ORDER BY b.created_at DESC, b.uuid ASC",
            SortMode::Oldest => "ORDER BY b.created_at ASC, b.uuid ASC",
            SortMode::Visited => {
                "ORDER BY b.last_opened_at IS NULL, b.last_opened_at DESC, b.uuid ASC"
            }
            SortMode::Title => "ORDER BY b.title COLLATE NOCASE ASC, b.uuid ASC",
            SortMode::Relevance => "ORDER BY b.created_at ASC, b.uuid ASC",
        };
        let count_sql = format!(
            "SELECT COUNT(DISTINCT b.uuid) FROM bookmarks b{joins}{where_clause}",
            joins = joins,
            where_clause = where_clause
        );
        let total: i64 = self
            .conn
            .query_row(&count_sql, rusqlite::params_from_iter(args.iter()), |row| {
                row.get(0)
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let cols: Vec<String> = BOOKMARK_COLS
            .split(',')
            .map(|c| format!("b.{}", c.trim()))
            .collect();
        let page_sql = format!(
            "SELECT DISTINCT {} FROM bookmarks b{joins}{where_clause} {order} LIMIT {limit} OFFSET {offset}",
            cols.join(", "),
            joins = joins,
            where_clause = where_clause,
            order = order,
            limit = limit,
            offset = offset
        );
        let mut stmt = self
            .conn
            .prepare(&page_sql)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), row_bookmark)
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let mut items = Vec::new();
        for b in rows {
            items.push(b.map_err(|e| CoreError::Storage(e.to_string()))?);
        }
        let items = self.with_attachments_batch(&mut items)?;
        Ok((items, total as usize))
    }

    pub fn tag_counts_sql(&self) -> Result<Vec<(String, usize)>, CoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT tag, COUNT(*) FROM bookmark_tags GROUP BY tag ORDER BY COUNT(*) DESC, tag ASC",
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn folder_counts_sql(&self) -> Result<Vec<(String, usize)>, CoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT CASE WHEN folder = '' THEN '/' ELSE folder END, COUNT(*) FROM bookmarks GROUP BY folder ORDER BY COUNT(*) DESC, folder ASC",
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn resolve_spec(&self, tokens: &[String]) -> Result<Vec<Bookmark>, CoreError> {
        let mut out = Vec::new();
        for t in tokens {
            if let Ok(n) = t.trim().parse::<i64>() {
                let mut stmt = self
                    .conn
                    .prepare(&format!(
                        "SELECT {BOOKMARK_COLS} FROM bookmarks WHERE short_id = ?1"
                    ))
                    .map_err(|e| CoreError::Storage(e.to_string()))?;
                let found: Option<Bookmark> = stmt
                    .query_row(params![n], row_bookmark)
                    .optional()
                    .map_err(|e| CoreError::Storage(e.to_string()))?;
                if let Some(b) = found {
                    out.push(self.with_attachments(b)?);
                    continue;
                }
            }
            let prefix = t.to_lowercase();
            let mut stmt = self
                .conn
                .prepare(&format!(
                    "SELECT {BOOKMARK_COLS} FROM bookmarks WHERE lower(uuid) LIKE ?1 || '%'"
                ))
                .map_err(|e| CoreError::Storage(e.to_string()))?;
            let rows = stmt
                .query_map(params![prefix], row_bookmark)
                .map_err(|e| CoreError::Storage(e.to_string()))?;
            let mut hits = Vec::new();
            for b in rows {
                hits.push(b.map_err(|e| CoreError::Storage(e.to_string()))?);
            }
            match hits.len() {
                0 => return Err(CoreError::NotFound(t.clone())),
                1 => out.push(self.with_attachments(hits.remove(0))?),
                _ => {
                    return Err(CoreError::Invalid(format!(
                        "ambiguous prefix {t:?} ({} matches)",
                        hits.len()
                    )))
                }
            }
        }
        Ok(out)
    }

    pub fn stamp_check(&mut self, uuid: &Uuid, status: Option<&str>) -> Result<(), CoreError> {
        let n = self
            .conn
            .execute(
                "UPDATE bookmarks SET last_checked_at = ?1, check_status = ?2 WHERE uuid = ?3",
                params![Utc::now().to_rfc3339(), status, uuid.to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            return Err(CoreError::NotFound(uuid.to_string()));
        }
        Ok(())
    }

    pub fn record_open(&mut self, uuid: &Uuid) -> Result<(), CoreError> {
        let n = self
            .conn
            .execute(
                "UPDATE bookmarks SET open_count = open_count + 1, last_opened_at = ?1
                 WHERE uuid = ?2",
                params![Utc::now().to_rfc3339(), uuid.to_string()],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            return Err(CoreError::NotFound(uuid.to_string()));
        }
        Ok(())
    }

    pub fn add_attachment(
        &mut self,
        uuid: &Uuid,
        name: String,
        path: String,
    ) -> Result<(), CoreError> {
        if self.get(uuid)?.is_none() {
            return Err(CoreError::NotFound(uuid.to_string()));
        }
        self.conn
            .execute(
                "INSERT INTO attachments (bookmark_uuid, name, path) VALUES (?1, ?2, ?3)",
                params![uuid.to_string(), name, path],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let payload = serde_json::json!({
            "uuid": uuid.to_string(),
            "name": name,
            "path": path,
        });
        self.append_oplog(Some(*uuid), "attach", payload)
    }

    pub fn remove_attachment(&mut self, uuid: &Uuid, path: &str) -> Result<bool, CoreError> {
        let n = self
            .conn
            .execute(
                "DELETE FROM attachments WHERE bookmark_uuid = ?1 AND path = ?2",
                params![uuid.to_string(), path],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(n > 0)
    }

    pub fn add_rule(
        &mut self,
        pattern: String,
        action_tags: Vec<String>,
        action_folder: Option<String>,
    ) -> Result<AutoRule, CoreError> {
        let rule = AutoRule {
            id: Uuid::new_v4().to_string(),
            pattern: pattern.clone(),
            action_tags,
            action_folder,
        };
        self.conn
            .execute(
                "INSERT INTO rules (id, pattern, action_tags, action_folder, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    rule.id,
                    rule.pattern,
                    serde_json::to_string(&rule.action_tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    rule.action_folder,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let payload = serde_json::to_value(&rule).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(None, "rule_put", payload)?;
        Ok(rule)
    }

    pub fn list_rules(&self) -> Result<Vec<AutoRule>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, pattern, action_tags, action_folder FROM rules ORDER BY pattern")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let tags_raw: String = row.get(2)?;
                Ok(AutoRule {
                    id: row.get(0)?,
                    pattern: row.get(1)?,
                    action_tags: serde_json::from_str(&tags_raw).unwrap_or_default(),
                    action_folder: row.get(3)?,
                })
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn find_rule(&self, id: &str) -> Result<Option<AutoRule>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, pattern, action_tags, action_folder FROM rules WHERE id = ?1")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        stmt.query_row(params![id], |row| {
            let tags_raw: String = row.get(2)?;
            Ok(AutoRule {
                id: row.get(0)?,
                pattern: row.get(1)?,
                action_tags: serde_json::from_str(&tags_raw).unwrap_or_default(),
                action_folder: row.get(3)?,
            })
        })
        .optional()
        .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn update_rule(&mut self, rule: &AutoRule) -> Result<(), CoreError> {
        let n = self
            .conn
            .execute(
                "UPDATE rules SET pattern = ?1, action_tags = ?2, action_folder = ?3 WHERE id = ?4",
                params![
                    rule.pattern,
                    serde_json::to_string(&rule.action_tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    rule.action_folder,
                    rule.id,
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            return Err(CoreError::NotFound(rule.id.clone()));
        }
        let payload = serde_json::to_value(rule).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(None, "rule_put", payload)
    }

    pub fn delete_rule(&mut self, id: &str) -> Result<bool, CoreError> {
        let existing = self.find_rule(id)?;
        let n = self
            .conn
            .execute("DELETE FROM rules WHERE id = ?1", params![id])
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        if n == 0 {
            return Ok(false);
        }
        let payload = serde_json::json!({
            "id": id,
            "pattern": existing.map(|r| r.pattern).unwrap_or_default(),
        });
        self.append_oplog(None, "rule_del", payload)?;
        Ok(true)
    }

    pub fn oplog_applied(&self, key: &(String, String, String, String)) -> Result<bool, CoreError> {
        let n: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM applied_oplog WHERE device = ?1 AND ts = ?2 AND op = ?3 AND uuid = ?4",
                params![key.0, key.1, key.2, key.3],
                |row| row.get(0),
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(n > 0)
    }

    pub fn oplog_mark_applied(
        &self,
        key: &(String, String, String, String),
    ) -> Result<(), CoreError> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO applied_oplog (device, ts, op, uuid) VALUES (?1, ?2, ?3, ?4)",
                params![key.0, key.1, key.2, key.3],
            )
            .map(|_| ())
            .map_err(|e| CoreError::Storage(e.to_string()))
    }

    pub fn add_rule_with_id(&mut self, rule: &AutoRule) -> Result<(), CoreError> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO rules (id, pattern, action_tags, action_folder, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    rule.id,
                    rule.pattern,
                    serde_json::to_string(&rule.action_tags)
                        .map_err(|e| CoreError::Storage(e.to_string()))?,
                    rule.action_folder,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let payload = serde_json::to_value(rule).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(None, "rule_put", payload)
    }

    pub fn delete_rule_quiet(&mut self, id: &str) -> Result<bool, CoreError> {
        let n = self
            .conn
            .execute("DELETE FROM rules WHERE id = ?1", params![id])
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(n > 0)
    }

    pub fn prune_oplog_older_than(&mut self, retention_days: i64) -> Result<usize, CoreError> {
        let cutoff = (Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();
        let n = self
            .conn
            .execute("DELETE FROM oplog WHERE ts < ?1", params![cutoff])
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        Ok(n)
    }

    pub(crate) fn append_oplog(
        &self,
        uuid: Option<Uuid>,
        op: &str,
        payload: serde_json::Value,
    ) -> Result<(), CoreError> {
        let device = self.cfg.effective_device_id();
        let ts = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO oplog (uuid, device_id, ts, op, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    uuid.map(|u| u.to_string()),
                    device,
                    ts,
                    op,
                    payload.to_string(),
                ],
            )
            .map(|_| ())
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        self.oplog_mark_applied(&(
            device,
            ts,
            op.to_string(),
            uuid.map(|u| u.to_string()).unwrap_or_default(),
        ))
    }

    pub fn oplog_entries(&self) -> Result<Vec<OpLogEntry>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT seq, uuid, device_id, ts, op, payload FROM oplog ORDER BY seq")
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let uuid_raw: Option<String> = row.get(1)?;
                let uuid = uuid_raw.map(|s| s.parse()).transpose().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let ts_raw: String = row.get(3)?;
                let ts = ts_raw.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let payload_raw: String = row.get(5)?;
                let payload = serde_json::from_str(&payload_raw).unwrap_or_default();
                Ok(OpLogEntry {
                    seq: row.get(0)?,
                    uuid,
                    device_id: row.get(2)?,
                    ts,
                    op: row.get(4)?,
                    payload,
                })
            })
            .map_err(|e| CoreError::Storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CoreError::Storage(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_store() -> Store {
        Store::open_in_memory(Config {
            base_dir: std::path::PathBuf::from("/tmp/liber-test"),
            device_id: "test-device".to_string(),
            ..Default::default()
        })
        .unwrap()
    }

    fn sample(url: &str) -> NewBookmark {
        NewBookmark {
            uuid: Uuid::new_v4(),
            url: url.to_string(),
            title: "Example".to_string(),
            description: String::new(),
            tags: vec![],
            folder: String::new(),
            html_file: "x.html".to_string(),
            markdown_file: None,
            archive_file: None,
            applied_rules: vec![],
        }
    }

    #[test]
    fn add_get_roundtrip() {
        let mut s = mem_store();
        let b = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let got = s.get(&b.uuid).unwrap().unwrap();
        assert_eq!(got.url, "https://example.com/a");
        assert_eq!(got.open_count, 0);
    }

    fn seeded() -> Store {
        let mut s = mem_store();
        let rows = [
            (
                "https://example.com/rust-guide",
                "Rust guide",
                vec!["prog"],
                "tech",
            ),
            (
                "https://example.com/rust-book",
                "Rust book",
                vec!["prog", "read"],
                "tech",
            ),
            (
                "https://example.com/pasta",
                "Pasta recipe",
                vec!["food"],
                "home",
            ),
            (
                "https://example.com/server",
                "Server setup",
                vec![],
                "tech/ops",
            ),
        ];
        for (url, title, tags, folder) in rows {
            let mut nb = sample(url);
            nb.title = title.to_string();
            nb.tags = tags.into_iter().map(str::to_string).collect();
            nb.folder = folder.to_string();
            s.add_bookmark(nb).unwrap();
        }
        s
    }

    #[test]
    fn query_matches_full_scan() {
        use crate::search::{bookmark_matches_query, order_results, parse_sort_mode, SearchFields};
        let s = seeded();
        let fields = SearchFields::all();
        for (q, tag, folder, sort) in [
            (None, None, None, ""),
            (Some("rust"), None, None, ""),
            (Some("rust"), None, None, "title"),
            (Some("tag:prog"), None, None, ""),
            (Some("title:server folder:tech/ops"), None, None, ""),
            (None, Some("prog"), None, "newest"),
            (None, None, Some("tech"), "oldest"),
            (Some("example"), Some("read"), None, "visited"),
        ] {
            let filter = BookmarkFilter {
                folder: folder.map(str::to_string),
                tag: tag.map(str::to_string),
                query: q.map(str::to_string),
                scope: SearchFields::all(),
                opened_only: false,
            };
            let mode = parse_sort_mode(sort).unwrap();
            let (page, total) = s.query_bookmarks(&filter, mode, 50, 0).unwrap();
            let page = order_results(page, q.unwrap_or(""), mode);
            let mut expected: Vec<_> = s
                .list()
                .unwrap()
                .into_iter()
                .filter(|b| match &filter.folder {
                    Some(f) => &b.folder == f,
                    None => true,
                })
                .filter(|b| match &filter.tag {
                    Some(t) => b.tags.iter().any(|x| x.eq_ignore_ascii_case(t)),
                    None => true,
                })
                .filter(|b| match &filter.query {
                    Some(qq) => bookmark_matches_query(b, qq, &fields),
                    None => true,
                })
                .collect();
            expected = order_results(expected, q.unwrap_or(""), mode);
            let expected: Vec<_> = expected.into_iter().take(50).collect();
            assert_eq!(total, expected.len(), "total for {q:?}/{tag:?}/{folder:?}");
            let got: Vec<String> = page.iter().map(|b| b.uuid.to_string()).collect();
            let want: Vec<String> = expected.iter().map(|b| b.uuid.to_string()).collect();
            assert_eq!(got, want, "page for {q:?}/{tag:?}/{folder:?}/{sort}");
        }
    }

    #[test]
    fn query_paginates() {
        let s = seeded();
        let filter = BookmarkFilter::default();
        let (p1, total) = s
            .query_bookmarks(&filter, crate::search::SortMode::Oldest, 2, 0)
            .unwrap();
        let (p2, _) = s
            .query_bookmarks(&filter, crate::search::SortMode::Oldest, 2, 2)
            .unwrap();
        assert_eq!(total, 4);
        assert_eq!(p1.len(), 2);
        assert_eq!(p2.len(), 2);
        assert_ne!(p1[0].uuid, p2[0].uuid);
    }

    #[test]
    fn counts_match_full_scan() {
        use crate::taxonomy::{folder_counts, tag_counts};
        let s = seeded();
        assert_eq!(tag_counts(&s).unwrap(), s.tag_counts_sql().unwrap());
        assert_eq!(folder_counts(&s).unwrap(), s.folder_counts_sql().unwrap());
    }

    #[test]
    fn tag_side_table_tracks_writes() {
        let mut s = mem_store();
        let mut nb = sample("https://example.com/a");
        nb.tags = vec!["x".to_string(), "y".to_string()];
        let b = s.add_bookmark(nb).unwrap();
        assert_eq!(
            s.tag_counts_sql().unwrap(),
            vec![("x".to_string(), 1), ("y".to_string(), 1)]
        );
        let mut got = s.get(&b.uuid).unwrap().unwrap();
        got.tags = vec!["z".to_string()];
        s.update_bookmark(&got).unwrap();
        assert_eq!(s.tag_counts_sql().unwrap(), vec![("z".to_string(), 1)]);
        s.delete_bookmark(&b.uuid).unwrap();
        assert!(s.tag_counts_sql().unwrap().is_empty());
    }

    #[test]
    fn applied_ledger_persists_on_add() {
        use crate::model::AppliedRule;
        let mut s = mem_store();
        let mut nb = sample("https://example.com/a");
        nb.applied_rules = vec![AppliedRule {
            rule_id: "r1".to_string(),
            folder: Some("tech".to_string()),
        }];
        let b = s.add_bookmark(nb).unwrap();
        let got = s.get(&b.uuid).unwrap().unwrap();
        assert_eq!(got.applied_rules.len(), 1);
        assert_eq!(got.applied_rules[0].rule_id, "r1");
    }

    #[test]
    fn duplicate_by_normalized_url() {
        let mut s = mem_store();
        s.add_bookmark(sample("https://example.com/")).unwrap();
        let err = s
            .add_bookmark(sample("https://example.com?utm_source=x"))
            .unwrap_err();
        assert!(matches!(err, CoreError::Duplicate(_)));
    }

    #[test]
    fn update_and_open() {
        let mut s = mem_store();
        let mut b = s.add_bookmark(sample("https://example.com/a")).unwrap();
        b.title = "New title".to_string();
        s.update_bookmark(&b).unwrap();
        s.record_open(&b.uuid).unwrap();
        s.record_open(&b.uuid).unwrap();
        let got = s.get(&b.uuid).unwrap().unwrap();
        assert_eq!(got.title, "New title");
        assert_eq!(got.open_count, 2);
        assert!(got.last_opened_at.is_some());
    }

    #[test]
    fn delete_writes_tombstone() {
        let mut s = mem_store();
        let b = s.add_bookmark(sample("https://example.com/a")).unwrap();
        assert!(s.delete_bookmark(&b.uuid).unwrap());
        assert!(!s.delete_bookmark(&b.uuid).unwrap());
        assert!(s.get(&b.uuid).unwrap().is_none());
        let ops = s.oplog_entries().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[1].op, "delete");
    }

    #[test]
    fn resolve_prefix_and_ambiguity() {
        let mut s = mem_store();
        let b = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let prefix = b.uuid.to_string()[..8].to_string();
        let hits = s.resolve_spec(&[prefix]).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].uuid, b.uuid);
        assert!(s.resolve_spec(&["zz-nope-99".to_string()]).is_err());
    }

    #[test]
    fn rules_crud() {
        let mut s = mem_store();
        let r = s
            .add_rule(
                "host:example.com".to_string(),
                vec!["news".to_string()],
                Some("tech".to_string()),
            )
            .unwrap();
        assert_eq!(s.list_rules().unwrap().len(), 1);
        assert!(s.find_rule(&r.id).unwrap().is_some());
        assert!(s.delete_rule(&r.id).unwrap());
        assert!(s.list_rules().unwrap().is_empty());
    }

    #[test]
    fn disk_open_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "d1".to_string(),
            ..Default::default()
        };
        let mut s = Store::open(cfg).unwrap();
        s.add_bookmark(sample("https://example.com/a")).unwrap();
        assert_eq!(s.list().unwrap().len(), 1);
        assert!(dir.path().join(".liber").join("store.db").exists());
    }

    #[test]
    fn short_ids_assign_monotonically_without_reuse() {
        let mut s = mem_store();
        let a = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let b = s.add_bookmark(sample("https://example.com/b")).unwrap();
        assert_eq!(a.short_id, Some(1));
        assert_eq!(b.short_id, Some(2));
        s.delete_bookmark(&b.uuid).unwrap();
        let c = s.add_bookmark(sample("https://example.com/c")).unwrap();
        assert_eq!(c.short_id, Some(3));
    }

    #[test]
    fn maintenance_status_counts() {
        let mut s = mem_store();
        let a = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let b = s.add_bookmark(sample("https://example.com/b")).unwrap();
        s.add_bookmark(sample("https://example.com/c")).unwrap();
        let st = s.maintenance_status().unwrap();
        assert_eq!(st.bookmarks, 3);
        assert!(st.oplog_entries >= 3);
        assert_eq!(st.quarantine, 0);
        assert_eq!(st.short_id_gaps, 0);
        s.delete_bookmark(&b.uuid).unwrap();
        let st = s.maintenance_status().unwrap();
        assert_eq!(st.bookmarks, 2);
        assert_eq!(st.short_id_gaps, 1);
        crate::check::quarantine_bookmark(&mut s, &a.uuid).unwrap();
        assert_eq!(s.maintenance_status().unwrap().quarantine, 1);
    }

    #[test]
    fn resolve_prefers_short_id() {
        let mut s = mem_store();
        let a = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let hits = s.resolve_spec(&["1".to_string()]).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].uuid, a.uuid);
        assert!(s.resolve_spec(&["99".to_string()]).is_err());
    }

    #[test]
    fn backfill_assigns_in_creation_order() {
        let mut s = mem_store();
        s.conn
            .execute("INSERT INTO bookmarks (uuid, url, url_norm, created_at, updated_at) VALUES ('11111111-1111-1111-1111-111111111111', 'https://example.com/old', 'https://example.com/old', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z')", [])
            .unwrap();
        let b = s.add_bookmark(sample("https://example.com/new")).unwrap();
        assert_eq!(b.short_id, Some(1));
        assert_eq!(s.backfill_short_ids().unwrap(), 1);
        let old = s
            .get(&uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(old.short_id, Some(2));
        assert_eq!(s.backfill_short_ids().unwrap(), 0);
    }

    #[test]
    fn compact_closes_gaps_and_resets_counter() {
        let mut s = mem_store();
        let a = s.add_bookmark(sample("https://example.com/a")).unwrap();
        let b = s.add_bookmark(sample("https://example.com/b")).unwrap();
        let c = s.add_bookmark(sample("https://example.com/c")).unwrap();
        assert_eq!(
            (a.short_id, b.short_id, c.short_id),
            (Some(1), Some(2), Some(3))
        );
        s.delete_bookmark(&b.uuid).unwrap();
        let moved = s.compact_short_ids().unwrap();
        assert_eq!(moved.len(), 1);
        assert_eq!(s.get(&a.uuid).unwrap().unwrap().short_id, Some(1));
        assert_eq!(s.get(&c.uuid).unwrap().unwrap().short_id, Some(2));
        assert!(s.compact_short_ids().unwrap().is_empty());
        let d = s.add_bookmark(sample("https://example.com/d")).unwrap();
        assert_eq!(d.short_id, Some(3));
    }

    #[test]
    fn replace_preserves_local_short_id() {
        let mut s = mem_store();
        let a = s.add_bookmark(sample("https://example.com/a")).unwrap();
        assert_eq!(a.short_id, Some(1));
        let mut remote = a.clone();
        remote.title = "Remote rename".to_string();
        remote.short_id = Some(7);
        s.replace_bookmark_exact(&remote).unwrap();
        assert_eq!(s.get(&a.uuid).unwrap().unwrap().short_id, Some(1));
    }

    #[test]
    fn update_bookmark_keeps_short_id() {
        let mut s = mem_store();
        let mut b = s.add_bookmark(sample("https://example.com/a")).unwrap();
        b.title = "New title".to_string();
        b.short_id = Some(99);
        s.update_bookmark(&b).unwrap();
        assert_eq!(s.get(&b.uuid).unwrap().unwrap().short_id, Some(1));
    }
}
