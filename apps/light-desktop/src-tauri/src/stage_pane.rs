//! The Stage drawn by the renderer, inside the desk's own window.
//!
//! The desk has two ways to show a Stage. Its web renderer draws one in the interface, and has
//! since before any of this existed. This is the other: a supervised renderer process draws the
//! same Stage with the native renderer, into a rectangle of the desk's window that the web layout
//! decides, with every menu, dialog and sheet drawn above it by the interface.
//!
//! The native one is not always available, and that is the normal case rather than a failure. It
//! needs a platform where the two processes can move a picture between them, a renderer beside the
//! application that answers the greeting, and an adapter that can draw on the desk's window. Where
//! any of that is missing the desk keeps its web renderer, which is why [`stage_pane_available`]
//! exists and why the interface asks before it embeds anything.
//!
//! What crosses the boundary is checked on the way in. The pane geometry comes from the web layer
//! and the frames come from another process, and neither may put the desk's window into a state
//! that fails validation — a scissor past the end of a surface would take the desk down, which is
//! precisely what running the renderer in another process was meant to prevent.

use crate::stage_compositor::StageCompositor;
mod benchmark;
mod closing;
pub(crate) mod still;
mod tick;
mod transport;
mod visibility;
mod writer;
use transport::{FromRenderer, read_renderer};
use visibility::PaneVisibility;
use writer::PaneWriter;
mod picture;
use benchmark::{BenchmarkAttachment, benchmark_attachment};
pub(crate) use benchmark::{FrameTelemetry, PaneHeartbeat, StagePaneBenchmarkSamples};
pub(crate) use picture::Picture;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use viz_helper::handshake::{HelperIdentity, greet_helper};
use viz_helper::pane::PaneRect;
use viz_helper::protocol::{
    DeskEndpoint, FrameTransport, FromHelper, PaneInput, SharedSurfaceHandle, ToHelper, decode,
    supported_transports,
};
use viz_helper::{SupervisedHelper, framing};

/// The embedded Stage, while there is one.
#[derive(Default)]
pub(crate) struct StagePane {
    inner: Mutex<Option<Running>>,
    capture: Mutex<still::CaptureState>,
    retiring: std::sync::atomic::AtomicBool,
    retirement_done: tokio::sync::Notify,
    retirement_error: Mutex<Option<String>>,
    /// The last thing that went wrong, for the interface to show and act on.
    trouble: Mutex<Option<String>>,
    /// What the operator has pointed at since the interface last asked.
    ///
    /// Queued rather than pushed: the interface polls the pane already, and a selection that
    /// arrived through a second mechanism could be applied out of order with the first.
    picked: Mutex<Vec<(Option<String>, bool)>>,
    /// Where the renderer's camera is, as it last reported.
    ///
    /// Read rather than tracked: the renderer owns the camera, so a desk keeping its own copy would
    /// drift the moment a mouse touched the pane.
    camera: Mutex<Option<[f32; 6]>>,
    telemetry: Mutex<Vec<FrameTelemetry>>,
    heartbeats: Mutex<Vec<PaneHeartbeat>>,
    heartbeat_overflow: std::sync::atomic::AtomicBool,
    /// Whether anything is embedded, readable without taking the lock.
    ///
    /// The frame pump asks this before posting anything to the main thread. A desk with no pane —
    /// which is most desks, most of the time — must not put work on the thread the interface
    /// paints on, and posting a task every few milliseconds from startup competes with the
    /// webview's own first paint.
    drawing: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
pub(crate) struct StagePanes {
    panes: Mutex<HashMap<String, RegisteredPane>>,
    /// Admission only; ongoing ownership is derived from the current panes' capture states.
    capture_start_gate: Mutex<()>,
}

struct RegisteredPane {
    pane: Arc<StagePane>,
    live_3d: bool,
}

impl StagePanes {
    #[cfg(test)]
    fn ensure_owner_available(&self, key: &str, live_3d: bool) -> Result<(), String> {
        let panes = self.panes.lock().map_err(|_| "the Stage pane registry")?;
        Self::ensure_owner_available_in(&panes, key, live_3d)
    }

