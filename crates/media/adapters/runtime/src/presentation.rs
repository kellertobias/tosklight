//! The presentation host.
//!
//! Windowed outputs need the platform's event loop, and every supported platform requires that
//! loop on the main thread. So the process is arranged the other way round from a plain server:
//! the asynchronous services run on a background runtime, and the main thread belongs to the
//! outputs.
//!
//! A process whose outputs are all off-screen never builds an event loop at all.

use std::sync::Arc;

use media_application::configuration::{MediaConfiguration, OutputConfiguration, OutputTarget};
use media_domain::geometry::Size;
use media_domain::{MasterState, MediaState, Timestamp};

use crate::dmx::SharedState;
use crate::layer_pipeline::LayerPipeline;
use media_playback::{ClipLoader, PlaybackSession};
use media_render::{LayerDraw, SourceTexture, SurfaceLost, WindowedOutput, select_monitor};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::shutdown::{Shutdown, ShutdownReason};

/// Whether this configuration needs a window at all.
pub fn needs_a_window(configuration: &MediaConfiguration) -> bool {
    configuration
        .outputs
        .iter()
        .any(|output| output.enabled && matches!(output.target, OutputTarget::Monitor { .. }))
}

/// Runs the platform event loop until every output has closed or shutdown is requested.
///
/// Returns once the loop exits. The caller's services keep running on their own runtime
/// throughout; nothing here waits on them, and they do not wait on this.
pub fn run_event_loop(
    configuration: &MediaConfiguration,
    state: SharedState,
    catalog: SharedCatalog,
    shutdown: Shutdown,
    diagnostics: Diagnostics,
    // The same reference point the network listeners stamp against, so a packet's arrival and a
    // frame's presentation sit on one timeline.
    started: std::time::Instant,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    // Outputs present continuously, so the loop should come back round rather than sleep until
    // the next input event.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut host = PresentationHost {
        configuration: Arc::new(configuration.clone()),
        catalog,
        outputs: Vec::new(),
        pending: configuration
            .outputs
            .iter()
            .filter(|output| output.enabled)
            .cloned()
            .collect(),
        state,
        shutdown,
        diagnostics,
        started,
        test_pattern_layer: test_pattern_layer(),
        loader: ClipLoader::new(configuration.playback.cache_budget_bytes),
        direct: None,
        clip_size: Size::new(2, 2),
    };
    event_loop.run_app(&mut host)?;
    Ok(())
}

/// Diagnostics an operator can ask for at launch.
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    /// A clip to play on layer one of every output, for trying the whole path without a desk or a
    /// catalog. A development affordance, not a product feature.
    pub play: Option<std::path::PathBuf>,
    /// Fill layer one with a flat colour so an operator can confirm the output really is on the
    /// monitor, at the size, and the right way up. A diagnostic, not a media source: it draws
    /// only while nothing else has been selected.
    pub test_pattern: bool,
}

/// The published library snapshot, shared with the services so both read one catalog.
pub type SharedCatalog = Arc<arc_swap::ArcSwap<media_domain::catalog::CatalogSnapshot>>;

struct HostedOutput {
    output: WindowedOutput,
    /// Kept alive for the surface's lifetime, and used to resolve resize events back to an output.
    window: Arc<Window>,
    test_pattern: Option<SourceTexture>,
    sources: crate::layer_sources::LayerSources,
    /// This output's path from addresses to textures.
    pipeline: LayerPipeline,
}

/// A clip loaded for the development `--play` affordance.
struct DirectClip {
    asset: media_domain::AssetId,
    session: PlaybackSession,
    layer: media_domain::LayerState,
}

struct PresentationHost {
    configuration: Arc<MediaConfiguration>,
    catalog: SharedCatalog,
    outputs: Vec<HostedOutput>,
    pending: Vec<OutputConfiguration>,
    state: SharedState,
    shutdown: Shutdown,
    diagnostics: Diagnostics,
    started: std::time::Instant,
    test_pattern_layer: media_domain::LayerState,
    loader: ClipLoader,
    direct: Option<DirectClip>,
    clip_size: Size,
}

