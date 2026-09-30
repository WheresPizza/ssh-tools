use crate::error::{AppError, Result};
use crate::types::SshHost;
use crate::utils::{permissions::*, ssh_dir::get_ssh_config_path};
use std::path::{Path, PathBuf};

/// Parse the ~/.ssh/config file into SshHost structs.
/// Uses a line-oriented approach to preserve comments and whitespace.
pub fn parse_ssh_config(content: &str) -> Vec<SshHost> {
    let revision = config_revision(content);
    let mut hosts = Vec::new();
    let mut current: Option<SshHost> = None;

    for (idx, line) in content.lines().enumerate() {
        let trimmed = split_comment(line).0.trim();

        // Skip comments and blank lines when not in a host block
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let (key, value) =
            if let Some(pos) = trimmed.find(|c: char| c == ' ' || c == '\t' || c == '=') {
                let k = trimmed[..pos].trim().to_lowercase();
                let v = trimmed[pos..]
                    .trim_start_matches(|c: char| c == ' ' || c == '\t' || c == '=')
                    .trim()
                    .to_string();
                (k, v)
            } else {
                continue;
            };

        if key == "match" {
            if let Some(host) = current.take() {
                hosts.push(host);
            }
            continue;
        }
        if key == "host" {
            if let Some(host) = current.take() {
                hosts.push(host);
            }
            current = Some(SshHost {
                source_path: String::new(),
                read_only: false,
                can_reorder: true,
                revision: revision.clone(),
                alias: normalize_alias(&value),
                hostname: None,
                user: None,
                port: None,
                identity_file: Vec::new(),
                proxy_jump: None,
                forward_agent: None,
                server_alive_interval: None,
                extra_fields: Vec::new(),
                line_start: idx as u32,
                line_end: idx as u32,
            });
        } else if let Some(ref mut host) = current {
            host.line_end = idx as u32;
            match key.as_str() {
                "hostname" if host.hostname.is_none() => host.hostname = Some(value),
                "user" if host.user.is_none() => host.user = Some(value),
                "port" if host.port.is_none() => host.port = value.parse().ok(),
                "identityfile" => host.identity_file.push(value),
                "proxyjump" if host.proxy_jump.is_none() => host.proxy_jump = Some(value),
                "forwardagent" if host.forward_agent.is_none() => {
                    host.forward_agent = Some(value.to_lowercase() == "yes")
                }
                "serveraliveinterval" if host.server_alive_interval.is_none() => {
                    host.server_alive_interval = value.parse().ok()
                }
                "hostname"
                | "user"
                | "port"
                | "proxyjump"
                | "forwardagent"
                | "serveraliveinterval" => {}
                _ => host.extra_fields.push((key, value)),
            }
        }
    }

    if let Some(host) = current {
        hosts.push(host);
    }

    hosts
}

/// Serialize a single SshHost back to config file lines
pub fn host_to_config_lines(host: &SshHost) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Host {}", host.alias));
    if let Some(ref v) = host.hostname {
        lines.push(format!("    HostName {}", v));
    }
    if let Some(ref v) = host.user {
        lines.push(format!("    User {}", v));
    }
    if let Some(v) = host.port {
        lines.push(format!("    Port {}", v));
    }
    for id in &host.identity_file {
        lines.push(format!("    IdentityFile {}", id));
    }
    if let Some(ref v) = host.proxy_jump {
        lines.push(format!("    ProxyJump {}", v));
    }
    if let Some(v) = host.forward_agent {
        lines.push(format!("    ForwardAgent {}", if v { "yes" } else { "no" }));
    }
    if let Some(v) = host.server_alive_interval {
        lines.push(format!("    ServerAliveInterval {}", v));
    }
    for (k, v) in &host.extra_fields {
        // Capitalize first letter of key
        let key_display = {
            let mut c = k.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        };
        lines.push(format!("    {} {}", key_display, v));
    }
    lines.join("\n")
}

fn normalize_alias(value: &str) -> String {
    shlex::split(value)
        .map(|parts| parts.join(" "))
        .unwrap_or_else(|| value.to_string())
}

