//! Opt-in helper observations, not display events. Async diagnostics may perturb measurements.
//! Missing terminal witness, truncated=true, or droppedEventsObserved>0 means incomplete coverage.
use serde::Serialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use viz_helper::protocol::FrameId;

const MAX_ROWS: usize = 32_768;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const QUEUE_ROWS: usize = 512;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    pid: u32,
    elapsed_micros: u64,
    event: &'static str,
    frame: Option<FrameId>,
    accepted: Option<bool>,
    blocked_micros: u64,
    total_blocked_micros: u64,
    dropped_events_observed: u64,
}

pub(super) struct Flow {
    epoch: Instant,
    sender: Option<SyncSender<Row>>,
    dropped: Arc<AtomicU64>,
    blocked_since: Option<Instant>,
    total_blocked: u64,
}
impl Flow {
    pub(super) fn from_environment(benchmark: bool) -> Option<Self> {
        if !benchmark {
            return None;
        }
        let requested = std::env::var_os("LIGHT_STAGE_FRAME_FLOW_PATH")?;
        let epoch = Instant::now();
        let epoch_micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_micros();
        let path = output_path(benchmark, Some(Path::new(&requested)), std::process::id())?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .ok()?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_ROWS);
        let dropped = Arc::new(AtomicU64::new(0));
        let writer_dropped = dropped.clone();
        std::thread::Builder::new()
            .name("stage-frame-flow".into())
            .spawn(move || {
                let _ = write_rows(
                    file,
                    receiver,
                    writer_dropped,
                    MAX_ROWS,
                    MAX_BYTES,
                    epoch_micros,
                );
            })
            .ok()?;
        let mut flow = Self {
            epoch,
            sender: Some(sender),
            dropped,
            blocked_since: None,
            total_blocked: 0,
        };
        flow.event("diagnostic_started", None, None, epoch);
        Some(flow)
    }
    pub(super) fn frame(&mut self, event: &'static str, frame: FrameId, accepted: Option<bool>) {
        if self.sender.is_some() {
            self.event(event, Some(frame), accepted, Instant::now());
        }
    }
    pub(super) fn wait(&mut self, now: Instant, due_no_free: bool) {
        if self.sender.is_none() {
            return;
        }
        match (self.blocked_since, due_no_free) {
            (None, true) => {
                self.blocked_since = Some(now);
                self.event("due_no_free_enter", None, None, now);
            }
            (Some(started), false) => {
                let blocked = micros(now.saturating_duration_since(started));
                self.total_blocked = self.total_blocked.saturating_add(blocked);
                self.blocked_since = None;
                self.emit(Row {
                    pid: std::process::id(),
                    elapsed_micros: micros(now.saturating_duration_since(self.epoch)),
                    event: "due_no_free_exit",
                    frame: None,
                    accepted: None,
                    blocked_micros: blocked,
                    total_blocked_micros: self.total_blocked,
                    dropped_events_observed: 0,
                });
            }
            _ => {}
        }
    }
    fn event(
        &mut self,
        event: &'static str,
        frame: Option<FrameId>,
        accepted: Option<bool>,
        now: Instant,
    ) {
        self.emit(Row {
            pid: std::process::id(),
            elapsed_micros: micros(now.saturating_duration_since(self.epoch)),
            event,
            frame,
            accepted,
            blocked_micros: 0,
            total_blocked_micros: self.total_blocked,
            dropped_events_observed: 0,
        });
    }
    fn emit(&mut self, mut row: Row) {
        row.dropped_events_observed = self.dropped.load(Ordering::Relaxed);
        let Some(sender) = self.sender.as_ref() else {
            return;
        };
        match sender.try_send(row) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                self.sender = None;
            }
        }
    }
}
impl Drop for Flow {
    fn drop(&mut self) {
        if self.sender.is_some() {
            self.wait(Instant::now(), false);
        }
    }
}
fn micros(duration: std::time::Duration) -> u64 {
    duration.as_micros().min(u128::from(u64::MAX)) as u64
}
fn output_path(benchmark: bool, requested: Option<&Path>, pid: u32) -> Option<PathBuf> {
    if !benchmark {
        return None;
    }
    let requested = requested?;
    if !requested.is_absolute() || !requested.parent()?.is_dir() {
        return None;
    }
    let mut name = requested.file_name()?.to_os_string();
    name.push(format!(".{pid}.jsonl"));
    Some(requested.with_file_name(name))
}
fn write_rows(
    mut file: impl Write,
    receiver: Receiver<Row>,
    dropped: Arc<AtomicU64>,
    max_rows: usize,
    max_bytes: usize,
    epoch_micros: u128,
) -> std::io::Result<()> {
    // Reserve one row and 1 KiB for a terminal integrity witness. Partial/error files have none.
    let mut header = serde_json::to_vec(&serde_json::json!({
        "event": "diagnostic_metadata", "schemaVersion": 1, "pid": std::process::id(),
        "epochAnchorMicros": epoch_micros, "monotonicZeroMicros": 0,
        "maxRows": max_rows, "maxBytes": max_bytes, "queueRows": QUEUE_ROWS,
        "sharedSlots": 2, "copySlots": 1,
        "boundaries": "helper observations/enqueues; diagnostics may perturb timing; not display evidence"
    }))?;
    header.push(b'\n');
    if max_rows < 2 || header.len() + 1024 > max_bytes {
        return Ok(());
    }
    file.write_all(&header)?;
    let mut rows = 1;
    let mut bytes = header.len();
    let mut truncated = false;
    for row in receiver {
        let mut encoded = serde_json::to_vec(&row)?;
        encoded.push(b'\n');
        if rows + 1 >= max_rows || bytes + encoded.len() + 1024 > max_bytes {
            dropped.fetch_add(1, Ordering::Relaxed);
            truncated = true;
            break;
        }
        file.write_all(&encoded)?;
        rows += 1;
        bytes += encoded.len();
    }
    let mut end = serde_json::to_vec(&serde_json::json!({
        "event": "diagnostic_stopped", "pid": std::process::id(), "rowsWritten": rows,
        "droppedEventsObserved": dropped.load(Ordering::Relaxed), "truncated": truncated,
        "boundaries": "helper observations/enqueues; not kernel delivery or display; diagnostics may perturb timing"
    }))?;
    end.push(b'\n');
    if rows < max_rows && bytes + end.len() <= max_bytes {
        file.write_all(&end)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
