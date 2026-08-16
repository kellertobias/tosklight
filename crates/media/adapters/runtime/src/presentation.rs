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
    shared: Shared,
    shutdown: Shutdown,
    diagnostics: Diagnostics,
    available_monitors: Arc<std::sync::RwLock<Vec<media_http::MonitorDevice>>>,
    // The same reference point the network listeners stamp against, so a packet's arrival and a
    // frame's presentation sit on one timeline.
    started: std::time::Instant,
    administration_endpoint: String,
) -> anyhow::Result<()> {
    let Shared {
        state,
        catalog,
        configuration: live,
        analysis,
        previews,
    } = shared;
    let event_loop = EventLoop::new()?;
    // Cocoa owns this thread. Rendering and surface reconstruction happen on the presentation
    // worker, so this loop sleeps until a native event or the lightweight shutdown check.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut host = PresentationHost {
        configuration: live,
        catalog,
        analysis,
        previews,
        last_preview_millis: std::collections::BTreeMap::new(),
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
        available_monitors,
        started,
        test_pattern_layer: test_pattern_layer(),
        loader: ClipLoader::new(configuration.playback.cache_budget_bytes),
        direct: None,
        clip_size: Size::new(2, 2),
        administration_endpoint,
        windows: Vec::new(),
        worker: None,
    };
    let result = event_loop.run_app(&mut host);
    host.stop_worker();
    result.map_err(Into::into)
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

/// What the outputs share with the services.
///
/// One value rather than four arguments, because they always travel together: the outputs present
/// exactly the state the API writes, from exactly the catalog it publishes.
pub struct Shared {
    pub state: SharedState,
    pub catalog: SharedCatalog,
    /// The live configuration, shared with the API. An accepted edit to a text source or a
    /// visualizer is on the next frame rather than on the next start: an operator typing the words
    /// a countdown will show has to see them.
    pub configuration: SharedConfiguration,
    pub analysis: media_audio::SharedAnalysis,
    pub previews: crate::preview::SharedPreviews,
}

/// The published library snapshot, shared with the services so both read one catalog.
pub type SharedCatalog = Arc<arc_swap::ArcSwap<media_domain::catalog::CatalogSnapshot>>;

/// The live configuration, shared with the API so both read one document.
///
/// What an output *is* — its monitor, its resolution, its presentation mode — is settled when the
/// surface opens, so changing those still needs a restart. What it *shows* is read every frame.
pub type SharedConfiguration = Arc<arc_swap::ArcSwap<MediaConfiguration>>;

struct HostedOutput {
    output: WindowedOutput,
    /// Kept alive for the surface's lifetime, and used to resolve resize events back to an output.
    window: Arc<Window>,
    test_pattern: Option<SourceTexture>,
    sources: crate::layer_sources::LayerSources,
    /// This output's path from addresses to textures.
    pipeline: LayerPipeline,
    opacity_cycle: crate::opacity_cycle::OpacityCycle,
    beat_move: crate::beat_move::BeatMove,
    beat_scale_turn: crate::beat_scale_turn::BeatScaleTurn,
    beat_scan: crate::beat_scan::BeatScan,
    beat_grid_wave: crate::beat_grid_wave::BeatGridWave,
    beat_form_flash: crate::beat_form_flash::BeatFormFlash,
    standby: Option<SourceTexture>,
}

/// A clip loaded for the development `--play` affordance.
struct DirectClip {
    asset: media_domain::AssetId,
    size: Size,
    session: PlaybackSession,
    layer: media_domain::LayerState,
}

struct PresentationHost {
    configuration: SharedConfiguration,
    catalog: SharedCatalog,
    /// The newest audio analysis, which generated sources react to.
    analysis: media_audio::SharedAnalysis,
    /// The output preview a subscribed console receives.
    previews: crate::preview::SharedPreviews,
    last_preview_millis: std::collections::BTreeMap<media_domain::OutputId, u64>,
    outputs: Vec<HostedOutput>,
    pending: Vec<OutputConfiguration>,
    state: SharedState,
    shutdown: Shutdown,
    diagnostics: Diagnostics,
    available_monitors: Arc<std::sync::RwLock<Vec<media_http::MonitorDevice>>>,
    started: std::time::Instant,
    test_pattern_layer: media_domain::LayerState,
    loader: ClipLoader,
    direct: Option<DirectClip>,
    clip_size: Size,
    administration_endpoint: String,
    /// Main-thread references ensure the final native-window drop happens on the Cocoa thread.
    windows: Vec<Arc<Window>>,
    worker: Option<PresentationWorker>,
}