fn split_comment(line: &str) -> (&str, &str) {
    let mut quote = None;
    let mut escaped = false;
    let mut previous_space = true;
    for (i, c) in line.char_indices() {
        if escaped {
            escaped = false;
            previous_space = c.is_whitespace();
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if matches!(c, '\'' | '"') {
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(c);
            }
        }
        if c == '#' && quote.is_none() && previous_space {
            return (&line[..i], line[i..].trim_end());
        }
        previous_space = c.is_whitespace();
    }
    (line, "")
}

fn directive(line: &str) -> (&str, &str) {
    let line = split_comment(line).0.trim();
    if line.starts_with('#') {
        return ("", "");
    }
    let pos = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    (
        &line[..pos],
        line[pos..].trim_start_matches(|c: char| c.is_whitespace() || c == '='),
    )
}

pub(crate) fn config_revision(content: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn ensure_revision(content: &str, revision: &str) -> Result<()> {
    if config_revision(content) != revision {
        return Err(AppError::InvalidInput(
            "SSH configuration changed since loading; refresh before saving".into(),
        ));
    }
    Ok(())
}

fn validate_host(host: &SshHost) -> Result<()> {
    if host.alias.trim().is_empty() || host.port == Some(0) {
        return Err(AppError::InvalidInput(
            "Alias is required and port must be between 1 and 65535".into(),
        ));
    }
    if host.extra_fields.iter().any(|(k, _)| {
        k.is_empty()
            || !k.chars().all(|c| c.is_ascii_alphabetic())
            || k.eq_ignore_ascii_case("Host")
            || k.eq_ignore_ascii_case("Match")
    }) {
        return Err(AppError::InvalidInput(
            "Invalid extra SSH directive name".into(),
        ));
    }
    let serialized = host_to_config_lines(host);
    let values = std::iter::once(host.alias.as_str())
        .chain(host.hostname.as_deref())
        .chain(host.user.as_deref())
        .chain(host.proxy_jump.as_deref())
        .chain(host.identity_file.iter().map(String::as_str))
        .chain(
            host.extra_fields
                .iter()
                .flat_map(|(k, v)| [k.as_str(), v.as_str()]),
        );
    if values.into_iter().any(|v| v.chars().any(char::is_control)) || serialized.contains('\0') {
        return Err(AppError::InvalidInput(
            "SSH fields must not contain control characters or newlines".into(),
        ));
    }
    Ok(())
}

fn host_range(content: &str, alias: &str) -> Result<std::ops::Range<usize>> {
    host_range_at(content, alias, None)
}

fn host_range_at(
    content: &str,
    alias: &str,
    line_start: Option<u32>,
) -> Result<std::ops::Range<usize>> {
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    let starts: Vec<_> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let (key, value) = directive(l);
            if key.eq_ignore_ascii_case("host")
                && normalize_alias(value) == alias
                && line_start.is_none_or(|start| start as usize == i)
            {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    if starts.len() != 1 {
        return Err(AppError::InvalidInput(
            "Host is missing or alias is ambiguous; refresh the configuration".into(),
        ));
    }
    let start = starts[0];
    let end = (start + 1..lines.len())
        .find(|&i| {
            let (key, _) = directive(lines[i]);
            key.eq_ignore_ascii_case("host") || key.eq_ignore_ascii_case("match")
        })
        .unwrap_or(lines.len());
    Ok(start..end)
}

/// Patch only changed directives. Unchanged blocks, comments, Match and Include stay byte-for-byte intact.
fn edit_host(content: &str, alias: &str, host: Option<&SshHost>) -> Result<String> {
    edit_host_at(content, alias, host, host.map(|h| h.line_start))
}

fn edit_host_at(
    content: &str,
    alias: &str,
    host: Option<&SshHost>,
    line_start: Option<u32>,
) -> Result<String> {
    let range = host_range_at(content, alias, line_start)?;
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    let mut result = lines[..range.start].concat();
    if let Some(host) = host {
        validate_host(host)?;
        let old = parse_ssh_config(content)
            .into_iter()
            .find(|h| h.line_start as usize == range.start)
            .ok_or_else(|| AppError::NotFound(alias.into()))?;
        if host.alias != alias
            && parse_ssh_config(content)
                .iter()
                .any(|h| h.alias == host.alias)
        {
            return Err(AppError::InvalidInput(
                "A host with this alias already exists".into(),
            ));
        }
        let before = host_to_config_lines(&old);
        let after = host_to_config_lines(host);
        let mut changed = std::collections::BTreeMap::<String, Vec<String>>::new();
        for line in before.lines().chain(after.lines()) {
            let key = directive(line).0.to_lowercase();
            let previous: Vec<_> = before
                .lines()
                .filter(|l| directive(l).0.eq_ignore_ascii_case(&key))
                .collect();
            let next: Vec<_> = after
                .lines()
                .filter(|l| directive(l).0.eq_ignore_ascii_case(&key))
                .collect();
            if previous != next {
                changed.insert(key, next.iter().map(|l| format!("{l}\n")).collect());
            }
        }
        let mut written = std::collections::HashSet::new();
        for line in &lines[range.clone()] {
            let key = directive(line).0.to_lowercase();
            if let Some(replacement) = changed.get(&key) {
                if written.insert(key) {
                    let comment = split_comment(line).1;
                    if !comment.is_empty() {
                        if let Some(first) = replacement.first() {
                            result.push_str(first.trim_end());
                            result.push(' ');
                            result.push_str(comment);
                            result.push('\n');
                            result.push_str(&replacement[1..].concat());
                        } else {
                            result.push_str("    ");
                            result.push_str(comment);
                            result.push('\n');
                        }
                    } else {
                        result.push_str(&replacement.concat());
                    }
                }
            } else {
                result.push_str(line);
            }
        }
        for (key, replacement) in changed {
            if !written.contains(&key) {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push_str(&replacement.concat());
            }
        }
    } else {
        // Keep comments and blank lines even when the host itself is removed.
        for line in &lines[range.clone()] {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                result.push_str(line);
            }
        }
    }
    result.push_str(&lines[range.end..].concat());
    Ok(result)
}

fn reorder_config(content: &str, aliases: &[String]) -> Result<String> {
    let hosts = parse_ssh_config(content);
    let expected: std::collections::HashSet<_> = hosts.iter().map(|h| &h.alias).collect();
    let supplied: std::collections::HashSet<_> = aliases.iter().collect();
    if hosts.len() != aliases.len() || expected != supplied || supplied.len() != aliases.len() {
        return Err(AppError::InvalidInput(
            "Host order is stale or contains duplicates; refresh and try again".into(),
        ));
    }
    if content.lines().any(|l| {
        let (k, _) = directive(l);
        k.eq_ignore_ascii_case("match") || k.eq_ignore_ascii_case("include")
    }) || hosts
        .iter()
        .any(|h| h.alias.chars().any(|c| "*?!".contains(c)))
    {
        return Err(AppError::InvalidInput("Reordering configurations with Match, Include or wildcard rules could change SSH behavior".into()));
    }
    let lines: Vec<_> = content.split_inclusive('\n').collect();
    let first = hosts
        .first()
        .map(|h| h.line_start as usize)
        .unwrap_or(lines.len());
    let mut result = lines[..first].concat();
    for alias in aliases {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&lines[host_range(content, alias)?].concat());
    }
    Ok(result)
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command(rename_all = "snake_case")]
pub async fn get_ssh_config() -> Result<Vec<SshHost>> {
    tauri::async_runtime::spawn_blocking(move || get_ssh_config_blocking())
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn get_ssh_config_blocking() -> Result<Vec<SshHost>> {
    let root = crate::utils::ssh_dir::get_ssh_dir()?;
    let mut hosts = Vec::new();
    for (path, content) in config_documents()? {
        let managed = crate::utils::ssh_dir::managed_path(&root, &path).is_ok();
        let mut parsed = parse_ssh_config(&content);
        let unique: std::collections::HashSet<_> = parsed.iter().map(|h| &h.alias).collect();
        let can_reorder = unique.len() == parsed.len()
            && !content.lines().any(|l| {
                let (key, _) = directive(l);
                key.eq_ignore_ascii_case("include") || key.eq_ignore_ascii_case("match")
            })
            && !parsed
                .iter()
                .any(|h| h.alias.chars().any(|c| "*?!".contains(c)));
        for host in &mut parsed {
            host.source_path = path.to_string_lossy().into();
            let profile_line = path == get_ssh_config_path()?
                && (host.line_start as usize) < super::profiles::managed_line_count(&content);
            host.read_only = !managed || profile_line;
            host.can_reorder = managed && can_reorder;
        }
        hosts.extend(parsed);
    }
    Ok(hosts)
}

pub(crate) fn config_documents() -> Result<Vec<(PathBuf, String)>> {
    let root = crate::utils::ssh_dir::get_ssh_dir()?;
    let main = get_ssh_config_path()?;
    let mut result = Vec::new();
    fn visit(
        path: PathBuf,
        root: &Path,
        result: &mut Vec<(PathBuf, String)>,
        stack: &mut Vec<PathBuf>,
    ) -> Result<()> {
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if stack.contains(&canonical) {
            return Err(AppError::InvalidInput(format!(
                "Include cycle at {}",
                path.display()
            )));
        }
        if result.iter().any(|(p, _)| p == &canonical) {
            return Ok(());
        }
        if stack.len() >= 16 || result.len() >= 256 {
            return Err(AppError::InvalidInput(
                "Too many included SSH configuration files".into(),
            ));
        }
        let metadata = std::fs::metadata(&canonical)?;
        if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
            return Err(AppError::InvalidInput(
                "Included config must be a regular file smaller than 2 MiB".into(),
            ));
        }
        let content = std::fs::read_to_string(&canonical)?;
        result.push((canonical.clone(), content.clone()));
        stack.push(canonical);
        for line in content.lines() {
            let (key, value) = directive(line);
            if !key.eq_ignore_ascii_case("include") {
                continue;
            }
            let patterns = shlex::split(value).ok_or_else(|| {
                AppError::InvalidInput("Invalid quoting in Include directive".into())
            })?;
            for pattern in patterns {
                let expanded = if let Some(tail) = pattern.strip_prefix("~/.ssh/") {
                    root.join(tail)
                } else if let Some(tail) = pattern.strip_prefix("~/") {
                    dirs::home_dir()
                        .ok_or_else(|| AppError::NotFound("Home directory".into()))?
                        .join(tail)
                } else if Path::new(&pattern).is_absolute() {
                    PathBuf::from(pattern)
                } else {
                    root.join(pattern)
                };
                let matches = glob::glob_with(
                    expanded
                        .to_str()
                        .ok_or_else(|| AppError::InvalidInput("Non-UTF8 Include path".into()))?,
                    glob::MatchOptions {
                        require_literal_leading_dot: true,
                        ..Default::default()
                    },
                )
                .map_err(|e| AppError::InvalidInput(e.to_string()))?;
                for entry in matches {
                    visit(
                        entry.map_err(|e| AppError::InvalidInput(e.to_string()))?,
                        root,
                        result,
                        stack,
                    )?;
                }
            }
        }
        stack.pop();
        Ok(())
    }
    visit(main, &root, &mut result, &mut Vec::new())?;
    Ok(result)
}