    fn ensure_owner_available_in(
        panes: &HashMap<String, RegisteredPane>,
        key: &str,
        live_3d: bool,
    ) -> Result<(), String> {
        if !live_3d {
            return Ok(());
        }
        if panes
            .iter()
            .any(|(candidate, registered)| candidate != key && registered.live_3d)
        {
            return Err(
                "Only one live 3D Stage is supported. Close the other 3D Stage or configure this screen as Stage - 2D."
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Explicit addresses stay pinned to their owning window even if the global live owner changes.
    /// An absent ID retains the legacy application's live-3D camera address.
    fn addressed_pane(
        &self,
        window_label: &str,
        pane_id: Option<&str>,
    ) -> Result<Option<Arc<StagePane>>, String> {
        match pane_id {
            Some(id) => self.pane(&format!("{window_label}:{id}")),
            None => self.live_3d_pane(),
        }
    }

    fn pane(&self, key: &str) -> Result<Option<Arc<StagePane>>, String> {
        Ok(self
            .panes
            .lock()
            .map_err(|_| "the Stage pane registry")?
            .get(key)
            .map(|registered| Arc::clone(&registered.pane)))
    }

    fn open<T>(
        &self,
        key: String,
        live_3d: bool,
        window: Arc<T>,
        surface_size: (u32, u32),
        pane_rect: PaneRect,
        scale: f32,
        window_visible: bool,
    ) -> Result<(), String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let pane = {
            let mut panes = self.panes.lock().map_err(|_| "the Stage pane registry")?;
            Self::ensure_owner_available_in(&panes, &key, live_3d)?;
            let registered = panes.entry(key.clone()).or_insert_with(|| RegisteredPane {
                pane: Arc::new(StagePane::default()),
                live_3d: false,
            });
            registered.live_3d = live_3d;
            Arc::clone(&registered.pane)
        };
        if let Err(error) = pane.open(
            window,
            surface_size,
            pane_rect,
            scale,
            format!("stage-pane:{key}"),
            window_visible,
        ) {
            if let Ok(mut panes) = self.panes.lock()
                && let Some(registered) = panes.get_mut(&key)
            {
                registered.live_3d = false;
            }
            return Err(error);
        }
        Ok(())
    }

    fn close(&self, key: &str) -> Result<(), String> {
        let pane = {
            let mut panes = self.panes.lock().map_err(|_| "the Stage pane registry")?;
            let Some(registered) = panes.get_mut(key) else {
                return Ok(());
            };
            registered.live_3d = false;
            Arc::clone(&registered.pane)
        };
        let result = pane.close();
        pane.capture_interrupted("Stage pane closed");
        result
    }

    fn drawing_panes(&self) -> Vec<(String, Arc<StagePane>)> {
        self.panes.lock().map_or_else(
            |_| Vec::new(),
            |panes| {
                panes
                    .iter()
                    .filter(|(_, registered)| registered.pane.needs_pump())
                    .map(|(key, registered)| {
                        (
                            key.split(':').next().unwrap_or_default().to_owned(),
                            Arc::clone(&registered.pane),
                        )
                    })
                    .collect()
            },
        )
    }

    fn live_3d_pane(&self) -> Result<Option<Arc<StagePane>>, String> {
        Ok(self
            .panes
            .lock()
            .map_err(|_| "the Stage pane registry")?
            .values()
            .find(|registered| registered.live_3d)
            .map(|registered| Arc::clone(&registered.pane)))
    }

    pub(crate) fn has_live_3d(&self) -> Result<bool, String> {
        Ok(self
            .panes
            .lock()
            .map_err(|_| "the Stage pane registry")?
            .values()
            .any(|registered| registered.live_3d))
    }
}

struct Running {
    helper: Option<SupervisedHelper>,
    /// Where the renderer hands over each surface, while the transport shares one.
    #[cfg(target_os = "macos")]
    rendezvous: Option<viz_surface::rendezvous::Rendezvous>,
    to_helper: Option<PaneWriter>,
    channel_error: Option<String>,
    inbox: Receiver<FromRenderer>,
    compositor: StageCompositor,
    identity: HelperIdentity,
    transport: FrameTransport,
    benchmark: Option<BenchmarkAttachment>,
    pane: PaneRect,
    scale: f32,
    surface_size: (u32, u32),
    visibility: PaneVisibility,
}

impl StagePane {
    /// Start the renderer and give it the pane to draw.
    ///
    /// The window is the desk's own native window; the compositor draws beneath the interface on
    /// it. Failure here is reported rather than fatal: the caller keeps its web renderer.
    /// Log every refusal on the way in. The interface falls back to its own renderer either way,
    /// so without this a Stage that could not embed is indistinguishable from one that did not try.
    pub(crate) fn open<T>(
        &self,
        window: Arc<T>,
        surface_size: (u32, u32),
        pane: PaneRect,
        scale: f32,
        target: String,
        window_visible: bool,
    ) -> Result<(), String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        self.close()?;
        self.assert_openable()?;
        let benchmark = benchmark_attachment()?;
        let (mut helper, to_helper, from_helper, identity) = start_helper()?;
        // A surface has no name this channel can carry, so a shared transport needs a channel a
        // mach right can cross. Opening one is also the only honest test of whether this process
        // may: a restricted process cannot register a name, and then the pair shares pixels
        // instead. Opened before the transports are offered, so what is offered is what works.
        #[cfg(target_os = "macos")]
        let rendezvous = viz_surface::rendezvous::Rendezvous::open(&format!(
            "{}-{}",
            std::process::id(),
            surface_service_counter()
        ))
        .ok();
        #[cfg(target_os = "macos")]
        let surface_service = rendezvous
            .as_ref()
            .map(|rendezvous| rendezvous.name().to_owned());
        #[cfg(not(target_os = "macos"))]
        let surface_service: Option<String> = None;

        let transport = match crate::packaged_benchmark::baseline_transport(
            &identity,
            desk_transports(surface_service.is_some()),
        ) {
            Ok(transport) => transport,
            Err(error) => {
                helper.stop();
                return Err(error);
            }
        };

        let (outbox, inbox) = std::sync::mpsc::sync_channel(128);
        std::thread::Builder::new()
            .name("stage-pane-channel".into())
            .spawn(move || read_renderer(from_helper, &outbox))
            .map_err(|error| error.to_string())?;

        let to_helper = match PaneWriter::start(to_helper) {
            Ok(writer) => writer,
            Err(error) => {
                helper.stop();
                return Err(error);
            }
        };
        let compositor =
            match StageCompositor::attach(window, surface_size.0, surface_size.1, scale) {
                Ok(compositor) => compositor,
                Err(error) => {
                    helper.stop();
                    eprintln!("stage pane: nothing can draw on the desk's window: {error}");
                    return Err(error);
                }
            };

        let mut running = Running {
            helper: Some(helper),
            #[cfg(target_os = "macos")]
            rendezvous,
            to_helper: Some(to_helper),
            channel_error: None,
            inbox,
            compositor,
            identity,
            transport,
            benchmark,
            pane,
            scale,
            surface_size,
            visibility: PaneVisibility::new(pane, window_visible),
        };
        running.compositor.set_pane(pane, scale);
        running.send(&Picture::startup().into_helper());
        running.send(&ToHelper::Embed {
            pane,
            scale,
            transport,
            // The rig comes from the desk's own server, which this process is the one that knows
            // how to find. Nothing about the show travels down the channel.
            desk: Some(DeskEndpoint {
                host: crate::server::address().ip().to_string(),
                port: crate::server::address().port(),
                // Named so a desk driving more than one renderer keeps a view per renderer, and so
                // this pane's camera is not the standalone visualizer's.
                target,
            }),
            surface_service,
        });
        running.refresh_visibility();
        // A renderer starts looking wherever its defaults put it. An operator who aimed this pane,
        // switched to another built-in and came back should find their view, not a reset one — so
        // the camera the last renderer reported is given straight back to this one.
        if let Some([x, y, z, pan, tilt, distance]) =
            *self.camera.lock().map_err(|_| "the Stage pane")?
        {
            running.send(&ToHelper::Input {
                input: PaneInput::Place {
                    x: Some(x),
                    y: Some(y),
                    z: Some(z),
                    pan: Some(pan),
                    tilt: Some(tilt),
                    distance: Some(distance),
                },
            });
        }
        eprintln!(
            "stage pane: embedded with {} over {transport:?}",
            running.identity.renderer
        );
        *self.inner.lock().map_err(|_| "the Stage pane")? = Some(running);
        self.drawing
            .store(true, std::sync::atomic::Ordering::Release);
        *self.trouble.lock().map_err(|_| "the Stage pane")? = None;
        wake_pump();
        Ok(())
    }

    /// Follow the layout. Cheap enough to call on every reported change.
    pub(crate) fn set_pane(
        &self,
        surface_size: (u32, u32),
        pane: PaneRect,
        scale: f32,
    ) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        let Some(running) = guard.as_mut() else {
            return Ok(());
        };
        running.pane = pane;
        running.visibility.set_geometry(pane);
        running.scale = scale;
        running.surface_size = surface_size;
        running.compositor.resize(surface_size.0, surface_size.1);
        running.compositor.set_pane(pane, scale);
        running.send(&ToHelper::Pane { pane, scale });
        running.refresh_visibility();
        Ok(())
    }

