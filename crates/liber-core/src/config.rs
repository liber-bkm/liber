use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::store::Config;
use crate::CoreError;

pub fn config_path() -> Result<PathBuf, CoreError> {
    if let Ok(p) = std::env::var("LIBER_CONFIG") {
        if !p.trim().is_empty() {
            return Ok(PathBuf::from(p.trim()));
        }
    }
    let mut dir =
        dirs::config_dir().ok_or_else(|| CoreError::Storage("no config dir".to_string()))?;
    dir.push("liber-rs");
    dir.push("config.json");
    Ok(dir)
}

fn default_base_dir() -> PathBuf {
    if let Some(data) = dirs::data_dir() {
        return data.join("liber-rs");
    }
    PathBuf::from("Bookmarks")
}

pub fn default_config() -> Config {
    Config {
        base_dir: default_base_dir(),
        archive_backend: "builtin".to_string(),
        ..Default::default()
    }
}

pub fn system_dirs_available() -> bool {
    dirs::config_dir().is_some()
}

pub fn load_config() -> Result<(Config, PathBuf), CoreError> {
    load_config_from(config_path()?, default_base_dir())
}

pub fn load_config_from(
    path: PathBuf,
    default_base: PathBuf,
) -> Result<(Config, PathBuf), CoreError> {
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut cfg = default_config();
            cfg.base_dir = default_base.clone();
            cfg.device_id = Uuid::new_v4().to_string();
            if let Err(werr) = save_config_to(&path, &cfg) {
                eprintln!("warning: could not write default config: {werr}");
            }
            apply_env_overrides(&mut cfg);
            return Ok((cfg, path));
        }
        Err(e) => return Err(CoreError::Storage(e.to_string())),
    };
    let mut cfg: Config =
        serde_json::from_slice(&data).map_err(|e| CoreError::Storage(e.to_string()))?;
    if cfg.base_dir.as_os_str().is_empty() {
        cfg.base_dir = default_base;
    }
    if cfg.archive_backend.is_empty() {
        cfg.archive_backend = "builtin".to_string();
    }
    if cfg.device_id.is_empty() {
        cfg.device_id = Uuid::new_v4().to_string();
    }
    apply_env_overrides(&mut cfg);
    Ok((cfg, path))
}

fn apply_env_overrides(cfg: &mut Config) {
    if let Ok(base) = std::env::var("LIBER_BASE_DIR") {
        if !base.trim().is_empty() {
            cfg.base_dir = PathBuf::from(base.trim());
        }
    }
    if let Ok(token) = std::env::var("LIBER_AUTH_TOKEN") {
        if !token.trim().is_empty() {
            cfg.auth_token = token.trim().to_string();
        }
    }
}

pub fn save_config_to(path: &PathBuf, cfg: &Config) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::Storage(e.to_string()))?;
    }
    let data = serde_json::to_string_pretty(cfg).map_err(|e| CoreError::Storage(e.to_string()))?;
    std::fs::write(path, data).map_err(|e| CoreError::Storage(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn save_config(cfg: &Config) -> Result<PathBuf, CoreError> {
    let path = config_path()?;
    save_config_to(&path, cfg)?;
    Ok(path)
}

pub const API_SETTABLE: &[&str] = &[
    "base_dir",
    "html_dir",
    "markdown_dir",
    "archive_dir",
    "attachment_dir",
    "archive_backend",
    "browser_cmd",
    "browser_path",
    "editor_cmd",
    "singlefile_cmd",
    "singlefile_browser_path",
    "monolith_cmd",
    "device_id",
];

pub fn validate_setting_value(key: &str, value: &str) -> Result<(), CoreError> {
    match key {
        "archive_backend" => {
            crate::archive::parse_backend(value)?;
            Ok(())
        }
        "base_dir" | "html_dir" | "markdown_dir" | "archive_dir" | "attachment_dir" => {
            if value.trim().is_empty() {
                return Err(CoreError::Invalid(format!("{key} can't be empty")));
            }
            Ok(())
        }
        "browser_cmd"
        | "browser_path"
        | "editor_cmd"
        | "singlefile_cmd"
        | "singlefile_browser_path"
        | "monolith_cmd"
        | "device_id" => Ok(()),
        _ => Err(CoreError::Invalid(format!("unknown key {key:?}"))),
    }
}

pub fn apply_setting(cfg: &mut Config, key: &str, value: &str) -> Result<(), CoreError> {
    validate_setting_value(key, value)?;
    match key {
        "base_dir" => cfg.base_dir = PathBuf::from(value.trim()),
        "html_dir" => cfg.html_dir = Some(PathBuf::from(value.trim())),
        "markdown_dir" => cfg.markdown_dir = Some(PathBuf::from(value.trim())),
        "archive_dir" => cfg.archive_dir = Some(PathBuf::from(value.trim())),
        "attachment_dir" => cfg.attachment_dir = Some(PathBuf::from(value.trim())),
        "archive_backend" => cfg.archive_backend = value.to_string(),
        "browser_cmd" => cfg.browser_cmd = value.to_string(),
        "browser_path" => cfg.browser_path = value.to_string(),
        "editor_cmd" => cfg.editor_cmd = value.to_string(),
        "singlefile_cmd" => cfg.singlefile_cmd = value.to_string(),
        "singlefile_browser_path" => cfg.singlefile_browser_path = value.to_string(),
        "monolith_cmd" => cfg.monolith_cmd = value.to_string(),
        "device_id" => cfg.device_id = value.to_string(),
        _ => unreachable!("validated key {key:?} has no field"),
    }
    Ok(())
}

fn expand_tilde(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if s == "~" || s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            if s == "~" {
                return home;
            }
            return home.join(&s[2..]);
        }
    }
    p.to_path_buf()
}

