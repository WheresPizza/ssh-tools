use crate::error::{AppError, Result};
use crate::types::SshHost;
use crate::utils::{permissions::*, ssh_dir::get_ssh_config_path};

/// Parse the ~/.ssh/config file into SshHost structs.
/// Uses a line-oriented approach to preserve comments and whitespace.
pub fn parse_ssh_config(content: &str) -> Vec<SshHost> {
    let mut hosts = Vec::new();
    let mut current: Option<SshHost> = None;

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments and blank lines when not in a host block
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        let (key, value) = if let Some(pos) = trimmed.find(|c: char| c == ' ' || c == '\t' || c == '=') {
            let k = trimmed[..pos].trim().to_lowercase();
            let v = trimmed[pos..].trim_start_matches(|c: char| c == ' ' || c == '\t' || c == '=').trim().to_string();
            (k, v)
        } else {
            continue;
        };

        if key == "host" {
            if let Some(host) = current.take() {
                hosts.push(host);
            }
            current = Some(SshHost {
                alias: value,
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
                "hostname" => host.hostname = Some(value),
                "user" => host.user = Some(value),
                "port" => host.port = value.parse().ok(),
                "identityfile" => host.identity_file.push(value),
                "proxyjump" => host.proxy_jump = Some(value),
                "forwardagent" => host.forward_agent = Some(value.to_lowercase() == "yes"),
                "serveraliveinterval" => host.server_alive_interval = value.parse().ok(),
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

/// Write all hosts back to config, rebuilding the host sections but preserving
/// non-host content (comments, Include lines, global options at the top)
fn rebuild_config(existing_content: &str, hosts: &[SshHost]) -> String {
    // Find where the first Host block starts
    let lines: Vec<&str> = existing_content.lines().collect();
    let mut preamble_end = 0usize;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.starts_with("host ") || trimmed == "host" {
            preamble_end = idx;
            break;
        }
        preamble_end = idx + 1;
    }

    let preamble: Vec<&str> = lines[..preamble_end].to_vec();
    let mut result = preamble.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }

    for host in hosts {
        result.push('\n');
        result.push_str(&host_to_config_lines(host));
        result.push('\n');
    }

    result
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_ssh_config() -> Result<Vec<SshHost>> {
    let config_path = get_ssh_config_path()?;
    if !config_path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&config_path)?;
    Ok(parse_ssh_config(&content))
}

#[tauri::command]
pub fn add_host(host: SshHost) -> Result<()> {
    let config_path = get_ssh_config_path()?;
    let existing = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    let mut hosts = parse_ssh_config(&existing);

    // Check for duplicate alias
    if hosts.iter().any(|h| h.alias == host.alias) {
        return Err(AppError::InvalidInput(format!("Host '{}' already exists", host.alias)));
    }

    hosts.push(host);
    let new_content = rebuild_config(&existing, &hosts);
    atomic_write(&config_path, &new_content)?;
    set_config_permissions(&config_path)?;
    Ok(())
}

#[tauri::command]
pub fn update_host(original_alias: String, host: SshHost) -> Result<()> {
    let config_path = get_ssh_config_path()?;
    let existing = std::fs::read_to_string(&config_path)?;
    let mut hosts = parse_ssh_config(&existing);

    let idx = hosts.iter().position(|h| h.alias == original_alias)
        .ok_or_else(|| AppError::NotFound(format!("Host '{}' not found", original_alias)))?;

    hosts[idx] = host;
    let new_content = rebuild_config(&existing, &hosts);
    atomic_write(&config_path, &new_content)?;
    set_config_permissions(&config_path)?;
    Ok(())
}

#[tauri::command]
pub fn delete_host(alias: String) -> Result<()> {
    let config_path = get_ssh_config_path()?;
    let existing = std::fs::read_to_string(&config_path)?;
    let mut hosts = parse_ssh_config(&existing);

    let len_before = hosts.len();
    hosts.retain(|h| h.alias != alias);

    if hosts.len() == len_before {
        return Err(AppError::NotFound(format!("Host '{}' not found", alias)));
    }

    let new_content = rebuild_config(&existing, &hosts);
    atomic_write(&config_path, &new_content)?;
    set_config_permissions(&config_path)?;
    Ok(())
}

#[tauri::command]
pub fn reorder_hosts(aliases: Vec<String>) -> Result<()> {
    let config_path = get_ssh_config_path()?;
    let existing = std::fs::read_to_string(&config_path)?;
    let hosts = parse_ssh_config(&existing);

    let mut reordered: Vec<SshHost> = Vec::new();
    for alias in &aliases {
        if let Some(host) = hosts.iter().find(|h| &h.alias == alias) {
            reordered.push(host.clone());
        }
    }

    // Add any hosts not in the aliases list at the end
    for host in &hosts {
        if !aliases.contains(&host.alias) {
            reordered.push(host.clone());
        }
    }

    let new_content = rebuild_config(&existing, &reordered);
    atomic_write(&config_path, &new_content)?;
    set_config_permissions(&config_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_host(alias: &str) -> SshHost {
        SshHost {
            alias: alias.to_string(), hostname: None, user: None, port: None,
            identity_file: Vec::new(), proxy_jump: None, forward_agent: None,
            server_alive_interval: None, extra_fields: Vec::new(),
            line_start: 0, line_end: 0,
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
        let cfg = "Host alpha\n    HostName 10.0.0.1\n\nHost beta\n    HostName 10.0.0.2\n    User bob\n";
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
        let cfg = "Host multi\n    IdentityFile ~/.ssh/id_rsa\n    IdentityFile ~/.ssh/id_ed25519\n";
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