    /// Forward what the operator did over the pane.
    ///
    /// The webview above the surface wins hit-testing whatever CSS says, so this is the only way
    /// pane input reaches the renderer at all.
    pub(crate) fn send_input(&self, input: PaneInput) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.send(&ToHelper::Input { input });
        }
        Ok(())
    }

    /// Tell the renderer what the operator has selected, so it can draw it.
    pub(crate) fn send_selection(&self, fixtures: Vec<String>) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.send(&ToHelper::Selection { fixtures });
        }
        Ok(())
    }

    pub(crate) fn camera(&self) -> Result<Option<[f32; 6]>, String> {
        Ok(*self.camera.lock().map_err(|_| "the Stage pane")?)
    }

    /// Put the camera at numbers rather than by dragging. `None` leaves that one alone.
    pub(crate) fn place_camera(
        &self,
        x: Option<f32>,
        y: Option<f32>,
        z: Option<f32>,
        pan: Option<f32>,
        tilt: Option<f32>,
        distance: Option<f32>,
    ) -> Result<(), String> {
        self.send_input(PaneInput::Place {
            x,
            y,
            z,
            pan,
            tilt,
            distance,
        })
    }

    /// Take what the operator pointed at since this was last asked.
    pub(crate) fn take_picked(&self) -> Result<Vec<(Option<String>, bool)>, String> {
        Ok(std::mem::take(
            &mut *self.picked.lock().map_err(|_| "the Stage pane")?,
        ))
    }

    pub(crate) fn trouble(&self) -> Result<Option<String>, String> {
        Ok(self.trouble.lock().map_err(|_| "the Stage pane")?.clone())
    }

    /// What is drawing and how it reaches the desk, for the diagnostics an operator reads.
    pub(crate) fn description(&self) -> Result<Option<String>, String> {
        if !self.is_drawing() {
            return Ok(None);
        }
        let guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        Ok(guard.as_ref().map(|running| {
            let transport = match running.transport {
                FrameTransport::Shared => "shared surface",
                FrameTransport::Copy => "copied frames",
            };
            format!("{} ({transport})", running.identity.renderer)
        }))
    }

    /// True while a renderer is drawing, without taking the lock the frame does.
    pub(crate) fn is_drawing(&self) -> bool {
        self.drawing.load(std::sync::atomic::Ordering::Acquire)
    }
}

