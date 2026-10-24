//! Slot transport decoding and native shared-surface imports.
use super::*;
use viz_helper::protocol::FrameId;

/// What the reading thread hands the desk.
pub(super) enum FromRenderer {
    /// A surface both processes address, replacing whatever was being drawn.
    Surface {
        generation: u64,
        slot: u8,
        handle: SharedSurfaceHandle,
        width: u32,
        height: u32,
    },
    /// A frame that came through the pipe.
    Frame {
        frame: FrameId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    FrameReady(FrameId),
    Presented(FrameTelemetry),
    BenchmarkHeartbeat(PaneHeartbeat),
    /// Where the camera now is.
    Camera([f32; 6]),
    Still(viz_helper::still::Status),
    /// What the operator pointed at in the pane.
    Picked {
        fixture: Option<String>,
        additive: bool,
    },
    /// Something the operator has to see. Not fatal on its own.
    Trouble(String),
    /// The channel ended, with the reason.
    Finished(String),
}

impl Running {
    /// The message only says one is waiting; the right itself came through the rendezvous, because
    /// a mach port name means nothing outside the task holding it and so cannot travel down the
    /// channel the message did.
    pub(super) fn adopt(
        &mut self,
        #[cfg_attr(target_os = "macos", allow(unused_variables))] handle: SharedSurfaceHandle,
        generation: u64,
        slot: u8,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let Some(rendezvous) = self.rendezvous.as_ref() else {
                return Err("a surface arrived with nowhere to have come from".to_owned());
            };
            // Already queued: the renderer sends the right before the message that announces it.
            let port = rendezvous
                // Mach send completes before the helper writes its Surface announcement.
                // MACH_RCV_TIMEOUT with zero only consumes an already queued right; never wait
                // on a producer or driver from the Control thread.
                .receive(std::time::Duration::ZERO)?
                .ok_or("the renderer announced a surface it never handed over")?;
            return self
                .compositor
                .import_shared_slot_port(generation, slot, port, width, height);
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.compositor
                .import_shared_slot(generation, slot, handle, width, height)
        }
    }
}

/// The reading thread: decode what the renderer says until the channel ends.
pub(super) fn read_renderer(
    mut from_helper: impl std::io::Read,
    outbox: &std::sync::mpsc::SyncSender<FromRenderer>,
) {
    loop {
        let frame = match framing::read_frame(&mut from_helper) {
            Ok(frame) => frame,
            Err(error) => {
                let _ = outbox.send(FromRenderer::Finished(error.to_string()));
                return;
            }
        };
        let sent = match decode::<FromHelper>(&frame) {
            Ok(FromHelper::Surface {
                generation,
                slot,
                handle,
                width,
                height,
            }) => outbox.send(FromRenderer::Surface {
                generation,
                slot,
                handle,
                width,
                height,
            }),
            Ok(FromHelper::Frame {
                frame,
                width,
                height,
                rgba,
            }) => outbox.send(FromRenderer::Frame {
                frame,
                width,
                height,
                rgba,
            }),
            Ok(FromHelper::FrameReady { frame }) => outbox.send(FromRenderer::FrameReady(frame)),
            Ok(FromHelper::FramePresented {
                sequence,
                source_frame,
                source_input_epoch_micros,
                presented_epoch_micros,
                cpu_micros,
                acquire_micros,
                gpu_micros,
                gpu_sample_id,
                gpu_sample_matches_frame,
                transport,
                instances,
                draw_calls,
                degraded,
                renderer,
                quality,
                mode,
                follow_preload,
                width,
                height,
            }) => outbox.send(FromRenderer::Presented(FrameTelemetry {
                pane_id: String::new(),
                attachment_id: String::new(),
                sequence,
                source_frame,
                source_input_epoch_micros,
                presented_epoch_micros,
                cpu_micros,
                acquire_micros,
                gpu_micros,
                gpu_sample_id,
                gpu_sample_matches_frame,
                transport: match transport {
                    viz_helper::protocol::FrameTransport::Shared => "shared",
                    viz_helper::protocol::FrameTransport::Copy => "copy",
                },
                instances,
                draw_calls,
                degraded,
                renderer,
                quality: match quality {
                    viz_helper::protocol::RenderQuality::Draft => "draft",
                    viz_helper::protocol::RenderQuality::Standard => "standard",
                    viz_helper::protocol::RenderQuality::High => "high",
                    viz_helper::protocol::RenderQuality::Ultra => "ultra",
                },
                mode,
                follow_preload,
                width,
                height,
            })),
            Ok(heartbeat @ FromHelper::BenchmarkHeartbeat { .. }) => {
                benchmark::forward_heartbeat(heartbeat, outbox)
            }
            Ok(FromHelper::Camera {
                x,
                y,
                z,
                pan,
                tilt,
                distance,
            }) => outbox.send(FromRenderer::Camera([x, y, z, pan, tilt, distance])),
            Ok(FromHelper::Picked { fixture, additive }) => {
                outbox.send(FromRenderer::Picked { fixture, additive })
            }
            Ok(FromHelper::StillStatus { status }) => outbox.send(FromRenderer::Still(status)),
            Ok(FromHelper::Error { detail }) => outbox.send(FromRenderer::Trouble(detail)),
            Ok(FromHelper::Stopping { detail }) => outbox.send(FromRenderer::Finished(detail)),
            // The greeting is over; anything belonging to it arriving now is a stream out of step.
            Ok(FromHelper::Ready { .. } | FromHelper::Capabilities { .. }) => Ok(()),
            Err(detail) => outbox.send(FromRenderer::Finished(detail)),
        };
        if sent.is_err() {
            return;
        }
        super::wake_pump();
    }
}
