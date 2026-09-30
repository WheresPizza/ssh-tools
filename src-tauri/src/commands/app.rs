use crate::error::Result;
use crate::types::AppConfig;
use crate::utils::ssh_dir::get_ssh_dir;
use std::path::PathBuf;

pub(crate) fn get_config_dir() -> Result<PathBuf> {
    if let Some(root) = crate::utils::ssh_dir::workspace_root()? {
        return Ok(root.join("settings"));
    }
    Ok(dirs::config_dir()
        .ok_or_else(|| crate::error::AppError::NotFound("Application config directory".into()))?
        .join("ssh-gui"))
}
fn config_file_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join("config.json"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_app_config() -> Result<AppConfig> {
    let path = config_file_path()?;
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_app_config(config: AppConfig) -> Result<()> {
    let dir = get_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = config_file_path()?;
    let content = serde_json::to_string_pretty(&config)?;
    crate::utils::permissions::atomic_write(&path, &content)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_ssh_dir_path() -> Result<String> {
    let dir = get_ssh_dir()?;
    Ok(dir.to_str().unwrap_or("").to_string())
}

#[derive(serde::Serialize)]
pub struct PermissionIssue {
    path: String,
    current: String,
    expected: String,
}

#[cfg(unix)]
fn permission_targets(dir: &std::path::Path) -> Result<Vec<(PathBuf, u32)>> {
    use crate::utils::permissions::reject_symlink;
    reject_symlink(dir)?;
    let mut targets = vec![(dir.to_path_buf(), 0o700)];
    for path in crate::utils::ssh_dir::regular_files(dir)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let mut parent = path.parent();
        while let Some(folder) = parent {
            if folder == dir {
                break;
            }
            if !targets.iter().any(|(p, _)| p == folder) {
                targets.push((folder.to_path_buf(), 0o700));
            }
            parent = folder.parent();
        }
        if ["config", "known_hosts", "authorized_keys"].contains(&name.as_ref())
            || name.ends_with(".ssh-gui.bak")
            || crate::utils::ssh_dir::validate_key_in_dir(dir, &path).is_ok()
        {
            targets.push((path, 0o600));
        } else if name.ends_with(".pub") {
            targets.push((path, 0o644));
        }
    }
    for (path, _) in if crate::utils::ssh_dir::get_ssh_dir().ok().as_deref() == Some(dir) {
        crate::commands::ssh_config::config_documents()?
    } else {
        Vec::new()
    } {
        if crate::utils::ssh_dir::managed_path(dir, &path).is_ok()
            && !targets.iter().any(|(p, _)| p == &path)
        {
            targets.push((path, 0o600));
        }
    }
    let backups: Vec<_> = targets
        .iter()
        .filter_map(|(path, _)| {
            let backup = crate::utils::permissions::backup_path(path);
            if backup.is_file() && crate::utils::ssh_dir::managed_path(dir, &backup).is_ok() {
                Some((backup, 0o600))
            } else {
                None
            }
        })
        .collect();
    targets.extend(backups);
    Ok(targets)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn audit_permissions() -> Result<Vec<PermissionIssue>> {
    tauri::async_runtime::spawn_blocking(move || audit_permissions_blocking())
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn audit_permissions_blocking() -> Result<Vec<PermissionIssue>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut issues = Vec::new();
        for (path, expected) in permission_targets(&get_ssh_dir()?)? {
            let actual = std::fs::metadata(&path)?.permissions().mode() & 0o777;
            if actual & !expected != 0 {
                let expected = restricted_mode(actual, expected);
                issues.push(PermissionIssue {
                    path: path.to_string_lossy().into(),
                    current: format!("{actual:03o}"),
                    expected: format!("{expected:03o}"),
                });
            }
        }
        Ok(issues)
    }
    #[cfg(not(unix))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn fix_permissions() -> Result<Vec<PermissionIssue>> {
    tauri::async_runtime::spawn_blocking(move || fix_permissions_blocking())
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn fix_permissions_blocking() -> Result<Vec<PermissionIssue>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _guard = crate::utils::SSH_WRITE_LOCK.lock().map_err(|_| {
            crate::error::AppError::InvalidInput("SSH write lock unavailable".into())
        })?;
        for (path, mode) in permission_targets(&get_ssh_dir()?)? {
            crate::utils::permissions::reject_symlink(&path)?;
            let actual = std::fs::metadata(&path)?.permissions().mode() & 0o777;
            if actual & !mode != 0 {
                std::fs::set_permissions(
                    &path,
                    std::fs::Permissions::from_mode(restricted_mode(actual, mode)),
                )?;
            }
        }
    }
    audit_permissions_blocking()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn permission_targets_skip_links_and_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config"), "").unwrap();
        std::fs::write(dir.path().join("notes"), "private notes").unwrap();
        std::os::unix::fs::symlink("notes", dir.path().join("key")).unwrap();
        let targets = permission_targets(dir.path()).unwrap();
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].1, 0o700);
        assert_eq!(targets[1].1, 0o600);
    }
}