impl Running {
    /// Take the surface the renderer announced.
    ///
    /// A renderer that has died is not an error to send to: the desk carries on either way, and
    /// the next thing it sends will find a channel again or report that it did not.
    fn send(&mut self, message: &ToHelper) {
        if let ToHelper::Picture { mode, quality, .. } = message {
            self.compositor.set_timing_view(*mode, *quality);
        }
        if self.channel_error.is_some() {
            return;
        }
        if let Some(writer) = self.to_helper.as_ref() {
            if let Err(detail) = writer.send(message.clone()) {
                self.channel_error = Some(detail);
                self.to_helper = None;
            }
        }
    }
}

fn start_helper() -> Result<
    (
        SupervisedHelper,
        std::process::ChildStdin,
        std::process::ChildStdout,
        HelperIdentity,
    ),
    String,
> {
    let program = match crate::visualizer::helper_binary() {
        Ok(program) => program,
        Err(error) => {
            eprintln!("stage pane: no renderer beside the application: {error}");
            return Err(error);
        }
    };
    // `--embed` opens no helper window: the desk owns the window and pane rectangle.
    let mut helper = crate::portable::renderer(program, vec!["--embed".to_owned()]);
    if let Err(error) = helper.start() {
        eprintln!("stage pane: the renderer would not start: {error}");
        return Err(error);
    }
    let (mut to_helper, mut from_helper) = helper
        .take_channel()
        .ok_or("the renderer started without a channel")?;
    eprintln!("stage pane: renderer started, greeting it");
    let identity = match greet_helper(&mut to_helper, &mut from_helper, "ToskLight Stage") {
        Ok(identity) => identity,
        Err(error) => {
            helper.stop();
            eprintln!("stage pane: the renderer would not agree a protocol: {error}");
            return Err(error.to_string());
        }
    };
    Ok((helper, to_helper, from_helper, identity))
}

