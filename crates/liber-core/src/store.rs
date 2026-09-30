use std::path::PathBuf;

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
