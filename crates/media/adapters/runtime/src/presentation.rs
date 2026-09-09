//! The presentation host.
//!
//! Windowed outputs need the platform's event loop, and every supported platform requires that
//! loop on the main thread. So the process is arranged the other way round from a plain server:
//! the asynchronous services run on a background runtime, and the main thread belongs to the
//! outputs.
//!
//! A process whose outputs are all off-screen never builds an event loop at all.

mod display;
mod frame;
mod worker_control;

#[cfg(test)]
use display::DisplayRectangle;
use display::{DisplayDirection, map_pixels, monitor_rectangle, nearest_display};
use frame::{operator_overlay, present_direct, present_standby};

use std::sync::Arc;

use media_application::configuration::{MediaConfiguration, OutputConfiguration, OutputTarget};
use media_domain::geometry::Size;
use media_domain::{MasterState, MediaState, Timestamp};

use crate::dmx::SharedState;
use crate::layer_pipeline::LayerPipeline;
use media_playback::{AsyncClipLoader, ClipLoader, MediaLoader, PlaybackSession};
use media_render::{LayerDraw, SourceTexture, SurfaceLost, WindowedOutput, select_monitor};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Icon, Window, WindowId};

use crate::shutdown::{Shutdown, ShutdownReason};

const APPLICATION_ICON_PNG: &[u8] =
    include_bytes!("../../../../../assets/branding/ToskLight Pixel.png");

fn application_icon() -> Option<Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(APPLICATION_ICON_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    Icon::from_rgba(
        buffer[..info.buffer_size()].to_vec(),
        info.width,
        info.height,
    )
    .ok()
}

