//! Window, input, and the frame loop.

use crate::pacing::{DEFAULT_FRAME_INTERVAL, FRAME_PACE, HIDDEN_FRAME_INTERVAL, paced_interval};
use crate::session::Session;
use crate::settings::{Options, Preferences, Startup};
use crate::ui::{self, QuickSettings, QuickSettingsOutcome};
use std::sync::Arc;
use std::time::{Duration, Instant};
use viz_desk::{DeskConnection, DeskProvider};
use viz_render::{CameraControl, FrameStats, Overlay, Renderer};
use viz_scene::{Camera, ConnectionState, ProviderKind, ViewMode};
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// How far a right-button drag turns the camera, in radians per unit of hand movement.
///
/// A drag across the width of a typical window turns roughly eighty degrees. That leaves enough
/// travel for precise framing without making an ordinary hand movement cross the whole scene.
/// It is deliberately not scaled by the display: the same hand movement has to turn the camera
/// the same amount on any screen.
const LOOK_RADIANS_PER_UNIT: f32 = 0.001;

/// A camera pan follows half of the pointer's physical travel. Full-speed tracking made small
/// trackpad and high-resolution mouse deltas move the rig too far to frame it precisely.
const PAN_PIXELS_PER_HAND_POINT: f32 = 0.5;

fn hand_points(physical_delta: f64, scale: f32) -> f32 {
    physical_delta as f32 / scale.max(f32::EPSILON)
}

fn pan_pixels(hand_delta: f32, scale: f32) -> f32 {
    hand_delta * scale * PAN_PIXELS_PER_HAND_POINT
}

/// The patched virtual camera has exactly one target: this application's own perspective
/// presentation. A helper embedded in the desk and every orthographic view remain local.
fn is_external_camera_target(embed: bool, mode: ViewMode) -> bool {
    !embed && !mode.is_orthographic()
}

struct WindowSurface(Arc<Window>);

impl viz_render::PresentationSurface for WindowSurface {
    fn create_surface(
        &self,
        instance: &viz_render::wgpu::Instance,
    ) -> Result<viz_render::wgpu::Surface<'static>, String> {
        instance
            .create_surface(self.0.clone())
            .map_err(|error| format!("surface: {error}"))
    }

    fn size(&self) -> (u32, u32) {
        let size = self.0.inner_size();
        (size.width.max(1), size.height.max(1))
    }
}

mod benchmark;
mod draw;
mod input;

use benchmark::BenchmarkSample;
use draw::gpu_label;

pub struct Application {
    options: Options,
    preferences: Preferences,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    session: Option<Session>,
    quick_settings: QuickSettings,
    /// Captures the operator has taken, and their Blender exports.
    snapshots: crate::snapshots::Snapshots,
    /// Whether a `--snapshot` run has already asked for its one capture.
    snapshot_requested: bool,
    camera: CameraControl,
    modifiers: ModifiersState,
    /// Which mouse buttons are currently held. The right button turns the camera on the spot —
    /// pan and tilt — and moves the view across the floor where there is no heading to turn; the
    /// middle button moves it on the camera plane. The left button is not a camera control.
    turning: bool,
    panning_camera_plane: bool,
    /// Whether the current drag has been fed by device motion. Device motion is the better source
    /// — it keeps reporting once the pointer has run into the edge of a display — so the cursor
    /// only drives the drag while no device motion has arrived for it, and never doubles it up.
    drag_moved_by_device: bool,
    /// Movement keys currently held, applied once per frame so speed follows frame time.
    walk_keys: WalkKeys,
    cursor: (f64, f64),
    epoch: Instant,
    /// When persistence of vision was last advanced, so the decay follows real time however long
    /// a frame took.
    last_persistence: Instant,
    /// Every laser's scan engine, and the override directory it watches.
    lasers: crate::lasers::Lasers,
    effects: crate::effects::Effects,
    physics: crate::physics::Physics,
    /// Absent for helper/embed launches, which must open no media sockets or decoders.
    media_workers: Option<crate::media_worker::MediaWorkers>,
    /// Advances only when a new decoded source frame reaches the GPU. This participates in the
    /// demand-redraw identity so live video cannot be suppressed as an otherwise unchanged scene.
    media_revision: u64,
    media_notice: Option<String>,
    last_frame: Instant,
    /// When the next frame is due. Frames are paced instead of drawn back to back so the event
    /// loop spends its time in the window system, where input is delivered, rather than parked
    /// inside the driver waiting for a drawable.
    next_frame: Instant,
    frame_interval: Duration,
    /// False while the window is hidden behind another one and nothing needs presenting.
    visible: bool,
    frame_intervals: Vec<f32>,
    frames_per_second: f32,
    stats: FrameStats,
    overlay: Overlay,
    redraw_gate: crate::redraw::RedrawGate,
    camera_is_local: bool,
    /// Ownership of the dedicated external 3D presentation camera. Embedded Stage helpers never
    /// consult this state: their camera remains the desk pane's local view.
    external_camera: ExternalCameraOwnership,
    /// The private server started for a show file, when the operator opened one.
    hosted_show: Option<crate::showfile::HostedShow>,
    /// The desk's channel, when this process was started as its helper. Held until the provider
    /// is built, because the handshake happens before the window exists.
    helper_source: Option<crate::helper_source::HelperSource>,
    /// The planning window opened when the visualizer was started with nothing to look at.
    planning_window: Option<crate::planner::PlanningWindow>,
    menu: Option<crate::menu::ApplicationMenu>,
    /// Scene revision the camera was last framed for, so a newly loaded scene is framed once.
    framed_revision: Option<u64>,
    /// How many times the source's view had been stated when it was last taken. A desk that says
    /// something new is obeyed; a desk that says nothing leaves the operator's selection alone.
    adopted_source_view: u64,
    /// A view named on the command line, held until the source's own view has been seen once.
    /// This launch asked for that view; the view the desk happened to have stored is not an
    /// instruction, and a `--capture` of a named view must produce that view.
    requested_view: Option<ViewMode>,
    /// Regions of the status surface the operator can act on, rebuilt every frame.
    hotspots: Vec<ui::HotspotRect>,
    /// The fixture the operator clicked on, held by identity so a repatch cannot turn it into a
    /// different one.
    selected: Option<uuid::Uuid>,
    startup_error: Option<String>,
    /// A reason the operator has to do something about — a planning window that would not open, a
    /// private server that has exited. Unlike a connection state, which the next retry rewrites,
    /// it stays on the surface until it is dealt with.
    lasting_failure: Option<String>,
    /// Where this window's preferences are kept, and what was last written there, so a change the
    /// operator made is still there next time they open the visualizer.
    preferences_path: Option<std::path::PathBuf>,
    stored_preferences: String,
    next_preferences_save: Instant,
    /// When the private show server and the planning window were last checked for having exited.
    next_child_check: Instant,
    presented_frames: u64,
    /// Per-view samples collected during a `--benchmark` run.
    benchmark: Vec<BenchmarkSample>,
    benchmark_view_index: usize,
    benchmark_view_started: Instant,
}

