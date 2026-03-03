use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::{AppError, Result};
use crate::types::{KeyAlgorithm, KeyGenParams, SshKeyInfo};
use crate::utils::{permissions::*, ssh_dir::get_ssh_dir};

fn get_key_algorithm_str(alg: &KeyAlgorithm) -> (&'static str, &'static str, Option<&'static str>) {
    match alg {
        KeyAlgorithm::Ed25519 => ("ed25519", "ed25519", None),
        KeyAlgorithm::Rsa2048 => ("rsa", "rsa", Some("2048")),
        KeyAlgorithm::Rsa4096 => ("rsa", "rsa", Some("4096")),
        KeyAlgorithm::EcdsaP256 => ("ecdsa", "ecdsa", Some("256")),
        KeyAlgorithm::EcdsaP384 => ("ecdsa", "ecdsa", Some("384")),
    }
}

fn parse_key_info(private_path: &Path) -> Result<SshKeyInfo> {
    let name = private_path.file_name()
        .and_then(|n: &std::ffi::OsStr| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let public_path = PathBuf::from(format!("{}.pub", private_path.display()));
    let public_path_str = public_path.to_str().unwrap_or("").to_string();

    // Get fingerprint via ssh-keygen -l
    let fingerprint_output = Command::new("ssh-keygen")
        .args(["-l", "-f", private_path.to_str().unwrap_or("")])
        .output();

    let (fingerprint, algorithm, bits, comment) = match fingerprint_output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            parse_fingerprint_line(&stdout)
        }
        _ => ("unknown".to_string(), "unknown".to_string(), None, None),
    };

    // Get file creation/modification time
    let created_at = std::fs::metadata(private_path).ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let secs = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            format_timestamp(secs)
        });

    // Check if key has passphrase (ssh-keygen -y with empty passphrase)
    let has_passphrase = check_has_passphrase(private_path);

    Ok(SshKeyInfo {
        name,
        private_path: private_path.to_str().unwrap_or("").to_string(),
        public_path: public_path_str,
        algorithm,
        bits,
        fingerprint,
        comment,
        has_passphrase,
        created_at,
    })
}

fn parse_fingerprint_line(line: &str) -> (String, String, Option<u32>, Option<String>) {
    // Format: "256 SHA256:xxxx comment (ED25519)"
    // or: "2048 SHA256:xxxx comment (RSA)"
    let parts: Vec<&str> = line.trim().splitn(4, ' ').collect();

    let bits: Option<u32> = parts.first().and_then(|s| s.parse().ok());
    let fingerprint = parts.get(1).unwrap_or(&"").to_string();
    let rest = parts.get(2..).unwrap_or(&[]).join(" ");

    // Extract algorithm from parentheses at end
    let algorithm = if let (Some(start), Some(end)) = (rest.rfind('('), rest.rfind(')')) {
        rest[start+1..end].to_lowercase()
    } else {
        "unknown".to_string()
    };

    // Comment is everything before the algorithm parentheses
    let comment = if let Some(paren_pos) = rest.rfind('(') {
        let c = rest[..paren_pos].trim().to_string();
        if c.is_empty() || c == "(none)" { None } else { Some(c) }
    } else {
        None
    };

    (fingerprint, algorithm, bits, comment)
}

fn check_has_passphrase(key_path: &Path) -> bool {
    // Try to load the key with empty passphrase; if it fails, it has a passphrase
    let output = Command::new("ssh-keygen")
        .args(["-y", "-P", "", "-f", key_path.to_str().unwrap_or("")])
        .output();

    match output {
        Ok(o) => !o.status.success(),
        Err(_) => false,
    }
}

fn format_timestamp(secs: u64) -> String {
    // Simple ISO-like date formatting without external crate
    let days_since_epoch = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;

    // Compute year/month/day from days_since_epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days_since_epoch as u32);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let month_days = [31, if is_leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for md in &month_days {
        if days < *md { break; }
        days -= *md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_ssh_keys() -> Result<Vec<SshKeyInfo>> {
    let ssh_dir = get_ssh_dir()?;
    let mut keys = Vec::new();

    let read_dir = std::fs::read_dir(&ssh_dir)?;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            // Skip public keys, config, known_hosts
            if name.ends_with(".pub") || name == "config" || name == "known_hosts"
               || name == "authorized_keys" || name.starts_with('.') {
                continue;
            }
            // Check if corresponding .pub exists
            let pub_path = PathBuf::from(format!("{}.pub", path.display()));
            if !pub_path.exists() {
                continue;
            }
            if let Ok(info) = parse_key_info(&path) {
                keys.push(info);
            }
        }
    }

    Ok(keys)
}

