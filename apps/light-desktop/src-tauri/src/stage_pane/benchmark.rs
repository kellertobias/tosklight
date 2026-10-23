use super::{PaneRect, Running, StagePane, StagePanes};

/// Decode only the passive diagnostic variant here; the ordinary transport reader remains in
/// the pane module. Unexpected input fails the owned channel instead of silently disappearing.
pub(super) fn forward_heartbeat(
    message: super::FromHelper,
    outbox: &std::sync::mpsc::Sender<super::FromRenderer>,
) -> Result<(), std::sync::mpsc::SendError<super::FromRenderer>> {
    let super::FromHelper::BenchmarkHeartbeat {
        epoch_micros,
        sequence,
        last_presented_sequence,
        source_frame,
        follow_preload,
        width,
        height,
        idle_unchanged,
        draw_healthy,
    } = message
    else {
        return outbox.send(super::FromRenderer::Finished(
            "unexpected benchmark heartbeat message".to_owned(),
        ));
    };
    outbox.send(super::FromRenderer::BenchmarkHeartbeat(PaneHeartbeat {
        pane_id: String::new(),
        attachment_id: String::new(),
        sequence,
        epoch_micros,
        received_epoch_micros: benchmark_epoch_micros(),
        last_presented_sequence,
        source_frame,
        follow_preload,
        width,
        height,
        attached: false,
        visible: false,
        idle_unchanged,
        draw_healthy,
    }))
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrameTelemetry {
    pub(super) pane_id: String,
    pub(super) attachment_id: String,
    pub(super) sequence: u64,
    pub(super) source_frame: u64,
    pub(super) source_input_epoch_micros: u64,
    pub(super) presented_epoch_micros: u64,
    /// Renderer elapsed wall time, not executing-thread CPU.
    pub(super) cpu_micros: u64,
    pub(super) acquire_micros: u64,
    pub(super) gpu_micros: Option<u64>,
    pub(super) gpu_sample_id: Option<u64>,
    pub(super) gpu_sample_matches_frame: bool,
    pub(super) transport: &'static str,
    pub(super) instances: u32,
    pub(super) draw_calls: u32,
    pub(super) degraded: bool,
    pub(super) renderer: String,
    pub(super) quality: &'static str,
    pub(super) mode: String,
    pub(super) follow_preload: bool,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Passive loop observations, not GPU presentation or compositor acknowledgements.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaneHeartbeat {
    pub(super) pane_id: String,
    pub(super) attachment_id: String,
    pub(super) sequence: u64,
    pub(super) epoch_micros: u64,
    pub(super) received_epoch_micros: u64,
    pub(super) last_presented_sequence: u64,
    pub(super) source_frame: u64,
    pub(super) follow_preload: bool,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) attached: bool,
    pub(super) visible: bool,
    pub(super) idle_unchanged: bool,
    pub(super) draw_healthy: bool,
}

#[derive(Default, serde::Serialize)]
pub(crate) struct StagePaneBenchmarkSamples {
    pub(super) frames: Vec<FrameTelemetry>,
    pub(super) heartbeats: Vec<PaneHeartbeat>,
    pub(super) compositors: Vec<CompositorTelemetry>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompositorTelemetry {
    pane_id: String,
    attachment_id: String,
    observed_epoch_micros: u64,
    measurement: crate::stage_compositor::CompositorMeasurements,
}

pub(super) struct BenchmarkAttachment {
    pub(super) id: String,
    pub(super) source_ready: bool,
    pub(super) draw_healthy: bool,
}

impl BenchmarkAttachment {
    fn attached_identity(&self, pipe_open: bool) -> Option<&str> {
        (pipe_open && self.source_ready && self.draw_healthy).then_some(self.id.as_str())
    }
}

pub(super) fn benchmark_attachment() -> Result<Option<BenchmarkAttachment>, String> {
    if std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT").is_none() {
        return Ok(None);
    }
    Ok(Some(BenchmarkAttachment {
        id: next_attachment_id()?,
        source_ready: false,
        draw_healthy: false,
    }))
}

// Unique within one owned desktop process/report. Exhaustion fails closed, never wraps/reuses.
pub(super) fn next_attachment_id() -> Result<String, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |value| value.checked_add(1),
    )
    .map(|previous| (previous + 1).to_string())
    .map_err(|_| "the Stage pane attachment identity exhausted".to_owned())
}

fn benchmark_epoch_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_micros()).ok())
        .unwrap_or(0)
}

fn benchmark_pane_visible(
    pane: PaneRect,
    scale: f32,
    surface_size: (u32, u32),
    window_visible: bool,
) -> bool {
    window_visible
        && scale.is_finite()
        && scale > 0.0
        && pane
            .to_pixels(scale, surface_size.0, surface_size.1)
            .is_some()
}

fn heartbeat_is_attached(current: Option<&str>, observed: &str, drawing: bool) -> bool {
    drawing && current.is_some_and(|identity| !identity.is_empty() && identity == observed)
}