/// Names each rendezvous a desk opens, so two panes in one process never collide.
fn surface_service_counter() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// What the desk can receive.
///
/// Read from the same place the renderer reads its own list, so the two halves cannot end up with
/// different ideas about what the platform offers — then narrowed by what this desk can actually
/// do. A build without shared-surface support, or a process that was refused a rendezvous, takes
/// the shared transport off the table rather than agreeing to one it cannot receive.
fn desk_transports(has_rendezvous: bool) -> Vec<FrameTransport> {
    let mut transports = supported_transports();
    if !viz_surface::is_supported() || !has_rendezvous {
        transports.retain(|transport| *transport != FrameTransport::Shared);
    }
    transports
}

/// Whether an embedded Stage is possible at all here.
///
/// Answered before anything is started, because the interface has to choose between the native
/// pane and its own web renderer before it lays either of them out.
pub(crate) fn embedding_possible() -> bool {
    crate::visualizer::helper_binary().is_ok()
}

/// No attached surface means no UI pump work. A helper message wakes this fallback wait.
const IDLE: std::time::Duration = std::time::Duration::from_millis(250);
const FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
static PUMP_WAKE: OnceLock<SyncSender<()>> = OnceLock::new();

pub(super) fn wake_pump() {
    if let Some(wake) = PUMP_WAKE.get() {
        let _ = wake.try_send(());
    }
}