/// Held movement keys.
#[derive(Clone, Copy, Debug, Default)]
struct WalkKeys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

/// Camera ownership for the one dedicated external 3D presentation.
///
/// `latest` is retained when the fixture is unpatched or its input becomes stale. That retention
/// is intentional: silence is not a camera reset. A local gesture latches `local_override` until
/// the operator explicitly releases it while current DMX data is available.
#[derive(Default)]
struct ExternalCameraOwnership {
    latest: Option<Camera>,
    live: bool,
    local_override: bool,
}

impl ExternalCameraOwnership {
    fn observe(&mut self, camera: Option<(Camera, bool)>) -> Option<Camera> {
        match camera {
            Some((camera, false)) => {
                self.latest = Some(camera);
                self.live = true;
            }
            Some((camera, true)) => {
                // A stale snapshot still carries the last authoritative pose. Remember it, but do
                // not call it live or offer a release-to-DMX action until input resumes.
                self.latest = Some(camera);
                self.live = false;
            }
            None => self.live = false,
        }
        (!self.local_override).then_some(self.latest).flatten()
    }

    fn latch_local(&mut self) {
        if self.latest.is_some() {
            self.local_override = true;
        }
    }

    fn release_to_dmx(&mut self) -> Option<Camera> {
        if !self.live {
            return None;
        }
        self.local_override = false;
        self.latest
    }

    fn has_pose(&self) -> bool {
        self.latest.is_some()
    }

    fn local_override(&self) -> bool {
        self.local_override
    }

    fn status(&self) -> crate::ui::DmxCameraControlStatus {
        if self.local_override {
            crate::ui::DmxCameraControlStatus::Local {
                can_release: self.live,
            }
        } else if self.latest.is_some() && !self.live {
            crate::ui::DmxCameraControlStatus::Held
        } else if self.live {
            crate::ui::DmxCameraControlStatus::Dmx
        } else {
            crate::ui::DmxCameraControlStatus::None
        }
    }
}

/// The session facts one benchmark frame needs, read before the session borrow ends.
struct Measured {
    latency: (f32, f32, f32),
    dmx_hz: f32,
    fixtures: usize,
    emitters: usize,
    show: String,
    inputs: Vec<viz_scene::InputMappingStatus>,
    scene_ready: bool,
}