enum RenderCommand {
    Resize { window: WindowId, size: Size },
    Stop,
}

struct PresentationWorker {
    commands: std::sync::mpsc::Sender<RenderCommand>,
    join: Option<std::thread::JoinHandle<()>>,
}

struct RenderWorkerState {
    configuration: SharedConfiguration,
    catalog: SharedCatalog,
    analysis: media_audio::SharedAnalysis,
    previews: crate::preview::SharedPreviews,
    last_preview_millis: std::collections::BTreeMap<media_domain::OutputId, u64>,
    outputs: Vec<HostedOutput>,
    state: SharedState,
    started: std::time::Instant,
    test_pattern_layer: media_domain::LayerState,
    loader: ClipLoader,
    direct: Option<DirectClip>,
    administration_endpoint: String,
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
            size: self.clip_size,
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
                    media_library::LibraryStorage::new(
                        self.configuration.load().library.root.clone(),
                    ),
                    output.size(),
                );
                pipeline.validate_visualizers();
                let standby = crate::standby::render(output.size(), &self.administration_endpoint)
                    .and_then(|frame| {
                        SourceTexture::from_rgba8(output.gpu(), frame.size, &frame.pixels)
                            .map_err(anyhow::Error::from)
                    })
                    .map_err(
                        |error| tracing::error!(%error, "cannot build the Media standby surface"),
                    )
                    .ok();
                self.outputs.push(HostedOutput {
                    output,
                    window: window.clone(),
                    test_pattern,
                    sources,
                    pipeline,
                    opacity_cycle: crate::opacity_cycle::OpacityCycle::default(),
                    beat_move: crate::beat_move::BeatMove::default(),
                    beat_scale_turn: crate::beat_scale_turn::BeatScaleTurn::default(),
                    beat_scan: crate::beat_scan::BeatScan::default(),
                    beat_grid_wave: crate::beat_grid_wave::BeatGridWave::default(),
                    beat_form_flash: crate::beat_form_flash::BeatFormFlash::default(),
                    standby,
                });
                self.windows.push(window);
            }
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot render to this output");
            }
        }
    }

    fn start_worker(&mut self) {
        let cache_budget = self.configuration.load().playback.cache_budget_bytes;
        let renderer = RenderWorkerState {
            configuration: self.configuration.clone(),
            catalog: self.catalog.clone(),
            analysis: self.analysis.clone(),
            previews: self.previews.clone(),
            last_preview_millis: std::mem::take(&mut self.last_preview_millis),
            outputs: std::mem::take(&mut self.outputs),
            state: self.state.clone(),
            started: self.started,
            test_pattern_layer: self.test_pattern_layer.clone(),
            loader: std::mem::replace(&mut self.loader, ClipLoader::new(cache_budget)),
            direct: self.direct.take(),
            administration_endpoint: self.administration_endpoint.clone(),
        };
        let shutdown = self.shutdown.clone();
        let (commands, receiver) = std::sync::mpsc::channel();
        match std::thread::Builder::new()
            .name("media-presentation".to_owned())
            .spawn(move || renderer.run(receiver, shutdown))
        {
            Ok(join) => {
                self.worker = Some(PresentationWorker {
                    commands,
                    join: Some(join),
                });
            }
            Err(error) => {
                tracing::error!(%error, "cannot start the Media presentation worker");
                self.shutdown.request(ShutdownReason::Requested);
            }
        }
    }

    fn stop_worker(&mut self) {
        self.shutdown.request(ShutdownReason::Requested);
        let Some(mut worker) = self.worker.take() else {
            return;
        };
        let _ = worker.commands.send(RenderCommand::Stop);
        if worker.join.take().is_some_and(|join| join.join().is_err()) {
            tracing::error!("the Media presentation worker panicked while stopping");
        }
    }
}

impl RenderWorkerState {
    fn now(&self) -> Timestamp {
        Timestamp::from_micros(self.started.elapsed().as_micros() as u64)
    }

