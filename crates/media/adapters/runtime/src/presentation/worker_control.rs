use super::*;

impl RenderWorkerState {
    pub(super) fn run(
        mut self,
        receiver: std::sync::mpsc::Receiver<RenderCommand>,
        shutdown: Shutdown,
    ) {
        loop {
            if !self.apply_pending_commands(&receiver) || shutdown.reason().is_some() {
                break;
            }

            self.present_all();

            let now = self.now();
            let wait = presentation_worker_wait(
                self.outputs
                    .iter()
                    .map(|hosted| hosted.output.time_until_deadline(now)),
            );
            if let Some(duration) = wait {
                match receiver.recv_timeout(duration) {
                    Ok(command) => {
                        if !self.apply_command(command) {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }

        for hosted in &self.outputs {
            let cadence = hosted.output.cadence();
            tracing::info!(
                id = %hosted.output.id(),
                frames = cadence.frames,
                measured_fps = cadence.frames_per_second(),
                "output stopped"
            );
        }
    }

    fn apply_pending_commands(
        &mut self,
        receiver: &std::sync::mpsc::Receiver<RenderCommand>,
    ) -> bool {
        let mut resizes = std::collections::BTreeMap::new();
        while let Ok(command) = receiver.try_recv() {
            match command {
                RenderCommand::Resize { window, size } => {
                    resizes.insert(window, size);
                }
                RenderCommand::ShowFullscreenHint { window } => {
                    self.show_fullscreen_hint(window);
                }
                RenderCommand::HideFullscreenHint { window } => {
                    self.hide_fullscreen_hint(window);
                }
                RenderCommand::Stop => return false,
            }
        }
        for (window, size) in resizes {
            self.resize(window, size);
        }
        true
    }

    fn apply_command(&mut self, command: RenderCommand) -> bool {
        match command {
            RenderCommand::Resize { window, size } => {
                self.resize(window, size);
                true
            }
            RenderCommand::ShowFullscreenHint { window } => {
                self.show_fullscreen_hint(window);
                true
            }
            RenderCommand::HideFullscreenHint { window } => {
                self.hide_fullscreen_hint(window);
                true
            }
            RenderCommand::Stop => false,
        }
    }

    fn resize(&mut self, window: WindowId, size: Size) {
        let Some(hosted) = self
            .outputs
            .iter_mut()
            .find(|hosted| hosted.window.id() == window)
        else {
            return;
        };
        hosted.output.resize(size);
        hosted.pipeline.resize(size);
        hosted.standby = crate::standby::render(size, &self.administration_endpoint)
            .and_then(|frame| {
                SourceTexture::from_rgba8(hosted.output.gpu(), frame.size, &frame.pixels)
                    .map_err(anyhow::Error::from)
            })
            .ok();
        hosted.fullscreen_hint = crate::fullscreen_hint::render(size)
            .and_then(|frame| {
                SourceTexture::from_rgba8(hosted.output.gpu(), frame.size, &frame.pixels)
                    .map_err(anyhow::Error::from)
            })
            .ok();
    }

    fn show_fullscreen_hint(&mut self, window: WindowId) {
        if let Some(hosted) = self
            .outputs
            .iter_mut()
            .find(|hosted| hosted.window.id() == window)
        {
            hosted.hint_visible_until =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
        }
    }

    fn hide_fullscreen_hint(&mut self, window: WindowId) {
        if let Some(hosted) = self
            .outputs
            .iter_mut()
            .find(|hosted| hosted.window.id() == window)
        {
            hosted.hint_visible_until = None;
        }
    }
}
