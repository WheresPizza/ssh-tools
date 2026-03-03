use std::process::Command;
use crate::error::{AppError, Result};
use crate::types::KnownHostEntry;
use crate::utils::ssh_dir::get_known_hosts_path;
use crate::utils::permissions::atomic_write;

fn parse_known_hosts(content: &str) -> Vec<KnownHostEntry> {
    let mut entries = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
        if parts.len() < 3 {
            continue;
        }

        entries.push(KnownHostEntry {
            line_number: idx as u32,
            hostname: parts[0].to_string(),
            key_type: parts[1].to_string(),
            key_data: parts[2].to_string(),
        });
    }

    entries
}

#[tauri::command]
pub fn list_known_hosts() -> Result<Vec<KnownHostEntry>> {
    let path = get_known_hosts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(parse_known_hosts(&content))
}

#[tauri::command]
pub fn delete_known_host(line_number: u32) -> Result<()> {
    let path = get_known_hosts_path()?;
    let content = std::fs::read_to_string(&path)?;

    let new_content: String = content
        .lines()
        .enumerate()
        .filter(|(idx, _)| *idx as u32 != line_number)
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n");

    let new_content = if new_content.is_empty() { new_content } else { new_content + "\n" };
    atomic_write(&path, &new_content)?;
    Ok(())
}

#[tauri::command]
pub fn delete_known_host_by_hostname(hostname: String) -> Result<()> {
    let path = get_known_hosts_path()?;
    let content = std::fs::read_to_string(&path)?;

    let new_content: String = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            let host = trimmed.splitn(2, ' ').next().unwrap_or("");
            host != hostname
        })
        .collect::<Vec<_>>()
        .join("\n");

    let new_content = if new_content.is_empty() { new_content } else { new_content + "\n" };
    atomic_write(&path, &new_content)?;
    Ok(())
}

#[tauri::command]
pub fn verify_known_host(hostname: String, key_type: String, stored_key_data: String) -> Result<bool> {
    if hostname.starts_with("|1|") {
        return Err(AppError::InvalidInput("Cannot verify hashed hostname".to_string()));
    }

    let output = Command::new("ssh-keyscan")
        .args(["-T", "10", &hostname])
        .output()
        .map_err(|e| AppError::Process(format!("ssh-keyscan failed: {}", e)))?;

    if output.stdout.is_empty() {
        return Err(AppError::Process("ssh-keyscan returned no output".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
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
        assert_eq!(parse_known_hosts(content)[0].key_data, "CCCC with extra");
    }
}
