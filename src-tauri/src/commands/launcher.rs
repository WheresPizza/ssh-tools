use crate::commands::app::{get_app_config, save_app_config};
use crate::error::{AppError, Result};
use crate::types::TerminalInfo;
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
    launch_terminal_command(&format!(
        "ssh -F {} -- {}",
        shell_quote(&crate::utils::ssh_dir::get_ssh_config_path()?.to_string_lossy()),
        shell_quote(&host_alias)
    ))
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
pub async fn copy_key_to_server(key_path: String, host_alias: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || copy_key_to_server_blocking(key_path, host_alias))
        .await
        .map_err(|e| crate::error::AppError::Process(format!("Background operation failed: {e}")))?
}

fn copy_key_to_server_blocking(key_path: String, host_alias: String) -> Result<()> {
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

    let cmd = format!(
        "ssh-copy-id -f -F {} -i {} {}",
        shell_quote(&crate::utils::ssh_dir::get_ssh_config_path()?.to_string_lossy()),
        shell_quote(&pub_path),
        shell_quote(&host_alias)
    );

    launch_terminal_command(&cmd)
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
    if program == "osascript" {
        let result = Command::new(program).args(args).output()?;
        if !result.status.success() {
            return Err(AppError::Process(
                String::from_utf8_lossy(&result.stderr).trim().into(),
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