fn canonical_demo_show_path() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("TOSKLIGHT_VIZ_DEMO_SHOW")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
    {
        return path
            .is_file()
            .then_some(path.clone())
            .ok_or_else(|| format!("{} is not a file", path.display()));
    }
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/demo.show");
    if checkout.is_file() {
        return Ok(checkout);
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("resolve visualizer executable: {error}"))?;
    let parent = executable
        .parent()
        .ok_or_else(|| "visualizer executable has no parent directory".to_owned())?;
    for candidate in [
        parent.join("demo-show/demo-show.show"),
        parent.join("demo-show/demo.show"),
        parent.join("../Resources/demo-show/demo-show.show"),
        parent.join("../Resources/demo-show/demo.show"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("this build has no assets/demo.show or packaged demo-show/demo-show.show".to_owned())
}

impl Application {
    pub fn new(options: Options) -> Self {
        let laser_scripts = crate::lasers::Lasers::directory(options.laser_scripts.clone());
        let preferences = Preferences::restored(&options);
        let options_for_paths = options.clone();
        let preferences_path = crate::settings::preferences_path(&options_for_paths);
        let physics_state_path = crate::physics::Physics::state_path(
            preferences_path.as_deref(),
            &options_for_paths.target,
        );
        let media_enabled = !options.helper && !options.embed;
        // The planning source is the Viz editor, so it is selectable exactly when that editor can
        // be started. When it cannot, the control says why in the words the failed launch would
        // have used rather than pretending the source does not exist.
        let planner = crate::planner::availability();
        let mut quick_settings = QuickSettings::new(&preferences, options.demo || planner.is_ok());
        let stored_preferences = preferences.to_file();
        if let Err(reason) = planner {
            quick_settings.planner_unavailable_reason = reason;
        }
        Self {
            options,
            preferences,
            window: None,
            renderer: None,
            session: None,
            quick_settings,
            snapshots: crate::snapshots::Snapshots::default(),
            snapshot_requested: false,
            camera: CameraControl::default(),
            modifiers: ModifiersState::empty(),
            turning: false,
            panning_camera_plane: false,
            drag_moved_by_device: false,
            walk_keys: WalkKeys::default(),
            cursor: (0.0, 0.0),
            epoch: Instant::now(),
            last_persistence: Instant::now(),
            lasers: crate::lasers::Lasers::new(laser_scripts),
            effects: crate::effects::Effects::new(),
            physics: crate::physics::Physics::with_state_path(physics_state_path),
            media_workers: media_enabled.then(crate::media_worker::MediaWorkers::default),
            media_revision: 0,
            media_notice: None,
            last_frame: Instant::now(),
            next_frame: Instant::now(),
            frame_interval: DEFAULT_FRAME_INTERVAL,
            visible: true,
            frame_intervals: Vec::with_capacity(240),
            frames_per_second: 0.0,
            stats: FrameStats::default(),
            overlay: Overlay::default(),
            redraw_gate: crate::redraw::RedrawGate::default(),
            camera_is_local: false,
            external_camera: ExternalCameraOwnership::default(),
            hosted_show: None,
            helper_source: None,
            planning_window: None,
            menu: None,
            framed_revision: None,
            adopted_source_view: 0,
            requested_view: None,
            hotspots: Vec::new(),
            selected: None,
            startup_error: None,
            lasting_failure: None,
            preferences_path,
            stored_preferences,
            next_preferences_save: Instant::now(),
            next_child_check: Instant::now(),
            presented_frames: 0,
            benchmark: Vec::new(),
            benchmark_view_index: 0,
            benchmark_view_started: Instant::now(),
        }
    }

    fn renderer_label(&self) -> String {
        self.renderer
            .as_ref()
            .map(gpu_label)
            .unwrap_or_else(|| "no renderer".into())
    }

    fn start_session(&mut self) {
        let (provider, kind) = self.build_provider();
        self.session = Some(Session::new(provider, kind, self.epoch));
    }

    /// Build the provider the current preferences select. Demo mode hosts the canonical show file
    /// through the same path as every other standalone show.
    fn build_provider(&mut self) -> (Box<dyn viz_scene::SceneProvider>, ProviderKind) {
        // Started by the desk: everything drawn arrives over the channel on stdin, and this
        // process chooses nothing. Taken once — the pipe cannot be read from twice — so a rebuild
        // after the desk has gone falls through to the built-in scene rather than hanging on a
        // channel nobody is writing to.
        if self.options.helper
            && let Some(source) = self.helper_source.take()
        {
            return (Box::new(source), ProviderKind::LightingDesk);
        }
        // The planning source is a document held by the Viz editor, so choosing it opens that
        // window if it is not already open and connects to what it serves.
        if self.preferences.source == ProviderKind::PlanningSoftware
            && self.planning_window.is_none()
            && !self.options.planning_server_requested
            // An opened show file is the source for as long as it is open; nothing else is being
            // asked for, and a planning window nobody wanted must not appear beside it.
            && self.hosted_show.is_none()
        {
            match crate::planner::PlanningWindow::open() {
                Ok(window) => {
                    self.planning_window = Some(window);
                    self.lasting_failure = None;
                }
                Err(error) => {
                    eprintln!("open the planning window: {error}");
                    self.lasting_failure = Some(format!("planning window: {error}"));
                }
            }
        }
        // An opened show file takes the place of the desk entirely: while one is open the
        // visualizer talks to its own private server and never to the running desk.
        // An opened show file or planning window takes the place of the desk entirely: while one
        // is open the visualizer talks to its own private source and never to a running desk.
        let local_port = self
            .hosted_show
            .as_ref()
            .map(crate::showfile::HostedShow::port)
            .or_else(|| self.planning_window.as_ref().map(|window| window.port()));
        let local_port = local_port.filter(|_| {
            self.hosted_show.is_some() || self.preferences.source == ProviderKind::PlanningSoftware
        });
        let (host, port) = match local_port {
            Some(port) => ("127.0.0.1".to_owned(), port),
            None => (self.preferences.host.clone(), self.preferences.port),
        };
        (
            Box::new(DeskProvider::start(
                DeskConnection {
                    host,
                    port,
                    user: self.preferences.user.clone(),
                    input_overrides: self.preferences.input_overrides.clone(),
                    target: self.options.target.clone(),
                    ..DeskConnection::default()
                },
                self.epoch,
            )),
            ProviderKind::LightingDesk,
        )
    }

    /// Open a show file: start its private server, then point the session at it.
    pub fn open_show_file(&mut self, path: &std::path::Path) {
        match crate::showfile::HostedShow::open(path) {
            Ok(hosted) => {
                self.hosted_show = Some(hosted);
                self.lasting_failure = None;
                self.framed_revision = None;
                self.camera_is_local = false;
                self.reconnect();
            }
            Err(error) => {
                if let Some(session) = self.session.as_mut() {
                    session.connection = viz_scene::ConnectionState::Failed {
                        boundary: path.display().to_string(),
                        detail: error.clone(),
                    };
                }
                eprintln!("open show file: {error}");
            }
        }
    }

    /// Close an opened show file and return to the desk the preferences name.
    pub fn close_show_file(&mut self) {
        self.lasting_failure = None;
        if self.hosted_show.take().is_some() {
            self.framed_revision = None;
            self.camera_is_local = false;
            self.reconnect();
        }
    }

    /// Ask the operator for a show file and open it.
    pub fn prompt_for_show_file(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open Show File")
            .add_filter("ToskLight show", &["show"]);
        if let Some(hosted) = self.hosted_show.as_ref()
            && let Some(parent) = hosted.path().parent()
        {
            dialog = dialog.set_directory(parent);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_show_file(&path);
        }
    }

    /// Open the product's rig-planning window and make its document the rendered source.
    fn open_rig_editor(&mut self) {
        self.lasting_failure = None;
        if self.planning_window.is_none() {
            match crate::planner::PlanningWindow::open() {
                Ok(window) => self.planning_window = Some(window),
                Err(error) => {
                    self.lasting_failure = Some(format!("rig editor: {error}"));
                    return;
                }
            }
        }
        self.hosted_show = None;
        self.preferences.source = ProviderKind::PlanningSoftware;
        self.framed_revision = None;
        self.camera_is_local = false;
        self.reconnect();
    }

    /// Open or close Quick Settings, with the source control saying what this build can connect.
    ///
    /// The active provider is asked rather than assumed: a source that reports itself unavailable
    /// says so in the panel instead of being offered and then failing.
    fn toggle_quick_settings(&mut self) {
        let preferences = self.preferences.clone();
        self.quick_settings.toggle(&preferences);
        if !self.quick_settings.open {
            return;
        }
        if let Some(capabilities) = self.session.as_ref().map(Session::capabilities)
            && capabilities.kind == ProviderKind::PlanningSoftware
            && !capabilities.available
        {
            self.quick_settings.planner_available = false;
            if let Some(reason) = capabilities.unavailable_reason {
                self.quick_settings.planner_unavailable_reason = reason;
            }
        }
    }

    /// Write the preferences when they have changed, at most every couple of seconds.
    ///
    /// Comparing what would be written against what was written last catches every way the
    /// operator can change a setting — Quick Settings, the wheel over a readout, a shortcut —
    /// without every one of those having to remember to save.
    fn save_preferences(&mut self) {
        // A scripted run is not an operator session: a capture, a smoke check, a snapshot or a
        // benchmark says what it wants for that one run and must not rewrite what the operator
        // left the window set to.
        if self.options.capture.is_some()
            || self.options.verify_only
            || self.options.snapshot
            || self.options.benchmark_seconds.is_some()
        {
            return;
        }
        let now = Instant::now();
        if now < self.next_preferences_save {
            return;
        }
        self.next_preferences_save = now + Duration::from_secs(2);
        let Some(path) = self.preferences_path.clone() else {
            return;
        };
        let text = self.preferences.to_file();
        if text == self.stored_preferences {
            return;
        }
        match crate::settings::store_preferences(&path, &self.preferences) {
            Ok(()) => self.stored_preferences = text,
            Err(error) => {
                // Losing a setting is a nuisance; a window that stops rendering because it could
                // not write one is not acceptable. Say so once and carry on.
                if !self.stored_preferences.is_empty() || self.presented_frames < 2 {
                    eprintln!("preferences {}: {error}", path.display());
                }
                self.stored_preferences = text;
            }
        }
    }

    fn adopt_connected_renderer_settings(&mut self, session: &mut Session) {
        let Some(update) = session.take_renderer_settings() else {
            return;
        };
        let before = self.preferences.to_file();
        self.preferences
            .adopt_file(&update.settings.to_file(), &self.options);
        if self.preferences.to_file() == before {
            return;
        }
        self.next_preferences_save = Instant::now();
        if self.quick_settings.open {
            self.quick_settings.refresh(&self.preferences);
        }
    }

    /// Notice a private server or planning window that has exited.
    ///
    /// Either one dying looks exactly like a slow connection from the inside, and an operator
    /// staring at an empty picture deserves to be told which process is gone rather than left
    /// watching a connection retry something that will never answer again.
    fn watch_children(&mut self) {
        let now = Instant::now();
        if now < self.next_child_check {
            return;
        }
        self.next_child_check = now + Duration::from_secs(1);
        let mut gone: Option<(String, String)> = None;
        if let Some(hosted) = self.hosted_show.as_mut()
            && hosted.exited()
        {
            gone = Some((
                hosted.label(),
                "the private server for this show file exited".to_owned(),
            ));
            self.hosted_show = None;
        }
        if let Some(planning) = self.planning_window.as_mut()
            && planning.exited()
        {
            gone = Some((
                "planning window".to_owned(),
                "the Viz editor was closed; open a show file or connect to a desk".to_owned(),
            ));
            self.planning_window = None;
        }
        if let Some((boundary, detail)) = gone {
            // The connection keeps its own states, and it will go on reporting a refused socket
            // over the top of this, so the reason is kept where it stays put until it is dealt
            // with. The picture that was last drawn stays on screen underneath it.
            self.lasting_failure = Some(format!("{boundary}: {detail}"));
            if let Some(session) = self.session.as_mut() {
                session.connection = ConnectionState::Failed { boundary, detail };
            }
        }
    }

    /// Stage a new connection. The current scene stays on screen until the candidate validates.
    fn reconnect(&mut self) {
        let (provider, kind) = self.build_provider();
        if let Some(session) = self.session.as_mut() {
            session.replace_provider(provider, kind);
        } else {
            self.session = Some(Session::new(provider, kind, self.epoch));
        }
    }

    fn view_mode(&self) -> ViewMode {
        self.session
            .as_ref()
            .map_or(ViewMode::Full3d, |session| session.source_view.mode)
    }

    /// A local camera gesture takes ownership only from the dedicated external 3D presentation.
    /// Orthographic views keep their own local navigation and an embedded Stage is never a DMX
    /// camera target.
    fn latch_local_camera_control(&mut self) {
        self.camera_is_local = true;
        if is_external_camera_target(self.options.embed, self.view_mode()) {
            self.external_camera.latch_local();
        }
    }

    /// Return the dedicated external camera to the latest live DMX pose. There is deliberately no
    /// release while the camera fixture is stale or absent: the last pose must be held instead of
    /// pretending that zero or a framed view came from DMX.
    fn release_local_camera_control(&mut self) {
        if !is_external_camera_target(self.options.embed, self.view_mode()) {
            return;
        }
        if let Some(camera) = self.external_camera.release_to_dmx() {
            self.camera.adopt(&camera);
            self.camera_is_local = false;
        }
    }

    fn set_view_mode(&mut self, mode: ViewMode) {
        let bounds = self
            .session
            .as_ref()
            .map(|session| session.scene.framing_bounds())
            .unwrap_or_default();
        if let Some(session) = self.session.as_mut() {
            session.source_view.mode = mode;
            session.source_view.camera = Camera::framed(mode, bounds);
        }
        self.framed_revision = None;
        self.camera.adopt(
            &self
                .session
                .as_ref()
                .map(|session| session.source_view.camera)
                .unwrap_or_default(),
        );
        self.camera_is_local = false;
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: Key, text: Option<String>) {
        if self.quick_settings_chord(&key) {
            self.toggle_quick_settings();
            return;
        }
        if self.quick_settings.open {
            self.handle_quick_settings_key(key, text);
            return;
        }
        // Open a show file. The menu bar offers the same command, but the shortcut has to work
        // on a platform that has no menu bar of its own.
        if matches!(key.as_ref(), Key::Character("o") | Key::Character("O"))
            && (self.modifiers.super_key() || self.modifiers.control_key())
        {
            self.prompt_for_show_file();
            return;
        }
        // Take a snapshot. `S` on its own walks the camera backwards, so this is a chord, and it
        // is the one every application uses for keeping what is on screen.
        if matches!(key.as_ref(), Key::Character("s") | Key::Character("S"))
            && (self.modifiers.super_key() || self.modifiers.control_key())
        {
            self.take_snapshot();
            return;
        }
        if let Key::Character(character) = key.as_ref()
            && let Some(mode) = Self::view_for_key(character)
        {
            self.set_view_mode(mode);
            return;
        }
        match key.as_ref() {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::Enter) => {
                self.toggle_quick_settings();
            }
            Key::Named(NamedKey::Space) => {
                // Hiding the overlays leaves the picture on its own, which is what a designer
                // wants when judging a look.
                self.preferences.overlays_hidden = !self.preferences.overlays_hidden;
                if self.preferences.overlays_hidden {
                    self.quick_settings.open = false;
                }
            }
            Key::Character("t") | Key::Character("T") => {
                self.preferences.theme = self.preferences.theme.toggled();
            }
            Key::Character("l") | Key::Character("L") => {
                self.preferences.show_labels = !self.preferences.show_labels;
            }
            Key::Character("c") | Key::Character("C") => {
                self.release_local_camera_control();
            }
            Key::Character("r") | Key::Character("R") => {
                if let Some(session) = self.session.as_mut() {
                    session.request_resync();
                }
            }
            _ => {}
        }
    }

    /// Physical pixel size of the surface being drawn into.
    fn surface_size(&self) -> (f32, f32) {
        self.window
            .as_ref()
            .map(|window| {
                let size = window.inner_size();
                (size.width.max(1) as f32, size.height.max(1) as f32)
            })
            .unwrap_or((1280.0, 720.0))
    }

    /// How long to leave before the next frame falls due.
    fn pace(&self) -> Duration {
        if self.visible {
            self.frame_interval
        } else {
            HIDDEN_FRAME_INTERVAL
        }
    }

    /// Re-read how fast the display this window is on actually refreshes.
    ///
    /// Presenting faster than that gains nothing — the drawable is only released on the refresh —
    /// and costs the responsiveness of every other input, so the refresh rate is where the pace
    /// starts before [`paced_interval`] measures the real thing.
    fn measure_frame_interval(&mut self) {
        let refresh = self
            .window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .filter(|rate| *rate >= 20_000);
        self.frame_interval = match refresh {
            Some(millihertz) => {
                let seconds = 1000.0 / f64::from(millihertz) * FRAME_PACE;
                Duration::from_secs_f64(seconds.clamp(0.002, 0.05))
            }
            None => DEFAULT_FRAME_INTERVAL,
        };
    }

    fn handle_quick_settings_key(&mut self, key: Key, text: Option<String>) {
        let mut preferences = self.preferences.clone();
        let outcome = match key.as_ref() {
            Key::Named(NamedKey::ArrowUp) => {
                self.quick_settings.move_selection(-1);
                QuickSettingsOutcome::None
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.quick_settings.move_selection(1);
                QuickSettingsOutcome::None
            }
            Key::Named(NamedKey::Tab) => {
                self.quick_settings
                    .move_tab(if self.modifiers.shift_key() { -1 } else { 1 });
                QuickSettingsOutcome::None
            }
            Key::Named(NamedKey::ArrowLeft) => self.quick_settings.adjust(-1, &mut preferences),
            Key::Named(NamedKey::ArrowRight) => self.quick_settings.adjust(1, &mut preferences),
            Key::Named(NamedKey::Enter) => self.quick_settings.activate(&mut preferences),
            Key::Named(NamedKey::Backspace) => {
                self.quick_settings.backspace();
                QuickSettingsOutcome::None
            }
            Key::Named(NamedKey::Escape) => {
                self.quick_settings.selected = self
                    .quick_settings
                    .rows()
                    .iter()
                    .position(|row| *row == ui::Row::Cancel)
                    .unwrap_or_default();
                self.quick_settings.activate(&mut preferences)
            }
            _ => {
                if let Some(text) = text {
                    for character in text.chars().filter(|character| !character.is_control()) {
                        self.quick_settings.type_character(character);
                    }
                }
                QuickSettingsOutcome::None
            }
        };
        let previous_settings = self.preferences.renderer_settings();
        let preferences_changed = preferences.to_file() != self.preferences.to_file();
        self.preferences = preferences;
        if preferences_changed {
            self.next_preferences_save = Instant::now();
            if let Some(session) = self.session.as_mut()
                && session.kind == ProviderKind::PlanningSoftware
            {
                let settings = self.preferences.renderer_settings();
                session.update_renderer_settings(viz_scene::RendererSettingsIntent {
                    request_id: viz_scene::uuid::Uuid::new_v4().to_string(),
                    source: "visualizer".into(),
                    changes: settings.changes_from(&previous_settings),
                });
            }
        }
        self.apply_outcome(outcome);
    }

    fn apply_outcome(&mut self, outcome: QuickSettingsOutcome) {
        match outcome {
            QuickSettingsOutcome::Connect { host, port } => {
                self.quick_settings.message = format!("Connecting to {host}:{port}");
                self.reconnect();
            }
            QuickSettingsOutcome::SourceChanged(kind) => {
                self.quick_settings.message = format!("Switching to {}", kind.label());
                self.reconnect();
            }
            QuickSettingsOutcome::ExportSnapshot(index) => {
                let blender = self.preferences.blender_path();
                self.snapshots.export(index, blender);
            }
            QuickSettingsOutcome::FocusView => self.focus_view(),
            QuickSettingsOutcome::Close
            | QuickSettingsOutcome::AppliedLocally
            | QuickSettingsOutcome::Invalid(_)
            | QuickSettingsOutcome::None => {}
        }
    }

    fn focus_view(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.source_view.camera =
            Camera::framed(session.source_view.mode, session.scene.framing_bounds());
        self.camera.adopt(&session.source_view.camera);
        self.framed_revision = Some(session.scene.revision);
        self.latch_local_camera_control();
        self.quick_settings.message = "Focused the current rig".into();
    }

    /// Freeze the picture exactly as it is and write it as a snapshot.
    ///
    /// Nothing is asked and nothing opens: the moment worth keeping is usually a moment that is
    /// about to be gone, and a dialog would take it. Where the capture went is said afterwards,
    /// on the status surface and in Quick Settings.
    fn take_snapshot(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if session.scene.fixtures.is_empty() {
            self.snapshots.report("Nothing to snapshot yet");
            return;
        }
        let camera = self.snapshot_camera();
        let context = viz_snapshot::CaptureContext {
            // The scene's own name is what the operator calls the show. The diagnostics identity
            // is that name with its id after it, which is for a status line and not for a folder.
            show: if session.scene.show_name.is_empty() {
                session.diagnostics.show_identity.clone()
            } else {
                session.scene.show_name.clone()
            },
            source: session.connection.summary(),
            scene_revision: session.scene.revision,
            look: viz_snapshot::SnapshotLook {
                fog: self.preferences.atmosphere.amount,
                ambient: self.preferences.ambient,
                exposure: self.preferences.exposure,
            },
            camera,
        };
        // The clone is the capture: from here on the snapshot is of this frame, whatever the desk
        // does next and however long the writing takes.
        let (scene, values) = (&session.scene, &session.values);
        self.snapshots.take(scene, values, context);
    }

    /// Drive a `--snapshot` run: settle, capture once, report, and leave.
    ///
    /// The same path the shortcut takes, without a hand on the keyboard, so a build machine or a
    /// script can produce the same capture an operator would. Returns `true` once the frame loop
    /// has nothing left to do.
    fn run_scripted_snapshot(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if !self.snapshot_requested {
            let settled = self.presented_frames >= u64::from(self.options.capture_frames);
            let ready = self
                .session
                .as_ref()
                .is_some_and(|session| !session.scene.fixtures.is_empty());
            if !settled || !ready {
                return false;
            }
            self.snapshot_requested = true;
            self.take_snapshot();
            return true;
        }
        if self.snapshots.busy() {
            return true;
        }
        match self.snapshots.last_written() {
            Some(directory) => println!("snapshot written to {}", directory.display()),
            None => eprintln!("snapshot: nothing was written"),
        }
        event_loop.exit();
        true
    }

    /// The camera the operator is looking through, for the package to open on the same view.
    fn snapshot_camera(&self) -> viz_snapshot::SnapshotCamera {
        let mode = self.view_mode();
        let source = self
            .session
            .as_ref()
            .map(|session| session.source_view.camera)
            .unwrap_or_default();
        let camera = self.camera.camera(&source, mode);
        viz_snapshot::SnapshotCamera {
            position: camera.position.to_array(),
            target: camera.target.to_array(),
            fov_degrees: camera.fov_degrees,
            orthographic: mode.is_orthographic(),
            orthographic_size: camera.orthographic_size,
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // A helper is the desk's window and wears the name the desk gave it, so the two never
        // disagree about what the operator is looking at.
        let title = self
            .helper_source
            .as_mut()
            .and_then(crate::helper_source::HelperSource::take_title)
            .unwrap_or_else(|| "ToskLight PreViz".to_owned());
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("window: {error}");
                event_loop.exit();
                return;
            }
        };
        let surface = WindowSurface(window.clone());
        let icon = crate::png::application_icon();
        if let Some(pixels) = icon.as_ref()
            && let Ok(window_icon) = winit::window::Icon::from_rgba(
                pixels.clone(),
                viz_render::overlay::ICON_SIZE as u32,
                viz_render::overlay::ICON_SIZE as u32,
            )
        {
            window.set_window_icon(Some(window_icon));
        }
        match Renderer::with_icon(&surface, icon.as_deref()) {
            Ok(mut renderer) => {
                renderer.set_media_content_enabled(!self.options.helper && !self.options.embed);
                self.renderer = Some(renderer);
            }
            Err(error) => {
                self.startup_error = Some(error.clone());
                eprintln!("renderer: {error}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        // Camera drags are driven from device motion, which some platforms only report on request.
        event_loop.listen_device_events(DeviceEvents::WhenFocused);
        self.measure_frame_interval();
        self.menu = crate::menu::ApplicationMenu::install();
        // What this launch was asked to look at. A desk that started the visualizer, or an
        // operator who named one, is visualized directly; a show file gets its own private
        // server; and a launch that named nothing opens the planning window so the operator can
        // choose or build a rig.
        // A source that could not be started is reported on the surface, not only on a console
        // nobody is looking at: the alternative is an empty picture and a connection failure that
        // names the wrong thing.
        let mut lasting_failure = None;
        match self.options.startup() {
            Startup::Show(path) => match crate::showfile::HostedShow::open(&path) {
                Ok(hosted) => self.hosted_show = Some(hosted),
                Err(error) => {
                    eprintln!("open show file: {error}");
                    lasting_failure = Some((path.display().to_string(), error));
                }
            },
            Startup::Demo => match canonical_demo_show_path() {
                Ok(path) => match crate::showfile::HostedShow::open(&path) {
                    Ok(hosted) => self.hosted_show = Some(hosted),
                    Err(error) => {
                        eprintln!("open canonical demo show: {error}");
                        lasting_failure = Some((path.display().to_string(), error));
                    }
                },
                Err(error) => {
                    eprintln!("find canonical demo show: {error}");
                    lasting_failure = Some(("canonical demo show".to_owned(), error));
                }
            },
            Startup::Planning => {
                // Nothing was named, so this launch is looking at a planning document: the source
                // control has to say so, and switching away from it has to mean something.
                self.preferences.source = ProviderKind::PlanningSoftware;
                match crate::planner::PlanningWindow::open() {
                    Ok(window) => self.planning_window = Some(window),
                    Err(error) => {
                        eprintln!("open the planning window: {error}");
                        lasting_failure = Some(("planning window".to_owned(), error));
                    }
                }
            }
            // A helper is driven entirely over its channel: nothing is opened, connected to or
            // hosted here. The channel loop is wired separately from this startup path.
            // The greeting happens before the window: the desk names the title it wants, and a
            // helper that cannot agree on the protocol must not open a window at all.
            Startup::Helper => {
                match crate::helper_source::HelperSource::start(
                    std::io::stdin(),
                    std::io::stdout(),
                    "viz-renderer".to_owned(),
                ) {
                    Ok(source) => self.helper_source = Some(source),
                    Err(error) => {
                        eprintln!("the desk's channel: {error}");
                        lasting_failure = Some(("the desk's channel".to_owned(), error));
                    }
                }
            }
            Startup::Desk => {}
        }
        // The connection the session then makes has its own states to report, so the reason this
        // launch could not open what it was asked for is kept beside them until it is fixed.
        self.lasting_failure =
            lasting_failure.map(|(boundary, detail)| format!("{boundary}: {detail}"));
        self.start_session();
        let mode = self.options.view.unwrap_or(ViewMode::Full3d);
        self.requested_view = self.options.view;
        self.set_view_mode(mode);
        self.framed_revision = None;
        if let Some(quality) = self.options.quality {
            self.preferences.quality_override = Some(quality);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                // Movement keys are held, not typed, so they are tracked in both directions and
                // never reach the Quick Settings text fields.
                let movement = self.track_walk_key(&event.logical_key, pressed);
                if movement && !self.quick_settings.open && !self.command_chord() {
                    return;
                }
                if pressed {
                    self.handle_key(
                        event_loop,
                        event.logical_key.clone(),
                        event.text.map(|text| text.to_string()),
                    );
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // The window may have landed on a display that refreshes at another rate. Only the
                // starting point is re-read here; the measured wait follows a change on its own.
                self.measure_frame_interval();
            }
            WindowEvent::Occluded(occluded) => {
                // A window that comes back from behind another one has to show a current frame at
                // once, not whatever was last presented before the system stopped compositing it.
                self.visible = !occluded;
                if !occluded {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.reconfigure();
                    }
                    self.next_frame = Instant::now();
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::Focused(true) => {
                self.next_frame = Instant::now();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                trace_input(&format!("button {button:?} {state:?}"));
                if pressed
                    && button == MouseButton::Left
                    && self.hotspot_under_cursor() == Some(ui::Hotspot::OpenSettings)
                {
                    self.toggle_quick_settings();
                    return;
                }
                // The left button belongs to the surface, not to the camera: it opens Quick
                // Settings above, it inspects a fixture here, and a drag with it moves nothing.
                if pressed && button == MouseButton::Left && self.hotspot_under_cursor().is_none() {
                    self.select_under_cursor();
                }
                match button {
                    MouseButton::Right => self.turning = pressed,
                    MouseButton::Middle => self.panning_camera_plane = pressed,
                    _ => {}
                }
                if button == MouseButton::Right || button == MouseButton::Middle {
                    // Each drag decides for itself which source is moving it.
                    self.drag_moved_by_device = false;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Where the pointer is, for the status surface — and the fallback source for a
                // drag. Device motion normally moves the camera, but a platform or a mouse utility
                // that keeps device motion from the application still reports the pointer, and a
                // drag has to work there too.
                let previous = self.cursor;
                self.cursor = (position.x, position.y);
                let dragging = self.turning || self.panning_camera_plane;
                trace_input(&format!(
                    "cursor {:.1},{:.1} dragging {dragging} device {}",
                    position.x, position.y, self.drag_moved_by_device
                ));
                if dragging && !self.drag_moved_by_device {
                    // The cursor is in physical pixels; a drag is calibrated on how far the hand
                    // moved, which is that divided by the scale of the display it moved across.
                    let scale = self.window_scale();
                    let hand_right = hand_points(position.x - previous.0, scale);
                    let hand_down = hand_points(position.y - previous.1, scale);
                    self.drag_camera(hand_right, hand_down, scale);
                }
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                trace_input(&format!("wheel {delta:?} phase {phase:?}"));
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 40.0,
                };
                // The wheel adjusts whichever value it is over, and moves the camera everywhere
                // else.
                if let Some(hotspot) = self.hotspot_under_cursor() {
                    self.adjust_hotspot(hotspot, amount);
                    return;
                }
                match delta {
                    // A notched wheel zooms, which is what a wheel does everywhere.
                    MouseScrollDelta::LineDelta(..) => {
                        self.camera.zoom((1.0 - amount * 0.08).clamp(0.5, 2.0));
                        self.latch_local_camera_control();
                    }
                    // Continuous two-axis scrolling is a hand moving, not a wheel turning: a
                    // trackpad, or a mouse utility that has claimed the right button for its own
                    // panning and delivers that drag as scrolling. It turns the camera, which is
                    // what the drag it stands in for does. Holding the command or control key
                    // keeps a zoom available to a machine that has no notched wheel at all.
                    MouseScrollDelta::PixelDelta(position) => {
                        if self.command_chord() {
                            self.camera.zoom((1.0 - amount * 0.08).clamp(0.5, 2.0));
                            self.latch_local_camera_control();
                        } else {
                            // The delta is in physical pixels; turning is calibrated on the hand.
                            let scale = self.window_scale();
                            let (hand_right, hand_down) = (
                                hand_points(position.x, scale),
                                hand_points(position.y, scale),
                            );
                            trace_input(&format!("turn {hand_right:.1},{hand_down:.1}"));
                            self.turn_camera(hand_right, hand_down, scale);
                            trace_input(&format!(
                                "camera yaw {:.3} pitch {:.3}",
                                self.camera.yaw, self.camera.pitch
                            ));
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => self.draw(event_loop),
            _ => {}
        }
    }

    /// Camera drags are driven from device motion where it is reported, and from the cursor where
    /// it is not.
    ///
    /// A window only sees a drag if the platform routed that button's press to it, and a right or
    /// middle button is exactly where that routing differs between platforms — a right-drag can be
    /// claimed for a context menu and never reach the window at all. Device motion is reported to
    /// the application before any of that, it is a delta already, and it keeps reporting after the
    /// pointer has run into the edge of a display, so it is what a camera drag wants when it is
    /// there. `CursorMoved` in the window event above stands in when it is not.
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device: DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                if !self.turning && !self.panning_camera_plane {
                    // Traced even when it is dropped: an operator reporting that a drag does
                    // nothing needs the log to say whether the motion arrived at all.
                    trace_input(&format!("motion {:.1},{:.1} (no drag)", delta.0, delta.1));
                    return;
                }
                self.drag_moved_by_device = true;
                let scale = self.window_scale();
                self.drag_camera(
                    hand_points(delta.0, scale),
                    hand_points(delta.1, scale),
                    scale,
                );
            }
            DeviceEvent::Button { button, state } => {
                trace_input(&format!("device button {button} {state:?}"));
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.watch_children();
        self.save_preferences();
        // The menu bar is drained on the main thread, which is where a file dialog has to open.
        let commands = self
            .menu
            .as_ref()
            .map(|menu| menu.drain())
            .unwrap_or_default();
        for command in commands {
            match command {
                crate::menu::MenuCommand::OpenShowFile => self.prompt_for_show_file(),
                crate::menu::MenuCommand::OpenRigEditor => self.open_rig_editor(),
                crate::menu::MenuCommand::CloseShowFile => self.close_show_file(),
                crate::menu::MenuCommand::ConnectToDesk => self.close_show_file(),
                crate::menu::MenuCommand::TakeSnapshot => self.take_snapshot(),
                crate::menu::MenuCommand::QuickSettings => {
                    self.toggle_quick_settings();
                }
            }
        }
        // Pace the frames. Presenting back to back parks the event loop inside the driver waiting
        // for a drawable for almost the whole refresh interval, and a mouse the operator is
        // dragging then has nowhere to be delivered: the picture falls seconds behind the hand.
        // Waiting for the frame to be due instead leaves that time in the window system.
        let interval = self.pace();
        let now = Instant::now();
        if now >= self.next_frame {
            // One frame is owed however late this is; missed frames are never chased.
            self.next_frame = now + interval;
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Whatever the operator last set is what they expect to find next time, including a change
        // made in the final seconds before they closed the window.
        self.next_preferences_save = Instant::now();
        self.save_preferences();
        if let Some(session) = self.session.as_mut() {
            session.shutdown();
        }
    }
}

/// Print one input event when `LIGHT_VIZ_INPUT_TRACE` is set.
///
/// Mice and trackpads differ in what they report for the same physical gesture, and an operator
/// describing "the right button zooms" needs a way to say what actually arrives.
fn trace_input(message: &str) {
    if std::env::var_os("LIGHT_VIZ_INPUT_TRACE").is_some() {
        eprintln!("input: {message}");
    }
}

#[cfg(test)]
mod tests;