/// Whether this configuration asks for output windows.
///
/// It no longer decides whether the event loop runs — the menu bar item does, so the loop runs
/// whenever a desktop is reachable. It decides whether an empty output list means failure.
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
    importer: media_library::Importer,
) -> anyhow::Result<()> {
    let Shared {
        state,
        catalog,
        configuration: live,
        analysis,
        previews,
        universe_inputs,
    } = shared;
    let event_loop = EventLoop::new()?;
    // Cocoa owns this thread. Rendering and surface reconstruction happen on the presentation
    // worker, so this loop sleeps until a native event or the lightweight shutdown check.
    event_loop.set_control_flow(ControlFlow::Wait);

    #[cfg(all(feature = "tray", any(target_os = "macos", target_os = "windows")))]
    let bulk_import =
        crate::bulk_import::BulkImport::new(importer.clone(), administration_endpoint.clone());
    #[cfg(not(all(feature = "tray", any(target_os = "macos", target_os = "windows"))))]
    let _ = importer;
    let mut host = PresentationHost {
        configuration: live,
        catalog,
        analysis,
        previews,
        universe_inputs,
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
        data_directory: crate::startup::current_portable_data_directory(configuration),
        windows: Vec::new(),
        window_modes: std::collections::HashMap::new(),
        modifiers: ModifiersState::empty(),
        entering_fullscreen: Vec::new(),
        worker: None,
        expects_outputs: needs_a_window(configuration),
        #[cfg(feature = "tray")]
        tray: None,
        #[cfg(all(feature = "tray", any(target_os = "macos", target_os = "windows")))]
        bulk_import,
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
    pub universe_inputs: crate::dmx::SharedUniverseInputs,
}

/// The published library snapshot, shared with the services so both read one catalog.
pub type SharedCatalog = Arc<arc_swap::ArcSwap<media_domain::catalog::CatalogSnapshot>>;

/// The live configuration, shared with the API so both read one document.
///
/// What an output *is* — its monitor, its resolution, its presentation mode — is settled when the
/// surface opens, so changing those still needs a restart. What it *shows* is read every frame.
pub type SharedConfiguration = Arc<arc_swap::ArcSwap<MediaConfiguration>>;

struct HostedOutput {
    configuration: OutputConfiguration,
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
    fullscreen_hint: Option<SourceTexture>,
    hint_visible_until: Option<std::time::Instant>,
}

/// A clip loaded for the development `--play` affordance.
struct DirectClip {
    path: std::path::PathBuf,
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
    universe_inputs: crate::dmx::SharedUniverseInputs,
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
    data_directory: Option<std::path::PathBuf>,
    /// Main-thread references ensure the final native-window drop happens on the Cocoa thread.
    windows: Vec<Arc<Window>>,
    window_modes: std::collections::HashMap<WindowId, WindowMode>,
    modifiers: ModifiersState,
    /// Windows configured for full screen, waiting for their first turn through the event loop.
    ///
    /// Each is paired with the monitor it belongs on, because `toggleFullScreen:` takes the
    /// window's current screen rather than a screen of its own.
    entering_fullscreen: Vec<(Arc<Window>, winit::monitor::MonitorHandle)>,
    worker: Option<PresentationWorker>,
    /// Whether this configuration asked for output windows at all. A server with none is a normal
    /// state — it still runs, and it still has a menu bar item — so an empty output list only
    /// means failure when outputs were expected.
    expects_outputs: bool,
    /// The menu bar item, held for as long as the loop runs. Dropping it removes the icon.
    ///
    /// A build without the tray feature has no desktop to draw on, so it carries no field.
    #[cfg(feature = "tray")]
    tray: Option<crate::tray::Tray>,
    #[cfg(all(feature = "tray", any(target_os = "macos", target_os = "windows")))]
    bulk_import: crate::bulk_import::BulkImport,
}

enum RenderCommand {
    Resize { window: WindowId, size: Size },
    ShowFullscreenHint { window: WindowId },
    HideFullscreenHint { window: WindowId },
    Stop,
}

#[derive(Clone, Copy)]
struct WindowMode {
    fullscreen: bool,
    normal_size: Size,
}

struct PresentationWorker {
    commands: std::sync::mpsc::Sender<RenderCommand>,
    join: Option<std::thread::JoinHandle<()>>,
}

struct RenderWorkerState {
    configuration: SharedConfiguration,
    catalog: SharedCatalog,
    analysis: media_audio::SharedAnalysis,
    sinks: CaptureSinks,
    outputs: Vec<HostedOutput>,
    state: SharedState,
    started: std::time::Instant,
    test_pattern_layer: media_domain::LayerState,
    loader: AsyncClipLoader,
    direct: Option<DirectClip>,
    administration_endpoint: String,
    operator_overlay_layer: media_domain::LayerState,
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
            path,
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

        let Some(selected) = select_monitor(monitor, event_loop.available_monitors()) else {
            // Opening on a different display would be worse than saying so: an operator would
            // have no way to tell the output had moved.
            tracing::error!(
                output = %configuration.name,
                ?monitor,
                "the configured monitor is not connected; this output stays closed"
            );
            return;
        };

        let attributes = Window::default_attributes()
            .with_title(format!("ToskLight Pixel — {}", configuration.name))
            .with_window_icon(application_icon())
            .with_inner_size(winit::dpi::PhysicalSize::new(
                configuration.resolution.width,
                configuration.resolution.height,
            ))
            .with_position(selected.position())
            // Windows otherwise briefly exposes a caption and frame before Winit completes its
            // borderless transition. A configured full-screen output must never have either.
            .with_decorations(!(*fullscreen && cfg!(target_os = "windows")));

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot open the output window");
                return;
            }
        };

        if *fullscreen {
            // Full screen is asked for once the window is on screen, not in its attributes.
            // Asking at creation goes through a window that does not exist yet, and macOS
            // answers by handing back the plain window and leaving full screen behind.
            self.entering_fullscreen.push((window.clone(), selected));
        }

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
                let fullscreen_hint = crate::fullscreen_hint::render(output.size())
                    .and_then(|frame| {
                        SourceTexture::from_rgba8(output.gpu(), frame.size, &frame.pixels)
                            .map_err(anyhow::Error::from)
                    })
                    .map_err(|error| {
                        tracing::error!(%error, "cannot build the full-screen operator hint")
                    })
                    .ok();
                self.outputs.push(HostedOutput {
                    configuration: configuration.clone(),
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
                    fullscreen_hint,
                    hint_visible_until: None,
                });
                self.window_modes.insert(
                    window.id(),
                    WindowMode {
                        fullscreen: *fullscreen,
                        normal_size: Size::new(
                            configuration.resolution.width,
                            configuration.resolution.height,
                        ),
                    },
                );
                self.windows.push(window);
            }
            Err(error) => {
                tracing::error!(output = %configuration.name, %error, "cannot render to this output");
            }
        }
    }

    fn start_worker(&mut self) {
        let loaded = self.configuration.load();
        let cache_budget = loaded.playback.cache_budget_bytes;
        let instance = crate::pixel_output::instance_cid(loaded.instance_id.as_str());
        let mut loader = AsyncClipLoader::new(cache_budget);
        if let Some(direct) = &self.direct {
            loader.begin_selection(direct.asset);
        }
        // Diagnostic startup may have read a clip synchronously before presentation starts.
        // From here onward all source I/O belongs to the disk workers.
        self.loader = ClipLoader::new(0);
        let renderer = RenderWorkerState {
            configuration: self.configuration.clone(),
            catalog: self.catalog.clone(),
            analysis: self.analysis.clone(),
            sinks: CaptureSinks {
                previews: self.previews.clone(),
                last_preview_millis: std::mem::take(&mut self.last_preview_millis),
                pixels: crate::pixel_output::PixelOutputs::default(),
                universe_inputs: self.universe_inputs.clone(),
                instance,
            },
            outputs: std::mem::take(&mut self.outputs),
            state: self.state.clone(),
            started: self.started,
            test_pattern_layer: self.test_pattern_layer.clone(),
            loader,
            direct: self.direct.take(),
            administration_endpoint: self.administration_endpoint.clone(),
            operator_overlay_layer: media_domain::LayerState {
                address: media_domain::MediaAddress::new(1, 1),
                source_status: media_domain::SourceStatus::Ready,
                scaling_mode: media_domain::ScalingMode::Stretch,
                ..Default::default()
            },
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

    fn show_fullscreen_hint(&self, window: WindowId) {
        if !cfg!(target_os = "windows") {
            return;
        }
        if !self
            .window_modes
            .get(&window)
            .is_some_and(|mode| mode.fullscreen)
        {
            return;
        }
        if let Some(worker) = &self.worker {
            let _ = worker
                .commands
                .send(RenderCommand::ShowFullscreenHint { window });
        }
    }

    fn handle_fullscreen_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        key: KeyCode,
    ) {
        if !windows_command_chord(self.modifiers) {
            return;
        }
        if key == KeyCode::Minus {
            self.restore_window(window_id);
            return;
        }
        let direction = match key {
            KeyCode::ArrowLeft => DisplayDirection::Left,
            KeyCode::ArrowRight => DisplayDirection::Right,
            KeyCode::ArrowUp => DisplayDirection::Up,
            KeyCode::ArrowDown => DisplayDirection::Down,
            _ => return,
        };
        self.move_fullscreen_window(event_loop, window_id, direction);
    }

    fn restore_window(&mut self, window_id: WindowId) {
        let Some(mode) = self.window_modes.get_mut(&window_id) else {
            return;
        };
        if !mode.fullscreen {
            return;
        }
        let Some(window) = self.windows.iter().find(|window| window.id() == window_id) else {
            return;
        };
        window.set_fullscreen(None);
        window.set_decorations(true);
        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
            mode.normal_size.width,
            mode.normal_size.height,
        ));
        mode.fullscreen = false;
        if let Some(worker) = &self.worker {
            let _ = worker
                .commands
                .send(RenderCommand::HideFullscreenHint { window: window_id });
        }
    }

    fn move_fullscreen_window(
        &self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        direction: DisplayDirection,
    ) {
        if !self
            .window_modes
            .get(&window_id)
            .is_some_and(|mode| mode.fullscreen)
        {
            return;
        }
        let Some(window) = self.windows.iter().find(|window| window.id() == window_id) else {
            return;
        };
        let Some(current) = window.current_monitor() else {
            return;
        };
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        let rectangles: Vec<_> = monitors.iter().map(monitor_rectangle).collect();
        let current = monitor_rectangle(&current);
        let Some(index) = nearest_display(current, &rectangles, direction) else {
            return;
        };
        window.set_decorations(false);
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(
            monitors[index].clone(),
        ))));
    }
}

