use crate::error::{AppError, Result};
use std::path::Path;

#[cfg(unix)]
pub fn set_private_key_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(unix)]
pub fn set_public_key_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))?;
    Ok(())
}

#[cfg(not(unix))]
pub fn set_private_key_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
#[cfg(not(unix))]
pub fn set_public_key_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// Create a private, unpredictable temporary file beside the target, then rename.
/// Refuse symlinks rather than replacing a linked SSH configuration unexpectedly.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    reject_symlink(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Missing parent directory".into()))?;
    reject_symlink(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    if path.exists() {
        let backup_path = backup_path(path);
        reject_symlink(&backup_path)?;
        let mut backup = tempfile::NamedTempFile::new_in(parent)?;
        std::io::copy(&mut std::fs::File::open(path)?, &mut backup)?;
        backup.as_file().sync_all()?;
        backup
            .persist(backup_path)
            .map_err(|e| AppError::Io(e.error))?;
    }
    #[cfg(unix)]
    if path.exists() {
        use std::os::unix::fs::PermissionsExt;
        let previous = std::fs::metadata(path)?.permissions().mode() & 0o600;
        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(previous))?;
    }
    temp.persist(path).map_err(|e| AppError::Io(e.error))?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn backup_path(path: &Path) -> std::path::PathBuf {
    path.with_file_name(format!(
        ".{}.ssh-gui.bak",
        path.file_name().unwrap_or_default().to_string_lossy()
    ))
}

pub fn reject_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(AppError::InvalidInput(format!(
            "Symbolic links are read-only: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_write_replaces_content_privately() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config");
        atomic_write(&path, "old").unwrap();
        atomic_write(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".config.ssh-gui.bak")).unwrap(),
            "old"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn atomic_write_refuses_link_without_touching_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, "original").unwrap();
        let link = dir.path().join("config");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(atomic_write(&link, "changed").is_err());
        assert_eq!(std::fs::read_to_string(target).unwrap(), "original");
    }
}