impl Config {
    fn kind_dir(&self, override_dir: &Option<PathBuf>, kind: &str) -> PathBuf {
        match override_dir {
            Some(d) => expand_tilde(d),
            None => self.profile_dir().join(kind),
        }
    }

    pub fn html_dir(&self) -> PathBuf {
        self.kind_dir(&self.html_dir, "html")
    }

    pub fn markdown_dir(&self) -> PathBuf {
        self.kind_dir(&self.markdown_dir, "markdown")
    }

    pub fn archive_dir(&self) -> PathBuf {
        self.kind_dir(&self.archive_dir, "archive")
    }

    pub fn attachment_dir(&self) -> PathBuf {
        self.kind_dir(&self.attachment_dir, "attachments")
    }

    pub fn tantivy_dir(&self) -> PathBuf {
        self.profile_dir().join(".liber").join("tantivy")
    }

    pub fn resolve_auth_token(&self, flag_val: &str) -> String {
        if !flag_val.trim().is_empty() {
            return flag_val.trim().to_string();
        }
        self.auth_token.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn isolated_env(dir: &std::path::Path) {
        std::env::set_var("LIBER_CONFIG", dir.join("config.json"));
        std::env::set_var("LIBER_BASE_DIR", dir.join("data"));
    }

    fn clear_env() {
        std::env::remove_var("LIBER_CONFIG");
        std::env::remove_var("LIBER_BASE_DIR");
    }

    #[test]
    fn default_then_reload_roundtrip() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        isolated_env(dir.path());
        let (cfg, _) = load_config().unwrap();
        assert!(!cfg.device_id.is_empty());
        let (again, _) = load_config().unwrap();
        assert_eq!(cfg.device_id, again.device_id);
        assert_eq!(again.base_dir, dir.path().join("data"));
        clear_env();
    }

    #[test]
    fn first_run_honors_base_dir_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        isolated_env(dir.path());
        assert!(!dir.path().join("config.json").exists());
        let (cfg, _) = load_config().unwrap();
        assert_eq!(cfg.base_dir, dir.path().join("data"));
        clear_env();
    }

    #[test]
    fn setting_validation_vectors() {
        assert!(validate_setting_value("editor_cmd", "hx").is_ok());
        assert!(validate_setting_value("device_id", "laptop").is_ok());
        assert!(validate_setting_value("archive_backend", "builtin").is_ok());
        assert!(validate_setting_value("archive_backend", "nope").is_err());
        assert!(validate_setting_value("html_dir", "~/docs").is_ok());
        assert!(validate_setting_value("html_dir", "   ").is_err());
        assert!(validate_setting_value("auth_token", "x").is_err());
        assert!(validate_setting_value("dns_fallback", "auto").is_err());
        assert!(validate_setting_value("bogus", "x").is_err());
    }

    #[test]
    fn apply_setting_roundtrip() {
        let mut cfg = default_config();
        apply_setting(&mut cfg, "editor_cmd", "hx").unwrap();
        assert_eq!(cfg.editor_cmd, "hx");
        apply_setting(&mut cfg, "archive_dir", "~/arc").unwrap();
        assert_eq!(cfg.archive_dir, Some(PathBuf::from("~/arc")));
        apply_setting(&mut cfg, "base_dir", "/tmp/base").unwrap();
        assert_eq!(cfg.base_dir, PathBuf::from("/tmp/base"));
        assert!(apply_setting(&mut cfg, "auth_token", "x").is_err());
    }
}
