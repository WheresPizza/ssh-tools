use crate::{
    error::{AppError, Result},
    utils::{process, ssh_dir::*},
};
use serde::Serialize;
use std::{path::PathBuf, process::Command, time::Duration};
#[derive(Serialize)]
pub struct DiagnosticStep {
    pub status: String,
    pub title: String,
    pub detail: String,
    pub suggestion: Option<String>,
}
#[derive(Serialize)]
pub struct DiagnosticReport {
    pub alias: String,
    pub config_path: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<String>,
    pub identities_only: bool,
    pub identity_files: Vec<String>,
    pub agent_fingerprints: Vec<String>,
    pub authenticated: Option<bool>,
    pub steps: Vec<DiagnosticStep>,
}
impl DiagnosticReport {
    fn step(
        &mut self,
        status: &str,
        title: &str,
        detail: impl Into<String>,
        suggestion: Option<&str>,
    ) {
        self.steps.push(DiagnosticStep {
            status: status.into(),
            title: title.into(),
            detail: detail.into(),
            suggestion: suggestion.map(str::to_string),
        });
    }
}
fn unsafe_evaluation(content: &str) -> bool {
    content.lines().any(|line| {
        let line = line.trim();
        let split = line
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(line.len());
        let first = &line[..split];
        let rest = line[split..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');
        let parts = shlex::split(rest).unwrap_or_default();
        if first.eq_ignore_ascii_case("match") {
            return parts
                .iter()
                .any(|p| p.eq_ignore_ascii_case("exec") || p.eq_ignore_ascii_case("!exec"));
        }
        if first.eq_ignore_ascii_case("include") {
            return parts.iter().any(|p| {
                p.contains('%') || p.contains('$') || (p.starts_with('~') && !p.starts_with("~/"))
            });
        }
        false
    })
}
fn classify_failure(stderr: &str, timed_out: bool) -> (&'static str, &'static str) {
    if stderr.contains("REMOTE HOST IDENTIFICATION HAS CHANGED") {
        ("Server identity changed", "Verify the new server fingerprint through a trusted channel before changing Known Hosts.")
    } else if stderr.contains("Host key verification failed")
        || stderr.contains("No ED25519 host key is known")
        || stderr.contains("No RSA host key is known")
        || stderr.contains("No ECDSA host key is known")
    {
        ("Server identity is not trusted", "Connect in your terminal and verify the fingerprint before accepting the host key, then retry.")
    } else if stderr.contains("Could not resolve hostname") {
        (
            "Hostname could not be resolved",
            "Check the hostname, DNS and VPN connection.",
        )
    } else if stderr.contains("Connection refused") {
        (
            "SSH port refused the connection",
            "Check the port and whether the SSH service is running.",
        )
    } else if stderr.contains("Permission denied") {
        ("Authentication rejected", "Check the SSH username, register the public key with the intended account, and load encrypted keys into the agent.")
    } else if timed_out || stderr.contains("timed out") {
        (
            "Connection timed out",
            "Check network access, VPN, firewall and ProxyJump settings.",
        )
    } else {
        ("SSH check failed", "Review the effective host and key configuration; try connecting in the terminal for interactive authentication.")
    }
}
pub(crate) fn diagnose_blocking(
    alias: String,
    network: bool,
    port_override: Option<u16>,
) -> Result<DiagnosticReport> {
    super::launcher::validate_destination(&alias)?;
    if port_override == Some(0) {
        return Err(AppError::InvalidInput(
            "Port must be between 1 and 65535".into(),
        ));
    }
    let path = get_ssh_config_path()?;
    let mut report = DiagnosticReport {
        alias: alias.clone(),
        config_path: path.to_string_lossy().into(),
        hostname: None,
        user: None,
        port: None,
        identities_only: false,
        identity_files: Vec::new(),
        agent_fingerprints: Vec::new(),
        authenticated: None,
        steps: Vec::new(),
    };
    let documents = super::ssh_config::config_documents()?;
    if documents.iter().any(|(_, text)| unsafe_evaluation(text)) {
        report.step("warning", "Configuration evaluation needs an interactive review", "Match exec or dynamically expanded Include paths could execute local commands while SSH evaluates its configuration. Automatic diagnostics did not run them.", Some("Review those directives and use your terminal for this configuration."));
        return Ok(report);
    }
    let mut command = Command::new("ssh");
    command.env("LC_ALL", "C").arg("-G").arg("-F").arg(&path);
    if let Some(port) = port_override {
        command.arg("-p").arg(port.to_string());
    }
    command.arg("--").arg(&alias);
    let effective = process::run(command, Duration::from_secs(5))?;
    if !effective.status.success() || effective.timed_out {
        report.step(
            "error",
            "Cannot evaluate SSH configuration",
            effective.stderr,
            Some("Fix SSH configuration syntax or restore the previous version."),
        );
        return Ok(report);
    }
    let fields: Vec<_> = effective
        .stdout
        .lines()
        .filter_map(|l| l.split_once(' '))
        .collect();
    let value = |name: &str| {
        fields
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.to_string())
    };
    report.hostname = value("hostname");
    report.user = value("user");
    report.port = value("port");
    report.identities_only = value("identitiesonly").as_deref() == Some("yes");
    report.identity_files = fields
        .iter()
        .filter(|(k, _)| *k == "identityfile")
        .map(|(_, v)| v.to_string())
        .collect();
    report.step(
        "ok",
        "Effective SSH configuration",
        format!(
            "OpenSSH evaluated {} configuration file(s). Host {}, user {}, port {}.",
            documents.len(),
            report.hostname.as_deref().unwrap_or("?"),
            report.user.as_deref().unwrap_or("?"),
            report.port.as_deref().unwrap_or("?")
        ),
        None,
    );
    match super::ssh_keys::list_agent_keys_blocking() {
        Ok(keys) => { report.agent_fingerprints = keys; report.step("ok", "SSH agent", format!("{} identities available", report.agent_fingerprints.len()), None); }
        Err(e) => report.step("warning", "SSH agent unavailable", e.to_string(), Some("Start an SSH agent and launch the app with its SSH_AUTH_SOCK, or use an unencrypted configured key.")),
    }
    let ssh_root = get_ssh_dir()?;
    let mut usable = 0;
    for identity in report.identity_files.clone() {
        if identity == "none" {
            continue;
        }
        let raw = identity.trim_matches('"');
        let path = if let Some(tail) = raw.strip_prefix("~/.ssh/") {
            ssh_root.join(tail)
        } else if let Some(tail) = raw.strip_prefix("~/") {
            dirs::home_dir().unwrap_or_default().join(tail)
        } else {
            PathBuf::from(raw)
        };
        if raw.contains('%') || raw.contains('$') {
            report.step("warning", "Dynamic identity path", identity, Some("OpenSSH expands this path at connection time; use the authentication test to check it."));
            continue;
        }
        if !path.exists() {
            report.step("warning", "Identity file not found", identity, Some("Select an existing key for this host or profile. Missing default key paths can be harmless."));
            continue;
        }
        match super::ssh_keys::parse_key_info(&path) {
            Ok(key) => {
                usable += 1;
                let encrypted_without_agent = key.has_passphrase == Some(true)
                    && !report.agent_fingerprints.contains(&key.fingerprint);
                report.step(if encrypted_without_agent { "warning" } else { "ok" }, "Configured key", format!("{} · {}", identity, key.fingerprint), if encrypted_without_agent { Some("Add this encrypted key to the agent; batch authentication cannot ask for its passphrase.") } else { None });
            }
            Err(e) => report.step(
                "error",
                "Cannot read configured key",
                format!("{identity}: {e}"),
                Some("Check the key format, companion public key and file permissions."),
            ),
        }
    }
    if usable == 0 && report.agent_fingerprints.is_empty() {
        report.step(
            "warning",
            "No available key found",
            "Neither a readable configured identity nor an agent key was found.",
            Some("Generate or import a key, then assign it to this host."),
        );
    }
    if !report.identities_only {
        report.step(
            "warning",
            "Agent may offer additional identities",
            "IdentitiesOnly is disabled for this destination.",
            Some("Enable IdentitiesOnly when separating Git accounts."),
        );
    }
    if !network {
        report.step(
            "warning",
            "Authentication not tested",
            "Local checks do not prove that the remote account accepts the key.",
            Some("Run Test authentication when the server is available."),
        );
        return Ok(report);
    }
    // No shell or remote command, no forwards, no interactive prompts, no changes to trust stores.
    let mut command = Command::new("ssh");
    command
        .env("LC_ALL", "C")
        .args(["-vv", "-N", "-T", "-F"])
        .arg(&path)
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "UpdateHostKeys=no",
            "-o",
            "ConnectTimeout=5",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "ClearAllForwardings=yes",
            "-o",
            "PermitLocalCommand=no",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "NumberOfPasswordPrompts=0",
        ]);
    if workspace_root()?.is_some() {
        command
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                get_known_hosts_path()?.display()
            ))
            .args(["-o", "GlobalKnownHostsFile=/dev/null"]);
    }
    if let Some(port) = port_override {
        command.arg("-p").arg(port.to_string());
    }
    command.arg("--").arg(&alias);
    let output = process::run(command, Duration::from_secs(8))?;
    let auth_line = output
        .stderr
        .lines()
        .find(|line| line.starts_with("Authenticated to ") && line.contains("using \"publickey\""));
    if let Some(line) = auth_line {
        report.authenticated = Some(true);
        report.step("ok", "Public-key authentication succeeded", line, None);
    } else {
        report.authenticated = Some(false);
        let (title, suggestion) = classify_failure(&output.stderr, output.timed_out);
        report.step(
            "error",
            title,
            output
                .stderr
                .lines()
                .filter(|l| !l.starts_with("debug"))
                .take(12)
                .collect::<Vec<_>>()
                .join("\n"),
            Some(suggestion),
        );
    }
    let offered = output
        .stderr
        .lines()
        .filter(|l| {
            l.starts_with("debug1: Offering public key:")
                || l.starts_with("debug1: Server accepts key:")
        })
        .take(16)
        .collect::<Vec<_>>();
    if !offered.is_empty() {
        report.step(
            "ok",
            "Keys offered during this test",
            offered.join("\n"),
            None,
        );
    }
    Ok(report)
}
#[tauri::command(rename_all = "snake_case")]
pub async fn diagnose_ssh(
    alias: String,
    network: bool,
    port: Option<u16>,
) -> Result<DiagnosticReport> {
    tauri::async_runtime::spawn_blocking(move || diagnose_blocking(alias, network, port))
        .await
        .map_err(|e| AppError::Process(e.to_string()))?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsafe_config_is_not_evaluated() {
        assert!(unsafe_evaluation("Match host * exec \"touch /tmp/no\""));
        assert!(unsafe_evaluation("Include ~/.ssh/%h/config"));
        assert!(!unsafe_evaluation(
            "# Match exec ignored\nHost normal\n Include conf.d/*"
        ));
    }
    #[test]
    fn failures_have_actionable_causes() {
        assert_eq!(
            classify_failure("Permission denied (publickey).", false).0,
            "Authentication rejected"
        );
        assert_eq!(
            classify_failure("Host key verification failed", false).0,
            "Server identity is not trusted"
        );
    }
}
