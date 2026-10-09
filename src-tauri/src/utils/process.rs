use crate::error::{AppError, Result};
use std::{
    io::Read,
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

pub struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
fn drain(mut stream: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut result = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 8192];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min((256 * 1024usize).saturating_sub(result.len()));
        truncated |= keep < count;
        result.extend_from_slice(&buffer[..keep]);
    }
    Ok((result, truncated))
}
/// Bound both running time and retained output, including pipes held by descendants.
pub fn run(mut command: Command, timeout: Duration) -> Result<BoundedOutput> {
    let (output, timed_out) = run_raw(&mut command, timeout)?;
    Ok(BoundedOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into(),
        stderr: String::from_utf8_lossy(&output.stderr).into(),
        timed_out,
    })
}
/// Bounded equivalent of Command::output, preserving bytes for structured parsers.
pub fn output(command: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    let (output, timed_out) = run_raw(command, timeout)?;
    if timed_out {
        return Err(AppError::Process(format!(
            "{} timed out after {} seconds",
            command.get_program().to_string_lossy(),
            timeout.as_secs()
        )));
    }
    Ok(output)
}
fn run_raw(command: &mut Command, timeout: Duration) -> Result<(std::process::Output, bool)> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || drain(stdout));
    let err = std::thread::spawn(move || drain(stderr));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let (stdout, out_truncated) = out
        .join()
        .map_err(|_| AppError::Process("Output reader failed".into()))??;
    let (stderr, err_truncated) = err
        .join()
        .map_err(|_| AppError::Process("Error reader failed".into()))??;
    if out_truncated || err_truncated {
        return Err(AppError::Process(
            "Command output exceeded 256 KiB; refusing a partial result".into(),
        ));
    }
    Ok((
        std::process::Output {
            status,
            stdout,
            stderr,
        },
        timed_out,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_structured_command_output() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "/usr/bin/head -c 300000 /dev/zero"]);
        assert!(output(&mut command, Duration::from_secs(3)).is_err());
    }
    #[test]
    fn timeout_kills_descendants_holding_pipes() {
        let start = Instant::now();
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30 & wait"]);
        let output = run(command, Duration::from_millis(100)).unwrap();
        assert!(output.timed_out);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
