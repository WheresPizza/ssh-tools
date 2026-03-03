use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Vec<String>,
    pub proxy_jump: Option<String>,
    pub forward_agent: Option<bool>,
    pub server_alive_interval: Option<u32>,
    pub extra_fields: Vec<(String, String)>,
    pub line_start: u32,
    pub line_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub name: String,
    pub private_path: String,
    pub public_path: String,
    pub algorithm: String,
    pub bits: Option<u32>,
    pub fingerprint: String,
    pub comment: Option<String>,
    pub has_passphrase: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyAlgorithm {
    Ed25519,
    Rsa2048,
    Rsa4096,
    EcdsaP256,
    EcdsaP384,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyGenParams {
    pub algorithm: KeyAlgorithm,
    pub comment: String,
    pub filename: String,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownHostEntry {
    pub line_number: u32,
    pub hostname: String,
    pub key_type: String,
    pub key_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub name: String,
    pub path: String,
    pub is_preferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub preferred_terminal: Option<String>,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            preferred_terminal: None,
            theme: "system".to_string(),
        }
    }
}
