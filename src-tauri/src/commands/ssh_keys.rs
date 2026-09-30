use crate::error::{AppError, Result};
use crate::types::{KeyAlgorithm, KeyGenParams, SshKeyInfo};
use crate::utils::{
    permissions::*,
    ssh_dir::{get_ssh_dir, validate_key_filename, validate_key_in_dir, validate_key_path},
};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn get_key_algorithm_str(alg: &KeyAlgorithm) -> (&'static str, &'static str, Option<&'static str>) {
    match alg {
        KeyAlgorithm::Ed25519 => ("ed25519", "ed25519", None),
        KeyAlgorithm::Rsa2048 => ("rsa", "rsa", Some("2048")),
        KeyAlgorithm::Rsa4096 => ("rsa", "rsa", Some("4096")),
        KeyAlgorithm::EcdsaP256 => ("ecdsa", "ecdsa", Some("256")),
        KeyAlgorithm::EcdsaP384 => ("ecdsa", "ecdsa", Some("384")),
    }
}

fn key_error(error: impl std::fmt::Display) -> AppError {
    AppError::SshKey(error.to_string())
}

pub(crate) fn read_key(path: &Path) -> Result<(ssh_key::PublicKey, bool)> {
    if std::fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(key_error("Key file is too large"));
    }
    let bytes = zeroize::Zeroizing::new(std::fs::read(path)?);
    if bytes.starts_with(b"-----BEGIN OPENSSH PRIVATE KEY-----") {
        let key = ssh_key::PrivateKey::from_openssh(&*bytes).map_err(key_error)?;
        let mut public = key.public_key().clone();
        let pub_path = PathBuf::from(format!("{}.pub", path.display()));
        if pub_path.exists() {
            reject_symlink(&pub_path)?;
            let companion = ssh_key::PublicKey::from_openssh(&std::fs::read_to_string(&pub_path)?)
                .map_err(key_error)?;
            if companion.key_data() != public.key_data() {
                return Err(key_error("Public key does not match the private key"));
            }
            public.set_comment(companion.comment());
        }
        return Ok((public, key.is_encrypted()));
    }
    // Legacy PEM is delegated to OpenSSH with an explicitly empty passphrase.
    // Distinguish encryption from file/format/permission errors.
    let output = Command::new("ssh-keygen")
        .env("LC_ALL", "C")
        .args([
            "-y",
            "-P",
            "",
            "-f",
            path.to_str().ok_or_else(|| key_error("Invalid key path"))?,
        ])
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        let public =
            ssh_key::PublicKey::from_openssh(String::from_utf8_lossy(&output.stdout).trim())
                .map_err(key_error)?;
        return Ok((public, false));
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("incorrect passphrase") || error.contains("bad passphrase") {
        let pub_path = PathBuf::from(format!("{}.pub", path.display()));
        reject_symlink(&pub_path)?;
        let public = ssh_key::PublicKey::from_openssh(
            &std::fs::read_to_string(pub_path)
                .map_err(|_| key_error("Encrypted legacy key needs its .pub companion"))?,
        )
        .map_err(key_error)?;
        Ok((public, true))
    } else {
        Err(key_error(error.trim()))
    }
}

