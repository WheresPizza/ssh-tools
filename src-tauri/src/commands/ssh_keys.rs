use crate::error::{AppError, Result};
use crate::types::{KeyAlgorithm, KeyGenParams, SshKeyInfo};
use crate::utils::{
    permissions::*,
    ssh_dir::{get_ssh_dir, validate_key_filename, validate_key_in_dir, validate_key_path},
};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process::Command;
use std::process::Stdio;

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
        if let Ok(key) = ssh_key::PrivateKey::from_openssh(&*bytes) {
            let mut public = key.public_key().clone();
            let pub_path = PathBuf::from(format!("{}.pub", path.display()));
            if pub_path.exists() {
                reject_symlink(&pub_path)?;
                let companion =
                    ssh_key::PublicKey::from_openssh(&std::fs::read_to_string(&pub_path)?)
                        .map_err(key_error)?;
                if companion.key_data() != public.key_data() {
                    return Err(key_error("Public key does not match the private key"));
                }
                public.set_comment(companion.comment());
            }
            return Ok((public, key.is_encrypted()));
        }
    }
    // ssh-key 0.6 cannot decode valid ECDSA mpints shorter than the curve width.
    // OpenSSH validates such unencrypted keys; do not classify them as broken.
    // Legacy PEM is delegated to OpenSSH with an explicitly empty passphrase.
    // Distinguish encryption from file/format/permission errors.
    let output = crate::utils::process::output(
        crate::utils::ssh_runtime::command("ssh-keygen")?
            .env("LC_ALL", "C")
            .args([
                "-y",
                "-P",
                "",
                "-f",
                path.to_str().ok_or_else(|| key_error("Invalid key path"))?,
            ])
            .stdin(Stdio::null()),
        std::time::Duration::from_secs(30),
    )?;
    if output.status.success() {
        let mut public =
            ssh_key::PublicKey::from_openssh(String::from_utf8_lossy(&output.stdout).trim())
                .map_err(key_error)?;
        let companion = PathBuf::from(format!("{}.pub", path.display()));
        if companion.exists() {
            reject_symlink(&companion)?;
            let stored = ssh_key::PublicKey::from_openssh(&std::fs::read_to_string(companion)?)
                .map_err(key_error)?;
            if stored.key_data() != public.key_data() {
                return Err(key_error("Public key does not match the private key"));
            }
            public.set_comment(stored.comment());
        }
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

    if matches!(
        params.algorithm,
        KeyAlgorithm::EcdsaP256 | KeyAlgorithm::EcdsaP384
    ) {
        // Generate ECDSA in the same codec that encrypts it, avoiding its short-mpint
        // decoding defect. OpenSSH interoperability is checked for both curves.
        let curve = if matches!(params.algorithm, KeyAlgorithm::EcdsaP256) {
            ssh_key::EcdsaCurve::NistP256
        } else {
            ssh_key::EcdsaCurve::NistP384
        };
        let mut private = ssh_key::PrivateKey::random(
            &mut ssh_key::rand_core::OsRng,
            ssh_key::Algorithm::Ecdsa { curve },
        )
        .map_err(key_error)?;
        private.set_comment(params.comment.clone());
        std::fs::write(
            &generated_path,
            private
                .to_openssh(ssh_key::LineEnding::LF)
                .map_err(key_error)?
                .as_bytes(),
        )?;
        std::fs::write(
            staging.path().join("key.pub"),
            format!(
                "{}\n",
                private.public_key().to_openssh().map_err(key_error)?
            ),
        )?;
    } else {
        let output = crate::utils::process::output(
            crate::utils::ssh_runtime::command("ssh-keygen")?.args(&cmd_args),
            std::time::Duration::from_secs(30),
        )?;
        if !output.status.success() {
            return Err(key_error(format!(
                "ssh-keygen failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
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
    // Validate and set permissions before publishing either file.
    parse_key_info(&generated_path)?;
    set_private_key_permissions(&generated_path)?;
    set_public_key_permissions(&staging.path().join("key.pub"))?;
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
    let _operation = super::operations::OPERATION_LOCK
        .lock()
        .map_err(|_| key_error("Operation lock unavailable"))?;
    if super::operations::list_blocking()?
        .iter()
        .any(|op| op.pending() && op.fingerprint.as_deref() == Some(&expected_fingerprint))
    {
        return Err(key_error(
            "Finish or interrupt the pending terminal operation before deleting this key.",
        ));
    }
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
    match remove_key_from_agent_inner(key_path.clone()) {
        Ok(()) | Err(AppError::AgentUnavailable) => {}
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
    crate::utils::ssh_runtime::AgentContext::resolve()?.fingerprints()
}

#[tauri::command(rename_all = "snake_case")]
pub async fn add_key_to_agent(
    key_path: String,
    options: Option<super::agent_policy::AgentOptions>,
) -> Result<super::operations::Operation> {
    tauri::async_runtime::spawn_blocking(move || {
        add_key_to_agent_blocking(key_path, options.unwrap_or_default())
    })
    .await
    .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}

fn add_key_to_agent_blocking(
    key_path: String,
    options: super::agent_policy::AgentOptions,
) -> Result<super::operations::Operation> {
    use super::{launcher::shell_quote, operations};
    use crate::utils::ssh_runtime::{self, AgentContext};
    let _guard = operations::OPERATION_LOCK
        .lock()
        .map_err(|_| AppError::Process("Operation lock unavailable".into()))?;
    let args = options.args()?;
    validate_key_path(&key_path)?;
    let agent = AgentContext::resolve()?;
    agent.fingerprints()?;
    let fingerprint = get_key_fingerprint_blocking(key_path.clone())?;
    if read_key(std::path::Path::new(&key_path))?.1 {
        let mut arguments = args;
        arguments.push(key_path.clone());
        let command = format!(
            "/usr/bin/env SSH_AUTH_SOCK={} SSH_ASKPASS_REQUIRE=never LC_ALL=C {} {}",
            shell_quote(&agent.socket.to_string_lossy()),
            shell_quote(&ssh_runtime::program("ssh-add")?.to_string_lossy()),
            arguments
                .iter()
                .map(|arg| shell_quote(arg))
                .collect::<Vec<_>>()
                .join(" ")
        );
        return operations::launch(
            "agent_add",
            key_path,
            &command,
            Some(agent),
            Some(fingerprint),
            Some(options),
        );
    }
    let mut command = agent.command()?;
    command
        .env("SSH_ASKPASS_REQUIRE", "never")
        .args(&args)
        .arg(&key_path);
    let output = crate::utils::process::run(command, std::time::Duration::from_secs(10))?;
    if output.timed_out {
        return Err(AppError::SshKey(
            "ssh-add timed out after 10 seconds".into(),
        ));
    }
    if !output.status.success() {
        return Err(AppError::SshKey(format!(
            "ssh-add failed: {}",
            output.stderr
        )));
    }
    if !agent.fingerprints()?.contains(&fingerprint) {
        return Err(AppError::SshKey(
            "ssh-add exited successfully but the fingerprint was not found in the selected agent"
                .into(),
        ));
    }
    super::agent_policy::record(fingerprint.clone(), &options, false);
    Ok(operations::Operation::completed(key_path, fingerprint))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn remove_key_from_agent(key_path: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || remove_key_from_agent_blocking(key_path))
        .await
        .map_err(|e| AppError::Process(format!("Background operation failed: {e}")))?
}
fn remove_key_from_agent_blocking(key_path: String) -> Result<()> {
    let _guard = super::operations::OPERATION_LOCK
        .lock()
        .map_err(|_| AppError::Process("Operation lock unavailable".into()))?;
    remove_key_from_agent_inner(key_path)
}
fn remove_key_from_agent_inner(key_path: String) -> Result<()> {
    validate_key_path(&key_path)?;
    let fingerprint = get_key_fingerprint_blocking(key_path.clone())?;
    if super::operations::list_blocking()?
        .iter()
        .any(|op| op.pending() && op.fingerprint.as_deref() == Some(&fingerprint))
    {
        return Err(AppError::InvalidInput(
            "Finish or interrupt the pending terminal operation before removing this key.".into(),
        ));
    }
    let agent = crate::utils::ssh_runtime::AgentContext::resolve()?;
    if !agent.fingerprints()?.contains(&fingerprint) {
        super::agent_policy::forget(&fingerprint);
        return Ok(());
    }
    let mut command = agent.command()?;
    command.args(["-d", &key_path]);
    let output = crate::utils::process::run(command, std::time::Duration::from_secs(5))?;
    if !output.status.success() || output.timed_out {
        return Err(AppError::SshKey(format!(
            "ssh-add -d failed: {}",
            output.stderr
        )));
    }
    if agent.fingerprints()?.contains(&fingerprint) {
        return Err(AppError::SshKey(
            "The key is still present in the selected agent".into(),
        ));
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
    fn reads_valid_openssh_ecdsa_with_short_private_scalar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture");
        std::fs::write(
            &path,
            include_bytes!("../../tests/fixtures/ecdsa-short-scalar"),
        )
        .unwrap();
        set_private_key_permissions(&path).unwrap();
        let info = parse_key_info(&path).expect("OpenSSH mpints may omit leading zero bytes");
        assert_eq!(info.algorithm, "ecdsa");
        assert_eq!(info.has_passphrase, Some(false));
        assert!(info.fingerprint.starts_with("SHA256:"));
    }
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