/// One UI task at a time, then wait until an exact compositor deadline or fresh helper input.
/// There is no fixed 8ms quantization or minimum 2ms floor on the 60Hz presentation deadline.
pub(crate) fn drive(app: &tauri::AppHandle) {
    let handle = app.clone();
    let (wake, events) = std::sync::mpsc::sync_channel(1);
    if PUMP_WAKE.set(wake).is_err() {
        return;
    }
    std::thread::Builder::new()
        .name("stage-pane".into())
        .spawn(move || {
            use tauri::Manager;
            loop {
                let panes = handle.state::<StagePanes>().drawing_panes();
                if panes.is_empty() {
                    let _ = events.recv_timeout(IDLE);
                    continue;
                }
                while events.try_recv().is_ok() {}
                let (done, wait) = std::sync::mpsc::channel();
                let owner = handle.clone();
                let posted = handle.run_on_main_thread(move || {
                    let mut deadline = std::time::Instant::now() + IDLE;
                    for (label, pane) in panes {
                        let Some(window) = owner.get_webview_window(&label) else {
                            let _ = pane.window_destroyed();
                            deadline =
                                deadline.min(std::time::Instant::now() + pane.next_poll_delay());
                            continue;
                        };
                        let visible = window.is_visible().unwrap_or(false)
                            && !window.is_minimized().unwrap_or(true);
                        let _ = pane.set_window_visible(visible);
                        let _ = pane.tick();
                        deadline = deadline.min(std::time::Instant::now() + pane.next_poll_delay());
                    }
                    let measured = std::time::Instant::now();
                    let _ = done.send((measured, deadline.saturating_duration_since(measured)));
                });
                if posted.is_err() {
                    return;
                }
                // A timeout is not cancellation: keep the same outstanding UI receipt.
                let (measured, delay) = loop {
                    match wait.recv_timeout(FRAME_TIMEOUT) {
                        Ok(completed) => break completed,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                };
                let _ = events.recv_timeout(delay.saturating_sub(measured.elapsed()));
            }
        })
        .ok();
}

/// Whether the desk can draw the Stage itself, or has to keep using its web renderer.
#[tauri::command]
pub(crate) fn stage_pane_available() -> bool {
    let available = embedding_possible();
    eprintln!("stage pane: asked whether one can be embedded -> {available}");
    if !available {
        // Said once, where an operator or a log can find it. A Stage that quietly stays on the web
        // renderer looks identical to one that chose to.
        eprintln!(
            "stage pane: no renderer to embed: {}",
            crate::visualizer::helper_binary().err().unwrap_or_default()
        );
    }
    available
}

/// The pane the interface laid out, in the points the web layout works in.
///
/// Named as the web layer names things. Tauri converts a command's own arguments from camelCase,
/// but not the fields inside one — so a struct spelled in snake_case here is rejected during
/// deserialization, before the command body runs, and the only sign is a rejected promise on the
/// other side of the boundary.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaneGeometry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale: f32,
    /// Physical pixel size of the whole window, which the compositor configures its surface to.
    surface_width: u32,
    surface_height: u32,
}