pub(crate) fn parse_key_info(private_path: &Path) -> Result<SshKeyInfo> {
    validate_key_in_dir(
        private_path
            .parent()
            .ok_or_else(|| key_error("Missing key directory"))?,
        private_path,
    )?;
    let (public, encrypted) = read_key(private_path)?;
    let key_data = public.key_data();
    let algorithm = public.algorithm().to_string();
    let bits = if let Some(rsa) = key_data.rsa() {
        let n = rsa.n.as_bytes();
        let trimmed = n
            .iter()
            .position(|b| *b != 0)
            .map(|i| &n[i..])
            .unwrap_or(&[]);
        trimmed
            .first()
            .map(|first| (trimmed.len() as u32 * 8) - first.leading_zeros())
    } else if key_data.ed25519().is_some() {
        Some(256)
    } else if algorithm.contains("nistp256") {
        Some(256)
    } else if algorithm.contains("nistp384") {
        Some(384)
    } else if algorithm.contains("nistp521") {
        Some(521)
    } else {
        None
    };
    let created_at = std::fs::metadata(private_path)?.modified().ok().map(|t| {
        format_timestamp(
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )
    });
    Ok(SshKeyInfo {
        name: private_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        private_path: private_path.to_string_lossy().into(),
        public_path: format!("{}.pub", private_path.display()),
        public_key_exists: PathBuf::from(format!("{}.pub", private_path.display())).is_file(),
        algorithm: if key_data.ed25519().is_some() {
            "ed25519".into()
        } else if key_data.rsa().is_some() {
            "rsa".into()
        } else if key_data.ecdsa().is_some() {
            "ecdsa".into()
        } else {
            algorithm
        },
        bits,
        fingerprint: public.fingerprint(ssh_key::HashAlg::Sha256).to_string(),
        comment: if public.comment().is_empty() {
            None
        } else {
            Some(public.comment().into())
        },
        has_passphrase: Some(encrypted),
        created_at,
        error: None,
    })
}

#[cfg(test)]
fn parse_fingerprint_line(line: &str) -> (String, String, Option<u32>, Option<String>) {
    // Format: "256 SHA256:xxxx comment (ED25519)"
    // or: "2048 SHA256:xxxx comment (RSA)"
    let parts: Vec<&str> = line.trim().splitn(4, ' ').collect();

    let bits: Option<u32> = parts.first().and_then(|s| s.parse().ok());
    let fingerprint = parts.get(1).unwrap_or(&"").to_string();
    let rest = parts.get(2..).unwrap_or(&[]).join(" ");

    // Extract algorithm from parentheses at end
    let algorithm = if let (Some(start), Some(end)) = (rest.rfind('('), rest.rfind(')')) {
        rest[start + 1..end].to_lowercase()
    } else {
        "unknown".to_string()
    };

    // Comment is everything before the algorithm parentheses
    let comment = if let Some(paren_pos) = rest.rfind('(') {
        let c = rest[..paren_pos].trim().to_string();
        if c.is_empty() || c == "(none)" {
            None
        } else {
            Some(c)
        }
    } else {
        None
    };

    (fingerprint, algorithm, bits, comment)
}

