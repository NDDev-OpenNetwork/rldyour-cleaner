//! Deadline-limited native commands with continuously drained, bounded output.
//! Commands are passed as argv, never through a shell. No process snapshot or
//! recursive cache walk is needed when the owner coordinates GC with its lock.
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 16 * 1024;
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

fn drain(mut source: impl Read) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        let count = source.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let keep = count.min(OUTPUT_LIMIT.saturating_sub(result.len()));
        result.extend_from_slice(&chunk[..keep]);
    }
    Ok(result)
}

pub fn run(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot launch native tool: {e}"))?;
    let out = child.stdout.take().expect("stdout pipe");
    let err = child.stderr.take().expect("stderr pipe");
    let stdout = match std::thread::Builder::new()
        .name("gc-stdout".into())
        .stack_size(128 * 1024)
        .spawn(move || drain(out))
    {
        Ok(thread) => thread,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.to_string());
        }
    };
    let stderr = match std::thread::Builder::new()
        .name("gc-stderr".into())
        .stack_size(128 * 1024)
        .spawn(move || drain(err))
    {
        Ok(thread) => thread,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout.join();
            return Err(e.to_string());
        }
    };
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("native GC timed out; no forced cleanup attempted".to_string());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    let stdout = stdout
        .join()
        .map_err(|_| "stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let stderr = stderr
        .join()
        .map_err(|_| "stderr reader failed")?
        .map_err(|e| e.to_string())?;
    let output = Output {
        stdout: String::from_utf8_lossy(&stdout).trim().into(),
        stderr: String::from_utf8_lossy(&stderr).trim().into(),
    };
    let status = status?;
    if !status.success() {
        return Err(format!("native tool failed ({status}): {}", output.stderr));
    }
    Ok(output)
}
