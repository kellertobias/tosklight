//! Owned process, finite deadline and bounded pipes; no detached blocking reader threads.
use super::{UltimateError, check_cancel};
use std::{
    collections::VecDeque,
    process::{ExitStatus, Stdio},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    task::JoinHandle,
};
const LOG_BYTES: usize = 32 * 1024;

#[derive(Default)]
struct Tail(VecDeque<u8>);
impl Tail {
    fn append(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.0.len() == LOG_BYTES {
                self.0.pop_front();
            }
            self.0.push_back(byte);
        }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.iter().copied().collect::<Vec<_>>()).into_owned()
    }
}
struct Readers(Vec<JoinHandle<()>>);
impl Drop for Readers {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}
pub(super) struct Outcome {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}
impl Outcome {
    pub fn diagnostic(&self) -> String {
        self.stderr
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .or_else(|| {
                self.stdout
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
            })
            .unwrap_or("no diagnostic output")
            .chars()
            .take(1024)
            .collect()
    }
}

pub(super) fn run(
    command: Command,
    timeout: Duration,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&str),
) -> Result<Outcome, UltimateError> {
    check_cancel(cancel)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_async(command, timeout, cancel, &mut progress))
}

async fn run_async(
    mut command: Command,
    timeout: Duration,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(&str),
) -> Result<Outcome, UltimateError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| UltimateError::Failed("Invalid Blender job deadline".into()))?;
    if timeout.is_zero() {
        return Err(UltimateError::TimedOut);
    }
    command
        .env(
            "TOSKLIGHT_CAPTURE_PARENT_PID",
            std::process::id().to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        UltimateError::Failed(format!(
            "Cannot run Blender: {error}; choose Ultra or configure a usable Blender"
        ))
    })?;
    let stdout = Arc::new(Mutex::new(Tail::default()));
    let stderr = Arc::new(Mutex::new(Tail::default()));
    let mut readers = Readers(Vec::new());
    if let Some(pipe) = child.stdout.take() {
        readers.0.push(read(pipe, stdout.clone()));
    }
    if let Some(pipe) = child.stderr.take() {
        readers.0.push(read(pipe, stderr.clone()));
    }
    let mut reported = Instant::now();
    let mut last_progress = None;
    loop {
        if let Err(error) = check_cancel(cancel) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
        if Instant::now() >= deadline {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(UltimateError::TimedOut);
        }
        if let Some(status) = child.try_wait()? {
            // Drain available EOF data, but inherited pipes can never extend a completed job
            // indefinitely. All unfinished readers are aborted/closed when Readers drops.
            for reader in &mut readers.0 {
                let _ = tokio::time::timeout(Duration::from_millis(100), reader).await;
            }
            return Ok(Outcome {
                status,
                stdout: text(&stdout),
                stderr: text(&stderr),
            });
        }
        if reported.elapsed() >= Duration::from_millis(250) {
            let log = text(&stdout);
            if let Some(detail) = changed_progress(&log, &mut last_progress) {
                progress(&detail);
            }
            reported = Instant::now();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
fn text(tail: &Mutex<Tail>) -> String {
    tail.lock().map(|tail| tail.text()).unwrap_or_default()
}
fn read(
    mut pipe: impl AsyncRead + Unpin + Send + 'static,
    tail: Arc<Mutex<Tail>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut bytes = [0; 4096];
        loop {
            match pipe.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if let Ok(mut tail) = tail.lock() {
                        tail.append(&bytes[..count]);
                    }
                }
            }
        }
    })
}

/// Explicit entrypoint milestones outrank incidental Cycles chatter in the existing bounded tail.
/// No extra queue is retained; at most one bounded last-delivered detail suppresses repeats.
fn changed_progress(log: &str, last: &mut Option<String>) -> Option<String> {
    let marker = log
        .split_inclusive('\n')
        .rev()
        .filter(|line| line.ends_with('\n'))
        .find_map(|line| {
            line.trim()
                .strip_prefix("TOSKLIGHT_PROGRESS ")
                .filter(|detail| !detail.is_empty())
        });
    let detail = marker.or_else(|| {
        log.lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("TOSKLIGHT_"))
    })?;
    let bounded: String = detail.chars().take(512).collect();
    if last.as_ref() == Some(&bounded) {
        return None;
    }
    *last = Some(bounded.clone());
    Some(bounded)
}
#[cfg(test)]
mod progress_tests {
    #[test]
    fn explicit_render_marker_survives_following_cycles_chatter_and_emits_only_changes() {
        let mut last = None;
        let first = "TOSKLIGHT_PROGRESS importing frozen geometry\nBlender memory statistics\n";
        assert_eq!(
            super::changed_progress(first, &mut last).as_deref(),
            Some("importing frozen geometry")
        );
        let second =
            format!("{first}TOSKLIGHT_PROGRESS rendering 3840x2160 Cycles PNG\nFra:1 Mem:120M\n");
        assert_eq!(
            super::changed_progress(&second, &mut last).as_deref(),
            Some("rendering 3840x2160 Cycles PNG")
        );
        assert_eq!(
            super::changed_progress(&format!("{second}Sample2\n"), &mut last),
            None
        );
        assert_eq!(
            super::changed_progress(
                &format!("{second}TOSKLIGHT_PROGRESS image written\n"),
                &mut last
            )
            .as_deref(),
            Some("image written")
        );
    }
    #[test]
    fn fragmented_marker_cannot_replace_a_complete_milestone_and_details_remain_bounded() {
        let mut last = None;
        let log =
            "TOSKLIGHT_PROGRESS importing frozen geometry\nTOSKLIGHT_PROGRESS rendering 3840x";
        assert_eq!(
            super::changed_progress(log, &mut last).as_deref(),
            Some("importing frozen geometry")
        );
        let long = format!("TOSKLIGHT_PROGRESS {}\n", "x".repeat(2_000));
        assert_eq!(
            super::changed_progress(&long, &mut last).unwrap().len(),
            512
        );
    }
}
