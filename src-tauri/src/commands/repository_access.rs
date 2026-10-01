//! Explicit, reviewed remote URL changes. Never execute repository commands or contact a server.
use super::{profiles, repositories, ssh_config::config_revision};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
fn invalid(s: impl Into<String>) -> AppError {
    AppError::InvalidInput(s.into())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AccessPlan {
    pub repository: String,
    pub remote: String,
    pub before: String,
    pub after: String,
    pub key_path: String,
    pub fingerprint: String,
    pub alias: String,
    pub revision: String,
    pub ssh_revision: String,
    pub warnings: Vec<String>,
}
fn config_path(repository: &str) -> Result<PathBuf> {
    let root = Path::new(repository);
    if !root.is_absolute() || !root.is_dir() {
        return Err(invalid("Choose an existing repository folder"));
    }
    let dotgit = root.join(".git");
    crate::utils::permissions::reject_symlink(&dotgit)?;
    if !dotgit.is_dir() {
        return Err(invalid("Access setup supports regular repositories. Linked worktrees and submodules remain read-only."));
    }
    let path = dotgit.join("config");
    repositories::bounded_text(&path)?;
    Ok(path)
}
// Deliberately accept only ordinary repository paths. Credentials, queries and unusual
// transports need manual review, rather than guessing or exposing embedded secrets.
fn remote_path(url: &str, profile: &profiles::GitProfile) -> Result<String> {
    let (host, path) = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git+ssh://"))
    {
        let (authority, path) = rest
            .split_once('/')
            .ok_or_else(|| invalid("Remote URL has no repository path"))?;
        let authority = if url.starts_with("https://") {
            if authority.contains('@') {
                return Err(invalid("Credential-bearing URLs require manual setup"));
            }
            authority
        } else {
            if let Some((user, _)) = authority.rsplit_once('@') {
                if user != profile.user {
                    return Err(invalid("Remote SSH user differs from this profile"));
                }
            }
            authority.rsplit('@').next().unwrap_or("")
        };
        let host = if let Some((host, port)) = authority.split_once(':') {
            if url.starts_with("https://") || port.parse::<u16>().ok() != Some(profile.port) {
                return Err(invalid("Remote port differs from this profile"));
            }
            host
        } else {
            authority
        };
        (host, path)
    } else {
        let (authority, path) = url
            .split_once(':')
            .ok_or_else(|| invalid("Only SSH and HTTPS Git remotes can be configured"))?;
        if let Some((user, _)) = authority.rsplit_once('@') {
            if user != profile.user {
                return Err(invalid("Remote SSH user differs from this profile"));
            }
        }
        (authority.rsplit('@').next().unwrap_or(""), path)
    };
    if !host.eq_ignore_ascii_case(&profile.hostname) && !host.eq_ignore_ascii_case(&profile.alias) {
        return Err(invalid("The remote host differs from this profile. Review its host or existing alias in Git Profiles."));
    }
    if path.is_empty()
        || path.starts_with('-')
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(invalid("This remote path requires manual setup"));
    }
    Ok(path.to_string())
}
fn plan(
    repository: String,
    remote: String,
    key_path: String,
    fingerprint: String,
    alias: String,
) -> Result<AccessPlan> {
    crate::utils::ssh_dir::validate_key_path(&key_path)?;
    let key = super::ssh_keys::parse_key_info(Path::new(&key_path))?;
    if key.fingerprint != fingerprint {
        return Err(invalid("Key changed; reopen its details"));
    }
    if remote.is_empty()
        || !remote
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(invalid("This remote name requires manual setup"));
    }
    let config = config_path(&repository)?;
    let content = repositories::bounded_text(&config)?;
    let entries = repositories::git_config(&config)?;
    if entries.iter().any(|(k, _)| {
        k.eq_ignore_ascii_case("core.sshcommand")
            || k.starts_with("include.")
            || k.starts_with("includeif.")
            || k.starts_with("url.")
            || k.eq_ignore_ascii_case("extensions.worktreeconfig")
    }) {
        return Err(invalid("Repository has SSH overrides, includes, URL rewrites or worktree configuration. Review these manually before setting access."));
    }
    let urls: Vec<_> = entries
        .iter()
        .filter(|(k, _)| k == &format!("remote.{remote}.url"))
        .map(|(_, v)| v)
        .collect();
    if urls.len() != 1
        || entries
            .iter()
            .any(|(k, _)| k == &format!("remote.{remote}.pushurl"))
    {
        return Err(invalid(
            "Choose a remote with exactly one URL and no separate push URL",
        ));
    }
    let snapshot = profiles::list_blocking()?;
    let profile = snapshot
        .profiles
        .iter()
        .find(|p| p.alias == alias && p.key_path == key_path)
        .ok_or_else(|| invalid("Choose a Git profile assigned to this key"))?;
    let path = remote_path(urls[0], profile)?;
    let after = format!("{}@{}:{}", profile.user, profile.alias, path);
    Ok(AccessPlan { repository, remote, before: urls[0].clone(), after, key_path, fingerprint, alias, revision: config_revision(&content), ssh_revision: snapshot.revision,
        warnings: vec!["This changes the remote URL for fetch and push. Global Git configuration, environment variables, SSH rules and agent identities may still change authentication. No connection is tested.".into()] })
}
#[tauri::command(rename_all = "snake_case")]
pub async fn preview_repository_access(
    repository: String,
    remote: String,
    key_path: String,
    fingerprint: String,
    alias: String,
) -> Result<AccessPlan> {
    tauri::async_runtime::spawn_blocking(move || {
        plan(repository, remote, key_path, fingerprint, alias)
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
fn apply(expected: AccessPlan) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| invalid("Write lock unavailable"))?;
    let path = config_path(&expected.repository)?;
    // Respect Git's conventional lock, including other Git processes.
    let lock = path.with_extension("lock");
    let lock_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|_| invalid("Git config is locked; finish the other operation and retry"))?;
    let result = (|| {
        let actual = plan(
            expected.repository.clone(),
            expected.remote.clone(),
            expected.key_path.clone(),
            expected.fingerprint.clone(),
            expected.alias.clone(),
        )?;
        if actual != expected {
            return Err(invalid(
                "Configuration changed. Preview access again before applying.",
            ));
        }
        let content = repositories::bounded_text(&path)?;
        let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        temp.write_all(content.as_bytes())?;
        let status = Command::new("git")
            .args(["config", "--no-includes", "--file"])
            .arg(temp.path())
            .arg("--replace-all")
            .arg(format!("remote.{}.url", expected.remote))
            .arg(&expected.after)
            .output()?;
        if !status.status.success() {
            return Err(invalid("Cannot prepare the remote URL change"));
        }
        let updated = repositories::bounded_text(temp.path())?;
        if repositories::bounded_text(&path)? != content {
            return Err(invalid("Git config changed; preview again"));
        }
        crate::utils::permissions::atomic_write(&path, &updated)
    })();
    drop(lock_file);
    let _ = std::fs::remove_file(lock);
    result
}
#[tauri::command(rename_all = "snake_case")]
pub async fn apply_repository_access(plan: AccessPlan) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || apply(plan))
        .await
        .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn inspect_repository(repository: String) -> Result<repositories::RepositoryScan> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = Path::new(&repository);
        if !path.is_absolute() || !path.join(".git").exists() {
            return Err(invalid("Choose a Git repository folder"));
        }
        let mut hosts = Vec::new();
        for (_, content) in super::ssh_config::config_documents()? {
            hosts.extend(super::ssh_config::parse_ssh_config(&format!(
                "Host *\n{content}"
            )));
        }
        repositories::scan_exact(
            repository,
            &hosts,
            &profiles::list_blocking()?.profiles,
            &crate::utils::ssh_dir::get_ssh_dir()?,
        )
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> profiles::GitProfile {
        profiles::GitProfile {
            name: "Work".into(),
            account: "me".into(),
            alias: "github-work".into(),
            hostname: "github.com".into(),
            user: "git".into(),
            port: 22,
            key_path: "/key".into(),
        }
    }
    #[test]
    fn remote_conversion_preserves_path_and_refuses_conflicting_destinations() {
        let p = profile();
        for url in [
            "git@github.com:team/repo.git",
            "https://github.com/team/repo.git",
            "ssh://git@github.com:22/team/repo.git",
            "git@github-work:team/repo.git",
        ] {
            assert_eq!(remote_path(url, &p).unwrap(), "team/repo.git");
        }
        for url in [
            "https://token@github.com/team/repo",
            "git@other:team/repo",
            "alice@github.com:team/repo",
            "ssh://git@github.com:2222/team/repo",
            "ext::evil",
            "git@github.com:../repo",
            "git@github.com:repo?secret",
            "file:///tmp/repo",
        ] {
            assert!(remote_path(url, &p).is_err(), "{url}");
        }
    }
    #[test]
    fn setup_refuses_worktrees_and_symlink_configs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".git"), "gitdir: elsewhere").unwrap();
        assert!(config_path(dir.path().to_str().unwrap()).is_err());
        std::fs::remove_file(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp/elsewhere", dir.path().join(".git/config")).unwrap();
            assert!(config_path(dir.path().to_str().unwrap()).is_err());
        }
    }
}