/// The diagnostic pattern's colour: unmistakably not black and unmistakably not media.
const TEST_PATTERN: [u8; 4] = [0, 96, 160, 255];

/// A drawable layer for the diagnostic pattern, stretched over the whole output.
fn test_pattern_layer() -> media_domain::LayerState {
    media_domain::LayerState {
        address: media_domain::MediaAddress::new(1, 1),
        source_status: media_domain::SourceStatus::Ready,
        scaling_mode: media_domain::ScalingMode::Stretch,
        ..Default::default()
    }
}

impl PresentationHost {
    /// Loads the clip named at launch, reporting as it goes.
    fn load_direct_clip(&mut self) {
        let Some(path) = self.diagnostics.play.clone() else {
            return;
        };
        let asset = media_domain::AssetId::new();
        let loaded = match self.loader.load(asset, &path, &mut |progress| {
            tracing::info!(?progress, "loading clip");
        }) {
            Ok(loaded) => loaded,
            Err(error) => {
                tracing::error!(path = %path.display(), %error, "cannot play that clip");
                return;
            }
        };

        self.clip_size = Size::new(loaded.width, loaded.height);
        // A clip an operator asked to see is in use, so it is pinned and never evicted.
        self.loader.cache_mut().pin(asset);
        tracing::info!(
            path = %path.display(),
            frames = loaded.presentation_micros.len(),
            width = loaded.width,
            height = loaded.height,
            tempo = loaded.timing.intrinsic_bpm,
            "playing"
        );
        self.direct = Some(DirectClip {
            asset,
            session: PlaybackSession::new(
                asset,
                loaded.timing,
                loaded.presentation_micros,
                Timestamp::ZERO,
                media_domain::PlayMode::Loop,
            ),
            layer: media_domain::LayerState {
                address: media_domain::MediaAddress::new(1, 1),
                source_status: media_domain::SourceStatus::Ready,
                scaling_mode: media_domain::ScalingMode::Fit,
                ..Default::default()
            },
        });
    }

    fn now(&self) -> Timestamp {
        Timestamp::from_micros(self.started.elapsed().as_micros() as u64)
    }

