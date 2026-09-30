//! Local key annotations and read-only audit. Never upload or rewrite key material.
use crate::{
    error::{AppError, Result},
    types::SshKeyInfo,
    utils::permissions::{atomic_write, reject_symlink},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KeyMetadata {
    pub tags: Vec<String>,
    pub purpose: String,
    pub note: String,
    pub replace_on: Option<String>,
}
#[derive(Serialize)]
pub struct MetadataSnapshot {
    pub entries: BTreeMap<String, KeyMetadata>,
    pub revision: String,
}
fn invalid(message: impl std::fmt::Display) -> AppError {
    AppError::InvalidInput(message.to_string())
}
fn read_metadata(path: &Path) -> Result<MetadataSnapshot> {
    reject_symlink(path)?;
    if let Some(parent) = path.parent() {
        reject_symlink(parent)?;
    }
    let text = match std::fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_file() || meta.len() > 1024 * 1024 {
                return Err(invalid("Metadata must be a regular file below 1 MiB"));
            }
            std::fs::read_to_string(path)?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let entries = if text.is_empty() {
        BTreeMap::new()
    } else {
        serde_json::from_str(&text)?
    };
    Ok(MetadataSnapshot {
        entries,
        revision: super::ssh_config::config_revision(&text),
    })
}
fn metadata_path() -> Result<std::path::PathBuf> {
    Ok(super::app::get_config_dir()?.join("key-metadata.json"))
}
fn valid_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let y: u32 = s[..4].parse().unwrap_or(0);
    let m: usize = s[5..7].parse().unwrap_or(0);
    let d: u32 = s[8..].parse().unwrap_or(0);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = [
        0,
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    y > 0 && (1..=12).contains(&m) && d > 0 && d <= days[m]
}
fn normalize(mut value: KeyMetadata) -> Result<KeyMetadata> {
    if value.tags.len() > 20 || value.note.len() > 4096 || value.purpose.len() > 256 {
        return Err(invalid(
            "Use at most 20 tags, 256 bytes for purpose and 4096 bytes for notes",
        ));
    }
    let mut tags = Vec::new();
    for raw in value.tags {
        let tag = raw.trim().to_string();
        if tag.is_empty() {
            continue;
        }
        if tag.len() > 64 || tag.chars().any(|c| c.is_control() || c == ',') {
            return Err(invalid(
                "Tags must be at most 64 bytes, without commas or control characters",
            ));
        }
        if !tags
            .iter()
            .any(|t: &String| t.to_lowercase() == tag.to_lowercase())
        {
            tags.push(tag);
        }
    }
    if value.purpose.chars().any(char::is_control)
        || value
            .note
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(invalid("Unsupported control characters in metadata"));
    }
    if value.replace_on.as_deref().is_some_and(|d| !valid_date(d)) {
        return Err(invalid("Replacement date must be a valid YYYY-MM-DD date"));
    }
    value.tags = tags;
    value.purpose = value.purpose.trim().to_string();
    Ok(value)
}
fn save_at(
    path: &Path,
    fingerprint: &str,
    value: KeyMetadata,
    revision: &str,
) -> Result<MetadataSnapshot> {
    let mut snapshot = read_metadata(path)?;
    if snapshot.revision != revision {
        return Err(invalid(
            "Key metadata changed. Reload before saving your edits.",
        ));
    }
    let value = normalize(value)?;
    if value == KeyMetadata::default() {
        snapshot.entries.remove(fingerprint);
    } else {
        snapshot.entries.insert(fingerprint.to_string(), value);
    }
    let text = serde_json::to_string_pretty(&snapshot.entries)?;
    if text.len() > 1024 * 1024 {
        return Err(invalid("Key metadata exceeds 1 MiB"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Missing metadata directory"))?;
    reject_symlink(parent)?;
    std::fs::create_dir_all(parent)?;
    atomic_write(path, &text)?;
    read_metadata(path)
}
#[tauri::command(rename_all = "snake_case")]
pub async fn list_key_metadata() -> Result<MetadataSnapshot> {
    tauri::async_runtime::spawn_blocking(|| read_metadata(&metadata_path()?))
        .await
        .map_err(invalid)?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn save_key_metadata(
    key_path: String,
    expected_fingerprint: String,
    metadata: KeyMetadata,
    revision: String,
) -> Result<MetadataSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = crate::utils::SSH_WRITE_LOCK
            .lock()
            .map_err(|_| invalid("Write lock unavailable"))?;
        crate::utils::ssh_dir::validate_key_path(&key_path)?;
        let key = super::ssh_keys::parse_key_info(Path::new(&key_path))?;
        if key.fingerprint != expected_fingerprint {
            return Err(invalid(
                "The key changed. Refresh the library before saving metadata.",
            ));
        }
        save_at(&metadata_path()?, &key.fingerprint, metadata, &revision)
    })
    .await
    .map_err(invalid)?
}
#[derive(Serialize)]
pub struct KeyFinding {
    pub code: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
}
#[derive(Serialize)]
pub struct KeyAuditEntry {
    pub key_path: String,
    pub fingerprint: String,
    pub findings: Vec<KeyFinding>,
    pub usage: Option<super::repositories::KeyUsage>,
    pub copies: Vec<String>,
}
#[derive(Serialize)]
pub struct KeyAudit {
    pub entries: Vec<KeyAuditEntry>,
    pub warnings: Vec<String>,
    pub checked_at: u64,
}
fn finding(code: &str, severity: &str, title: &str, detail: impl Into<String>) -> KeyFinding {
    KeyFinding {
        code: code.into(),
        severity: severity.into(),
        title: title.into(),
        detail: detail.into(),
    }
}
fn file_findings(key: &SshKeyInfo, copies: &[String]) -> Vec<KeyFinding> {
    let mut found = Vec::new();
    if let Some(error) = &key.error {
        found.push(finding(
            "unreadable",
            "error",
            "Key or public companion cannot be read",
            error,
        ));
    }
    if key.has_passphrase == Some(false) {
        found.push(finding(
            "no-passphrase",
            "warning",
            "No passphrase",
            "The private key file is not encrypted with a passphrase.",
        ));
    }
    if !key.public_key_exists && key.error.is_none() {
        found.push(finding("missing-public", "info", "Public companion missing", "The public part can be recovered for supported formats; the private key is still present."));
    }
    if copies.len() > 1 {
        found.push(finding(
            "copies",
            "info",
            "Multiple files share this identity",
            format!(
                "{} files have the same fingerprint. Deleting one copy does not revoke the others.",
                copies.len()
            ),
        ));
    }
    // Legacy unencrypted parsing can succeed even when a separately supplied public file is wrong.
    if key.error.is_none() && key.public_key_exists {
        let result = (|| -> Result<()> {
            let path = Path::new(&key.public_path);
            reject_symlink(path)?;
            if std::fs::metadata(path)?.len() > 64 * 1024 {
                return Err(invalid("Public companion is too large"));
            }
            let public = ssh_key::PublicKey::from_openssh(&std::fs::read_to_string(path)?)
                .map_err(invalid)?;
            if public.fingerprint(ssh_key::HashAlg::Sha256).to_string() != key.fingerprint {
                return Err(invalid(
                    "Public companion fingerprint does not match the private key",
                ));
            }
            Ok(())
        })();
        if let Err(e) = result {
            found.push(finding(
                "public-mismatch",
                "error",
                "Public companion is invalid or mismatched",
                e.to_string(),
            ));
        }
        if key.has_passphrase == Some(true) {
            if let Ok(mut file) = std::fs::File::open(&key.private_path) {
                use std::io::Read;
                let mut header = zeroize::Zeroizing::new([0u8; 35]);
                if file.read_exact(&mut header[..]).is_ok()
                    && !header.starts_with(b"-----BEGIN OPENSSH PRIVATE KEY-----")
                {
                    found.push(finding("legacy-pair", "info", "Encrypted legacy pair is not verified", "Its public companion supplies the fingerprint; pairing cannot be proven without unlocking."));
                }
            }
        }
    }
    found
}
fn audit_blocking() -> Result<KeyAudit> {
    let keys = super::ssh_keys::list_keys_in_dir(&crate::utils::ssh_dir::get_ssh_dir()?)?;
    let mut warnings = Vec::new();
    let paths = keys
        .iter()
        .map(|key| key.private_path.clone())
        .collect::<Vec<_>>();
    let usages = match super::repositories::usage_for_keys(&paths) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("Local relationship checks incomplete: {error}"));
            None
        }
    };
    let permissions = match super::app::audit_permissions_blocking() {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("Permission checks incomplete: {error}"));
            Vec::new()
        }
    };
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in &keys {
        if key.error.is_none() && !key.fingerprint.is_empty() {
            identities
                .entry(key.fingerprint.clone())
                .or_default()
                .push(key.private_path.clone());
        }
    }
    let mut entries = Vec::new();
    for key in &keys {
        let copies = identities
            .get(&key.fingerprint)
            .cloned()
            .unwrap_or_default();
        let mut findings = file_findings(key, &copies);
        for issue in &permissions {
            if Path::new(&key.private_path).starts_with(&issue.path)
                || key.public_path == issue.path
            {
                findings.push(finding(
                    "permissions",
                    "warning",
                    "Excess file or directory permissions",
                    format!(
                        "{}: {} → {}. Review the permissions panel before repairing.",
                        issue.path, issue.current, issue.expected
                    ),
                ));
            }
        }
        let usage = usages
            .as_ref()
            .and_then(|map| map.get(&key.private_path))
            .cloned();
        if let Some(usage) = &usage {
            if usage.hosts.is_empty() && usage.profiles.is_empty() && usage.repositories.is_empty()
            {
                findings.push(finding("no-local-links", "info", "No local links found",
                    "No references were found in the checked configuration and selected folders. This does not mean the key is unused on remote systems or in unscanned projects."));
            }
        }
        entries.push(KeyAuditEntry {
            key_path: key.private_path.clone(),
            fingerprint: key.fingerprint.clone(),
            findings,
            usage,
            copies,
        });
    }
    Ok(KeyAudit {
        entries,
        warnings,
        checked_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}
#[tauri::command(rename_all = "snake_case")]
pub async fn audit_ssh_keys() -> Result<KeyAudit> {
    tauri::async_runtime::spawn_blocking(audit_blocking)
        .await
        .map_err(invalid)?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_is_shared_by_fingerprint_and_rejects_stale_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.json");
        let initial = read_metadata(&path).unwrap();
        let data = KeyMetadata {
            tags: vec![" work ".into(), "WORK".into()],
            note: "Keep until rotation".into(),
            ..Default::default()
        };
        let saved = save_at(&path, "SHA256:identity", data.clone(), &initial.revision).unwrap();
        assert_eq!(saved.entries["SHA256:identity"].tags, vec!["work"]);
        assert!(save_at(&path, "SHA256:other", data, &initial.revision).is_err());
        assert_eq!(read_metadata(&path).unwrap().entries, saved.entries);
        assert!(save_at(
            &path,
            "SHA256:identity",
            KeyMetadata::default(),
            &saved.revision
        )
        .unwrap()
        .entries
        .is_empty());
    }
    #[test]
    fn metadata_validation_and_corrupt_files() {
        assert!(valid_date("2028-02-29"));
        assert!(!valid_date("2027-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(normalize(KeyMetadata {
            tags: vec!["bad\nvalue".into()],
            ..Default::default()
        })
        .is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta");
        std::fs::write(&path, "not json").unwrap();
        assert!(read_metadata(&path).is_err());
        assert!(save_at(&path, "SHA256:test", KeyMetadata::default(), "stale").is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "not json");
    }
    #[test]
    fn audit_distinguishes_missing_and_invalid_public_companions() {
        let dir = tempfile::tempdir().unwrap();
        let public_path = dir.path().join("key.pub");
        let mut key = SshKeyInfo {
            name: "fixture".into(),
            private_path: dir.path().join("key").to_string_lossy().into(),
            public_path: public_path.to_string_lossy().into(),
            public_key_exists: false,
            algorithm: "ed25519".into(),
            bits: Some(256),
            fingerprint: "SHA256:fixture".into(),
            comment: None,
            has_passphrase: Some(false),
            error: None,
            created_at: None,
        };
        assert!(file_findings(&key, &[])
            .iter()
            .any(|f| f.code == "missing-public"));
        std::fs::write(&public_path, "invalid public companion").unwrap();
        key.public_key_exists = true;
        assert!(file_findings(&key, &[])
            .iter()
            .any(|f| f.code == "public-mismatch"));
        assert_eq!(
            std::fs::read_to_string(public_path).unwrap(),
            "invalid public companion"
        );
        key.has_passphrase = None;
        key.error = Some("Unreadable".into());
        let findings = file_findings(&key, &[]);
        assert!(findings.iter().any(|f| f.code == "unreadable"));
        assert!(!findings.iter().any(|f| f.code == "no-passphrase"));
    }
    #[cfg(unix)]
    #[test]
    fn metadata_is_private_and_rejects_symlinks() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta");
        let snapshot = read_metadata(&path).unwrap();
        save_at(
            &path,
            "SHA256:test",
            KeyMetadata::default(),
            &snapshot.revision,
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let link = dir.path().join("link");
        symlink(path, &link).unwrap();
        assert!(read_metadata(&link).is_err());
    }
}
