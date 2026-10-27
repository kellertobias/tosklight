//! One FIFO pipe owner. The render loop only transfers owned messages with `try_send`.
use std::io::Write;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
    mpsc::{SyncSender, sync_channel},
};
use viz_helper::protocol::FromHelper;

const QUEUE_MESSAGES: usize = 32;
const QUEUE_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Writer {
    sender: SyncSender<(FromHelper, usize)>,
    bytes: Arc<AtomicUsize>,
    failure: Arc<OnceLock<String>>,
    byte_limit: usize,
}

impl Writer {
    pub(super) fn start(output: impl Write + Send + 'static) -> Result<Self, String> {
        Self::with_limits(output, QUEUE_MESSAGES, QUEUE_BYTES)
    }

    fn with_limits(
        mut output: impl Write + Send + 'static,
        capacity: usize,
        byte_limit: usize,
    ) -> Result<Self, String> {
        let (sender, receiver) = sync_channel::<(FromHelper, usize)>(capacity);
        let bytes = Arc::new(AtomicUsize::new(0));
        let failure = Arc::new(OnceLock::new());
        let worker_bytes = bytes.clone();
        let worker_failure = failure.clone();
        // Deliberately detached: neither Drop nor the render loop may join a blocked pipe writer.
        std::thread::Builder::new()
            .name("viz-helper-writer".into())
            .spawn(move || {
                while let Ok((message, cost)) = receiver.recv() {
                    if worker_failure.get().is_some() {
                        break;
                    }
                    let result = viz_helper::protocol::encode(&message)
                        .map_err(|error| error.to_string())
                        .and_then(|payload| {
                            viz_helper::framing::write_frame(&mut output, &payload)
                                .map_err(|error| error.to_string())
                        });
                    worker_bytes.fetch_sub(cost, Ordering::AcqRel);
                    if let Err(error) = result {
                        let _ = worker_failure
                            .set(format!("the desk's channel writer failed: {error}"));
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            sender,
            bytes,
            failure,
            byte_limit,
        })
    }

    pub(super) fn failure(&self) -> Option<&str> {
        self.failure.get().map(String::as_str)
    }

    pub(super) fn send(&self, message: FromHelper) {
        if self.failure().is_some() {
            return;
        }
        let cost = retained_bytes(&message);
        if cost > viz_helper::framing::MAX_FRAME {
            let _ = self
                .failure
                .set("the desk's channel writer refused an oversized owned message".into());
            return;
        }
        // Includes the in-progress write. The worker can retain at most one additional encoded
        // payload; owned message size is capped above and framing checks the final encoded size.
        // RGBA is moved, never cloned on this path.
        if self
            .bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(cost)
                    .filter(|total| *total <= self.byte_limit)
            })
            .is_err()
        {
            let _ = self
                .failure
                .set("the desk's channel writer exceeded its bounded byte budget".into());
            return;
        }
        if let Err(error) = self.sender.try_send((message, cost)) {
            self.bytes.fetch_sub(cost, Ordering::AcqRel);
            let _ = self.failure.set(format!(
                "the desk's channel writer cannot accept a message: {error}"
            ));
        }
    }
}

fn retained_bytes(message: &FromHelper) -> usize {
    let heap = match message {
        FromHelper::Frame { rgba, .. } => rgba.capacity(),
        FromHelper::Ready { renderer, .. } => renderer.capacity(),
        FromHelper::Error { detail } | FromHelper::Stopping { detail } => detail.capacity(),
        FromHelper::FramePresented { renderer, mode, .. } => {
            renderer.capacity().saturating_add(mode.capacity())
        }
        FromHelper::Picked { fixture, .. } => fixture.as_ref().map_or(0, String::capacity),
        FromHelper::Capabilities { transports } => transports
            .capacity()
            .saturating_mul(std::mem::size_of::<viz_helper::protocol::FrameTransport>()),
        _ => 0,
    };
    heap.saturating_add(std::mem::size_of::<FromHelper>())
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
