//! Durable, secret-free completion receipts for commands that run in a terminal.
//! Opening a terminal is never evidence that an agent or server operation succeeded.
use super::{agent_policy::AgentOptions, launcher::shell_quote};
use crate::{
    error::{AppError, Result},
    utils::{permissions::reject_symlink, ssh_runtime::AgentContext},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(crate) static OPERATION_LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Started,
    AwaitingInput,
    Completed,
    Cancelled,
    Failed,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub state: OperationState,
    pub detail: String,
    pub fingerprint: Option<String>,
    pub exit_code: Option<i32>,
    pub started_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<AgentOptions>,
}
impl Operation {
    pub fn pending(&self) -> bool {
        matches!(
            self.state,
            OperationState::Started | OperationState::AwaitingInput
        )
    }
    pub fn completed(target: String, fingerprint: String) -> Self {
        Self {
            id: String::new(),
            kind: "agent_add".into(),
            target,
            state: OperationState::Completed,
            detail: "Fingerprint verified in the selected SSH agent.".into(),
            fingerprint: Some(fingerprint),
            exit_code: Some(0),
            started_at: now(),
            agent: None,
            options: None,
        }
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn root() -> Result<PathBuf> {
    let base = super::app::get_config_dir()?;
    reject_symlink(&base)?;
    std::fs::create_dir_all(&base)?;
    let root = base.join("operations");
    reject_symlink(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if !root.exists() {
            std::fs::DirBuilder::new().mode(0o700).create(&root)?;
        }
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
fn directory(id: &str) -> Result<PathBuf> {
    if !id.starts_with("op-")
        || id.len() > 80
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(AppError::InvalidInput(
            "Invalid operation identifier".into(),
        ));
    }
    let path = root()?.join(id);
    reject_symlink(&path)?;
    Ok(path)
}
fn save(dir: &Path, operation: &Operation) -> Result<()> {
    use std::io::Write;
    let path = dir.join("operation.json");
    reject_symlink(&path)?;
    let mut staged = tempfile::NamedTempFile::new_in(dir)?;
    staged.write_all(&serde_json::to_vec(operation)?)?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|e| AppError::Io(e.error))?;
    Ok(())
}
fn read_small(path: &Path) -> Result<String> {
    reject_symlink(path)?;
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 32768 {
        return Err(AppError::InvalidInput("Invalid operation receipt".into()));
    }
    use std::io::Read;
    let mut text = String::new();
    file.take(32769).read_to_string(&mut text)?;
    if text.len() > 32768 {
        return Err(AppError::InvalidInput("Invalid operation receipt".into()));
    }
    Ok(text)
}

fn refresh(dir: &Path, mut operation: Operation) -> Result<Operation> {
    if !operation.pending() {
        return Ok(operation);
    }
    let receipt = dir.join("exit-code");
    if !receipt.exists() {
        return Ok(operation);
    }
    let code = read_small(&receipt)?
        .trim()
        .parse::<i32>()
        .map_err(|_| AppError::Process("Invalid terminal completion code".into()))?;
    operation.exit_code = Some(code);
    if [129, 130, 143].contains(&code) {
        operation.state = OperationState::Cancelled;
        operation.detail = "The terminal operation was interrupted. Check the current agent state before retrying.".into();
    } else if code != 0 {
        operation.state = OperationState::Failed;
        operation.detail = format!("Command exited with code {code}. Read the error in the terminal; incorrect or cancelled passphrase input can cause this. No success was recorded.");
    } else if operation.kind == "agent_add" {
        let agent = operation
            .agent
            .as_ref()
            .ok_or_else(|| AppError::Process("Operation agent is missing".into()))?;
        match agent.fingerprints() {
            Ok(keys)
                if operation
                    .fingerprint
                    .as_ref()
                    .is_some_and(|fp| keys.contains(fp)) =>
            {
                operation.state = OperationState::Completed;
                operation.detail =
                    "Fingerprint verified in the same SSH agent used for this operation.".into();
                if let (Some(fp), Some(options)) = (&operation.fingerprint, &operation.options) {
                    super::agent_policy::record(fp.clone(), options, true);
                }
            }
            Ok(_) => {
                operation.state = OperationState::Failed;
                operation.detail = "ssh-add exited successfully, but the fingerprint is absent from the selected agent (it may have expired or been removed). Retry after checking the agent.".into();
            }
            Err(e) => {
                operation.state = OperationState::Failed;
                operation.detail =
                    format!("ssh-add exited successfully, but agent verification failed: {e}");
            }
        }
    } else {
        operation.state = OperationState::Completed;
        operation.detail = "Public key installation command completed. Run Test authentication for this host to verify access with the intended identity.".into();
    }
    save(dir, &operation)?;
    Ok(operation)
}

pub(crate) fn list_blocking() -> Result<Vec<Operation>> {
    let mut operations = Vec::new();
    for entry in std::fs::read_dir(root()?)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || !entry.file_name().to_string_lossy().starts_with("op-") {
            continue;
        }
        let dir = directory(&entry.file_name().to_string_lossy())?;
        let path = dir.join("operation.json");
        if !path.exists() {
            continue;
        }
        let operation: Operation = serde_json::from_str(&read_small(&path)?)?;
        if operation.id != entry.file_name().to_string_lossy() {
            return Err(AppError::InvalidInput(
                "Operation identifier mismatch".into(),
            ));
        }
        operations.push(refresh(&dir, operation)?);
    }
    operations.sort_by_key(|op| op.started_at);
    Ok(operations)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_terminal_operations() -> Result<Vec<Operation>> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| AppError::Process("Operation lock unavailable".into()))?;
        list_blocking()
    })
    .await
    .map_err(|e| AppError::Process(e.to_string()))?
}
#[tauri::command(rename_all = "snake_case")]
pub async fn dismiss_terminal_operation(id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = OPERATION_LOCK
            .lock()
            .map_err(|_| AppError::Process("Operation lock unavailable".into()))?;
        let dir = directory(&id)?;
        let operation = refresh(
            &dir,
            serde_json::from_str(&read_small(&dir.join("operation.json"))?)?,
        )?;
        if operation.pending() {
            return Err(AppError::InvalidInput(
                "Finish or interrupt this operation in the terminal first.".into(),
            ));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Process(e.to_string()))?
}