impl PaneGeometry {
    fn rect(self) -> PaneRect {
        PaneRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

#[tauri::command]
pub(crate) fn open_stage_pane(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    visualizer: tauri::State<'_, crate::visualizer::Visualizer>,
    pane_id: String,
    live_3d: bool,
    geometry: PaneGeometry,
) -> Result<(), String> {
    if live_3d && visualizer.is_open()? {
        return Err(
            "Only one live 3D Stage is supported. Close the Visualizer or configure this screen as Stage - 2D."
                .to_owned(),
        );
    }
    let key = format!("{}:{pane_id}", window.label());
    panes.open(
        key.clone(),
        live_3d,
        Arc::new(window.clone()),
        (geometry.surface_width, geometry.surface_height),
        geometry.rect(),
        geometry.scale,
        window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true),
    )?;
    panes
        .pane(&key)?
        .ok_or("Stage pane did not attach")?
        .register_capture(window, pane_id)
}

#[tauri::command]
pub(crate) fn set_stage_pane(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    geometry: PaneGeometry,
) -> Result<(), String> {
    let key = format!("{}:{pane_id}", window.label());
    let Some(pane) = panes.pane(&key)? else {
        return Ok(());
    };
    pane.set_pane(
        (geometry.surface_width, geometry.surface_height),
        geometry.rect(),
        geometry.scale,
    )
}

#[tauri::command]
pub(crate) fn set_stage_pane_visibility(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    visible: bool,
) -> Result<(), String> {
    if let Some(pane) = panes.pane(&format!("{}:{pane_id}", window.label()))? {
        pane.set_web_visible(visible)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn close_stage_pane(
    window: tauri::WebviewWindow,
    pane_id: String,
) -> Result<(), String> {
    let key = format!("{}:{pane_id}", window.label());
    closing::close_command(window, key).await
}

/// What the operator did over the pane, already coalesced by the web layer into one step.
#[tauri::command]
pub(crate) fn stage_pane_input(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: Option<String>,
    gesture: String,
    x: f32,
    y: f32,
) -> Result<(), String> {
    let Some(pane) = panes.addressed_pane(window.label(), pane_id.as_deref())? else {
        return Ok(());
    };
    let input = match gesture.as_str() {
        // A pick carries where in the pane it happened, as a fraction of the pane's own size.
        "pick" => PaneInput::Pick {
            x,
            y,
            additive: false,
        },
        "pick-add" => PaneInput::Pick {
            x,
            y,
            additive: true,
        },
        "orbit" => PaneInput::Orbit { dx: x, dy: y },
        "pan" => PaneInput::Pan { dx: x, dy: y },
        "truck" => PaneInput::Truck { dx: x, dy: y },
        "fly" => PaneInput::Fly {
            forward: y,
            right: x,
        },
        "zoom" => PaneInput::Zoom { amount: y },
        "frame" => PaneInput::Frame,
        other => return Err(format!("no such pane gesture: {other}")),
    };
    pane.send_input(input)
}

/// What the operator has selected, for the renderer to draw.
#[tauri::command]
pub(crate) fn set_stage_pane_selection(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    fixtures: Vec<String>,
) -> Result<(), String> {
    let Some(pane) = panes.pane(&format!("{}:{pane_id}", window.label()))? else {
        return Ok(());
    };
    pane.send_selection(fixtures)
}

#[tauri::command]
pub(crate) fn set_stage_pane_picture(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
    picture: Picture,
) -> Result<(), String> {
    let Some(pane) = panes.pane(&format!("{}:{pane_id}", window.label()))? else {
        return Ok(());
    };
    pane.send_picture(picture)
}

/// What is drawing the pane and how it reaches the desk, plus whatever last went wrong.
///
/// One call rather than two so the interface cannot show a renderer and a failure that were true
/// at different moments.
/// What the operator pointed at since this was last asked, and whether they were extending.
///
/// Drained by the interface, which owns what is selected. The renderer resolved the geometry; only
/// the desk decides what pointing at a fixture means.
#[tauri::command]
pub(crate) fn take_stage_pane_picks(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
) -> Result<Vec<(Option<String>, bool)>, String> {
    panes
        .pane(&format!("{}:{pane_id}", window.label()))?
        .map_or_else(|| Ok(Vec::new()), |pane| pane.take_picked())
}

/// Where the renderer's camera is, as `[x, y, z, pan, tilt, distance]`.
#[tauri::command]
pub(crate) fn stage_pane_camera(
    panes: tauri::State<'_, StagePanes>,
) -> Result<Option<[f32; 6]>, String> {
    panes
        .live_3d_pane()?
        .map_or_else(|| Ok(None), |pane| pane.camera())
}

/// Put the camera at numbers, for a control surface that addresses it that way.
#[tauri::command]
pub(crate) fn place_stage_pane_camera(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: Option<String>,
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    pan: Option<f32>,
    tilt: Option<f32>,
    distance: Option<f32>,
) -> Result<(), String> {
    let Some(pane) = panes.addressed_pane(window.label(), pane_id.as_deref())? else {
        return Ok(());
    };
    pane.place_camera(x, y, z, pan, tilt, distance)
}

#[tauri::command]
pub(crate) fn stage_pane_status(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: Option<String>,
) -> Result<(Option<String>, Option<String>), String> {
    let pane = if let Some(pane_id) = pane_id {
        panes.pane(&format!("{}:{pane_id}", window.label()))?
    } else {
        panes.live_3d_pane()?
    };
    let Some(pane) = pane else {
        return Ok((None, None));
    };
    Ok((pane.description()?, pane.trouble()?))
}

#[tauri::command]
pub(crate) fn take_stage_pane_benchmark_samples(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
) -> Result<StagePaneBenchmarkSamples, String> {
    if std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT").is_none() {
        return Ok(StagePaneBenchmarkSamples::default());
    }
    // A query failure is unknown, never affirmative visibility. This describes host attachment
    // and bounds, not occlusion, scanout, or acknowledgement of any individual helper frame.
    let window_visible =
        window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true);
    panes.take_benchmark_samples(window.label(), window_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desk must never announce a transport it has no code to receive. Getting this wrong is a
    /// negotiation that succeeds and a pane that stays black.
    #[test]
    fn the_desk_offers_only_what_it_can_receive() {
        assert!(
            desk_transports(true).contains(&FrameTransport::Copy),
            "copying needs nothing from the platform and is always available"
        );
        assert_eq!(
            desk_transports(true).contains(&FrameTransport::Shared),
            viz_surface::is_supported(),
            "a shared transport is offered exactly where one is compiled in"
        );
        assert!(
            !desk_transports(false).contains(&FrameTransport::Shared),
            "a desk refused a rendezvous has nowhere for a surface to arrive, and says so"
        );
    }

    /// A pane nobody opened is not an error to close, resize or send input to. The interface calls
    /// these from layout and pointer handlers, which run whether or not a renderer is there.
    #[test]
    fn a_pane_that_was_never_opened_is_harmless_to_drive() {
        let pane = StagePane::default();
        pane.close().expect("closing nothing");
        pane.set_pane((1_920, 1_080), PaneRect::default(), 2.0)
            .expect("laying out nothing");
        pane.send_input(PaneInput::Zoom { amount: 1.0 })
            .expect("input with nothing to send it to");
        pane.tick().expect("drawing nothing");
        assert_eq!(pane.description().expect("description"), None);
        assert_eq!(pane.trouble().expect("trouble"), None);
    }

    #[test]
    fn explicit_camera_address_cannot_retarget_to_another_live_owner_or_window() {
        let panes = StagePanes::default();
        let owned = Arc::new(StagePane::default());
        let other = Arc::new(StagePane::default());
        {
            let mut registry = panes.panes.lock().unwrap();
            registry.insert(
                "main:stage".into(),
                RegisteredPane {
                    pane: owned.clone(),
                    live_3d: false,
                },
            );
            registry.insert(
                "other:stage".into(),
                RegisteredPane {
                    pane: other.clone(),
                    live_3d: true,
                },
            );
        }
        assert!(Arc::ptr_eq(
            &panes
                .addressed_pane("main", Some("stage"))
                .unwrap()
                .unwrap(),
            &owned
        ));
        assert!(Arc::ptr_eq(
            &panes
                .addressed_pane("other", Some("stage"))
                .unwrap()
                .unwrap(),
            &other
        ));
        assert!(Arc::ptr_eq(
            &panes.addressed_pane("main", None).unwrap().unwrap(),
            &other
        ));
        panes.panes.lock().unwrap().remove("main:stage");
        assert!(
            panes
                .addressed_pane("main", Some("stage"))
                .unwrap()
                .is_none(),
            "retired explicit target cannot fall back to another pane"
        );
    }

    #[test]
    fn one_live_3d_owner_does_not_block_independent_2d_panes() {
        let panes = StagePanes::default();
        panes.panes.lock().expect("registry").insert(
            "main:stage".into(),
            RegisteredPane {
                pane: Arc::new(StagePane::default()),
                live_3d: true,
            },
        );

        assert!(panes.ensure_owner_available("main:stage", true).is_ok());
        assert!(
            panes
                .ensure_owner_available("screen-plan:stage", false)
                .is_ok()
        );
        assert!(
            panes
                .ensure_owner_available("screen-viz:stage", true)
                .unwrap_err()
                .contains("Only one live 3D Stage")
        );
    }
}
