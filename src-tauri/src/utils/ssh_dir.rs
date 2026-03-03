use std::path::PathBuf;
use crate::error::{AppError, Result};

pub fn get_ssh_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::NotFound("Home directory not found".to_string()))?;
    let ssh_dir = home.join(".ssh");
    if !ssh_dir.exists() {
        std::fs::create_dir_all(&ssh_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(ssh_dir)
}

pub fn get_ssh_config_path() -> Result<PathBuf> {
    Ok(get_ssh_dir()?.join("config"))
}

pub fn get_known_hosts_path() -> Result<PathBuf> {
    Ok(get_ssh_dir()?.join("known_hosts"))
}