pub(crate) fn resolve_config_path(source: &str) -> Result<PathBuf> {
    let main = get_ssh_config_path()?;
    let selected = if source.is_empty() {
        main.clone()
    } else {
        PathBuf::from(source)
    };
    crate::utils::ssh_dir::managed_path(&crate::utils::ssh_dir::get_ssh_dir()?, &selected)?;
    if selected == main
        || config_documents()?
            .iter()
            .any(|(path, _)| path == &selected)
    {
        Ok(selected)
    } else {
        Err(AppError::InvalidInput(
            "File is not part of the current SSH configuration".into(),
        ))
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn add_host(host: SshHost) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || add_host_blocking(host))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn add_host_blocking(host: SshHost) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    validate_host(&host)?;
    let path = resolve_config_path(&host.source_path)?;
    let mut content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    if parse_ssh_config(&content)
        .iter()
        .any(|h| h.alias == host.alias)
    {
        return Err(AppError::InvalidInput(
            "A host with this alias already exists".into(),
        ));
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push('\n');
    content.push_str(&host_to_config_lines(&host));
    content.push('\n');
    atomic_write(&path, &content)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_host(original_alias: String, host: SshHost) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || update_host_blocking(original_alias, host))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn update_host_blocking(original_alias: String, host: SshHost) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    let path = resolve_config_path(&host.source_path)?;
    let existing = std::fs::read_to_string(&path)?;
    ensure_revision(&existing, &host.revision)?;
    atomic_write(&path, &edit_host(&existing, &original_alias, Some(&host))?)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_host(
    alias: String,
    revision: String,
    source_path: String,
    line_start: Option<u32>,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        delete_host_blocking(alias, revision, source_path, line_start)
    })
    .await
    .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn delete_host_blocking(
    alias: String,
    revision: String,
    source_path: String,
    line_start: Option<u32>,
) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    let path = resolve_config_path(&source_path)?;
    let existing = std::fs::read_to_string(&path)?;
    ensure_revision(&existing, &revision)?;
    atomic_write(&path, &edit_host_at(&existing, &alias, None, line_start)?)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn reorder_hosts(
    aliases: Vec<String>,
    revision: String,
    source_path: String,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        reorder_hosts_blocking(aliases, revision, source_path)
    })
    .await
    .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn reorder_hosts_blocking(
    aliases: Vec<String>,
    revision: String,
    source_path: String,
) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    let path = resolve_config_path(&source_path)?;
    let existing = std::fs::read_to_string(&path)?;
    ensure_revision(&existing, &revision)?;
    atomic_write(&path, &reorder_config(&existing, &aliases)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_host(alias: &str) -> SshHost {
        SshHost {
            source_path: String::new(),
            read_only: false,
            can_reorder: true,
            revision: String::new(),
            alias: alias.to_string(),
            hostname: None,
            user: None,
            port: None,
            identity_file: Vec::new(),
            proxy_jump: None,
            forward_agent: None,
            server_alive_interval: None,
            extra_fields: Vec::new(),
            line_start: 0,
            line_end: 0,
        }
    }

    #[test]
    fn parse_empty_returns_no_hosts() {
        assert!(parse_ssh_config("").is_empty());
    }

    #[test]
    fn parse_single_host_all_fields() {
        let cfg = "Host myserver\n    HostName 10.0.0.1\n    User alice\n    Port 2222\n    IdentityFile ~/.ssh/id_ed25519\n    ForwardAgent yes\n    ServerAliveInterval 60\n";
        let hosts = parse_ssh_config(cfg);
        assert_eq!(hosts.len(), 1);
        let h = &hosts[0];
        assert_eq!(h.alias, "myserver");
        assert_eq!(h.hostname, Some("10.0.0.1".to_string()));
        assert_eq!(h.user, Some("alice".to_string()));
        assert_eq!(h.port, Some(2222));
        assert_eq!(h.identity_file, vec!["~/.ssh/id_ed25519"]);
        assert_eq!(h.forward_agent, Some(true));
        assert_eq!(h.server_alive_interval, Some(60));
    }

    #[test]
    fn parse_two_hosts() {
        let cfg =
            "Host alpha\n    HostName 10.0.0.1\n\nHost beta\n    HostName 10.0.0.2\n    User bob\n";
        let hosts = parse_ssh_config(cfg);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "alpha");
        assert_eq!(hosts[1].user, Some("bob".to_string()));
    }

    #[test]
    fn parse_ignores_comment_lines() {
        let cfg = "# comment\nHost myserver\n    # another\n    HostName 1.2.3.4\n";
        let hosts = parse_ssh_config(cfg);
        assert_eq!(hosts.len(), 1);
        assert!(hosts[0].extra_fields.is_empty());
    }

    #[test]
    fn parse_multiple_identity_files() {
        let cfg =
            "Host multi\n    IdentityFile ~/.ssh/id_rsa\n    IdentityFile ~/.ssh/id_ed25519\n";
        assert_eq!(parse_ssh_config(cfg)[0].identity_file.len(), 2);
    }

    #[test]
    fn parse_forward_agent_no() {
        let cfg = "Host x\n    ForwardAgent no\n";
        assert_eq!(parse_ssh_config(cfg)[0].forward_agent, Some(false));
    }

    #[test]
    fn parse_unknown_key_to_extra_fields() {
        let cfg = "Host x\n    ControlMaster auto\n";
        let h = &parse_ssh_config(cfg)[0];
        assert_eq!(h.extra_fields.len(), 1);
        assert_eq!(h.extra_fields[0].1, "auto");
    }

    #[test]
    fn parse_invalid_port_gives_none() {
        let cfg = "Host x\n    Port notanumber\n";
        assert_eq!(parse_ssh_config(cfg)[0].port, None);
    }

    #[test]
    fn serialize_minimal_host() {
        let host = minimal_host("myserver");
        assert_eq!(host_to_config_lines(&host), "Host myserver");
    }

    #[test]
    fn serialize_forward_agent_false_writes_no() {
        let mut host = minimal_host("x");
        host.forward_agent = Some(false);
        assert!(host_to_config_lines(&host).contains("    ForwardAgent no"));
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let cfg = "Host myserver\n    HostName 10.0.0.1\n    User alice\n    Port 2222\n    ForwardAgent yes\n    ServerAliveInterval 60\n";
        let hosts = parse_ssh_config(cfg);
        let serialized = host_to_config_lines(&hosts[0]);
        let reparsed = parse_ssh_config(&serialized);
        assert_eq!(reparsed.len(), 1);
        let h = &reparsed[0];
        assert_eq!(h.hostname, Some("10.0.0.1".to_string()));
        assert_eq!(h.user, Some("alice".to_string()));
        assert_eq!(h.port, Some(2222));
        assert_eq!(h.forward_agent, Some(true));
        assert_eq!(h.server_alive_interval, Some(60));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn edit_preserves_comments_other_hosts_and_match() {
        let source = "# header\nInclude ~/.ssh/conf.d/*\nHost one\n  # key comment\n  HostName old\n  User alice\nMatch exec true\n  User conditional\nHost two\n  HostName two\n";
        let mut host = parse_ssh_config(source).remove(0);
        assert_eq!(host.user.as_deref(), Some("alice"));
        host.hostname = Some("new".into());
        let result = edit_host(source, "one", Some(&host)).unwrap();
        assert_eq!(result, source.replace("  HostName old", "    HostName new"));
    }
    #[test]
    fn unchanged_edit_is_lossless() {
        let source = "Host\tone\n\tHostName=example\n# comment\n  User alice";
        let host = parse_ssh_config(source).remove(0);
        assert_eq!(edit_host(source, "one", Some(&host)).unwrap(), source);
    }
    #[test]
    fn rejects_directive_injection_and_duplicate_alias() {
        let source = "Host one\nHost two\n";
        let mut host = parse_ssh_config(source).remove(0);
        host.alias = "two".into();
        assert!(edit_host(source, "one", Some(&host)).is_err());
        host.alias = "one\nProxyCommand bad".into();
        assert!(edit_host(source, "one", Some(&host)).is_err());
    }
    #[test]
    fn reordering_preserves_raw_blocks_and_requires_complete_order() {
        let source = "# header\nHost one\n  # comment\n  User alice\nHost two\n\tUser bob\n";
        let result = reorder_config(source, &["two".into(), "one".into()]).unwrap();
        assert_eq!(
            result,
            "# header\nHost two\n\tUser bob\nHost one\n  # comment\n  User alice\n"
        );
        assert!(reorder_config(source, &["one".into()]).is_err());
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    #[test]
    fn revision_detects_external_edits() {
        let original = "Host one\n  User alice\n";
        let revision = parse_ssh_config(original)[0].revision.clone();
        assert!(ensure_revision(original, &revision).is_ok());
        assert!(ensure_revision("Host one\n  User bob\n", &revision).is_err());
    }
    #[test]
    fn first_value_wins_without_removing_duplicate_directives() {
        let text = "Host one\n  User alice\n  User bob\n";
        let host = parse_ssh_config(text).remove(0);
        assert_eq!(host.user.as_deref(), Some("alice"));
        assert_eq!(edit_host(text, "one", Some(&host)).unwrap(), text);
    }
}

#[cfg(test)]
mod comment_tests {
    use super::*;
    #[test]
    fn updates_keep_inline_comments_and_quoted_hashes() {
        let text = "Host one # work\n  HostName old # address\n  IdentityFile \"/keys/a # b\"\n";
        let mut host = parse_ssh_config(text).remove(0);
        assert_eq!(host.alias, "one");
        assert_eq!(host.identity_file, vec!["\"/keys/a # b\""]);
        host.hostname = Some("new".into());
        let result = edit_host(text, "one", Some(&host)).unwrap();
        assert!(result.contains("Host one # work"));
        assert!(result.contains("HostName new # address"));
    }
}
