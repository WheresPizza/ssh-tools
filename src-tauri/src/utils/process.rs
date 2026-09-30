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
fn drain(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let keep = count.min((256 * 1024usize).saturating_sub(result.len()));
        result.extend_from_slice(&buffer[..keep]);
    }
    Ok(result)
}
/// Bound both running time and retained output, including pipes held by descendants.
pub fn run(mut command: Command, timeout: Duration) -> Result<BoundedOutput> {
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
    let stdout = out
        .join()
        .map_err(|_| AppError::Process("Output reader failed".into()))??;
    let stderr = err
        .join()
        .map_err(|_| AppError::Process("Error reader failed".into()))??;
    Ok(BoundedOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout).into(),
        stderr: String::from_utf8_lossy(&stderr).into(),
        timed_out,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
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