#[tauri::command]
pub fn generate_ssh_key(params: KeyGenParams) -> Result<SshKeyInfo> {
    if params.filename.contains('/') || params.filename.contains("..") {
        return Err(AppError::InvalidInput("Filename must not contain path separators".to_string()));
    }
    let ssh_dir = get_ssh_dir()?;
    let key_path = ssh_dir.join(&params.filename);

    if key_path.exists() {
        return Err(AppError::InvalidInput(format!("Key file '{}' already exists", params.filename)));
    }

    let (type_flag, _, bits_opt) = get_key_algorithm_str(&params.algorithm);

    let mut cmd_args = vec![
        "-t".to_string(), type_flag.to_string(),
        "-C".to_string(), params.comment.clone(),
        "-f".to_string(), key_path.to_str().unwrap_or("").to_string(),
    ];

    if let Some(bits) = bits_opt {
        cmd_args.push("-b".to_string());
        cmd_args.push(bits.to_string());
    }

    // Passphrase: use -N flag
    let passphrase = params.passphrase.clone().unwrap_or_default();
    cmd_args.push("-N".to_string());
    cmd_args.push(passphrase);

    let output = Command::new("ssh-keygen")
        .args(&cmd_args)
        .output()
        .map_err(|e| AppError::Process(format!("Failed to run ssh-keygen: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SshKey(format!("ssh-keygen failed: {}", stderr)));
    }

    // Set correct permissions
    set_private_key_permissions(&key_path)?;
    let pub_path = PathBuf::from(format!("{}.pub", key_path.display()));
    if pub_path.exists() {
        set_public_key_permissions(&pub_path)?;
    }

    parse_key_info(&key_path)
}

#[tauri::command]
pub fn get_public_key(key_path: String) -> Result<String> {
    let pub_path = PathBuf::from(format!("{}.pub", key_path));
    let content = std::fs::read_to_string(&pub_path)
        .map_err(|e| AppError::Io(e))?;
    Ok(content.trim().to_string())
}

#[tauri::command]
pub fn delete_ssh_key(key_path: String) -> Result<()> {
    let path = PathBuf::from(&key_path);
    let pub_path = PathBuf::from(format!("{}.pub", key_path));

    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    if pub_path.exists() {
        std::fs::remove_file(&pub_path)?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_key_fingerprint(key_path: String) -> Result<String> {
    let output = Command::new("ssh-keygen")
        .args(["-l", "-f", &key_path])
        .output()
        .map_err(|e| AppError::Process(format!("ssh-keygen failed: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::SshKey("Could not get fingerprint".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (fp, _, _, _) = parse_fingerprint_line(&stdout);
    Ok(fp)
}

#[tauri::command]
pub fn list_agent_keys() -> Result<Vec<String>> {
    let output = Command::new("ssh-add")
        .arg("-l")
        .output()
        .map_err(|e| AppError::Process(format!("ssh-add failed: {}", e)))?;

    // Exit code 1 = no identities, exit code 2 = no agent — both return empty
    match output.status.code() {
        Some(1) | Some(2) => return Ok(Vec::new()),
        _ => {}
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let fingerprints = stdout
        .lines()
        .filter_map(|line| {
            // Format: "256 SHA256:xxx comment (ED25519)"
            let mut parts = line.trim().splitn(3, ' ');
            parts.next(); // bits
            parts.next().map(|fp| fp.to_string())
        })
        .collect();

    Ok(fingerprints)
}

#[tauri::command]
pub fn add_key_to_agent(key_path: String) -> Result<()> {
    let output = Command::new("ssh-add")
        .arg(&key_path)
        .output()
        .map_err(|e| AppError::Process(format!("ssh-add failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SshKey(format!("ssh-add failed: {}", stderr)));
    }

    Ok(())
}

#[tauri::command]
pub fn remove_key_from_agent(key_path: String) -> Result<()> {
    let output = Command::new("ssh-add")
        .args(["-d", &key_path])
        .output()
        .map_err(|e| AppError::Process(format!("ssh-add failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SshKey(format!("ssh-add -d failed: {}", stderr)));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ed25519_fingerprint() {
        let line = "256 SHA256:abc123 alice@laptop (ED25519)";
        let (fp, alg, bits, comment) = parse_fingerprint_line(line);
        assert_eq!(fp, "SHA256:abc123");
        assert_eq!(alg, "ed25519");
        assert_eq!(bits, Some(256));
        assert_eq!(comment, Some("alice@laptop".to_string()));
    }

    #[test]
    fn parse_rsa_fingerprint() {
        let line = "2048 SHA256:xyz987 my-key (RSA)";
        let (_fp, alg, bits, comment) = parse_fingerprint_line(line);
        assert_eq!(alg, "rsa");
        assert_eq!(bits, Some(2048));
        assert_eq!(comment, Some("my-key".to_string()));
    }

    #[test]
    fn parse_fingerprint_none_comment_returns_none() {
        let line = "256 SHA256:abc (none) (ED25519)";
        let (_fp, _alg, _bits, comment) = parse_fingerprint_line(line);
        assert_eq!(comment, None);
    }

    #[test]
    fn parse_fingerprint_empty_comment_returns_none() {
        let line = "256 SHA256:abc  (ED25519)";
        let (_fp, _alg, _bits, comment) = parse_fingerprint_line(line);
        assert_eq!(comment, None);
    }

    #[test]
    fn parse_fingerprint_no_algorithm() {
        let line = "256 SHA256:abc alice@host";
        let (_fp, alg, _bits, _comment) = parse_fingerprint_line(line);
        assert_eq!(alg, "unknown");
    }

    #[test]
    fn parse_fingerprint_empty_string() {
        let (fp, alg, bits, comment) = parse_fingerprint_line("");
        assert_eq!(fp, "");
        assert_eq!(alg, "unknown");
        assert_eq!(bits, None);
        assert_eq!(comment, None);
    }

    #[test]
    fn format_timestamp_epoch_zero() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
    }

    #[test]
    fn format_timestamp_known_date() {
        // 2024-01-01 00:00 UTC = 1704067200
        assert_eq!(format_timestamp(1_704_067_200), "2024-01-01 00:00");
    }

    #[test]
    fn format_timestamp_with_time() {
        let ts = 1_704_067_200 + 13 * 3600 + 30 * 60;
        assert_eq!(format_timestamp(ts), "2024-01-01 13:30");
    }

    #[test]
    fn leap_year_div_4() { assert!(is_leap(2024)); }
    #[test]
    fn non_leap_century() { assert!(!is_leap(1900)); }
    #[test]
    fn leap_400() { assert!(is_leap(2000)); }
    #[test]
    fn non_leap_ordinary() { assert!(!is_leap(2023)); }
}