/// The helper's existing pane_pixels/fits_in_one_frame contract. This validates evidence only;
/// it neither resizes the helper nor changes the compositor's negotiated transport.
fn benchmark_raster_size(
    pane: PaneRect,
    scale: f32,
    transport: super::FrameTransport,
) -> Option<(u32, u32)> {
    if !scale.is_finite()
        || scale <= 0.0
        || !pane.width.is_finite()
        || !pane.height.is_finite()
        || pane.width <= 0.0
        || pane.height <= 0.0
    {
        return None;
    }
    let width = ((pane.width * scale).round().max(1.0) as u32).min(16_384);
    let height = ((pane.height * scale).round().max(1.0) as u32).min(16_384);
    if transport == super::FrameTransport::Shared {
        return Some((width, height));
    }
    let budget = (viz_helper::framing::MAX_FRAME as u64) - 4_096;
    let pixels = u64::from(width) * u64::from(height) * 4;
    if pixels <= budget {
        return Some((width, height));
    }
    let ratio = (budget as f64 / pixels as f64).sqrt();
    Some((
        ((f64::from(width) * ratio).floor() as u32).max(1),
        ((f64::from(height) * ratio).floor() as u32).max(1),
    ))
}

impl StagePanes {
    pub(super) fn take_benchmark_samples(
        &self,
        window_label: &str,
        window_visible: bool,
    ) -> Result<StagePaneBenchmarkSamples, String> {
        let panes = self.panes.lock().map_err(|_| "the Stage pane registry")?;
        let mut result = StagePaneBenchmarkSamples::default();
        let prefix = format!("{window_label}:");
        for (pane_id, registered) in panes.iter().filter(|(key, _)| key.starts_with(&prefix)) {
            let mut running = registered.pane.inner.lock().map_err(|_| "the Stage pane")?;
            if registered
                .pane
                .heartbeat_overflow
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err("the Stage pane benchmark heartbeat queue overflowed".to_owned());
            }
            if let Some(current) = running.as_mut() {
                if let Some(measurement) = current.compositor.drain_measurements() {
                    result.compositors.push(CompositorTelemetry {
                        pane_id: pane_id.clone(),
                        attachment_id: current
                            .benchmark
                            .as_ref()
                            .map_or_else(String::new, |b| b.id.clone()),
                        observed_epoch_micros: benchmark_epoch_micros(),
                        measurement,
                    });
                }
            }
            let mut samples = registered
                .pane
                .telemetry
                .lock()
                .map_err(|_| "the Stage pane benchmark telemetry")?;
            for mut sample in samples.drain(..) {
                sample.pane_id.clone_from(pane_id);
                result.frames.push(sample);
            }
            let mut heartbeats = registered
                .pane
                .heartbeats
                .lock()
                .map_err(|_| "the Stage pane benchmark heartbeats")?;
            for mut heartbeat in heartbeats.drain(..) {
                heartbeat.pane_id.clone_from(pane_id);
                heartbeat.attached = heartbeat_is_attached(
                    running.as_ref().and_then(|current| {
                        current.benchmark.as_ref().and_then(|benchmark| {
                            benchmark.attached_identity(current.to_helper.is_some())
                        })
                    }),
                    &heartbeat.attachment_id,
                    registered.pane.is_drawing(),
                );
                heartbeat.visible = heartbeat.attached
                    && running.as_ref().is_some_and(|current| {
                        benchmark_pane_visible(
                            current.pane,
                            current.scale,
                            current.surface_size,
                            window_visible,
                        )
                    });
                // A still picture from an earlier size cannot prove the current layout idle.
                heartbeat.idle_unchanged &= running.as_ref().is_some_and(|current| {
                    benchmark_raster_size(current.pane, current.scale, current.transport)
                        == Some((heartbeat.width, heartbeat.height))
                });
                result.heartbeats.push(heartbeat);
            }
        }
        Ok(result)
    }
}

impl StagePane {
    pub(super) fn record_benchmark_frame(
        &self,
        running: &Running,
        mut sample: FrameTelemetry,
    ) -> Result<(), String> {
        if let Some(benchmark) = running.benchmark.as_ref() {
            sample.attachment_id.clone_from(&benchmark.id);
        }
        let mut samples = self
            .telemetry
            .lock()
            .map_err(|_| "the Stage pane benchmark telemetry")?;
        if samples.len() >= 2_048 {
            let overflow = samples.len() + 1 - 2_048;
            samples.drain(..overflow);
        }
        samples.push(sample);
        Ok(())
    }

    pub(super) fn record_benchmark_heartbeat(
        &self,
        running: &Running,
        mut heartbeat: PaneHeartbeat,
    ) -> Result<(), String> {
        // This inbox belongs only to the current Running. An old pipe cannot be retagged with a
        // replacement attachment identity; it feeds the dropped old receiver instead.
        if let Some(benchmark) = running.benchmark.as_ref() {
            heartbeat.attachment_id.clone_from(&benchmark.id);
            self.push_benchmark_heartbeat(heartbeat)?;
        }
        Ok(())
    }