fn windows_command_chord(modifiers: ModifiersState) -> bool {
    modifiers.control_key()
        && modifiers.shift_key()
        && !modifiers.alt_key()
        && !modifiers.super_key()
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
        // Every output in this pass composites from the same configuration snapshot.
        let configuration = self.configuration.load();
        // Without an input device, time-driven visualizers run while audio-driven ones rest.
        let heard = self.analysis.load();
        let mut reports = Vec::new();

        for hosted in &mut self.outputs {
            if !hosted.output.should_present(now) {
                continue;
            }
            let Some(output_state) = state.output(hosted.output.id()) else {
                continue;
            };
            let resolved_output = crate::effect_banks::resolve_output(output_state, &configuration);
            let output_state = &resolved_output;
            let master = output_state.master;
            let region = shown_region(&configuration, output_state.id);
            let status_overlay = configuration
                .output(output_state.id)
                .is_some_and(|output| output.status_overlay);
            if present_standby(
                &mut self.sinks,
                &self.test_pattern_layer,
                &self.operator_overlay_layer,
                hosted,
                output_state,
                status_overlay,
                now,
                region,
            ) {
                continue;
            }

            if present_direct(
                &mut self.loader,
                self.direct.as_mut(),
                &mut self.sinks,
                &self.operator_overlay_layer,
                hosted,
                output_state,
                &master,
                now,
                region,
            ) {
                continue;
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
            let turn = &mut hosted.beat_scale_turn;
            let effective_layers = turn.apply(&effective_layers, seconds, heard.beat);
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
            let overlay = operator_overlay(
                hosted.hint_visible_until,
                hosted.fullscreen_hint.as_ref(),
                &self.operator_overlay_layer,
            );
            present(
                &mut hosted.output,
                &draws,
                &master,
                master_mask,
                now,
                region,
                overlay,
            );
            capture_previews(
                &mut self.sinks,
                &hosted.configuration,
                &mut hosted.output,
                output_state,
                &effective_layers,
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
        self.state.rcu(|current| {
            with_reports(current, &reports, now)
                .map(Arc::new)
                .unwrap_or_else(|| Arc::clone(current))
        });
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
/// The slice of the canvas this output's screen shows.
///
/// The first enabled region, because one window shows one slice; a second region on the same output
/// describes a second screen, which is a window of its own rather than a second view in this one.
fn shown_region(
    configuration: &MediaConfiguration,
    id: media_domain::OutputId,
) -> Option<&media_domain::display_region::DisplayRegion> {
    configuration
        .outputs
        .iter()
        .find(|candidate| candidate.id == id)?
        .pixel_map
        .regions
        .iter()
        .find(|region| region.enabled)
}

fn present(
    output: &mut WindowedOutput,
    draws: &[LayerDraw<'_>],
    master: &MasterState,
    master_mask: Option<&SourceTexture>,
    now: Timestamp,
    region: Option<&media_domain::display_region::DisplayRegion>,
    overlay: Option<LayerDraw<'_>>,
) {
    let result = if let Some(overlay) = overlay {
        output.present_with_overlay(draws, master, master_mask, now, region, overlay)
    } else {
        output.present(draws, master, master_mask, now, region)
    };
    match result {
        Ok(()) | Err(SurfaceLost::Recovered | SurfaceLost::Timeout) => {}
        Err(error) => {
            tracing::error!(id = %output.id(), %error, "output stopped presenting");
        }
    }
}

/// Everywhere a finished frame goes that is not the screen it was drawn for.
struct CaptureSinks {
    previews: crate::preview::SharedPreviews,
    last_preview_millis: std::collections::BTreeMap<media_domain::OutputId, u64>,
    pixels: crate::pixel_output::PixelOutputs,
    universe_inputs: crate::dmx::SharedUniverseInputs,
    instance: [u8; 16],
}

#[allow(clippy::too_many_arguments)]
fn capture_previews(
    sinks: &mut CaptureSinks,
    configuration: &OutputConfiguration,
    output: &mut WindowedOutput,
    state: &media_domain::OutputState,
    preview_layers: &[media_domain::LayerState],
    draws: &[LayerDraw<'_>],
    master: &MasterState,
    master_mask: Option<&SourceTexture>,
    now: Timestamp,
) {
    let output_id = state.id;
    // Pixel mapping is output rather than a preview: it happens whether or not anyone is watching
    // a thumbnail, and on its own cadence.
    map_pixels(
        &mut sinks.pixels,
        configuration,
        output,
        master,
        master_mask,
        now,
        sinks.instance,
        &sinks.universe_inputs,
    );
    let program = sinks.previews.for_output(output_id);
    let wanted = program.is_some_and(|preview| preview.wanted())
        || state.layers.iter().enumerate().any(|(layer, _)| {
            sinks
                .previews
                .for_layer(output_id, layer)
                .is_some_and(|preview| preview.wanted())
        });
    if !wanted {
        if sinks.last_preview_millis.remove(&output_id).is_some() {
            output.release_preview();
        }
        return;
    }
    if !crate::preview::due(
        sinks.last_preview_millis.get(&output_id).copied(),
        now.as_millis(),
    ) {
        return;
    }
    sinks.last_preview_millis.insert(output_id, now.as_millis());
    if let Some(preview) = program.filter(|preview| preview.wanted()) {
        let size = preview.requested_size();
        let captured = output.capture_preview(size, master, master_mask);
        preview.publish_pixels(&captured, size, size, false);
    }
    for (layer_index, _) in state.layers.iter().enumerate() {
        let Some(preview) = sinks
            .previews
            .for_layer(output_id, layer_index)
            .filter(|preview| preview.wanted())
        else {
            continue;
        };
        let size = preview.requested_size();
        let effective_layer = preview_layers.get(layer_index);
        if let Some(draw) = effective_layer.and_then(|effective_layer| {
            draws
                .iter()
                .find(|draw| std::ptr::eq(draw.state, effective_layer))
                .copied()
        }) {
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
        // The status item cannot be created before the application has finished launching, which
        // is exactly what reaching this callback means.
        #[cfg(feature = "tray")]
        if self.tray.is_none() {
            self.tray = crate::tray::show(
                &self.shutdown,
                self.data_directory.as_deref(),
                #[cfg(target_os = "windows")]
                &self.administration_endpoint,
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                self.bulk_import.clone(),
            );
        }
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
            if self.expects_outputs {
                tracing::error!("no output could be opened; stopping");
                event_loop.exit();
            }
            // Nothing to present. The server keeps serving, and the menu bar item keeps saying so.
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
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. }
                if cfg!(target_os = "windows")
                    && event.state == ElementState::Pressed
                    && !event.repeat =>
            {
                if let PhysicalKey::Code(key) = event.physical_key {
                    self.handle_fullscreen_key(event_loop, id, key);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.show_fullscreen_hint(id),
            WindowEvent::RedrawRequested => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // The window exists and has been through one pass of the loop, so the platform can now
        // take it into its own full screen — the same transition the maximize button performs.
        for (window, monitor) in self.entering_fullscreen.drain(..) {
            #[cfg(target_os = "windows")]
            window.set_decorations(false);
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(monitor))));
        }
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
#[path = "presentation_tests.rs"]
mod tests;
