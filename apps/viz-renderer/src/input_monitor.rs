//! Opt-in passive observations for the Editor that owns this window's child pipe.
//! Serialization and pipe backpressure never run on the renderer thread.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use viz_scene::input_monitor::{MAX_MONITOR_BYTES, MONITOR_ENV, MonitorSnapshot};

const PERIOD: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
struct Pending {
    latest: Mutex<Option<MonitorSnapshot>>,
    wake: Condvar,
    stopped: AtomicBool,
    owners: AtomicUsize,
}

#[derive(Debug)]
pub struct InputMonitor {
    pending: Arc<Pending>,
    next: Instant,
}

impl InputMonitor {
    pub fn from_environment() -> Option<Self> {
        let enabled = enabled(
            std::env::var("TOSKLIGHT_VIZ_LAUNCHED_BY").ok().as_deref(),
            std::env::var(MONITOR_ENV).ok().as_deref(),
            std::env::args().any(|argument| argument == "--embed" || argument == "--helper"),
        );
        enabled.then(|| Self::with_writer(std::io::stdout()))
    }

    pub(crate) fn with_writer(writer: impl Write + Send + 'static) -> Self {
        let pending = Arc::new(Pending {
            owners: AtomicUsize::new(1),
            ..Default::default()
        });
        let worker = pending.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("viz-input-monitor".into())
            .spawn(move || write_latest(worker, writer))
        {
            eprintln!("could not start passive DMX monitor: {error}");
            pending.stopped.store(true, Ordering::Relaxed);
        }
        Self {
            pending,
            next: Instant::now(),
        }
    }

    #[cfg(test)]
    pub(crate) fn has_pending_observation(&self) -> bool {
        self.pending.latest.lock().unwrap().is_some()
    }

    pub fn due(&mut self, now: Instant) -> bool {
        if now < self.next || self.pending.stopped.load(Ordering::Relaxed) {
            return false;
        }
        self.next = now + PERIOD;
        true
    }

    pub fn publish(&self, snapshot: MonitorSnapshot) -> bool {
        // The consumer takes the slot and releases its lock before any serialization or IO.
        // Contention drops an observation; it can never stall a render or queue old frames.
        if let Ok(mut latest) = self.pending.latest.try_lock() {
            *latest = Some(snapshot);
            self.pending.wake.notify_one();
            true
        } else {
            false
        }
    }
}

impl Clone for InputMonitor {
    fn clone(&self) -> Self {
        self.pending.owners.fetch_add(1, Ordering::Relaxed);
        Self {
            pending: self.pending.clone(),
            next: Instant::now(),
        }
    }
}

impl Drop for InputMonitor {
    fn drop(&mut self) {
        if self.pending.owners.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.pending.stopped.store(true, Ordering::Relaxed);
            self.pending.wake.notify_one();
        }
        // Never join a writer blocked on an unresponsive parent. The process owns the pipe.
    }
}

fn enabled(launcher: Option<&str>, monitor: Option<&str>, embedded: bool) -> bool {
    launcher == Some("editor") && monitor == Some("1") && !embedded
}

fn write_latest(pending: Arc<Pending>, mut writer: impl Write) {
    loop {
        let snapshot = {
            let Ok(mut latest) = pending.latest.lock() else {
                return;
            };
            while latest.is_none() && !pending.stopped.load(Ordering::Relaxed) {
                let Ok((next, _)) = pending.wake.wait_timeout(latest, PERIOD) else {
                    return;
                };
                latest = next;
            }
            if pending.stopped.load(Ordering::Relaxed) {
                return;
            }
            latest.take()
        };
        let Some(snapshot) = snapshot else { continue };
        if !snapshot.validate() {
            continue;
        }
        let mut bytes = BoundedBuffer(Vec::new());
        if serde_json::to_writer(&mut bytes, &snapshot).is_err() {
            continue;
        }
        bytes.0.push(b'\n');
        if writer
            .write_all(&bytes.0)
            .and_then(|()| writer.flush())
            .is_err()
        {
            pending.stopped.store(true, Ordering::Relaxed);
            return;
        }
    }
}

