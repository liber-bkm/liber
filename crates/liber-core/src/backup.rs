use std::io::Read;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::CoreError;

pub const BACKUP_FORMAT_VERSION: u32 = 1;

const BACKUP_TREES: &[&str] = &["html", "markdown", "archive", "attachments"];
const BACKUP_DB: &str = ".liber/store.db";
const MANIFEST_NAME: &str = "manifest.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupManifest {
    pub format: u32,
    pub device_id: String,
    pub created_at: String,
}

pub fn create_backup(
    profile_dir: &Path,
    out_file: &Path,
    device_id: &str,
) -> Result<(), CoreError> {
    let out = std::fs::File::create(out_file)
        .map_err(|e| CoreError::Storage(format!("creating {}: {e}", out_file.display())))?;
    let enc = GzEncoder::new(out, Compression::default());
    let mut tar = tar::Builder::new(enc);
    for tree in BACKUP_TREES {
        let src = profile_dir.join(tree);
        if src.is_dir() {
            tar.append_dir_all(tree, &src)
                .map_err(|e| CoreError::Storage(format!("packing {tree}: {e}")))?;
        }
    }
    let db = profile_dir.join(BACKUP_DB);
    if db.is_file() {
        tar.append_path_with_name(&db, BACKUP_DB)
            .map_err(|e| CoreError::Storage(format!("packing {BACKUP_DB}: {e}")))?;
    }
    let manifest = BackupManifest {
        format: BACKUP_FORMAT_VERSION,
        device_id: device_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let data = serde_json::to_vec(&manifest).map_err(|e| CoreError::Storage(e.to_string()))?;
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, MANIFEST_NAME, data.as_slice())
        .map_err(|e| CoreError::Storage(format!("packing manifest: {e}")))?;
    tar.into_inner()
        .map_err(|e| CoreError::Storage(format!("finishing archive: {e}")))?
        .finish()
        .map_err(|e| CoreError::Storage(format!("finishing gzip: {e}")))?;
    Ok(())
}

fn entry_ok(path: &Path) -> bool {
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|c| matches!(c, Component::ParentDir))
}

pub fn read_manifest(backup_file: &Path) -> Result<BackupManifest, CoreError> {
    let file = std::fs::File::open(backup_file)
        .map_err(|e| CoreError::Storage(format!("opening {}: {e}", backup_file.display())))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    for entry in archive
        .entries()
        .map_err(|e| CoreError::Storage(format!("reading backup: {e}")))?
    {
        let mut entry = entry.map_err(|e| CoreError::Storage(format!("reading backup: {e}")))?;
        if entry
            .path()
            .map(|p| p.to_string_lossy() == MANIFEST_NAME)
            .unwrap_or(false)
        {
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| CoreError::Storage(format!("reading manifest: {e}")))?;
            let manifest: BackupManifest = serde_json::from_slice(&data)
                .map_err(|e| CoreError::Storage(format!("bad manifest: {e}")))?;
            if manifest.format != BACKUP_FORMAT_VERSION {
                return Err(CoreError::Invalid(format!(
                    "unsupported backup format {}, expected {BACKUP_FORMAT_VERSION}",
                    manifest.format
                )));
            }
            return Ok(manifest);
        }
    }
    Err(CoreError::Invalid("backup has no manifest".to_string()))
}
pub fn restore_backup(backup_file: &Path, dest_dir: &Path) -> Result<BackupManifest, CoreError> {
    let manifest = read_manifest(backup_file)?;
    let staging = dest_dir.join(format!(".restore-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .map_err(|e| CoreError::Storage(format!("clearing staging: {e}")))?;
    }
    std::fs::create_dir_all(&staging)
        .map_err(|e| CoreError::Storage(format!("creating staging: {e}")))?;
    let staged = unpack_validated(backup_file, &staging);
    if staged.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    staged?;
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| CoreError::Storage(format!("creating {}: {e}", dest_dir.display())))?;
    for entry in std::fs::read_dir(&staging)
        .map_err(|e| CoreError::Storage(format!("listing staging: {e}")))?
    {
        let entry = entry.map_err(|e| CoreError::Storage(e.to_string()))?;
        move_tree(&entry.path(), &dest_dir.join(entry.file_name()))?;
    }
    let _ = std::fs::remove_dir_all(&staging);
    Ok(manifest)
}