fn format_timestamp(secs: u64) -> String {
    // Simple ISO-like date formatting without external crate
    let days_since_epoch = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;

    // Compute year/month/day from days_since_epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days_since_epoch as u32);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year, month, day, hours, minutes
    )
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= *md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command(rename_all = "snake_case")]
pub async fn list_ssh_keys() -> Result<Vec<SshKeyInfo>> {
    tauri::async_runtime::spawn_blocking(move || list_ssh_keys_blocking())
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn list_ssh_keys_blocking() -> Result<Vec<SshKeyInfo>> {
    list_keys_in_dir(&get_ssh_dir()?)
}

pub(crate) fn list_keys_in_dir(ssh_dir: &Path) -> Result<Vec<SshKeyInfo>> {
    let mut keys = Vec::new();
    for path in crate::utils::ssh_dir::regular_files(ssh_dir)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.ends_with(".pub")
            || name.ends_with(".ssh-gui.bak")
            || ["config", "known_hosts", "authorized_keys"].contains(&name.as_ref())
        {
            continue;
        }
        if validate_key_in_dir(ssh_dir, &path).is_err() {
            // Preserve unreadable private-key candidates in the list instead of silently hiding them.
            if !name.starts_with("id_")
                && !PathBuf::from(format!("{}.pub", path.display())).exists()
            {
                continue;
            }
        }
        let label = path
            .strip_prefix(ssh_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        match parse_key_info(&path) {
            Ok(mut key) => {
                key.name = label;
                keys.push(key);
            }
            Err(error) => keys.push(SshKeyInfo {
                name: label,
                private_path: path.to_string_lossy().into(),
                public_path: format!("{}.pub", path.display()),
                public_key_exists: PathBuf::from(format!("{}.pub", path.display())).is_file(),
                algorithm: "unreadable".into(),
                bits: None,
                fingerprint: String::new(),
                comment: None,
                has_passphrase: None,
                created_at: None,
                error: Some(error.to_string()),
            }),
        }
    }
    Ok(keys)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn generate_ssh_key(params: KeyGenParams) -> Result<SshKeyInfo> {
    tauri::async_runtime::spawn_blocking(move || generate_ssh_key_blocking(params))
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn generate_ssh_key_blocking(params: KeyGenParams) -> Result<SshKeyInfo> {
    validate_key_filename(&params.filename)?;
    generate_key_in_dir(params, &get_ssh_dir()?)
}

fn generate_key_in_dir(mut params: KeyGenParams, ssh_dir: &Path) -> Result<SshKeyInfo> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| AppError::InvalidInput("SSH write lock unavailable".into()))?;
    validate_key_filename(&params.filename)?;
    if params.comment.chars().any(char::is_control)
        || params
            .passphrase
            .as_ref()
            .is_some_and(|p| p.chars().any(char::is_control))
    {
        return Err(key_error(
            "Comments and passphrases must not contain control characters",
        ));
    }
    reject_symlink(&ssh_dir)?;
    let key_path = ssh_dir.join(&params.filename);
    let pub_path = ssh_dir.join(format!("{}.pub", params.filename));
    for path in [&key_path, &pub_path] {
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(AppError::InvalidInput(
                "Private or public key filename already exists".into(),
            ));
        }
    }
    // Generate inside a private temporary directory, then publish without overwriting existing files.
    let staging = tempfile::tempdir_in(&ssh_dir)?;
    let generated_path = staging.path().join("key");
    let (type_flag, _, bits_opt) = get_key_algorithm_str(&params.algorithm);

    let mut cmd_args = vec![
        "-t".to_string(),
        type_flag.to_string(),
        "-C".to_string(),
        params.comment.clone(),
        "-f".to_string(),
        generated_path.to_str().unwrap_or("").to_string(),
    ];

    if let Some(bits) = bits_opt {
        cmd_args.push("-b".to_string());
        cmd_args.push(bits.to_string());
    }

    let passphrase = zeroize::Zeroizing::new(params.passphrase.take().unwrap_or_default());
    cmd_args.push("-N".to_string());
    cmd_args.push(String::new());

    let output = Command::new("ssh-keygen")
        .args(&cmd_args)
        .output()
        .map_err(|e| AppError::Process(format!("Failed to run ssh-keygen: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SshKey(format!("ssh-keygen failed: {}", stderr)));
    }

    if !passphrase.is_empty() {
        let bytes = zeroize::Zeroizing::new(std::fs::read(&generated_path)?);
        let private = ssh_key::PrivateKey::from_openssh(&*bytes).map_err(key_error)?;
        let encrypted = private
            .encrypt(&mut ssh_key::rand_core::OsRng, passphrase.as_bytes())
            .map_err(key_error)?;
        let pem = encrypted
            .to_openssh(ssh_key::LineEnding::LF)
            .map_err(key_error)?;
        std::fs::write(&generated_path, pem.as_bytes())?;
    }
    std::fs::hard_link(&generated_path, &key_path)?;
    if let Err(e) = std::fs::hard_link(staging.path().join("key.pub"), &pub_path) {
        let _ = std::fs::remove_file(&key_path);
        return Err(e.into());
    }
    // Set correct permissions
    set_private_key_permissions(&key_path)?;
    let pub_path = PathBuf::from(format!("{}.pub", key_path.display()));
    if pub_path.exists() {
        set_public_key_permissions(&pub_path)?;
    }

    parse_key_info(&key_path)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_public_key(key_path: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || get_public_key_blocking(key_path))
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

pub(crate) fn get_public_key_blocking(key_path: String) -> Result<String> {
    validate_key_path(&key_path)?;
    let (public, _) = read_key(Path::new(&key_path))?;
    public.to_openssh().map_err(key_error)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_ssh_key(key_path: String, expected_fingerprint: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        delete_ssh_key_blocking(key_path, expected_fingerprint)
    })
    .await
    .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn delete_ssh_key_blocking(key_path: String, expected_fingerprint: String) -> Result<()> {
    validate_key_path(&key_path)?;
    let path = PathBuf::from(&key_path);
    let pub_path = PathBuf::from(format!("{}.pub", key_path));

    reject_symlink(&pub_path)?;
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| key_error("SSH write lock unavailable"))?;
    if pub_path.exists() && !std::fs::metadata(&pub_path)?.is_file() {
        return Err(key_error("Public key path is not a regular file"));
    }
    if get_key_fingerprint_blocking(key_path.clone())? != expected_fingerprint {
        return Err(key_error(
            "Key changed since loading; refresh before deleting",
        ));
    }
    match list_agent_keys_blocking() {
        Ok(keys) => {
            if keys.contains(&get_key_fingerprint_blocking(key_path.clone())?) {
                remove_key_from_agent_blocking(key_path.clone())?;
            }
        }
        Err(AppError::AgentUnavailable) => {}
        Err(e) => return Err(e),
    }
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    if pub_path.exists() {
        std::fs::remove_file(&pub_path)?;
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_key_fingerprint(key_path: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || get_key_fingerprint_blocking(key_path))
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn get_key_fingerprint_blocking(key_path: String) -> Result<String> {
    validate_key_path(&key_path)?;
    Ok(read_key(Path::new(&key_path))?
        .0
        .fingerprint(ssh_key::HashAlg::Sha256)
        .to_string())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_agent_keys() -> Result<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || list_agent_keys_blocking())
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

pub(crate) fn list_agent_keys_blocking() -> Result<Vec<String>> {
    let output = Command::new("ssh-add")
        .arg("-l")
        .output()
        .map_err(|e| AppError::Process(format!("ssh-add failed: {}", e)))?;

    // OpenSSH distinguishes an empty agent from an unreachable agent.
    match output.status.code() {
        Some(1) => return Ok(Vec::new()),
        Some(2) => return Err(AppError::AgentUnavailable),
        _ => {}
    }

    if !output.status.success() {
        return Err(AppError::Process(
            "Unable to list SSH agent identities".into(),
        ));
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

#[tauri::command(rename_all = "snake_case")]
pub async fn add_key_to_agent(
    key_path: String,
    options: Option<super::agent_policy::AgentOptions>,
) -> Result<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        add_key_to_agent_blocking(key_path, options.unwrap_or_default())
    })
    .await
    .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn add_key_to_agent_blocking(
    key_path: String,
    options: super::agent_policy::AgentOptions,
) -> Result<bool> {
    let args = options.args()?;
    validate_key_path(&key_path)?;
    list_agent_keys_blocking()?;
    let fingerprint = get_key_fingerprint_blocking(key_path.clone())?;
    if read_key(std::path::Path::new(&key_path))?.1 {
        let mut arguments = args.clone();
        arguments.push(key_path.clone());
        let quoted = arguments
            .iter()
            .map(|arg| crate::commands::launcher::shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ");
        crate::commands::launcher::launch_terminal_command(&format!(
            "env SSH_AUTH_SOCK={} ssh-add {}",
            crate::commands::launcher::shell_quote(
                &std::env::var("SSH_AUTH_SOCK").map_err(|_| AppError::AgentUnavailable)?
            ),
            quoted
        ))?;
        super::agent_policy::record(fingerprint, &options, true);
        return Ok(false);
    }
    let mut command = Command::new("ssh-add");
    command
        .env("SSH_ASKPASS_REQUIRE", "never")
        .args(&args)
        .arg(&key_path);
    let output = crate::utils::process::run(command, std::time::Duration::from_secs(10))?;
    if !output.status.success() || output.timed_out {
        return Err(AppError::SshKey(format!(
            "ssh-add failed: {}",
            output.stderr
        )));
    }
    super::agent_policy::record(fingerprint, &options, false);
    Ok(true)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_key_from_agent(key_path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || remove_key_from_agent_blocking(key_path))
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn remove_key_from_agent_blocking(key_path: String) -> Result<()> {
    validate_key_path(&key_path)?;
    let fingerprint = get_key_fingerprint_blocking(key_path.clone())?;
    let output = Command::new("ssh-add")
        .args(["-d", &key_path])
        .output()
        .map_err(|e| AppError::Process(format!("ssh-add failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SshKey(format!("ssh-add -d failed: {}", stderr)));
    }
    super::agent_policy::forget(&fingerprint);
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
    fn leap_year_div_4() {
        assert!(is_leap(2024));
    }
    #[test]
    fn non_leap_century() {
        assert!(!is_leap(1900));
    }
    #[test]
    fn leap_400() {
        assert!(is_leap(2000));
    }
    #[test]
    fn non_leap_ordinary() {
        assert!(!is_leap(2023));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    fn generate_real_keypairs_for_every_supported_algorithm() {
        let dir = tempfile::tempdir().unwrap();
        for (index, algorithm) in [
            KeyAlgorithm::Ed25519,
            KeyAlgorithm::Rsa2048,
            KeyAlgorithm::Rsa4096,
            KeyAlgorithm::EcdsaP256,
            KeyAlgorithm::EcdsaP384,
        ]
        .into_iter()
        .enumerate()
        {
            let info = generate_key_in_dir(
                KeyGenParams {
                    algorithm,
                    filename: format!("test-{index}"),
                    comment: "integration test".into(),
                    passphrase: None,
                },
                dir.path(),
            )
            .unwrap();
            assert!(info.fingerprint.starts_with("SHA256:"));
            assert_eq!(info.has_passphrase, Some(false));
            assert!(Path::new(&info.public_path).exists());
            assert_eq!(info.comment.as_deref(), Some("integration test"));
        }
    }
    #[test]
    fn encrypted_algorithms_are_readable_by_openssh() {
        let dir = tempfile::tempdir().unwrap();
        for (index, algorithm) in [
            KeyAlgorithm::Ed25519,
            KeyAlgorithm::Rsa2048,
            KeyAlgorithm::Rsa4096,
            KeyAlgorithm::EcdsaP256,
            KeyAlgorithm::EcdsaP384,
        ]
        .into_iter()
        .enumerate()
        {
            let info = generate_key_in_dir(
                KeyGenParams {
                    algorithm,
                    filename: format!("encrypted-{index}"),
                    comment: "fixture".into(),
                    passphrase: Some("fixture-only-password".into()),
                },
                dir.path(),
            )
            .unwrap();
            let output = Command::new("ssh-keygen")
                .args([
                    "-y",
                    "-P",
                    "fixture-only-password",
                    "-f",
                    &info.private_path,
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let key =
                ssh_key::PublicKey::from_openssh(std::str::from_utf8(&output.stdout).unwrap())
                    .unwrap();
            assert_eq!(
                key.fingerprint(ssh_key::HashAlg::Sha256).to_string(),
                info.fingerprint
            );
        }
    }
    #[test]
    fn encrypted_keys_and_existing_public_files_are_handled() {
        let dir = tempfile::tempdir().unwrap();
        let params = || KeyGenParams {
            algorithm: KeyAlgorithm::Ed25519,
            filename: "encrypted".into(),
            comment: "".into(),
            passphrase: Some("test-only-passphrase".into()),
        };
        let info = generate_key_in_dir(params(), dir.path()).unwrap();
        assert_eq!(info.has_passphrase, Some(true));
        let original = std::fs::read(&info.private_path).unwrap();
        assert!(generate_key_in_dir(params(), dir.path()).is_err());
        assert_eq!(std::fs::read(&info.private_path).unwrap(), original);
        std::fs::remove_file(&info.private_path).unwrap();
        assert!(generate_key_in_dir(params(), dir.path()).is_err());
        assert!(!Path::new(&info.private_path).exists());
    }
}
