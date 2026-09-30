use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AgentOptions {
    pub lifetime_seconds: Option<u32>,
    #[serde(default)]
    pub confirm: bool,
}
impl AgentOptions {
    pub fn args(&self) -> Result<Vec<String>> {
        let mut args = Vec::new();
        if let Some(seconds) = self.lifetime_seconds {
            if seconds == 0 || seconds > 604800 {
                return Err(AppError::InvalidInput(
                    "Agent lifetime must be between 1 second and 7 days".into(),
                ));
            }
            args.extend(["-t".into(), seconds.to_string()]);
        }
        if self.confirm {
            args.push("-c".into());
        }
        Ok(args)
    }
}
#[derive(Clone, Serialize)]
pub struct AgentEnrollment {
    pub fingerprint: String,
    pub lifetime_seconds: Option<u32>,
    pub confirm: bool,
    pub requested_at: u64,
    pub interactive: bool,
}
static ENROLLMENTS: Mutex<Vec<AgentEnrollment>> = Mutex::new(Vec::new());
pub(crate) fn record(fingerprint: String, options: &AgentOptions, interactive: bool) {
    if let Ok(mut entries) = ENROLLMENTS.lock() {
        entries.retain(|entry| entry.fingerprint != fingerprint);
        if entries.len() >= 512 {
            entries.remove(0);
        }
        entries.push(AgentEnrollment {
            fingerprint,
            lifetime_seconds: options.lifetime_seconds,
            confirm: options.confirm,
            requested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            interactive,
        });
    }
}
pub(crate) fn forget(fingerprint: &str) {
    if let Ok(mut entries) = ENROLLMENTS.lock() {
        entries.retain(|entry| entry.fingerprint != fingerprint);
    }
}
#[tauri::command(rename_all = "snake_case")]
pub fn list_agent_enrollments() -> Result<Vec<AgentEnrollment>> {
    ENROLLMENTS
        .lock()
        .map(|entries| entries.clone())
        .map_err(|_| AppError::Process("Agent enrollment state unavailable".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_lifetime_and_confirmation_options() {
        assert_eq!(
            AgentOptions {
                lifetime_seconds: Some(3600),
                confirm: true
            }
            .args()
            .unwrap(),
            vec!["-t", "3600", "-c"]
        );
        assert!(AgentOptions {
            lifetime_seconds: Some(0),
            confirm: false
        }
        .args()
        .is_err());
        assert!(AgentOptions {
            lifetime_seconds: Some(604801),
            confirm: false
        }
        .args()
        .is_err());
        assert!(AgentOptions::default().args().unwrap().is_empty());
    }
}