    fn push_benchmark_heartbeat(&self, heartbeat: PaneHeartbeat) -> Result<(), String> {
        let mut heartbeats = self
            .heartbeats
            .lock()
            .map_err(|_| "the Stage pane benchmark heartbeats")?;
        if heartbeats.len() >= 2_048 {
            self.heartbeat_overflow
                .store(true, std::sync::atomic::Ordering::Release);
        } else {
            heartbeats.push(heartbeat);
        }
        Ok(())
    }

    pub(super) fn clear_benchmark_samples(&self) -> Result<(), String> {
        self.telemetry
            .lock()
            .map_err(|_| "the Stage pane benchmark telemetry")?
            .clear();
        self.heartbeats
            .lock()
            .map_err(|_| "the Stage pane benchmark heartbeats")?
            .clear();
        self.heartbeat_overflow
            .store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod benchmark_heartbeat_tests {
    use super::*;

    fn heartbeat(attachment_id: &str) -> PaneHeartbeat {
        PaneHeartbeat {
            pane_id: String::new(),
            attachment_id: attachment_id.to_owned(),
            sequence: 1,
            epoch_micros: 1,
            received_epoch_micros: 2,
            last_presented_sequence: 1,
            source_frame: 7,
            follow_preload: true,
            width: 800,
            height: 600,
            attached: false,
            visible: false,
            idle_unchanged: true,
            draw_healthy: true,
        }
    }

    #[test]
    fn attachment_proof_requires_source_and_successful_host_draw() {
        let mut attachment = BenchmarkAttachment {
            id: "1".to_owned(),
            source_ready: false,
            draw_healthy: false,
        };
        assert_eq!(attachment.attached_identity(true), None);
        attachment.source_ready = true;
        assert_eq!(attachment.attached_identity(true), None);
        attachment.draw_healthy = true;
        assert_eq!(attachment.attached_identity(true), Some("1"));
        assert_eq!(attachment.attached_identity(false), None);
        attachment.source_ready = false;
        assert_eq!(attachment.attached_identity(true), None);
    }

    #[test]
    fn old_attachment_cannot_become_current_and_queue_overflow_is_explicit() {
        assert!(heartbeat_is_attached(Some("2"), "2", true));
        assert!(!heartbeat_is_attached(Some("2"), "1", true));
        assert!(!heartbeat_is_attached(None, "1", true));
        assert!(!heartbeat_is_attached(Some("2"), "2", false));
        let pane = StagePane::default();
        for _ in 0..2_049 {
            pane.push_benchmark_heartbeat(heartbeat("1")).unwrap();
        }
        assert_eq!(pane.heartbeats.lock().unwrap().len(), 2_048);
        assert!(
            pane.heartbeat_overflow
                .load(std::sync::atomic::Ordering::Acquire)
        );
        pane.clear_benchmark_samples().unwrap();
        assert!(pane.heartbeats.lock().unwrap().is_empty());
        assert!(
            !pane
                .heartbeat_overflow
                .load(std::sync::atomic::Ordering::Acquire)
        );
    }

    #[test]
    fn resized_host_requires_current_helper_raster_including_copy_budget() {
        assert_eq!(
            benchmark_raster_size(
                PaneRect::full(800.0, 600.0),
                1.0,
                super::super::FrameTransport::Copy
            ),
            Some((800, 600))
        );
        assert_ne!(
            benchmark_raster_size(
                PaneRect::full(900.0, 600.0),
                1.0,
                super::super::FrameTransport::Copy
            ),
            Some((800, 600))
        );
        let (width, height) = benchmark_raster_size(
            PaneRect::full(16_384.0, 16_384.0),
            1.0,
            super::super::FrameTransport::Copy,
        )
        .unwrap();
        assert!(
            u64::from(width) * u64::from(height) * 4
                <= viz_helper::framing::MAX_FRAME as u64 - 4_096
        );
        assert_eq!(
            benchmark_raster_size(
                PaneRect::full(16_384.0, 16_384.0),
                1.0,
                super::super::FrameTransport::Shared
            ),
            Some((16_384, 16_384))
        );
    }

    #[test]
    fn attachment_generation_never_reuses_a_closed_pane_identity() {
        let old = next_attachment_id().unwrap();
        let replacement = next_attachment_id().unwrap();
        assert_ne!(old, replacement);
        assert!(old.parse::<u64>().unwrap() < replacement.parse::<u64>().unwrap());
    }

    #[test]
    fn visibility_requires_known_visible_window_and_nonempty_current_bounds() {
        let pane = PaneRect::full(800.0, 600.0);
        assert!(benchmark_pane_visible(pane, 1.0, (800, 600), true));
        assert!(!benchmark_pane_visible(pane, 1.0, (800, 600), false));
        assert!(!benchmark_pane_visible(pane, f32::NAN, (800, 600), true));
        assert!(!benchmark_pane_visible(pane, 1.0, (0, 600), true));
        assert!(!benchmark_pane_visible(
            PaneRect::default(),
            1.0,
            (800, 600),
            true
        ));
        assert!(!benchmark_pane_visible(
            PaneRect { x: 900.0, ..pane },
            1.0,
            (800, 600),
            true
        ));
        assert!(!benchmark_pane_visible(
            PaneRect {
                x: f32::NAN,
                ..pane
            },
            1.0,
            (800, 600),
            true
        ));
    }
}
