//! Git account profiles are stored in one generated section of the SSH config.
//! Keeping metadata and SSH directives in the same atomic write avoids split state.
use super::ssh_config::{config_documents, config_revision, parse_ssh_config};
use crate::{
    error::{AppError, Result},
    utils::{permissions::atomic_write, ssh_dir::get_ssh_config_path},
};
use serde::{Deserialize, Serialize};

const BEGIN: &str = "# BEGIN SSH GUI GIT PROFILES\n";
const END: &str = "# END SSH GUI GIT PROFILES\n";
const META: &str = "# ssh-gui-profile ";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitProfile {
    pub name: String,
    pub account: String,
    pub alias: String,
    pub hostname: String,
    pub user: String,
    pub port: u16,
    pub key_path: String,
}
#[derive(Serialize)]
pub struct ProfileSnapshot {
    pub profiles: Vec<GitProfile>,
    pub revision: String,
}
fn invalid(message: impl Into<String>) -> AppError {
    AppError::InvalidInput(message.into())
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
fn validate(profile: &GitProfile) -> Result<()> {
    if !token(&profile.alias) || !token(&profile.user) || profile.port == 0 {
        return Err(invalid("Alias and SSH user must contain only letters, numbers, dots, underscores or hyphens; port must be 1–65535"));
    }
    if profile.hostname.is_empty()
        || profile.hostname.starts_with('-')
        || !profile
            .hostname
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".:-".contains(&c))
    {
        return Err(invalid(
            "Enter a hostname or IP address without a URL scheme",
        ));
    }
    if [&profile.name, &profile.account, &profile.key_path]
        .iter()
        .any(|v| v.trim().is_empty() || v.len() > 4096 || v.chars().any(char::is_control))
    {
        return Err(invalid("Profile name, account label and key path are required and must not contain control characters"));
    }
    // SSH performs percent expansion in IdentityFile; never silently select another file.
    if profile.key_path.contains('%') {
        return Err(invalid(
            "Profile key paths cannot contain percent expansion tokens",
        ));
    }
    Ok(())
}
fn ssh_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
pub(crate) fn render(profiles: &[GitProfile]) -> Result<String> {
    if profiles.is_empty() {
        return Ok(String::new());
    }
    let mut section = BEGIN.to_string();
    for p in profiles {
        validate(p)?;
        section.push_str(&format!("{META}{}\nHost {}\n    HostName {}\n    User {}\n    Port {}\n    IdentityFile {}\n    IdentitiesOnly yes\n\n", serde_json::to_string(p)?, p.alias, p.hostname, p.user, p.port, ssh_quote(&p.key_path)));
    }
    // Restore an all-host scope before the user's original configuration.
    section.push_str("Host *\n");
    section.push_str(END);
    Ok(section)
}
pub(crate) fn split(content: &str) -> Result<(Vec<GitProfile>, &str)> {
    if !content.starts_with(BEGIN) {
        if content.contains(BEGIN.trim()) || content.contains(END.trim()) {
            return Err(invalid("Git profile section was moved or damaged; restore the config backup before editing profiles"));
        }
        return Ok((Vec::new(), content));
    }
    let (section, rest) = content
        .split_once(END)
        .ok_or_else(|| invalid("Git profile section is incomplete; restore the config backup"))?;
    if rest.contains(BEGIN.trim()) || rest.contains(END.trim()) {
        return Err(invalid("Duplicate Git profile sections"));
    }
    let profiles: Vec<GitProfile> = section
        .lines()
        .filter_map(|l| l.strip_prefix(META))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    if render(&profiles)? != format!("{section}{END}") {
        return Err(invalid("Generated Git profile directives were edited externally; restore the config backup before editing profiles"));
    }
    Ok((profiles, rest))
}
pub(crate) fn managed_line_count(content: &str) -> usize {
    if !content.starts_with(BEGIN) {
        return 0;
    }
    content
        .split_once(END)
        .map(|(section, _)| section.lines().count() + 1)
        .unwrap_or(0)
}
fn read_content() -> Result<String> {
    match std::fs::read_to_string(get_ssh_config_path()?) {
        Ok(c) => Ok(c),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn list_blocking() -> Result<ProfileSnapshot> {
    let content = read_content()?;
    let (profiles, _) = split(&content)?;
    Ok(ProfileSnapshot {
        profiles,
        revision: config_revision(&content),
    })
}
#[tauri::command(rename_all = "snake_case")]
pub async fn list_git_profiles() -> Result<ProfileSnapshot> {
    tauri::async_runtime::spawn_blocking(list_blocking)
        .await
        .map_err(|e| invalid(e.to_string()))?
}
fn save_blocking(
    profile: GitProfile,
    original_alias: Option<String>,
    revision: String,
) -> Result<ProfileSnapshot> {
    let _guard = crate::utils::SSH_WRITE_LOCK
        .lock()
        .map_err(|_| invalid("SSH write lock unavailable"))?;
    validate(&profile)?;
    super::ssh_keys::get_public_key_blocking(profile.key_path.clone())?;
    let content = read_content()?;
    if config_revision(&content) != revision {
        return Err(invalid(
            "SSH config changed; refresh profiles before saving",
        ));
    }
    let (mut profiles, original) = split(&content)?;
    let index = if let Some(alias) = &original_alias {
        Some(
            profiles
                .iter()
                .position(|p| &p.alias == alias)
                .ok_or_else(|| invalid("Profile no longer exists"))?,
        )
    } else {
        None
    };
    if profiles
        .iter()
        .enumerate()
        .any(|(i, p)| Some(i) != index && p.alias == profile.alias)
    {
        return Err(invalid("A profile with this alias already exists"));
    }
    let main = get_ssh_config_path()?;
    for (path, document) in config_documents()? {
        let text = if path == main { original } else { &document };
        if parse_ssh_config(text).iter().any(|h| {
            h.alias
                .split_whitespace()
                .any(|alias| alias == profile.alias)
        }) {
            return Err(invalid(
                "This alias already exists in the SSH configuration; choose a unique profile alias",
            ));
        }
    }
    if let Some(i) = index {
        profiles[i] = profile;
    } else {
        profiles.push(profile);
    }
    atomic_write(&main, &(render(&profiles)? + original))?;
    list_blocking()
}
#[tauri::command(rename_all = "snake_case")]
pub async fn save_git_profile(
    profile: GitProfile,
    original_alias: Option<String>,
    revision: String,
) -> Result<ProfileSnapshot> {
    tauri::async_runtime::spawn_blocking(move || save_blocking(profile, original_alias, revision))
        .await
        .map_err(|e| invalid(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_git_profile(alias: String, revision: String) -> Result<ProfileSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = crate::utils::SSH_WRITE_LOCK
            .lock()
            .map_err(|_| invalid("SSH write lock unavailable"))?;
        let content = read_content()?;
        if config_revision(&content) != revision {
            return Err(invalid(
                "SSH config changed; refresh profiles before deleting",
            ));
        }
        let (mut profiles, original) = split(&content)?;
        let index = profiles
            .iter()
            .position(|p| p.alias == alias)
            .ok_or_else(|| invalid("Profile no longer exists"))?;
        profiles.remove(index);
        atomic_write(&get_ssh_config_path()?, &(render(&profiles)? + original))?;
        list_blocking()
    })
    .await
    .map_err(|e| invalid(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> GitProfile {
        GitProfile {
            name: "Work".into(),
            account: "work-account".into(),
            alias: "github-work".into(),
            hostname: "github.com".into(),
            user: "git".into(),
            port: 22,
            key_path: "/tmp/keys/work key".into(),
        }
    }
    #[test]
    fn profile_roundtrip_preserves_original_bytes() {
        let original = "# defaults\nServerAliveInterval 30\nHost *\n  ForwardAgent no\n";
        let generated = render(&[sample()]).unwrap() + original;
        let (profiles, rest) = split(&generated).unwrap();
        assert_eq!(profiles, vec![sample()]);
        assert_eq!(rest, original);
        assert!(generated.contains("IdentityFile \"/tmp/keys/work key\""));
        assert!(generated.contains("IdentitiesOnly yes"));
    }
    #[test]
    fn external_changes_and_injection_are_rejected() {
        let generated = render(&[sample()]).unwrap();
        assert!(split(&generated.replace("User git", "User attacker")).is_err());
        let mut p = sample();
        p.alias = "work\nProxyCommand bad".into();
        assert!(render(&[p]).is_err());
        let mut p = sample();
        p.key_path = "/tmp/%h".into();
        assert!(render(&[p]).is_err());
    }
}
