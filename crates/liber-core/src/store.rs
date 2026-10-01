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
    pub auth_token: String,
    #[serde(default)]
    pub archive_backend: String,
    #[serde(default)]
    pub browser_cmd: String,
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

pub struct Store {
    conn: Connection,
    pub cfg: Config,
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
    })
}

const BOOKMARK_COLS: &str = "uuid, url, title, description, tags, folder,
    created_at, updated_at, html_file, markdown_file, archive_file,
    open_count, last_opened_at, last_checked_at, check_status, applied_rules";

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
        Ok(Self { conn, cfg })
    }

    pub fn open_in_memory(cfg: Config) -> Result<Self, CoreError> {
        let conn = Self::connect(None)?;
        Self::migrate(&conn)?;
        Ok(Self { conn, cfg })
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
        };
        self.conn
            .execute(
                "INSERT INTO bookmarks (uuid, url, url_norm, title, description, tags,
                 folder, created_at, updated_at, html_file, markdown_file, archive_file,
                 open_count, applied_rules)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, '[]')",
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
                ],
            )
            .map_err(|e| CoreError::Storage(e.to_string()))?;
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
        let payload = serde_json::to_value(b).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(Some(b.uuid), "upsert", payload)
    }

    pub fn replace_bookmark_exact(&mut self, b: &Bookmark) -> Result<(), CoreError> {
        self.conn
            .execute(
                "INSERT INTO bookmarks (uuid, url, url_norm, title, description, tags,
                 folder, created_at, updated_at, html_file, markdown_file, archive_file,
                 open_count, last_opened_at, last_checked_at, check_status, applied_rules)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                 ON CONFLICT(uuid) DO UPDATE SET url = excluded.url, url_norm = excluded.url_norm,
                 title = excluded.title, description = excluded.description, tags = excluded.tags,
                 folder = excluded.folder, created_at = excluded.created_at, updated_at = excluded.updated_at,
                 html_file = excluded.html_file, markdown_file = excluded.markdown_file,
                 archive_file = excluded.archive_file, open_count = excluded.open_count,
                 last_opened_at = excluded.last_opened_at, last_checked_at = excluded.last_checked_at,
                 check_status = excluded.check_status, applied_rules = excluded.applied_rules",
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
        let payload = serde_json::to_value(b).map_err(|e| CoreError::Storage(e.to_string()))?;
        self.append_oplog(Some(b.uuid), "upsert", payload)
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
            out.push(self.with_attachments(b)?);
        }
        Ok(out)
    }

    pub fn resolve_spec(&self, tokens: &[String]) -> Result<Vec<Bookmark>, CoreError> {
        let mut out = Vec::new();
        for t in tokens {
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
        let payload = serde_json::json!({"uuid": uuid.to_string(), "name": name});
        self.append_oplog(Some(*uuid), "upsert", payload)
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
}
