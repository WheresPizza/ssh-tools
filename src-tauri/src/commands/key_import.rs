use crate::{
    error::{AppError, Result},
    types::SshKeyInfo,
    utils::{permissions::*, ssh_dir::*},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Serialize)]
pub struct ImportInspection {
    pub key: SshKeyInfo,
    pub duplicates: Vec<String>,
    pub public_recovered: bool,
    pub revision: String,
    pub warning: Option<String>,
}
fn invalid(message: impl Into<String>) -> AppError {
    AppError::InvalidInput(message.into())
}
fn stage(source: &Path) -> Result<(tempfile::TempDir, SshKeyInfo, String, bool)> {
    if !source.is_absolute() {
        return Err(invalid("Choose an absolute private key path"));
    }
    reject_symlink(source)?;
    let metadata = std::fs::metadata(source)?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(invalid(
            "Private key must be a regular file smaller than 1 MiB",
        ));
    }
    let bytes = zeroize::Zeroizing::new(std::fs::read(source)?);
    let mut digest = Sha256::new();
    digest.update(&*bytes);
    let staging = tempfile::tempdir()?;
    let key = staging.path().join("key");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&key)?;
    set_private_key_permissions(&key)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let companion = PathBuf::from(format!("{}.pub", source.display()));
    reject_symlink(&companion)?;
    let recovered = !companion.exists();
    if !recovered {
        let metadata = std::fs::metadata(&companion)?;
        if !metadata.is_file() || metadata.len() > 65536 {
            return Err(invalid(
                "Public key companion is not a regular file smaller than 64 KiB",
            ));
        }
        let public = std::fs::read(&companion)?;
        digest.update(&public);
        std::fs::write(staging.path().join("key.pub"), public)?;
    }
    let info = super::ssh_keys::parse_key_info(&key)?;
    Ok((staging, info, format!("{:x}", digest.finalize()), recovered))
}
fn duplicates(fingerprint: &str) -> Result<Vec<String>> {
    Ok(super::ssh_keys::list_keys_in_dir(&get_ssh_dir()?)?
        .into_iter()
        .filter(|k| k.error.is_none() && k.fingerprint == fingerprint)
        .map(|k| k.private_path)
        .collect())
}
#[tauri::command(rename_all = "snake_case")]
pub async fn inspect_key_import(source_path: String) -> Result<ImportInspection> {
    tauri::async_runtime::spawn_blocking(move || {
        let (staging, mut key, revision, public_recovered) = stage(Path::new(&source_path))?;
        let duplicates = duplicates(&key.fingerprint)?;
        let warning = if key.has_passphrase == Some(true) && !std::fs::read(staging.path().join("key"))?.starts_with(b"-----BEGIN OPENSSH PRIVATE KEY-----") {
            Some("This encrypted legacy key uses its supplied public companion. Its private/public pairing cannot be verified without unlocking it.".into())
        } else { None };
        key.private_path = source_path.clone(); key.public_path = format!("{source_path}.pub");
        key.name = Path::new(&source_path).file_name().unwrap_or_default().to_string_lossy().into();
        Ok(ImportInspection { key, duplicates, public_recovered, revision, warning })
    }).await.map_err(|e| invalid(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn import_ssh_key(
    source_path: String,
    filename: String,
    revision: String,
) -> Result<SshKeyInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = crate::utils::SSH_WRITE_LOCK
            .lock()
            .map_err(|_| invalid("SSH write lock unavailable"))?;
        validate_key_filename(&filename)?;
        let (staging, info, actual, _) = stage(Path::new(&source_path))?;
        if actual != revision {
            return Err(invalid(
                "Source key or public companion changed; inspect it again before importing",
            ));
        }
        if !duplicates(&info.fingerprint)?.is_empty() {
            return Err(invalid(
                "This key is already in the library; use the existing key instead",
            ));
        }
        let target = get_ssh_dir()?.join(&filename);
        let public_target = target.with_file_name(format!("{filename}.pub"));
        for path in [&target, &public_target] {
            if std::fs::symlink_metadata(path).is_ok() {
                return Err(invalid("Private or public filename already exists"));
            }
        }
        // Restage in the destination filesystem so hard-link publication is exclusive even across volumes.
        let destination_stage = tempfile::tempdir_in(get_ssh_dir()?)?;
        let private = destination_stage.path().join("key");
        let public = destination_stage.path().join("key.pub");
        std::fs::copy(staging.path().join("key"), &private)?;
        set_private_key_permissions(&private)?;
        let encoded = super::ssh_keys::read_key(&staging.path().join("key"))?
            .0
            .to_openssh()
            .map_err(|e| invalid(e.to_string()))?;
        std::fs::write(&public, format!("{encoded}\n"))?;
        set_public_key_permissions(&public)?;
        std::fs::File::open(&private)?.sync_all()?;
        std::fs::File::open(&public)?.sync_all()?;
        std::fs::hard_link(&private, &target)?;
        if let Err(e) = std::fs::hard_link(&public, &public_target) {
            let _ = std::fs::remove_file(&target);
            return Err(e.into());
        }
        std::fs::File::open(get_ssh_dir()?)?.sync_all()?;
        super::ssh_keys::parse_key_info(&target)
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn restore_public_key(key_path: String, expected_fingerprint: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = crate::utils::SSH_WRITE_LOCK
            .lock()
            .map_err(|_| invalid("SSH write lock unavailable"))?;
        validate_key_path(&key_path)?;
        let key = super::ssh_keys::parse_key_info(Path::new(&key_path))?;
        if key.fingerprint != expected_fingerprint {
            return Err(invalid(
                "Key changed; refresh before restoring its public companion",
            ));
        }
        let public = super::ssh_keys::get_public_key_blocking(key_path.clone())?;
        let path = PathBuf::from(format!("{key_path}.pub"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o644);
        }
        let mut file = options.open(path)?;
        writeln!(file, "{public}")?;
        file.sync_all()?;
        Ok(())
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
