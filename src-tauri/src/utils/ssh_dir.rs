use crate::error::{AppError, Result};
use std::path::PathBuf;

/// An explicit workspace keeps development and acceptance checks away from real credentials.
pub fn workspace_root() -> Result<Option<PathBuf>> {
    match std::env::var_os("SSH_GUI_WORKSPACE") {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(AppError::InvalidInput(
                    "SSH_GUI_WORKSPACE must be absolute".into(),
                ));
            }
            std::fs::create_dir_all(&path)?;
            super::permissions::reject_symlink(&path)?;
            Ok(Some(path.canonicalize()?))
        }
        None => Ok(None),
    }
}

pub fn get_ssh_dir() -> Result<PathBuf> {
    let ssh_dir = match workspace_root()? {
        Some(root) => root.join("ssh"),
        None => dirs::home_dir()
            .ok_or_else(|| AppError::NotFound("Home directory not found".into()))?
            .join(".ssh"),
    };
    super::permissions::reject_symlink(&ssh_dir)?;
    if !ssh_dir.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&ssh_dir)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(&ssh_dir)?;
    }
    Ok(ssh_dir.canonicalize()?)
}

/// Validate every component below the managed root before reading or mutating a path.
pub fn managed_path(root: &std::path::Path, path: &std::path::Path) -> Result<PathBuf> {
    use std::path::Component;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| AppError::InvalidInput("Path is outside the SSH directory".into()))?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AppError::InvalidInput(
            "Expected a file beneath the SSH directory".into(),
        ));
    }
    super::permissions::reject_symlink(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        super::permissions::reject_symlink(&current)?;
    }
    Ok(current)
}

pub fn regular_files(root: &std::path::Path) -> Result<Vec<PathBuf>> {
    fn visit(dir: &std::path::Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        if depth > 32 {
            return Err(AppError::InvalidInput(
                "SSH directory nesting exceeds 32 levels".into(),
            ));
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                visit(&entry.path(), files, depth + 1)?;
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
        Ok(())
    }
    super::permissions::reject_symlink(root)?;
    let mut files = Vec::new();
    visit(root, &mut files, 0)?;
    files.sort();
    Ok(files)
}

pub fn get_ssh_config_path() -> Result<PathBuf> {
    Ok(get_ssh_dir()?.join("config"))
}

pub fn get_known_hosts_path() -> Result<PathBuf> {
    Ok(get_ssh_dir()?.join("known_hosts"))
}

/// Keys operated on by the GUI must be regular files beneath the SSH directory.
/// Resolve neither symlinks nor arbitrary frontend-supplied paths for mutations.
pub fn validate_key_path(value: &str) -> Result<PathBuf> {
    let dir = get_ssh_dir()?;
    validate_key_in_dir(&dir, PathBuf::from(value).as_path())
}

pub fn validate_key_in_dir(dir: &std::path::Path, path: &std::path::Path) -> Result<PathBuf> {
    use std::io::Read;
    managed_path(dir, path)?;
    let mut file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(AppError::InvalidInput("Key must be a regular file".into()));
    }
    let mut header = [0; 100];
    let n = file.read(&mut header)?;
    let header = String::from_utf8_lossy(&header[..n]);
    if !header.starts_with("-----BEGIN ")
        || !header
            .lines()
            .next()
            .unwrap_or("")
            .contains("PRIVATE KEY-----")
    {
        return Err(AppError::InvalidInput(
            "File is not a supported private SSH key".into(),
        ));
    }
    Ok(path.to_path_buf())
}

pub fn validate_key_filename(name: &str) -> Result<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with(".pub")
        || name
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !"_-.".contains(c))
        || [
            "config",
            "known_hosts",
            "authorized_keys",
            "environment",
            "rc",
        ]
        .contains(&name)
    {
        return Err(AppError::InvalidInput("Use a key filename containing letters, digits, dash, underscore or dot (not an SSH configuration filename)".into()));
    }
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn rejects_reserved_and_path_filenames() {
        for name in [
            "",
            "config",
            "known_hosts",
            "authorized_keys",
            "../key",
            ".key",
            "key.pub",
            "key/name",
            "key\\name",
        ] {
            assert!(validate_key_filename(name).is_err(), "{name}");
        }
        assert!(validate_key_filename("github-work_ed25519").is_ok());
    }
    #[test]
    fn refuses_nonkeys_and_outside_paths() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(&config, "Host example").unwrap();
        assert!(validate_key_in_dir(dir.path(), &config).is_err());
        assert!(validate_key_in_dir(dir.path(), std::path::Path::new("/etc/passwd")).is_err());
    }
}
