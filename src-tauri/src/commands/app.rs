use crate::error::Result;
use crate::types::AppConfig;
use crate::utils::ssh_dir::get_ssh_dir;
use std::path::PathBuf;

fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("ssh-gui")
}

fn config_file_path() -> PathBuf {
    get_config_dir().join("config.json")
}

#[tauri::command]
pub fn get_app_config() -> Result<AppConfig> {
    let path = config_file_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command]
pub fn save_app_config(config: AppConfig) -> Result<()> {
    let dir = get_config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = config_file_path();
    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

#[tauri::command]
pub fn get_ssh_dir_path() -> Result<String> {
    let dir = get_ssh_dir()?;
    Ok(dir.to_str().unwrap_or("").to_string())
}