// Caller holds OPERATION_LOCK, also covering duplicate checks and the launch.
pub(crate) fn launch(
    kind: &str,
    target: String,
    command: &str,
    agent: Option<AgentContext>,
    fingerprint: Option<String>,
    options: Option<AgentOptions>,
) -> Result<Operation> {
    let existing = list_blocking()?;
    if let Some(operation) = existing
        .iter()
        .find(|op| op.kind == kind && op.target == target && op.pending())
    {
        return Ok(operation.clone());
    }
    if existing.len() >= 256 {
        return Err(AppError::InvalidInput(
            "Dismiss completed terminal operations before starting more.".into(),
        ));
    }
    let dir = tempfile::Builder::new()
        .prefix("op-")
        .tempdir_in(root()?)?
        .keep();
    let mut operation = Operation {
        id: dir.file_name().unwrap().to_string_lossy().into(),
        kind: kind.into(),
        target,
        state: OperationState::Started,
        detail: "Starting terminal…".into(),
        fingerprint,
        exit_code: None,
        started_at: now(),
        agent,
        options,
    };
    save(&dir, &operation)?;
    let script = receipt_script(&dir, command);
    std::fs::write(dir.join("run.sh"), script)?;
    // This is the only string interpreted by the user's interactive shell.
    let launch = format!(
        "/bin/sh {}",
        shell_quote(&dir.join("run.sh").to_string_lossy())
    );
    match super::launcher::launch_terminal_command(&launch) {
        Ok(()) => {
            operation.state = OperationState::AwaitingInput;
            operation.detail = "Complete the prompt in the terminal; use Ctrl+C there to cancel. Waiting for the command's exit status.".into();
        }
        Err(e) if e.to_string().contains("did not acknowledge") => {
            operation.detail =
                format!("{e} Completion is still unconfirmed; do not start the command again.");
        }
        Err(e) => {
            operation.state = OperationState::Failed;
            operation.detail = format!("Terminal launch failed: {e}");
            save(&dir, &operation)?;
            return Err(e);
        }
    }
    save(&dir, &operation)?;
    Ok(operation)
}
fn receipt_script(dir: &Path, command: &str) -> String {
    let dir = shell_quote(&dir.to_string_lossy());
    format!("#!/bin/sh\numask 077\noperation_dir={dir}\n/bin/mkdir \"$operation_dir/running\" 2>/dev/null || exit 70\nfinish() {{\n  result=$?\n  trap - EXIT HUP INT TERM\n  printf '%s\\n' \"$result\" > \"$operation_dir/exit-code.tmp\"\n  /bin/mv \"$operation_dir/exit-code.tmp\" \"$operation_dir/exit-code\"\n}}\ntrap finish EXIT\ntrap 'exit 129' HUP\ntrap 'exit 130' INT\ntrap 'exit 143' TERM\n{command}\nresult=$?\nprintf '\\nSSH GUI: command exited with code %s. Return to the app for verification.\\n' \"$result\"\nexit \"$result\"\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_records_actual_exit_and_cannot_run_twice() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("run.sh");
        std::fs::write(&script, receipt_script(dir.path(), "/bin/sh -c 'exit 7'")).unwrap();
        let status = std::process::Command::new("/bin/sh")
            .arg(&script)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(7));
        assert_eq!(
            read_small(&dir.path().join("exit-code")).unwrap().trim(),
            "7"
        );
        assert_eq!(
            std::process::Command::new("/bin/sh")
                .arg(script)
                .status()
                .unwrap()
                .code(),
            Some(70)
        );
    }
    #[test]
    fn receipt_survives_terminal_interrupt() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("run.sh");
        std::fs::write(&script, receipt_script(dir.path(), "kill -INT $$")).unwrap();
        assert_eq!(
            std::process::Command::new("/bin/sh")
                .arg(script)
                .status()
                .unwrap()
                .code(),
            Some(130)
        );
        assert_eq!(
            read_small(&dir.path().join("exit-code")).unwrap().trim(),
            "130"
        );
    }
}