/// Serialization cannot allocate beyond the same byte limit enforced by the parent reader.
struct BoundedBuffer(Vec<u8>);
impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) >= MAX_MONITOR_BYTES {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use viz_scene::input_monitor::{MonitorContext, ReceivedDmx};

    fn publish_until_queued(monitor: &InputMonitor, snapshot: MonitorSnapshot) {
        let until = Instant::now() + Duration::from_secs(1);
        while !monitor.publish(snapshot.clone()) {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
    }

    fn snapshot(revision: u64) -> MonitorSnapshot {
        MonitorSnapshot {
            version: 1,
            generated_millis: viz_scene::input_monitor::unix_millis(),
            context: MonitorContext {
                producer: "network_receiver".into(),
                endpoint: "owned".into(),
                show_id: None,
                show_revision: revision,
                mappings: vec![],
            },
            received: ReceivedDmx {
                universes: vec![],
                inputs: vec![],
                warnings: vec![],
            },
        }
    }

    struct BlockedWriter {
        entered: Sender<()>,
        release: Receiver<()>,
        writes: Sender<Vec<u8>>,
    }
    impl Write for BlockedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.entered
                .send(())
                .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
            self.release
                .recv()
                .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
            self.writes
                .send(bytes.to_vec())
                .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn standalone_and_embedded_never_enable_the_editor_pipe() {
        assert!(enabled(Some("editor"), Some("1"), false));
        assert!(!enabled(None, Some("1"), false));
        assert!(!enabled(Some("editor"), None, false));
        assert!(!enabled(Some("editor"), Some("1"), true));
    }

    #[test]
    fn blocked_stdout_does_not_block_render_polls_and_retains_only_latest_observation() {
        let (entered, entries) = channel();
        let (release, releases) = channel();
        let (writes, written) = channel();
        let monitor = InputMonitor::with_writer(BlockedWriter {
            entered,
            release: releases,
            writes,
        });
        publish_until_queued(&monitor, snapshot(1));
        entries.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        for revision in 2..=100 {
            monitor.publish(snapshot(revision));
        }
        assert!(start.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
        let first: MonitorSnapshot =
            serde_json::from_slice(&written.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
        assert_eq!(first.context.show_revision, 1);
        entries.recv_timeout(Duration::from_secs(2)).unwrap();
        release.send(()).unwrap();
        let latest: MonitorSnapshot =
            serde_json::from_slice(&written.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
        assert_eq!(latest.context.show_revision, 100);
        assert!(written.try_recv().is_err());
        drop(monitor);
    }

    #[test]
    fn dropping_monitor_never_waits_for_blocked_parent() {
        let (entered, entries) = channel();
        let (release, releases) = channel();
        let (writes, _written) = channel();
        let monitor = InputMonitor::with_writer(BlockedWriter {
            entered,
            release: releases,
            writes,
        });
        publish_until_queued(&monitor, snapshot(1));
        entries.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        drop(monitor);
        assert!(start.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
    }

    #[test]
    fn broken_parent_pipe_disables_only_optional_monitor() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut monitor = InputMonitor::with_writer(Broken);
        publish_until_queued(&monitor, snapshot(1));
        let deadline = Instant::now() + Duration::from_secs(2);
        while !monitor.pending.stopped.load(Ordering::Relaxed) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!monitor.due(Instant::now()));
    }

    #[test]
    fn serialization_is_bounded_before_allocating_or_writing_an_oversize_message() {
        let mut output = BoundedBuffer(vec![0; MAX_MONITOR_BYTES - 10]);
        let before = output.0.len();
        assert!(output.write_all(&[0; 10]).is_err());
        assert_eq!(output.0.len(), before);
        let mut oversized = snapshot(1);
        oversized.context.mappings = vec!["x".repeat(1024); 4096];
        let mut output = BoundedBuffer(Vec::new());
        assert!(serde_json::to_writer(&mut output, &oversized).is_err());
        assert!(output.0.len() < MAX_MONITOR_BYTES);
    }
}
