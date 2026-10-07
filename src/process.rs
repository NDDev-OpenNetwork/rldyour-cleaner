//! One deadline includes child execution and pipe draining. Platform-owned
//! process trees prevent inherited streams from stranding a reader thread.
use crate::os::{self, Pipe, ProcessTree};
use std::{
    io,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const OUTPUT_LIMIT: usize = 16 * 1024;
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}
fn drain(mut source: impl Pipe, deadline: Instant, cancelled: &AtomicBool) -> io::Result<Vec<u8>> {
    let mut output = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    os::prepare_pipe(&source)?;
    loop {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "output drain cancelled",
            ));
        }
        match os::read_pipe(&mut source, &mut chunk)? {
            Some(0) => return Ok(output),
            Some(count) => {
                let keep = count.min(OUTPUT_LIMIT.saturating_sub(output.len()));
                output.extend_from_slice(&chunk[..keep]);
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}
pub fn run(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("invalid command deadline")?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, mut tree) = ProcessTree::spawn(command)
        .map_err(|e| format!("cannot launch contained native tool: {e}"))?;
    let out = child.stdout.take().expect("stdout pipe");
    let err = child.stderr.take().expect("stderr pipe");
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_out = Arc::clone(&cancelled);
    let stdout = match std::thread::Builder::new()
        .name("gc-stdout".into())
        .stack_size(128 * 1024)
        .spawn(move || drain(out, deadline, &cancel_out))
    {
        Ok(thread) => thread,
        Err(e) => {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.to_string());
        }
    };
    let cancel_err = Arc::clone(&cancelled);
    let stderr = match std::thread::Builder::new()
        .name("gc-stderr".into())
        .stack_size(128 * 1024)
        .spawn(move || drain(err, deadline, &cancel_err))
    {
        Ok(thread) => thread,
        Err(e) => {
            cancelled.store(true, Ordering::Relaxed);
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout.join();
            return Err(e.to_string());
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => break Err(
                "native GC timed out; owned process tree terminated; no forced cleanup attempted"
                    .to_string(),
            ),
            Err(e) => break Err(e.to_string()),
        }
    };
    tree.terminate(); // also terminates descendants after a parent exits early
    if status.is_err() {
        cancelled.store(true, Ordering::Relaxed);
        let _ = child.kill();
    }
    let _ = child.wait();
    let out = stdout
        .join()
        .map_err(|_| "stdout reader failed")?
        .map_err(|e| e.to_string());
    let err = stderr
        .join()
        .map_err(|_| "stderr reader failed")?
        .map_err(|e| e.to_string());
    let status = status?;
    let output = Output {
        stdout: String::from_utf8_lossy(&out?).trim().into(),
        stderr: String::from_utf8_lossy(&err?).trim().into(),
    };
    if !status.success() {
        return Err(format!("native tool failed ({status}): {}", output.stderr));
    }
    Ok(output)
}