    fn open(&mut self, event_loop: &ActiveEventLoop, configuration: &OutputConfiguration) {
        let OutputTarget::Monitor {
            monitor,
            fullscreen,
        } = &configuration.target
        else {
            return; // Off-screen outputs never reach the event loop.
        };

        let selected = select_monitor(monitor, event_loop.available_monitors());
        if selected.is_none() {
            // Opening on a different display would be worse than saying so: an operator would
            // have no way to tell the output had moved.
            tracing::error!(
                output = %configuration.name,
                ?monitor,
                "the configured monitor is not connected; this output stays closed"
            );
            return;
        }

        let mut attributes = Window::default_attributes()
            .with_title(format!("ToskLight Media — {}", configuration.name))
            .with_inner_size(winit::dpi::PhysicalSize::new(
                configuration.resolution.width,
                configuration.resolution.height,
            ));
        if *fullscreen {
            attributes =
                attributes.with_fullscreen(Some(winit::window::Fullscreen::Borderless(selected)));
        }

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot open the output window");
                return;
            }
        };

        match WindowedOutput::open(configuration.id, window.clone(), configuration.presentation) {
            Ok(output) => {
                tracing::info!(
                    output = %configuration.name,
                    id = %configuration.id,
                    refresh_millihertz = output.monitor_refresh_millihertz(),
                    "output presenting"
                );
                // The pattern must be uploaded to this output's own device: two devices cannot
                // share a texture.
                let test_pattern = self
                    .diagnostics
                    .test_pattern
                    .then(|| SourceTexture::solid(output.gpu(), Size::new(2, 2), TEST_PATTERN).ok())
                    .flatten();
                let source_size = self.clip_size;
                let sources = crate::layer_sources::LayerSources::new(output.gpu(), source_size);
                let mut pipeline = LayerPipeline::new(
                    output.gpu(),
                    configuration.id,
                    media_library::LibraryStorage::new(self.configuration.library.root.clone()),
                    output.size(),
                );
                pipeline.validate_visualizers();
                self.outputs.push(HostedOutput {
                    output,
                    window,
                    test_pattern,
                    sources,
                    pipeline,
                });
            }
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot render to this output");
            }
        }
    }

    fn present_all(&mut self) {
        let now = self.now();
        let seconds = self.started.elapsed().as_secs_f32();
        let state = self.state.load();
        let catalog = self.catalog.load();
        // Audio capture is product- and platform-owned and arrives with its own slice. Silence is
        // a real analysis, not a placeholder: time-driven visualizers run, audio-driven ones rest.
        let analysis = media_domain::audio::Analysis::default();
        let mut reports = Vec::new();

        for hosted in &mut self.outputs {
            if !hosted.output.should_present(now) {
                continue;
            }
            let Some(output_state) = state.output(hosted.output.id()) else {
                continue;
            };
            let master = output_state.master;

            // A clip named at launch plays on layer one. It is a development affordance and it
            // takes precedence over the real path so a machine with no library still proves it.
            if let Some(direct) = self.direct.as_mut() {
                let delivery =
                    direct
                        .session
                        .deliver(&direct.layer, media_domain::ResolvedTempo::None, now);
                if let Some(frame) = delivery.frame
                    && hosted
                        .sources
                        .prepare(0, direct.asset, frame, self.loader.cache_mut())
                    && let Some(texture) = hosted.sources.texture(0)
                {
                    let draws = [LayerDraw {
                        state: &direct.layer,
                        source: texture,
                        mask: None,
                    }];
                    present(&mut hosted.output, &draws, &master, None, now);
                    hosted.window.request_redraw();
                    continue;
                }
            }

            // The real path: every layer's address becomes a texture, or reports why it did not.
            let prepared = hosted.pipeline.prepare(
                output_state,
                crate::layer_pipeline::FrameContext {
                    catalog: &catalog,
                    configuration: &self.configuration,
                    analysis: &analysis,
                    seconds,
                    now,
                },
                &mut self.loader,
            );
            reports.extend(
                prepared
                    .statuses
                    .iter()
                    .map(|(layer, status)| (output_state.id, *layer, *status)),
            );

            let mut draws = hosted.pipeline.draws(output_state, &prepared);
            // The diagnostic pattern occupies layer one only while nothing else has been
            // selected, so it can never hide a running show.
            if draws.is_empty()
                && let Some(pattern) = hosted.test_pattern.as_ref()
            {
                draws.push(LayerDraw {
                    state: &self.test_pattern_layer,
                    source: pattern,
                    mask: None,
                });
            }

            let master_mask = prepared
                .master_mask
                .and_then(|slot| hosted.pipeline.texture(slot));
            present(&mut hosted.output, &draws, &master, master_mask, now);
            hosted.window.request_redraw();
        }

        self.publish(reports, now);
    }

    /// Tells the reducer what each layer's source did, so the API, the UI, and CITP all report the
    /// lifecycle the renderer actually saw rather than each guessing at it.
    fn publish(
        &self,
        reports: Vec<(media_domain::OutputId, usize, media_domain::SourceStatus)>,
        now: Timestamp,
    ) {
        if reports.is_empty() {
            return;
        }
        if let Some(next) = with_reports(&self.state.load(), &reports, now) {
            self.state.store(Arc::new(next));
        }
    }
}