#[cfg(unix)]
fn restricted_mode(actual: u32, allowed: u32) -> u32 {
    actual & allowed
}

#[cfg(all(test, unix))]
mod permission_regression_tests {
    use super::*;
    #[test]
    fn repairing_permissions_never_expands_access() {
        assert_eq!(restricted_mode(0o400, 0o600), 0o400);
        assert_eq!(restricted_mode(0o400, 0o644), 0o400);
        assert_eq!(restricted_mode(0o444, 0o600), 0o400);
        assert_eq!(restricted_mode(0o777, 0o700), 0o700);
        assert_eq!(restricted_mode(0o644, 0o600), 0o600);
    }
}

#[derive(serde::Serialize)]
pub struct BackupInfo {
    path: String,
    current_revision: String,
    backup_revision: String,
    preview: String,
}

fn backup_target(value: &str) -> Result<PathBuf> {
    let known = crate::utils::ssh_dir::get_known_hosts_path()?;
    if PathBuf::from(value) == known {
        crate::utils::ssh_dir::managed_path(&get_ssh_dir()?, &known)?;
        Ok(known)
    } else {
        crate::commands::ssh_config::resolve_config_path(value)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_backups() -> Result<Vec<BackupInfo>> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut paths: Vec<_> = crate::commands::ssh_config::config_documents()
            .unwrap_or_default()
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        let main = crate::utils::ssh_dir::get_ssh_config_path()?;
        if !paths.contains(&main) {
            paths.push(main);
        }
        paths.push(crate::utils::ssh_dir::get_known_hosts_path()?);
        let mut backups = Vec::new();
        for path in paths {
            if backup_target(&path.to_string_lossy()).is_err() {
                continue;
            }
            let backup = crate::utils::permissions::backup_path(&path);
            crate::utils::permissions::reject_symlink(&backup)?;
            if !backup.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&backup)?;
            let current = if path.exists() {
                std::fs::read_to_string(&path)?
            } else {
                String::new()
            };
            backups.push(BackupInfo {
                path: path.to_string_lossy().into(),
                current_revision: crate::commands::ssh_config::config_revision(&current),
                backup_revision: crate::commands::ssh_config::config_revision(&content),
                preview: content.chars().take(16000).collect(),
            });
        }
        Ok(backups)
    })
    .await
    .map_err(|e| crate::error::AppError::Process(e.to_string()))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn restore_backup(
    path: String,
    current_revision: String,
    backup_revision: String,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = crate::utils::SSH_WRITE_LOCK.lock().map_err(|_| {
            crate::error::AppError::InvalidInput("SSH write lock unavailable".into())
        })?;
        let target = backup_target(&path)?;
        let backup = crate::utils::permissions::backup_path(&target);
        crate::utils::permissions::reject_symlink(&backup)?;
        let content = std::fs::read_to_string(&backup)?;
        let current = if target.exists() {
            std::fs::read_to_string(&target)?
        } else {
            String::new()
        };
        if crate::commands::ssh_config::config_revision(&current) != current_revision
            || crate::commands::ssh_config::config_revision(&content) != backup_revision
        {
            return Err(crate::error::AppError::InvalidInput(
                "The file or backup changed; review the backup again".into(),
            ));
        }
        crate::utils::permissions::atomic_write(&target, &content)
    })
    .await
    .map_err(|e| crate::error::AppError::Process(e.to_string()))?
}

#[derive(serde::Serialize)]
pub struct WorkspaceInfo {
    ssh_dir: String,
    isolated: bool,
}
#[tauri::command(rename_all = "snake_case")]
pub fn get_workspace() -> Result<WorkspaceInfo> {
    Ok(WorkspaceInfo {
        ssh_dir: get_ssh_dir()?.to_string_lossy().into(),
        isolated: crate::utils::ssh_dir::workspace_root()?.is_some(),
    })
}
