//! Drain bounded helper messages, then admit at most one changed composition.
use super::*;

#[derive(Default)]
struct InboxEvents {
    ended: bool,
    trouble: Option<String>,
    camera: Option<[f32; 6]>,
    picked: Vec<(Option<String>, bool)>,
}

impl StagePane {
    pub(crate) fn tick(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if self.retiring.load(std::sync::atomic::Ordering::Acquire) {
            return self.poll_close_locked(&mut guard);
        }
        let Some(running) = guard.as_mut() else {
            return Ok(());
        };
        running.refresh_visibility();
        let mut events = match drain_inbox(self, running) {
            Ok(events) => events,
            Err(detail) => InboxEvents {
                ended: true,
                trouble: Some(detail),
                ..InboxEvents::default()
            },
        };
        if let Some(error) = running.channel_error.take().or_else(|| {
            running
                .to_helper
                .as_ref()
                .and_then(|writer| writer.failure().map(str::to_owned))
        }) {
            events.ended = true;
            events.trouble = Some(error);
        }
        let result = if events.ended {
            self.begin_close_locked(&mut guard)
        } else {
            let result = running.compositor.draw();
            let ready = running.compositor.has_front_picture();
            if let Some(benchmark) = running.benchmark.as_mut() {
                benchmark.source_ready = ready;
                benchmark.draw_healthy = result.is_ok() && ready;
            }
            running.refresh_visibility();
            result.map(|_| ())
        };
        if events.trouble.is_none() {
            events.trouble = result.clone().err();
        }
        drop(guard);
        publish_events(self, events)?;
        result
    }
}

fn drain_inbox(pane: &StagePane, running: &mut Running) -> Result<InboxEvents, String> {
    let mut events = InboxEvents::default();
    // The reader has a bounded queue. A damaged helper cannot monopolize the Control thread.
    for _ in 0..128 {
        match running.inbox.try_recv() {
            Ok(message) => {
                accept_message(pane, running, message, &mut events)?;
                if events.ended {
                    break;
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                events.ended = true;
                break;
            }
        }
    }
    Ok(events)
}

fn accept_message(
    pane: &StagePane,
    running: &mut Running,
    message: FromRenderer,
    events: &mut InboxEvents,
) -> Result<(), String> {
    let adoption = match message {
        FromRenderer::Surface {
            generation,
            slot,
            handle,
            width,
            height,
        } => Some(running.adopt(handle, generation, slot, width, height)),
        FromRenderer::Frame {
            frame,
            width,
            height,
            rgba,
        } => Some(running.compositor.accept_copy(frame, width, height, rgba)),
        FromRenderer::FrameReady(frame) => Some(running.compositor.mark_ready(frame)),
        FromRenderer::Presented(sample) => {
            pane.record_benchmark_frame(running, sample)?;
            None
        }
        FromRenderer::BenchmarkHeartbeat(heartbeat) => {
            pane.record_benchmark_heartbeat(running, heartbeat)?;
            None
        }
        FromRenderer::Camera(camera) => {
            events.camera = Some(camera);
            None
        }
        FromRenderer::Picked { fixture, additive } => {
            events.picked.push((fixture, additive));
            None
        }
        FromRenderer::Trouble(detail) => {
            events.trouble = Some(detail);
            None
        }
        FromRenderer::Finished(detail) => {
            events.trouble = Some(detail);
            events.ended = true;
            None
        }
    };
    if let Some(Err(detail)) = adoption {
        events.ended = true;
        events.trouble = Some(detail);
        if let Some(benchmark) = running.benchmark.as_mut() {
            benchmark.draw_healthy = false;
        }
    }
    Ok(())
}

fn publish_events(pane: &StagePane, events: InboxEvents) -> Result<(), String> {
    if let Some(camera) = events.camera {
        *pane.camera.lock().map_err(|_| "the Stage pane")? = Some(camera);
    }
    if !events.picked.is_empty() {
        pane.picked
            .lock()
            .map_err(|_| "the Stage pane")?
            .extend(events.picked);
    }
    if let Some(detail) = events.trouble {
        *pane.trouble.lock().map_err(|_| "the Stage pane")? = Some(detail);
    }
    Ok(())
}