    fn present_all(&mut self) {
        let now = self.now();
        let seconds = self.started.elapsed().as_secs_f32();
        let state = self.state.load();
        let catalog = self.catalog.load();
        // Read once per pass rather than once per output, so every output on this frame composites
        // from the same document even if an edit lands between two of them.
        let configuration = self.configuration.load();
        // Silence when no input device is open, which is a real analysis rather than a
        // placeholder: time-driven visualizers run and audio-driven ones rest.
        let heard = self.analysis.load();
        let mut reports = Vec::new();

        for hosted in &mut self.outputs {
            if !hosted.output.should_present(now) {
                continue;
            }
            let Some(output_state) = state.output(hosted.output.id()) else {
                continue;
            };
            let master = output_state.master;

            let status_overlay = configuration
                .output(output_state.id)
                .is_some_and(|output| output.status_overlay);
            if crate::standby::visible(
                status_overlay,
                output_state.ownership.dmx.is_some(),
                output_state.ownership.web_takeover,
            ) && let Some(standby) = hosted.standby.as_ref()
            {
                let draws = [LayerDraw {
                    state: &self.test_pattern_layer,
                    source: standby,
                    mask: None,
                }];
                present(
                    &mut hosted.output,
                    &draws,
                    &MasterState::default(),
                    None,
                    now,
                );
                capture_previews(
                    &mut self.last_preview_millis,
                    &self.previews,
                    &mut hosted.output,
                    output_state,
                    &draws,
                    &MasterState::default(),
                    None,
                    now,
                );
                continue;
            }

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
                        .prepare(0, direct.asset, frame, direct.size, self.loader.cache_mut())
                        .unwrap_or(false)
                    && let Some(texture) = hosted.sources.texture(0)
                {
                    let draws = [LayerDraw {
                        state: &direct.layer,
                        source: texture,
                        mask: None,
                    }];
                    present(&mut hosted.output, &draws, &master, None, now);
                    capture_previews(
                        &mut self.last_preview_millis,
                        &self.previews,
                        &mut hosted.output,
                        output_state,
                        &draws,
                        &master,
                        None,
                        now,
                    );
                    continue;
                }
            }

            // The real path: every layer's address becomes a texture, or reports why it did not.
            let prepared = hosted.pipeline.prepare(
                output_state,
                crate::layer_pipeline::FrameContext {
                    catalog: &catalog,
                    configuration: &configuration,
                    analysis: &heard.analysis,
                    now_unix_millis: unix_millis(),
                    beat: heard.beat,
                    bpm: heard.bpm,
                    beat_phase: heard.beat_phase,
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

            let effective_layers = hosted.opacity_cycle.apply(
                output_state,
                &prepared,
                seconds,
                heard.bpm,
                heard.beat_phase,
            );
            let effective_layers = hosted
                .beat_move
                .apply(&effective_layers, seconds, heard.beat);
            let effective_layers =
                hosted
                    .beat_scale_turn
                    .apply(&effective_layers, seconds, heard.beat);
            let effective_layers = hosted.beat_scan.apply(
                &effective_layers,
                seconds,
                heard.beat,
                heard.analysis.peak.max(heard.analysis.energy * 4.0),
            );
            let effective_layers = hosted.beat_grid_wave.apply(
                &effective_layers,
                seconds,
                heard.beat,
                heard.analysis.peak.max(heard.analysis.energy * 4.0),
            );
            let effective_layers =
                hosted
                    .beat_form_flash
                    .apply(&effective_layers, seconds, heard.beat);
            let mut draws = hosted
                .pipeline
                .draws_from_layers(&effective_layers, &prepared);
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

            capture_previews(
                &mut self.last_preview_millis,
                &self.previews,
                &mut hosted.output,
                output_state,
                &draws,
                &master,
                master_mask,
                now,
            );
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

    fn run(mut self, receiver: std::sync::mpsc::Receiver<RenderCommand>, shutdown: Shutdown) {
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
    }
}

/// Wall-clock time, which only a clock and a target countdown consult.
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

fn presentation_worker_wait(
    waits: impl IntoIterator<Item = std::time::Duration>,
) -> Option<std::time::Duration> {
    let mut shortest = None;
    for wait in waits {
        if wait.is_zero() {
            return None;
        }
        shortest = Some(shortest.map_or(wait, |current: std::time::Duration| current.min(wait)));
    }
    shortest
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

fn capture_previews(
    last_preview_millis: &mut std::collections::BTreeMap<media_domain::OutputId, u64>,
    previews: &crate::preview::SharedPreviews,
    output: &mut WindowedOutput,
    state: &media_domain::OutputState,
    draws: &[LayerDraw<'_>],
    master: &MasterState,
    master_mask: Option<&SourceTexture>,
    now: Timestamp,
) {
    let output_id = state.id;
    let program = previews.for_output(output_id);
    let wanted = program.is_some_and(|preview| preview.wanted())
        || state.layers.iter().enumerate().any(|(layer, _)| {
            previews
                .for_layer(output_id, layer)
                .is_some_and(|preview| preview.wanted())
        });
    if !wanted {
        if last_preview_millis.remove(&output_id).is_some() {
            output.release_preview();
        }
        return;
    }
    if !crate::preview::due(
        last_preview_millis.get(&output_id).copied(),
        now.as_millis(),
    ) {
        return;
    }
    last_preview_millis.insert(output_id, now.as_millis());
    if let Some(preview) = program.filter(|preview| preview.wanted()) {
        let size = preview.requested_size();
        let captured = output.capture_preview(size, master, master_mask);
        preview.publish_pixels(&captured, size, size, false);
    }
    for (layer_index, layer_state) in state.layers.iter().enumerate() {
        let Some(preview) = previews
            .for_layer(output_id, layer_index)
            .filter(|preview| preview.wanted())
        else {
            continue;
        };
        let size = preview.requested_size();
        if let Some(draw) = draws
            .iter()
            .find(|draw| std::ptr::eq(draw.state, layer_state))
            .copied()
        {
            let captured = output.capture_layer_preview(size, draw, output_id, now);
            preview.publish_pixels(&captured, size, size, true);
        } else {
            preview.publish_pixels(
                &vec![0; size.width as usize * size.height as usize * 4],
                size,
                size,
                true,
            );
        }
    }
}

/// Applies the renderer's source reports to the authoritative state.
///
/// Returns the next state when anything changed, so a frame in which nothing loaded or failed
/// publishes nothing at all rather than churning a snapshot every sixtieth of a second.
pub(crate) fn with_reports(
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
        let monitors = media_render::monitors(event_loop.available_monitors())
            .into_iter()
            .map(|(index, name, handle)| {
                let size = handle.size();
                media_http::MonitorDevice {
                    index,
                    name,
                    width: size.width,
                    height: size.height,
                    refresh_millihertz: handle.refresh_rate_millihertz(),
                }
            })
            .collect();
        if let Ok(mut available) = self.available_monitors.write() {
            *available = monitors;
        }
        if self.worker.is_some() {
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
            return;
        }
        self.start_worker();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self.windows.iter().any(|window| window.id() == id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.shutdown.request(ShutdownReason::Requested);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let size = Size::new(size.width.max(1), size.height.max(1));
                // Sending is deliberately the only work on Cocoa's thread. The worker coalesces
                // a resize gesture to the newest size before rebuilding the GPU and standby data.
                if let Some(worker) = &self.worker {
                    let _ = worker
                        .commands
                        .send(RenderCommand::Resize { window: id, size });
                }
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
        // Shutdown can originate on a service thread. This tiny timed wake observes it without
        // putting any rendering or resize work back onto the native event loop.
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
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
    fn presentation_worker_waits_for_the_earliest_fixed_deadline() {
        assert_eq!(
            presentation_worker_wait([
                std::time::Duration::from_millis(33),
                std::time::Duration::from_millis(16),
            ]),
            Some(std::time::Duration::from_millis(16))
        );
        assert_eq!(presentation_worker_wait([std::time::Duration::ZERO]), None);
        assert_eq!(presentation_worker_wait([]), None);
    }

    #[test]
    fn an_unlocked_output_keeps_a_mixed_worker_running_immediately() {
        assert_eq!(
            presentation_worker_wait([
                std::time::Duration::from_millis(16),
                std::time::Duration::ZERO,
                std::time::Duration::from_millis(33),
            ]),
            None
        );
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
