use std::path::PathBuf;

use serde::Serialize;

use crate::store::Config;
use crate::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub path: PathBuf,
    pub active: bool,
    pub default: bool,
}

pub fn validate_profile_name(raw: &str) -> Result<Option<String>, CoreError> {
    let name = raw.trim().to_string();
    if name.is_empty() || name == "default" {
        return Ok(None);
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(CoreError::Invalid(format!("invalid profile name {name:?}")));
    }
    Ok(Some(name))
}

pub fn list_profiles(cfg: &Config) -> Vec<ProfileInfo> {
    let active = cfg.active_profile.as_deref().unwrap_or("default");
    let mut out = vec![ProfileInfo {
        name: "default".to_string(),
        path: cfg.base_dir.clone(),
        active: active == "default",
        default: true,
    }];
    let mut names: Vec<String> = cfg.profiles.clone();
    names.sort();
    names.dedup();
    for name in names {
        out.push(ProfileInfo {
            path: cfg.base_dir.join(&name),
            active: active == name,
            name,
            default: false,
        });
    }
    out
}

pub fn switch_profile(cfg: &mut Config, raw: &str) -> Result<String, CoreError> {
    cfg.active_profile = validate_profile_name(raw)?;
    if let Some(name) = &cfg.active_profile {
        if !cfg.profiles.iter().any(|p| p == name) {
            cfg.profiles.push(name.clone());
        }
    }
    std::fs::create_dir_all(cfg.profile_dir()).map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(cfg.active_profile.clone().unwrap_or("default".to_string()))
}

pub fn delete_profile(cfg: &mut Config, raw: &str) -> Result<String, CoreError> {
    let name = match validate_profile_name(raw)? {
        Some(name) => name,
        None => {
            return Err(CoreError::Invalid(
                "the default profile cannot be deleted".to_string(),
            ))
        }
    };
    let active = cfg.active_profile.as_deref().unwrap_or("default");
    if name == active {
        return Err(CoreError::Invalid(format!(
            "cannot delete the active profile {name:?}"
        )));
    }
    if !cfg.profiles.iter().any(|p| p == &name) {
        return Err(CoreError::NotFound(format!("no such profile {name:?}")));
    }
    cfg.profiles.retain(|p| p != &name);
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn cfg_in(dir: &std::path::Path) -> Config {
        Config {
            base_dir: dir.to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        }
    }

    fn seed_profile(dir: &std::path::Path, name: Option<&str>) {
        let mut cfg = cfg_in(dir);
        cfg.active_profile = name.map(str::to_string);
        let _ = Store::open(cfg).unwrap();
    }

    #[test]
    fn name_validation() {
        assert_eq!(validate_profile_name("").unwrap(), None);
        assert_eq!(validate_profile_name("default").unwrap(), None);
        assert_eq!(
            validate_profile_name("work").unwrap(),
            Some("work".to_string())
        );
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name("a\\b").is_err());
        assert!(validate_profile_name(".").is_err());
        assert!(validate_profile_name("..").is_err());
    }

    #[test]
    fn list_switch_delete() {
        let dir = tempfile::tempdir().unwrap();
        seed_profile(dir.path(), None);
        let mut cfg = cfg_in(dir.path());
        let names: Vec<String> = list_profiles(&cfg).iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["default".to_string()]);
        assert!(list_profiles(&cfg)[0].active);

        assert_eq!(switch_profile(&mut cfg, "work").unwrap(), "work");
        assert!(list_profiles(&cfg)[1].active);
        assert_eq!(cfg.profiles, vec!["work".to_string()]);
        assert!(switch_profile(&mut cfg, "default").unwrap() == "default");
        assert!(cfg.active_profile.is_none());
        assert!(switch_profile(&mut cfg, "a/b").is_err());

        assert_eq!(delete_profile(&mut cfg, "work").unwrap(), "work");
        assert!(cfg.profiles.is_empty());
        assert_eq!(list_profiles(&cfg).len(), 1);
        assert!(delete_profile(&mut cfg, "default").is_err());
        assert!(delete_profile(&mut cfg, "missing").is_err());
        assert_eq!(switch_profile(&mut cfg, "work").unwrap(), "work");
        assert!(delete_profile(&mut cfg, "work").is_err());
    }
}
