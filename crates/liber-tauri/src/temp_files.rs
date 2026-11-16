use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(target_os = "android")]
use tauri::Manager;

const STALE_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn staging_root(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(target_os = "android")]
    let base = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    #[cfg(not(target_os = "android"))]
    let base = {
        let _ = app;
        std::env::temp_dir()
    };
    base.join("liber-open")
}

pub fn staged_path(app: &tauri::AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let dir = staging_root(app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let safe: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(dir.join(format!("liber-{safe}")))
}

pub fn cleanup_stale(app: &tauri::AppHandle) -> std::io::Result<usize> {
    cleanup_dir(&staging_root(app), STALE_AFTER)
}

fn cleanup_dir(dir: &Path, max_age: Duration) -> std::io::Result<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if old && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn cleanup_removes_only_stale_files() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("liber-temp-cleanup-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stale = dir.join("liber-old.html");
        let fresh = dir.join("liber-new.html");
        std::fs::write(&stale, b"old").unwrap();
        std::fs::write(&fresh, b"new").unwrap();

        let old = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&stale)
            .unwrap();
        file.set_modified(old).unwrap();

        let removed = cleanup_dir(&dir, STALE_AFTER).unwrap();
        assert_eq!(removed, 1);
        assert!(!stale.exists());
        assert!(fresh.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