/// Presents one frame, keeping a lost surface from becoming a lost output.
fn present(
    output: &mut WindowedOutput,
    draws: &[LayerDraw<'_>],
    master: &MasterState,
    master_mask: Option<&SourceTexture>,
    now: Timestamp,
) {
    match output.present(draws, master, master_mask, now) {
        Ok(()) | Err(SurfaceLost::Recovered | SurfaceLost::Timeout) => {}
        Err(error) => {
            tracing::error!(id = %output.id(), %error, "output stopped presenting");
        }
    }
}

/// Applies the renderer's source reports to the authoritative state.
///
/// Returns the next state when anything changed, so a frame in which nothing loaded or failed
/// publishes nothing at all rather than churning a snapshot every sixtieth of a second.
fn with_reports(
    state: &MediaState,
    reports: &[(media_domain::OutputId, usize, media_domain::SourceStatus)],
    now: Timestamp,
) -> Option<MediaState> {
    let mut next = MediaState::clone(state);
    let mut changed = false;
    for (output, layer, status) in reports {
        let command = media_domain::Command::new(
            media_domain::CommandKind::ReportSourceStatus {
                output: *output,
                layer: *layer,
                status: *status,
            },
            media_domain::CommandSource::Internal,
            now,
        );
        if media_domain::apply(&mut next, &command) == media_domain::Applied::Changed {
            changed = true;
        }
    }
    changed.then_some(next)
}

impl ApplicationHandler for PresentationHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.outputs.is_empty() {
            return; // Already open; this is a wake, not a first start.
        }
        self.load_direct_clip();
        for configuration in std::mem::take(&mut self.pending) {
            self.open(event_loop, &configuration);
            self.pending.push(configuration);
        }
        if self.outputs.is_empty() {
            tracing::error!("no output could be opened; stopping");
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(hosted) = self
            .outputs
            .iter_mut()
            .find(|hosted| hosted.window.id() == id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.shutdown.request(ShutdownReason::Requested);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                // Only this output is rebuilt. Another output on another display keeps presenting
                // at its own size and its own cadence.
                let size = Size::new(size.width.max(1), size.height.max(1));
                hosted.output.resize(size);
                // Generated sources are output-sized by definition, so they follow the surface.
                hosted.pipeline.resize(size);
            }
            WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.shutdown.reason().is_some() {
            event_loop.exit();
            return;
        }
        self.present_all();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        for hosted in &self.outputs {
            let cadence = hosted.output.cadence();
            tracing::info!(
                id = %hosted.output.id(),
                frames = cadence.frames,
                measured_fps = cadence.frames_per_second(),
                "output stopped"
            );
        }
        self.shutdown.request(ShutdownReason::Requested);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_application::configuration::MonitorSelector;

    fn monitor_output() -> OutputConfiguration {
        let mut output = OutputConfiguration::new("Main");
        output.target = OutputTarget::Monitor {
            monitor: MonitorSelector::Index(0),
            fullscreen: false,
        };
        output
    }

    #[test]
    fn an_all_off_screen_configuration_never_builds_an_event_loop() {
        let configuration = MediaConfiguration::default();
        assert!(matches!(
            configuration.outputs[0].target,
            OutputTarget::OffScreen
        ));
        assert!(!needs_a_window(&configuration));
    }

    #[test]
    fn a_monitor_bound_output_needs_the_platform_event_loop() {
        let configuration = MediaConfiguration {
            outputs: vec![monitor_output()],
            ..Default::default()
        };
        assert!(needs_a_window(&configuration));
    }

    #[test]
    fn a_disabled_monitor_output_does_not_open_a_window() {
        let mut output = monitor_output();
        output.enabled = false;
        let configuration = MediaConfiguration {
            outputs: vec![output],
            ..Default::default()
        };
        assert!(!needs_a_window(&configuration));
    }

    #[test]
    fn what_the_renderer_saw_reaches_the_authoritative_state() {
        let id = media_domain::OutputId::new();
        let state = MediaState::with_outputs(vec![media_domain::OutputState::new(
            id,
            media_domain::LayerPersonality::TwoLayers,
        )]);
        let failure = media_domain::SourceStatus::Failed {
            failure: media_domain::SourceFailure::MissingFile,
        };

        let next = with_reports(&state, &[(id, 0, failure)], Timestamp::from_millis(0))
            .expect("a new status is a change");
        assert_eq!(next.output(id).unwrap().layers[0].source_status, failure);
        assert_eq!(
            next.output(id).unwrap().layers[1].source_status,
            media_domain::SourceStatus::Unselected,
            "one layer's failure is not another's"
        );

        assert!(
            with_reports(&next, &[(id, 0, failure)], Timestamp::from_millis(16)).is_none(),
            "reporting the same status again publishes nothing"
        );
        assert!(
            with_reports(
                &state,
                &[(media_domain::OutputId::new(), 0, failure)],
                Timestamp::from_millis(0)
            )
            .is_none(),
            "a report for an output that is not here changes nothing"
        );
    }
}
