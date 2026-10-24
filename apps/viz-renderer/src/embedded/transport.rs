//! GPU-completed images and raster generations, kept separate from scene preparation.
use super::*;

impl PaneState {
    /// Follow the pane the desk's layout reports.
    ///
    /// A shared surface is a fixed size, so a resize replaces it and the desk is told — which is
    /// why the handle is announced on every change rather than once at the start.
    pub(super) fn resize_for(&mut self, embedding: &Embedding) -> Result<(), String> {
        let size = pane_pixels(embedding);
        if !self.visible || size == self.size || !self.producer.available(Instant::now()) {
            return Ok(());
        }
        if !self.producer.resize(Instant::now())? {
            return Ok(());
        }
        self.size = size;
        self.renderer.resize(size.0, size.1);
        // A shared surface is a fixed size, so the next frame builds and announces a new one.
        self.shared = std::array::from_fn(|_| None);
        self.redraw_gate.invalidate();
        Ok(())
    }

    /// Device callbacks must be progressed even when every slot belongs to the desk.
    pub(super) fn poll_transport(&mut self, source: &mut HelperSource) -> Result<(), String> {
        self.renderer
            .device()
            .poll(wgpu::PollType::Poll)
            .map_err(|error| format!("Stage GPU polling failed: {error}"))?;
        self.poll_copy(source);
        while let Ok(frame) = self.completions.try_recv() {
            if self.producer.completed(frame) {
                source.send_frame_ready(frame);
            }
        }
        for frame in source.take_released_frames() {
            self.producer.release(frame);
        }
        Ok(())
    }

    /// Shared images stay immutable while the desk retains them. The callback owns backing memory
    /// until GPU writing completes; only then is readiness announced from the render loop.
    pub(super) fn draw_shared(
        &mut self,
        frame: FrameId,
        time: f32,
        source: &mut HelperSource,
    ) -> Result<viz_render::FrameStats, String> {
        let slot = frame.slot as usize;
        if self.shared[slot].is_none() {
            let surface = Arc::new(
                viz_surface::create(self.renderer.device(), self.size.0, self.size.1)
                    .map_err(|error| error.to_string())?,
            );
            #[cfg(target_os = "macos")]
            {
                let service = self
                    .surface_service
                    .as_deref()
                    .ok_or("the desk asked for a shared surface without saying where to send it")?;
                viz_surface::rendezvous::send_port(service, surface.mach_port())?;
            }
            source.send_surface(frame, surface.handle(), self.size.0, self.size.1);
            self.shared[slot] = Some(surface);
        }
        let surface = Arc::clone(
            self.shared[slot]
                .as_ref()
                .ok_or("Stage image slot has no surface")?,
        );
        let stats = self
            .renderer
            .render_into(
                surface.view(),
                &self.scene,
                &self.values,
                &self.view,
                &self.overlay,
                time,
            )
            .map_err(|error| error.to_string())?;
        let completed = self.completed.clone();
        self.renderer.queue().on_submitted_work_done(move || {
            let _backing = surface;
            let _ = completed.send(frame);
        });
        Ok(stats)
    }

    /// Readback completes independently of the input loop. One reserved copy slot stays Rendering
    /// until its pixels are available; resizing therefore cannot overtake this capture.
    pub(super) fn draw_copy(
        &mut self,
        frame: FrameId,
        time: f32,
        _source: &mut HelperSource,
    ) -> Result<viz_render::FrameStats, String> {
        if self.pending_copy.is_some() {
            return Err("Stage copy readback is already pending".to_owned());
        }
        let pending = self
            .renderer
            .begin_capture(&self.scene, &self.values, &self.view, &self.overlay, time)
            .map_err(|error| error.to_string())?;
        self.pending_copy = Some((frame, pending));
        Ok(self.renderer.stats())
    }

    fn poll_copy(&mut self, source: &mut HelperSource) {
        let Some((frame, capture)) = self.pending_copy.as_mut() else {
            return;
        };
        let frame = *frame;
        match capture.try_complete() {
            Ok(None) => {}
            Ok(Some(image)) => {
                self.pending_copy = None;
                if self.producer.completed(frame) {
                    // Both messages use the same writer: pixels always precede readiness.
                    source.send_frame(frame, image.width, image.height, image.rgba);
                    source.send_frame_ready(frame);
                }
            }
            Err(error) => {
                self.pending_copy = None;
                self.producer.cancel(frame);
                self.redraw_gate.invalidate();
                if let Some(benchmark) = self.benchmark.as_mut() {
                    benchmark.last_success = None;
                    benchmark.idle_unchanged = false;
                    benchmark.draw_healthy = false;
                }
                // begin() already advanced the 60 Hz deadline, so failures cannot create an
                // unbounded retry loop. Report once for this failed reservation.
                source.report(&format!("Stage copy readback failed for frame {}: {error}; retrying at the next render deadline", frame.sequence));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_copy_holds_resize_and_error_retry_keeps_deadline_and_exact_identity() {
        let now = Instant::now();
        let mut producer = producer::Producer::new(false, now);
        let failed = producer.begin(now).unwrap();
        producer.submitted(failed);
        assert!(!producer.resize(now + Duration::from_millis(2)).unwrap());
        assert!(producer.begin(now + Duration::from_millis(17)).is_none());
        producer.cancel(failed);
        assert!(
            producer.begin(now + Duration::from_millis(2)).is_none(),
            "failure must not bypass pacing"
        );
        let retry = producer.begin(now + Duration::from_millis(17)).unwrap();
        assert!(retry.sequence > failed.sequence);
        producer.cancel(failed);
        assert!(
            !producer.resize(now + Duration::from_millis(18)).unwrap(),
            "late failure cannot cancel retry"
        );
        assert!(producer.completed(retry));
        assert!(!producer.release(failed));
        assert!(producer.release(retry));
    }
}
