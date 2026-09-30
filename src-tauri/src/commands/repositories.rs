//! Read-only repository discovery. Never run hooks, repository scripts or SSH commands.
use crate::{
    error::{AppError, Result},
    types::SshHost,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RepositoryRemote {
    pub repository: String,
    pub name: String,
    pub direction: String,
    pub url: String,
    pub ssh_host: Option<String>,
    pub ssh_user: Option<String>,
    pub ssh_port: Option<u16>,
    pub profiles: Vec<String>,
    pub key_paths: Vec<String>,
    pub warnings: Vec<String>,
}
#[derive(Serialize)]
pub struct RepositoryScan {
    pub roots: Vec<String>,
    pub remotes: Vec<RepositoryRemote>,
    pub warnings: Vec<String>,
    pub repositories: usize,
}
#[derive(Serialize)]
pub struct KeyUsage {
    pub hosts: Vec<String>,
    pub profiles: Vec<String>,
    pub repositories: Vec<String>,
    pub roots: Vec<String>,
    pub warnings: Vec<String>,
}
fn invalid(s: impl Into<String>) -> AppError {
    AppError::InvalidInput(s.into())
}
fn roots_path() -> Result<PathBuf> {
    Ok(super::app::get_config_dir()?.join("repository-roots.json"))
}
fn roots_blocking() -> Result<Vec<String>> {
    let path = roots_path()?;
    crate::utils::permissions::reject_symlink(&path)?;
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}
#[tauri::command(rename_all = "snake_case")]
pub async fn set_repository_roots(roots: Vec<String>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        if roots.len() > 32 {
            return Err(invalid("Choose at most 32 project folders"));
        }
        let mut normalized = Vec::new();
        for value in roots {
            let path = PathBuf::from(value);
            if !path.is_absolute() || !path.is_dir() {
                return Err(invalid("Select an existing absolute project folder"));
            }
            let path = path.canonicalize()?.to_string_lossy().to_string();
            if !normalized.contains(&path) {
                normalized.push(path);
            }
        }
        let _guard = crate::utils::SSH_WRITE_LOCK
            .lock()
            .map_err(|_| invalid("Settings lock unavailable"))?;
        let path = roots_path()?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        crate::utils::permissions::atomic_write(&path, &serde_json::to_string_pretty(&normalized)?)
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}
fn bounded_text(path: &Path) -> Result<String> {
    crate::utils::permissions::reject_symlink(path)?;
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() || meta.len() > 2 * 1024 * 1024 {
        return Err(invalid(
            "Git metadata must be a regular file smaller than 2 MiB",
        ));
    }
    Ok(std::fs::read_to_string(path)?)
}
fn git_config(path: &Path) -> Result<Vec<(String, String)>> {
    bounded_text(path)?;
    // --file and --no-includes prevent repository configuration from causing further reads.
    let output = Command::new("git")
        .args(["config", "--no-includes", "--file"])
        .arg(path)
        .args(["--null", "--list"])
        .output()?;
    if !output.status.success() {
        return Err(invalid(format!(
            "Cannot parse Git config: {}",
            path.display()
        )));
    }
    let data = String::from_utf8(output.stdout).map_err(|_| invalid("Non-UTF8 Git config"))?;
    Ok(data
        .split('\0')
        .filter_map(|entry| entry.split_once('\n'))
        .map(|(k, v)| (k.into(), v.into()))
        .collect())
}
fn config_for_repository(repository: &Path) -> Result<Vec<(String, String)>> {
    let dotgit = repository.join(".git");
    crate::utils::permissions::reject_symlink(&dotgit)?;
    let git_dir = if dotgit.is_dir() {
        dotgit
    } else {
        let text = bounded_text(&dotgit)?;
        let target = text
            .trim()
            .strip_prefix("gitdir: ")
            .ok_or_else(|| invalid("Invalid .git worktree pointer"))?;
        repository.join(target).canonicalize()?
    };
    let common_path = git_dir.join("commondir");
    let common = if common_path.exists() {
        git_dir
            .join(bounded_text(&common_path)?.trim())
            .canonicalize()?
    } else {
        git_dir.clone()
    };
    let mut config = git_config(&common.join("config"))?;
    if config
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("extensions.worktreeconfig") && v == "true")
        && git_dir.join("config.worktree").exists()
    {
        config.extend(git_config(&git_dir.join("config.worktree"))?);
    }
    Ok(config)
}
/// Extract only the SSH destination. Never execute or expand a remote URL.
pub(crate) fn remote_host(value: &str) -> Option<String> {
    if let Some(rest) = value
        .strip_prefix("ssh://")
        .or_else(|| value.strip_prefix("git+ssh://"))
    {
        let authority = rest.split('/').next()?;
        let host = authority.rsplit('@').next()?;
        if let Some(host) = host.strip_prefix('[') {
            return Some(host.split(']').next()?.into());
        }
        return Some(host.split(':').next()?.into());
    }
    if value.contains("::") && !value.contains('[') {
        return None;
    }
    if value.contains("://") || value.starts_with('/') || value.starts_with('.') {
        return None;
    }
    let separator = if let Some(open) = value.find('[') {
        let close = value[open..].find(']')? + open;
        if value.as_bytes().get(close + 1) != Some(&b':') {
            return None;
        }
        close + 1
    } else {
        value.find(':')?
    };
    let host = value[..separator]
        .rsplit('@')
        .next()?
        .trim_start_matches('[')
        .trim_end_matches(']');
    if host.is_empty() || host.contains('/') || host.chars().any(char::is_whitespace) {
        None
    } else {
        Some(host.into())
    }
}
fn remote_overrides(value: &str) -> (Option<String>, Option<u16>) {
    let is_url = value.starts_with("ssh://") || value.starts_with("git+ssh://");
    let authority = if is_url {
        value
            .split_once("://")
            .unwrap()
            .1
            .split('/')
            .next()
            .unwrap_or("")
    } else {
        value.split(':').next().unwrap_or("")
    };
    let user = authority
        .rsplit_once('@')
        .map(|(credentials, _)| credentials.split(':').next().unwrap_or("").to_string());
    let port = if is_url {
        let host = authority.rsplit('@').next().unwrap_or("");
        if host.starts_with('[') {
            host.split_once("]:")
                .and_then(|(_, port)| port.parse::<u16>().ok())
        } else {
            host.rsplit_once(':')
                .and_then(|(_, port)| port.parse::<u16>().ok())
        }
    } else {
        None
    };
    (user, port)
}
fn redacted_url(value: &str) -> String {
    if let Some((scheme, rest)) = value.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        if let Some((credentials, host)) = authority.rsplit_once('@') {
            if credentials.contains(':') || !matches!(scheme, "ssh" | "git+ssh") {
                return format!("{scheme}://[credentials hidden]@{host}/{path}");
            }
        }
    }
    value.to_string()
}
fn matches_host(patterns: &str, hostname: &str) -> bool {
    let mut matched = false;
    for pattern in patterns.split_whitespace() {
        let (negated, pattern) = pattern
            .strip_prefix('!')
            .map(|p| (true, p))
            .unwrap_or((false, pattern));
        if glob::Pattern::new(&pattern.to_ascii_lowercase())
            .is_ok_and(|p| p.matches(&hostname.to_ascii_lowercase()))
        {
            if negated {
                return false;
            }
            matched = true;
        }
    }
    matched
}
fn identity_path(value: &str, ssh_dir: &Path) -> Option<String> {
    let parts = shlex::split(value)?;
    if parts.len() != 1 || parts[0] == "none" || parts[0].contains('%') || parts[0].contains('$') {
        return None;
    }
    let raw = &parts[0];
    let path = if let Some(tail) = raw.strip_prefix("~/.ssh/") {
        ssh_dir.join(tail)
    } else if let Some(tail) = raw.strip_prefix("~/") {
        dirs::home_dir()?.join(tail)
    } else if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        return None;
    };
    Some(path.to_string_lossy().into())
}
fn discover(
    path: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    repositories: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) {
    if depth > 12 || visited.len() >= 20000 || repositories.len() >= 2000 {
        warnings.push(format!("Scan limit reached at {}", path.display()));
        return;
    }
    if path.is_symlink() || !visited.insert(path.to_path_buf()) {
        return;
    }
    if path.join(".git").exists() {
        repositories.push(path.to_path_buf());
    }
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            warnings.push(format!("Cannot scan {}: {e}", path.display()));
            return;
        }
    };
    for entry in entries {
        if visited.len() >= 20000 || repositories.len() >= 2000 {
            warnings.push(format!("Scan limit reached at {}", path.display()));
            return;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warnings.push(e.to_string());
                continue;
            }
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || ["node_modules", "target", "vendor", "dist", "build"].contains(&name.as_ref())
        {
            continue;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            discover(&entry.path(), depth + 1, visited, repositories, warnings);
        }
    }
}
fn scan(
    roots: Vec<String>,
    hosts: &[SshHost],
    profiles: &[super::profiles::GitProfile],
    ssh_dir: &Path,
) -> Result<RepositoryScan> {
    let mut warnings = Vec::new();
    let mut repositories = Vec::new();
    let mut visited = HashSet::new();
    for root in &roots {
        discover(
            Path::new(root),
            0,
            &mut visited,
            &mut repositories,
            &mut warnings,
        );
    }
    repositories.sort();
    repositories.dedup();
    let count = repositories.len();
    let mut remotes = Vec::new();
    for repository in repositories {
        let entries = match config_for_repository(&repository) {
            Ok(e) => e,
            Err(e) => {
                warnings.push(format!("{}: {e}", repository.display()));
                continue;
            }
        };
        let mut repo_warnings = Vec::new();
        if entries
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("core.sshcommand"))
        {
            repo_warnings.push("Repository overrides core.sshCommand; key mapping may differ. The command was not executed.".into());
        }
        if entries
            .iter()
            .any(|(k, _)| k.starts_with("include.") || k.starts_with("includeif."))
        {
            repo_warnings
                .push("Git config includes were not evaluated; mapping may be incomplete.".into());
        }
        for (key, url) in &entries {
            let Some(rest) = key.strip_prefix("remote.") else {
                continue;
            };
            let Some((name, field)) = rest.rsplit_once('.') else {
                continue;
            };
            if field != "url" && field != "pushurl" {
                continue;
            }
            let destination = remote_host(url);
            let (ssh_user, ssh_port) = if destination.is_some() {
                remote_overrides(url)
            } else {
                (None, None)
            };
            let linked: Vec<_> = profiles
                .iter()
                .filter(|p| {
                    destination
                        .as_deref()
                        .is_some_and(|host| p.alias.eq_ignore_ascii_case(host))
                })
                .map(|p| p.name.clone())
                .collect();
            let mut key_paths = Vec::new();
            let mut explicit_identity = false;
            if let Some(hostname) = &destination {
                for host in hosts.iter().filter(|h| matches_host(&h.alias, hostname)) {
                    explicit_identity |= !host.identity_file.is_empty();
                    for id in &host.identity_file {
                        if let Some(path) = identity_path(id, ssh_dir) {
                            key_paths.push(path);
                        }
                    }
                }
            }
            if destination.is_some() && !explicit_identity {
                for name in [
                    "id_ed25519",
                    "id_ed25519_sk",
                    "id_rsa",
                    "id_ecdsa",
                    "id_ecdsa_sk",
                    "id_dsa",
                ] {
                    let candidate = ssh_dir.join(name);
                    if candidate.is_file() && !candidate.is_symlink() {
                        key_paths.push(candidate.to_string_lossy().into());
                    }
                }
            }
            key_paths.sort();
            key_paths.dedup();
            remotes.push(RepositoryRemote {
                repository: repository.to_string_lossy().into(),
                name: name.into(),
                direction: if field == "pushurl" {
                    "push".into()
                } else {
                    "fetch/push".into()
                },
                url: redacted_url(url),
                ssh_host: destination,
                ssh_user,
                ssh_port,
                profiles: linked,
                key_paths,
                warnings: repo_warnings.clone(),
            });
        }
    }
    Ok(RepositoryScan {
        roots,
        remotes,
        warnings,
        repositories: count,
    })
}
pub(crate) fn scan_blocking() -> Result<RepositoryScan> {
    let mut hosts = Vec::new();
    for (_, content) in super::ssh_config::config_documents()? {
        hosts.extend(super::ssh_config::parse_ssh_config(&format!(
            "Host *\n{content}"
        )));
    }
    let profiles = super::profiles::list_blocking()?.profiles;
    scan(
        roots_blocking()?,
        &hosts,
        &profiles,
        &crate::utils::ssh_dir::get_ssh_dir()?,
    )
}
#[tauri::command(rename_all = "snake_case")]
pub async fn scan_repositories() -> Result<RepositoryScan> {
    tauri::async_runtime::spawn_blocking(scan_blocking)
        .await
        .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn get_key_usage(key_path: String) -> Result<KeyUsage> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::utils::ssh_dir::validate_key_path(&key_path)?;
        let root = crate::utils::ssh_dir::get_ssh_dir()?;
        let mut hosts = Vec::new();
        for (source, content) in super::ssh_config::config_documents()? {
            for host in super::ssh_config::parse_ssh_config(&format!("Host *\n{content}")) {
                if host.identity_file.iter().any(|id| identity_path(id, &root).as_deref() == Some(&key_path)) { hosts.push(format!("{} ({})",host.alias,source.display())); }
            }
        }
        if ["id_ed25519", "id_ed25519_sk", "id_rsa", "id_ecdsa", "id_ecdsa_sk", "id_dsa"].iter().any(|name| root.join(name) == PathBuf::from(&key_path)) {
            hosts.push("OpenSSH default identity; destinations without explicit IdentityFile may use this key".into());
        }
        let profiles = super::profiles::list_blocking()?.profiles.into_iter().filter(|p| p.key_path == key_path).map(|p| p.name).collect();
        let found = scan_blocking()?;
        let repositories = found.remotes.iter().filter(|r| r.key_paths.contains(&key_path)).map(|r| format!("{} · {}",r.repository,r.name)).collect();
        let mut warnings = found.warnings;
        if found.remotes.iter().any(|r| !r.warnings.is_empty()) { warnings.push("Some repositories override SSH or include additional Git configuration; their mapping may be incomplete.".into()); }
        Ok(KeyUsage { hosts, profiles, repositories, roots: found.roots, warnings })
    }).await.map_err(|e| invalid(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_urls_and_secrets() {
        assert_eq!(
            remote_host("git@github-work:org/repo.git").as_deref(),
            Some("github-work")
        );
        assert_eq!(
            remote_host("ssh://git@[::1]:2222/team/repo").as_deref(),
            Some("::1")
        );
        assert_eq!(remote_host("https://github.com/org/repo"), None);
        assert_eq!(remote_host("../local/repo"), None);
        assert_eq!(remote_host("ext::command"), None);
        assert_eq!(remote_host("git@[::1]:repo").as_deref(), Some("::1"));
        assert_eq!(
            remote_overrides("ssh://alice@[::1]:2222/team/repo"),
            (Some("alice".into()), Some(2222))
        );

        assert!(!redacted_url("https://token:secret@git.test/repo").contains("secret"));
        assert!(!redacted_url("https://secret@git.test/repo").contains("secret"));
        assert!(matches_host("* !excluded", "work"));
        assert!(!matches_host("* !excluded", "excluded"));
    }
    #[test]
    fn scan_maps_local_remotes_and_never_executes_repository_commands() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("project");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(repo.join(".git/config"), "[remote \"origin\"]\nurl = git@work:team/repo.git\n[core]\nsshCommand = touch SHOULD_NOT_EXIST\n").unwrap();
        let hosts =
            super::super::ssh_config::parse_ssh_config("Host work\n IdentityFile /fixture/work\n");
        let result = scan(
            vec![dir.path().to_string_lossy().into()],
            &hosts,
            &[],
            Path::new("/fixture"),
        )
        .unwrap();
        assert_eq!(result.repositories, 1);
        assert_eq!(result.remotes[0].key_paths, vec!["/fixture/work"]);
        assert!(!result.remotes[0].warnings.is_empty());
        assert!(!repo.join("SHOULD_NOT_EXIST").exists());
    }
    #[test]
    fn worktree_config_uses_common_directory() {
        let dir = tempfile::tempdir().unwrap();
        let common = dir.path().join("main.git");
        let worktree = dir.path().join("worktree");
        std::fs::create_dir_all(common.join("worktrees/w")).unwrap();
        std::fs::create_dir(&worktree).unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}/worktrees/w\n", common.display()),
        )
        .unwrap();
        std::fs::write(common.join("worktrees/w/commondir"), "../..\n").unwrap();
        std::fs::write(
            common.join("config"),
            "[remote \"origin\"]\nurl = ssh://git@work/repo\n",
        )
        .unwrap();
        assert!(config_for_repository(&worktree)
            .unwrap()
            .iter()
            .any(|(k, v)| k == "remote.origin.url" && v.ends_with("/repo")));
    }
    #[test]
    fn scan_includes_default_identities_when_no_explicit_key_is_configured() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("project");
        let ssh = dir.path().join("ssh");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir(&ssh).unwrap();
        std::fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\nurl = git@work:repo.git\n",
        )
        .unwrap();
        std::fs::write(ssh.join("id_ed25519"), "fixture").unwrap();
        let result = scan(vec![repo.to_string_lossy().into()], &[], &[], &ssh).unwrap();
        assert_eq!(
            result.remotes[0].key_paths,
            vec![ssh.join("id_ed25519").to_string_lossy().to_string()]
        );
        let hosts = super::super::ssh_config::parse_ssh_config("Host *\n IdentityFile none\n");
        assert!(scan(vec![repo.to_string_lossy().into()], &hosts, &[], &ssh)
            .unwrap()
            .remotes[0]
            .key_paths
            .is_empty());
    }
}
