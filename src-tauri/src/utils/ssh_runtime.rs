//! System executables and an operation-scoped agent, independent of terminal startup files.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf, process::Command, time::Duration};

pub fn program(name: &str) -> Result<PathBuf> {
    if ![
        "ssh",
        "ssh-add",
        "ssh-keygen",
        "ssh-keyscan",
        "ssh-copy-id",
        "git",
    ]
    .contains(&name)
    {
        return Err(AppError::InvalidInput("Unsupported system tool".into()));
    }
    // macOS's system OpenSSH must not be replaced by a shell alias or an arbitrary PATH entry.
    let system = PathBuf::from("/usr/bin").join(name);
    if system.is_file() {
        return Ok(system);
    }
    if name == "ssh-copy-id" {
        for base in ["/opt/homebrew/bin", "/usr/local/bin"] {
            let path = PathBuf::from(base).join(name);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(AppError::NotFound(format!(
        "{name} is unavailable. Install this tool before retrying."
    )))
}

pub fn command(name: &str) -> Result<Command> {
    let mut command = Command::new(program(name)?);
    command.env("LC_ALL", "C");
    Ok(command)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentContext {
    pub socket: PathBuf,
    pub source: String,
}

impl AgentContext {
    pub fn resolve() -> Result<Self> {
        Self::select(std::env::var_os("SSH_AUTH_SOCK"), || {
            // An isolated workspace must never fall back to a user's real agent.
            if crate::utils::ssh_dir::workspace_root()?.is_some() {
                return Err(AppError::AgentUnavailable);
            }
            #[cfg(target_os = "macos")]
            {
                let mut command = Command::new("/bin/launchctl");
                command.args(["getenv", "SSH_AUTH_SOCK"]);
                let output = super::process::run(command, Duration::from_secs(3))?;
                if output.timed_out || !output.status.success() {
                    return Err(AppError::AgentUnavailable);
                }
                let value = output.stdout.trim();
                if !value.is_empty() {
                    return Ok(OsString::from(value));
                }
            }
            Err(AppError::AgentUnavailable)
        })
    }
    fn select(
        explicit: Option<OsString>,
        fallback: impl FnOnce() -> Result<OsString>,
    ) -> Result<Self> {
        let (value, source) = match explicit {
            // Even an explicitly empty value is an explicit (invalid) agent choice.
            Some(value) => (value, "environment"),
            None => (fallback()?, "macOS launchd"),
        };
        let socket = PathBuf::from(value);
        if !socket.is_absolute() {
            return Err(AppError::AgentUnavailable);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if !std::fs::metadata(&socket)
                .map(|m| m.file_type().is_socket())
                .unwrap_or(false)
            {
                return Err(AppError::AgentUnavailable);
            }
        }
        Ok(Self {
            socket,
            source: source.into(),
        })
    }
    pub fn command(&self) -> Result<Command> {
        let mut command = command("ssh-add")?;
        self.apply(&mut command);
        Ok(command)
    }
    pub fn apply(&self, command: &mut Command) {
        command.env("SSH_AUTH_SOCK", &self.socket);
    }
    pub fn fingerprints(&self) -> Result<Vec<String>> {
        let mut command = self.command()?;
        command.args(["-l", "-E", "sha256"]);
        let output = super::process::run(command, Duration::from_secs(5))?;
        if output.timed_out {
            return Err(AppError::Process(
                "SSH agent did not respond within 5 seconds".into(),
            ));
        }
        match output.status.code() {
            Some(1) => return Ok(Vec::new()),
            Some(2) => return Err(AppError::AgentUnavailable),
            Some(0) => {}
            _ => {
                return Err(AppError::Process(
                    "Unable to list SSH agent identities".into(),
                ))
            }
        }
        Ok(output
            .stdout
            .lines()
            .filter_map(|line| line.split_whitespace().nth(1).map(str::to_owned))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_explicit_agent_never_calls_fallback() {
        for value in ["", "relative.sock", "/does-not-exist/agent.sock"] {
            assert!(AgentContext::select(Some(value.into()), || panic!(
                "must not replace an explicit agent"
            ))
            .is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn absent_environment_can_select_system_socket() {
        let root = tempfile::tempdir_in("/private/tmp").unwrap();
        let socket = root.path().join("agent.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let agent = AgentContext::select(None, || Ok(socket.clone().into_os_string())).unwrap();
        assert_eq!(agent.socket, socket);
        assert_eq!(agent.source, "macOS launchd");
    }
    #[test]
    fn system_openssh_is_absolute_and_allowlisted() {
        assert_eq!(
            program("ssh-add").unwrap(),
            PathBuf::from("/usr/bin/ssh-add")
        );
        assert!(program("../bin/sh").is_err());
    }
}
