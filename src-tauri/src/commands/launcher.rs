use crate::commands::app::{get_app_config, save_app_config};
use crate::error::{AppError, Result};
use crate::types::TerminalInfo;
use crate::utils::terminal::{build_launch_args, detect_terminals};
use std::process::Command;
use std::sync::Mutex;

pub static PREFERRED_TERMINAL: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
pub fn launch_ssh_connection(host_alias: String) -> Result<()> {
    let terminals = detect_terminals();

    let preferred = PREFERRED_TERMINAL.lock().unwrap().clone();
    let terminal = preferred
        .as_deref()
        .and_then(|p| terminals.iter().find(|t| t.name == p || t.path == p))
        .or_else(|| terminals.first())
        .ok_or_else(|| AppError::NotFound("No terminal found".to_string()))?;

    let ssh_command = format!("ssh {}", host_alias);
    let (program, args) = build_launch_args(&terminal.name, &ssh_command);

    Command::new(&program)
        .args(&args)
        .spawn()
        .map_err(|e| AppError::Process(format!("Failed to launch terminal '{}': {}", program, e)))?;

    Ok(())
}

#[tauri::command]
pub fn get_detected_terminal() -> Result<Vec<TerminalInfo>> {
    let preferred = PREFERRED_TERMINAL.lock().unwrap().clone();
    let mut terminals = detect_terminals();

    // Mark preferred
    if let Some(ref pref) = preferred {
        for t in &mut terminals {
            if &t.name == pref || &t.path == pref {
                t.is_preferred = true;
            }
        }
    } else if let Some(t) = terminals.first_mut() {
        t.is_preferred = true;
    }

    Ok(terminals)
}

#[tauri::command]
pub fn set_preferred_terminal(terminal: String) -> Result<()> {
    {
        let mut pref = PREFERRED_TERMINAL.lock().unwrap();
        *pref = Some(terminal.clone());
    }
    let mut config = get_app_config()?;
    config.preferred_terminal = Some(terminal);
    save_app_config(config)?;
    Ok(())
}

#[tauri::command]
pub fn copy_key_to_server(key_path: String, host_alias: String) -> Result<()> {
    let pub_path = format!("{}.pub", key_path);
    if !std::path::Path::new(&pub_path).exists() {
        return Err(AppError::NotFound(format!("Public key not found: {}", pub_path)));
    }

    let cmd = format!("ssh-copy-id -i {} {}", pub_path, host_alias);

    let terminals = detect_terminals();
    let preferred = PREFERRED_TERMINAL.lock().unwrap().clone();
    let terminal = preferred
        .as_deref()
        .and_then(|p| terminals.iter().find(|t| t.name == p || t.path == p))
        .or_else(|| terminals.first())
        .ok_or_else(|| AppError::NotFound("No terminal found".to_string()))?;

    let (program, args) = build_launch_args(&terminal.name, &cmd);
    Command::new(&program)
        .args(&args)
        .spawn()
        .map_err(|e| AppError::Process(format!("Failed to launch terminal: {}", e)))?;

    Ok(())
}