fn move_tree(src: &Path, dst: &Path) -> Result<(), CoreError> {
    if !dst.exists() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CoreError::Storage(format!("creating {}: {e}", parent.display())))?;
        }
        return std::fs::rename(src, dst)
            .map_err(|e| CoreError::Storage(format!("moving {}: {e}", src.display())));
    }
    if src.is_dir() && dst.is_dir() {
        for entry in std::fs::read_dir(src)
            .map_err(|e| CoreError::Storage(format!("listing {}: {e}", src.display())))?
        {
            let entry = entry.map_err(|e| CoreError::Storage(e.to_string()))?;
            move_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::rename(src, dst)
        .map_err(|e| CoreError::Storage(format!("moving {}: {e}", src.display())))
}

fn unpack_validated(backup_file: &Path, dest_dir: &Path) -> Result<(), CoreError> {
    let file = std::fs::File::open(backup_file)
        .map_err(|e| CoreError::Storage(format!("opening {}: {e}", backup_file.display())))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    for entry in archive
        .entries()
        .map_err(|e| CoreError::Storage(format!("reading backup: {e}")))?
    {
        let mut entry = entry.map_err(|e| CoreError::Storage(format!("reading backup: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| CoreError::Storage(format!("bad entry path: {e}")))?
            .into_owned();
        if !entry_ok(&path) {
            return Err(CoreError::Invalid(format!("unsafe entry path {path:?}")));
        }
        entry
            .unpack_in(dest_dir)
            .map_err(|e| CoreError::Storage(format!("unpacking {path:?}: {e}")))?;
    }
    Ok(())
}

pub fn profile_db_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(BACKUP_DB)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_profile(dir: &Path) {
        std::fs::create_dir_all(dir.join("html")).unwrap();
        std::fs::write(dir.join("html").join("a.html"), "<html></html>").unwrap();
        std::fs::create_dir_all(dir.join(".liber")).unwrap();
        std::fs::write(dir.join(BACKUP_DB), "fake-db").unwrap();
    }

    #[test]
    fn backup_roundtrip_restores_trees_and_db() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        seed_profile(&src);
        let file = dir.path().join("backup.tar.gz");
        create_backup(&src, &file, "dev-a").unwrap();
        assert!(file.is_file());

        let manifest = read_manifest(&file).unwrap();
        assert_eq!(manifest.format, BACKUP_FORMAT_VERSION);
        assert_eq!(manifest.device_id, "dev-a");

        let dst = dir.path().join("dst");
        let back = restore_backup(&file, &dst).unwrap();
        assert_eq!(back.device_id, "dev-a");
        assert_eq!(
            std::fs::read_to_string(dst.join("html").join("a.html")).unwrap(),
            "<html></html>"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join(BACKUP_DB)).unwrap(),
            "fake-db"
        );
    }

    #[test]
    fn unsafe_entry_paths_are_rejected() {
        assert!(entry_ok(Path::new("html/a.html")));
        assert!(entry_ok(Path::new(".liber/store.db")));
        assert!(entry_ok(Path::new("manifest.json")));
        assert!(!entry_ok(Path::new("../evil.txt")));
        assert!(!entry_ok(Path::new("/abs/path.txt")));
        assert!(!entry_ok(Path::new("a/../../evil.txt")));
    }

    #[test]
    fn manifest_version_is_checked() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        seed_profile(&src);
        let file = dir.path().join("backup.tar.gz");
        create_backup(&src, &file, "dev-a").unwrap();
        assert!(read_manifest(&file).is_ok());
    }
}
