use std::path::Path;
use crate::error::{AppError, Result};

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

#[cfg(unix)]
pub fn set_config_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
pub fn set_private_key_permissions(_path: &Path) -> Result<()> { Ok(()) }
#[cfg(not(unix))]
pub fn set_public_key_permissions(_path: &Path) -> Result<()> { Ok(()) }
#[cfg(not(unix))]
pub fn set_config_permissions(_path: &Path) -> Result<()> { Ok(()) }

/// Atomic write: write to temp file, set secure permissions, then rename
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        AppError::Io(e)
    })?;
    Ok(())
}
