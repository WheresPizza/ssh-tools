use crate::commands::app::{get_app_config, save_app_config};
use crate::error::{AppError, Result};
use crate::types::TerminalInfo;
use crate::utils::ssh_runtime;
use crate::utils::terminal::{build_launch_args, detect_terminals};
use std::process::Command;
use std::sync::Mutex;

pub static PREFERRED_TERMINAL: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command(rename_all = "snake_case")]
pub async fn launch_ssh_connection(host_alias: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || launch_ssh_connection_blocking(host_alias))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn launch_ssh_connection_blocking(host_alias: String) -> Result<()> {
    validate_destination(&host_alias)?;
    let command = format!(
        "{} {} {} -- {}",
        terminal_environment()?,
        shell_quote(&ssh_runtime::program("ssh")?.to_string_lossy()),
        connection_arguments()?,
        shell_quote(&host_alias)
    );
    launch_terminal_command(&command)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_detected_terminal() -> Result<Vec<TerminalInfo>> {
    let preferred = PREFERRED_TERMINAL
        .lock()
        .map_err(|_| AppError::Process("Terminal preference unavailable".into()))?
        .clone();
    let mut terminals = detect_terminals();

    // Mark preferred
    if let Some(ref pref) = preferred {
        for t in &mut terminals {
            if &t.name == pref || &t.path == pref {
                t.is_preferred = true;
            }
        }
    }
    if !terminals.iter().any(|t| t.is_preferred) {
        if let Some(t) = terminals.first_mut() {
            t.is_preferred = true;
        }
    }

    Ok(terminals)
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_preferred_terminal(terminal: String) -> Result<()> {
    if !detect_terminals().iter().any(|t| t.name == terminal) {
        return Err(AppError::InvalidInput(
            "Select an installed supported terminal".into(),
        ));
    }
    let mut config = get_app_config()?;
    config.preferred_terminal = Some(terminal.clone());
    save_app_config(config)?;
    *PREFERRED_TERMINAL
        .lock()
        .map_err(|_| AppError::Process("Terminal preference unavailable".into()))? = Some(terminal);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn copy_key_to_server(
    key_path: String,
    host_alias: String,
) -> Result<super::operations::Operation> {
    tauri::async_runtime::spawn_blocking(move || copy_key_to_server_blocking(key_path, host_alias))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn copy_key_to_server_blocking(
    key_path: String,
    host_alias: String,
) -> Result<super::operations::Operation> {
    let _guard = super::operations::OPERATION_LOCK
        .lock()
        .map_err(|_| AppError::Process("Operation lock unavailable".into()))?;
    validate_destination(&host_alias)?;
    crate::utils::ssh_dir::validate_key_path(&key_path)?;
    let pub_path = format!("{}.pub", key_path);
    crate::utils::permissions::reject_symlink(std::path::Path::new(&pub_path))?;
    let public = crate::commands::ssh_keys::get_public_key_blocking(key_path.clone())?;
    if !std::path::Path::new(&pub_path).exists() {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o644);
        }
        let mut file = options.open(&pub_path)?;
        writeln!(file, "{public}")?;
        file.sync_all()?;
    }

    let mut environment = terminal_environment()?;
    if let Some(workspace) = crate::utils::ssh_dir::workspace_root()? {
        let home = workspace.join("transfer-home");
        std::fs::create_dir_all(home.join(".ssh"))?;
        environment.push_str(&format!(" HOME={}", shell_quote(&home.to_string_lossy())));
    }
    let cmd = format!(
        "{} {} -f {} -i {} {}",
        environment,
        shell_quote(&ssh_runtime::program("ssh-copy-id")?.to_string_lossy()),
        connection_arguments()?,
        shell_quote(&pub_path),
        shell_quote(&host_alias)
    );

    super::operations::launch(
        "server_install",
        format!("{key_path} → {host_alias}"),
        &cmd,
        None,
        None,
        None,
    )
}

// An absent agent is compatible with SSH's direct key/password authentication.
// Preserve explicit values (including unavailable sockets), never inherit the terminal's agent.
fn terminal_environment() -> Result<String> {
    let socket = match std::env::var_os("SSH_AUTH_SOCK") {
        Some(value) => value.to_string_lossy().into_owned(),
        None => ssh_runtime::AgentContext::resolve()
            .map(|a| a.socket.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    Ok(format!("/usr/bin/env PATH=/usr/bin:/bin:/usr/sbin:/sbin LC_ALL=C SSH_ASKPASS_REQUIRE=never SSH_AUTH_SOCK={}", shell_quote(&socket)))
}
fn connection_arguments() -> Result<String> {
    let config = crate::utils::ssh_dir::get_ssh_config_path()?;
    let mut args = vec![
        "-F".to_owned(),
        if config.exists() {
            config.to_string_lossy().into_owned()
        } else {
            "/dev/null".into()
        },
    ];
    if crate::utils::ssh_dir::workspace_root()?.is_some() {
        args.extend([
            "-o".into(),
            format!(
                "UserKnownHostsFile={}",
                crate::utils::ssh_dir::get_known_hosts_path()?.display()
            ),
            "-o".into(),
            "GlobalKnownHostsFile=/dev/null".into(),
        ]);
    }
    Ok(args
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" "))
}

/// POSIX shell quoting; AppleScript escaping is a separate layer in terminal.rs.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub fn validate_destination(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('-')
        || value
            .chars()
            .any(|c| !c.is_alphanumeric() && !"._-:@[]%".contains(c))
    {
        return Err(AppError::InvalidInput(
            "Select a single SSH host alias, not a pattern or option".into(),
        ));
    }
    Ok(())
}

pub fn launch_terminal_command(command: &str) -> Result<()> {
    #[cfg(test)]
    if let Some(path) = std::env::var_os("SSH_GUI_CAPTURE_TERMINAL") {
        std::fs::write(path, command)?;
        return Ok(());
    }
    let terminals = detect_terminals();
    let preferred = PREFERRED_TERMINAL
        .lock()
        .map_err(|_| AppError::Process("Terminal preference unavailable".into()))?
        .clone();
    let terminal = terminals
        .iter()
        .find(|t| {
            preferred.as_deref() == Some(t.name.as_str())
                || preferred.as_deref() == Some(t.path.as_str())
        })
        .or_else(|| terminals.first())
        .ok_or_else(|| AppError::NotFound("No terminal found".into()))?;
    let (program, args) = build_launch_args(&terminal.name, command);
    if program == "/usr/bin/osascript" {
        let mut command = Command::new(program);
        command.args(args);
        let result = crate::utils::process::run(command, std::time::Duration::from_secs(30))?;
        if result.timed_out {
            return Err(AppError::Process("Terminal did not acknowledge the launch within 30 seconds. Check the terminal and macOS Automation permissions before retrying.".into()));
        }
        if !result.status.success() {
            return Err(AppError::Process(
                format!("Terminal automation failed: {}. Check System Settings → Privacy & Security → Automation → SSH GUI.", result.stderr.trim()),
            ));
        }
    } else {
        Command::new(program).args(args).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_metacharacters_are_literal() {
        let value = "key with ' quote; $(echo bad) `echo bad` \\";
        let output = Command::new("sh")
            .args(["-c", &format!("printf %s {}", shell_quote(value))])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
    }
    #[test]
    fn rejects_options_and_patterns() {
        for alias in [
            "",
            "-oProxyCommand=bad",
            "*",
            "one two",
            "!excluded",
            "host;id",
            "$(id)",
            "user@host|id",
            "host/path",
        ] {
            assert!(validate_destination(alias).is_err());
        }
        assert!(validate_destination("github-work").is_ok());
    }
}
