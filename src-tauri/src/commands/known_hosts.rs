use crate::error::{AppError, Result};
use crate::types::KnownHostEntry;
use crate::utils::permissions::atomic_write;
use crate::utils::ssh_dir::get_known_hosts_path;
use std::process::Command;

fn parse_known_hosts(content: &str) -> Vec<KnownHostEntry> {
    let mut entries = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let first = match parts.next() {
            Some(v) => v,
            None => continue,
        };
        let (marker, hostname) = if first.starts_with('@') {
            (
                Some(first),
                match parts.next() {
                    Some(v) => v,
                    None => continue,
                },
            )
        } else {
            (None, first)
        };
        let key_type = match parts.next() {
            Some(v) => v,
            None => continue,
        };
        let key_data = match parts.next() {
            Some(v) => v,
            None => continue,
        };
        entries.push(KnownHostEntry {
            line_number: idx as u32,
            hostname: hostname.to_string(),
            key_type: key_type.to_string(),
            key_data: key_data.to_string(),
            marker: marker.map(str::to_string),
        });
    }

    entries
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_known_hosts() -> Result<Vec<KnownHostEntry>> {
    tauri::async_runtime::spawn_blocking(move || list_known_hosts_blocking())
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn list_known_hosts_blocking() -> Result<Vec<KnownHostEntry>> {
    let path = get_known_hosts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(parse_known_hosts(&content))
}

fn remove_entries(content: &str, entries: &[KnownHostEntry]) -> Result<String> {
    let actual = parse_known_hosts(content);
    for expected in entries {
        if !actual.iter().any(|entry| entry == expected) {
            return Err(AppError::InvalidInput(
                "Known hosts changed since loading; refresh before removing entries".into(),
            ));
        }
    }
    let lines: std::collections::HashSet<_> = entries.iter().map(|e| e.line_number).collect();
    Ok(content
        .split_inclusive('\n')
        .enumerate()
        .filter(|(i, _)| !lines.contains(&(*i as u32)))
        .map(|(_, l)| l)
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_known_hosts(entries: Vec<KnownHostEntry>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || delete_known_hosts_blocking(entries))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn delete_known_hosts_blocking(entries: Vec<KnownHostEntry>) -> Result<()> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    let path = get_known_hosts_path()?;
    let content = std::fs::read_to_string(&path)?;
    atomic_write(&path, &remove_entries(&content, &entries)?)
}

fn scan_target(hostname: &str) -> Result<(String, u16)> {
    let hostname = hostname.split(',').next().unwrap_or("");
    if hostname.starts_with("|1|") {
        return Err(AppError::InvalidInput(
            "A hashed hostname cannot be scanned without its original name".into(),
        ));
    }
    let (host, port) = if hostname.starts_with('[') {
        let (host, port) = hostname[1..]
            .rsplit_once("]:")
            .ok_or_else(|| AppError::InvalidInput("Invalid bracketed hostname".into()))?;
        (
            host,
            port.parse::<u16>()
                .map_err(|_| AppError::InvalidInput("Invalid host port".into()))?,
        )
    } else {
        (hostname, 22)
    };
    if host.is_empty()
        || host.starts_with('-')
        || port == 0
        || host
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && !".-_:".contains(c))
    {
        return Err(AppError::InvalidInput(
            "Expected a hostname or IP address".into(),
        ));
    }
    Ok((host.into(), port))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn verify_known_host(
    hostname: String,
    key_type: String,
    stored_key_data: String,
) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        verify_known_host_blocking(hostname, key_type, stored_key_data)
    })
    .await
    .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn verify_known_host_blocking(
    hostname: String,
    key_type: String,
    stored_key_data: String,
) -> Result<bool> {
    let (host, port) = scan_target(&hostname)?;
    let output = Command::new("ssh-keyscan")
        .args(["-T", "5", "-p", &port.to_string(), &host])
        .output()
        .map_err(|e| AppError::Process(format!("ssh-keyscan failed: {}", e)))?;

    if output.stdout.is_empty() {
        return Err(AppError::Process(
            "ssh-keyscan returned no output".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        if parts[1] == key_type {
            return Ok(parts[2].trim() == stored_key_data.trim());
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_nothing() {
        assert!(parse_known_hosts("").is_empty());
    }

    #[test]
    fn parse_skips_comments_and_blanks() {
        assert!(parse_known_hosts("# comment\n\n# another\n").is_empty());
    }

    #[test]
    fn parse_single_entry_fields_and_line_number() {
        let entries = parse_known_hosts("github.com ssh-ed25519 AAAA1234\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hostname, "github.com");
        assert_eq!(entries[0].key_type, "ssh-ed25519");
        assert_eq!(entries[0].key_data, "AAAA1234");
        assert_eq!(entries[0].line_number, 0);
    }

    #[test]
    fn parse_line_numbers_skip_comments() {
        let content = "# header\ngithub.com ssh-ed25519 AAAA\ngitlab.com ssh-rsa BBBB\n";
        let entries = parse_known_hosts(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].line_number, 1);
        assert_eq!(entries[1].line_number, 2);
    }

    #[test]
    fn parse_skips_lines_with_fewer_than_3_parts() {
        let content = "hostname-only\nhostname keytype\nhostname keytype data\n";
        let entries = parse_known_hosts(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hostname, "hostname");
    }

    #[test]
    fn parse_key_data_captures_everything_after_second_space() {
        let content = "host.example.com ecdsa-sha2-nistp256 CCCC with extra\n";
        assert_eq!(parse_known_hosts(content)[0].key_data, "CCCC");
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn parses_markers_tabs_and_comments() {
        let entries =
            parse_known_hosts("@cert-authority\t*.example.com   ssh-ed25519 AAAA comment\n");
        assert_eq!(entries[0].marker.as_deref(), Some("@cert-authority"));
        assert_eq!(entries[0].hostname, "*.example.com");
        assert_eq!(entries[0].key_data, "AAAA");
    }
    #[test]
    fn scans_nonstandard_ports_ipv6_and_lists() {
        assert_eq!(scan_target("[::1]:2222").unwrap(), ("::1".into(), 2222));
        assert_eq!(
            scan_target("example.com,127.0.0.1").unwrap(),
            ("example.com".into(), 22)
        );
        assert!(scan_target("-f/etc/passwd").is_err());
        assert!(scan_target("|1|hash|hash").is_err());
    }
    #[test]
    fn bulk_delete_is_atomic_and_rejects_stale_rows() {
        let source = "# comment\na ssh-ed25519 AAAA\nb ssh-rsa BBBB\nc ssh-ed25519 CCCC\n";
        let rows = parse_known_hosts(source);
        assert_eq!(
            remove_entries(source, &[rows[0].clone(), rows[2].clone()]).unwrap(),
            "# comment\nb ssh-rsa BBBB\n"
        );
        assert!(remove_entries(&format!("# inserted\n{source}"), &rows).is_err());
    }
}
