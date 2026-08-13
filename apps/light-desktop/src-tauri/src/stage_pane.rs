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
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use viz_helper::handshake::{HelperIdentity, greet_helper};
use viz_helper::pane::PaneRect;
use viz_helper::protocol::{
    DeskEndpoint, FrameTransport, FromHelper, PaneInput, SharedSurfaceHandle, ToHelper, decode,
    encode, supported_transports,
};
use viz_helper::{SupervisedHelper, framing};

/// What the reading thread hands the desk.
enum FromRenderer {
    /// A surface both processes address, replacing whatever was being drawn.
    Surface {
        handle: SharedSurfaceHandle,
        width: u32,
        height: u32,
    },
    /// A frame that came through the pipe.
    Frame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Presented(FrameTelemetry),
    /// Where the camera now is.
    Camera([f32; 6]),
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

/// The embedded Stage, while there is one.
#[derive(Default)]
pub(crate) struct StagePane {
    inner: Mutex<Option<Running>>,
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
    /// Whether anything is embedded, readable without taking the lock.
    ///
    /// The frame pump asks this before posting anything to the main thread. A desk with no pane —
    /// which is most desks, most of the time — must not put work on the thread the interface
    /// paints on, and posting a task every few milliseconds from startup competes with the
    /// webview's own first paint.
    drawing: std::sync::atomic::AtomicBool,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrameTelemetry {
    pane_id: String,
    sequence: u64,
    source_frame: u64,
    source_input_epoch_micros: u64,
    presented_epoch_micros: u64,
    cpu_micros: u64,
    acquire_micros: u64,
    gpu_micros: Option<u64>,
    instances: u32,
    draw_calls: u32,
    degraded: bool,
    renderer: String,
    quality: &'static str,
    follow_preload: bool,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub(crate) struct StagePanes {
    panes: Mutex<HashMap<String, RegisteredPane>>,
}

struct RegisteredPane {
    pane: Arc<StagePane>,
    live_3d: bool,
}

impl StagePanes {
    fn take_benchmark_samples(&self) -> Result<Vec<FrameTelemetry>, String> {
        let panes = self.panes.lock().map_err(|_| "the Stage pane registry")?;
        let mut result = Vec::new();
        for (pane_id, registered) in panes.iter() {
            let mut samples = registered
                .pane
                .telemetry
                .lock()
                .map_err(|_| "the Stage pane benchmark telemetry")?;
            for mut sample in samples.drain(..) {
                sample.pane_id.clone_from(pane_id);
                result.push(sample);
            }
        }
        Ok(result)
    }
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
        user: String,
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
            user,
            format!("stage-pane:{key}"),
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
        pane.close()
    }

    fn drawing_panes(&self) -> Vec<Arc<StagePane>> {
        self.panes.lock().map_or_else(
            |_| Vec::new(),
            |panes| {
                panes
                    .values()
                    .filter(|registered| registered.pane.is_drawing())
                    .map(|registered| Arc::clone(&registered.pane))
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
    helper: SupervisedHelper,
    /// Where the renderer hands over each surface, while the transport shares one.
    #[cfg(target_os = "macos")]
    rendezvous: Option<viz_surface::rendezvous::Rendezvous>,
    to_helper: Option<std::process::ChildStdin>,
    inbox: Receiver<FromRenderer>,
    compositor: StageCompositor,
    identity: HelperIdentity,
    transport: FrameTransport,
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
        user: String,
        target: String,
    ) -> Result<(), String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        self.close()?;
        self.telemetry
            .lock()
            .map_err(|_| "the Stage pane benchmark telemetry")?
            .clear();
        let program = match crate::visualizer::helper_binary() {
            Ok(program) => program,
            Err(error) => {
                eprintln!("stage pane: no renderer beside the application: {error}");
                return Err(error);
            }
        };
        // `--embed` implies `--helper` and opens no window: the desk owns the window, and this
        // process only ever draws the rectangle it is given.
        let mut helper = SupervisedHelper::new(program, vec!["--embed".to_owned()]);
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

        // What both sides can do decides what happens next — including nothing at all, which is
        // the desk keeping its web renderer rather than an error anybody has to handle.
        let Some(transport) = identity.embeddable_with(&desk_transports(surface_service.is_some()))
        else {
            helper.stop();
            eprintln!(
                "stage pane: no shared transport; desk offers {:?}, renderer offers {:?}",
                desk_transports(surface_service.is_some()),
                identity.transports
            );
            return Err(
                "this renderer and this desk share no way to move a picture between them"
                    .to_owned(),
            );
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

        let (outbox, inbox) = channel();
        std::thread::Builder::new()
            .name("stage-pane-channel".into())
            .spawn(move || read_renderer(from_helper, &outbox))
            .map_err(|error| error.to_string())?;

        let mut running = Running {
            helper,
            #[cfg(target_os = "macos")]
            rendezvous,
            to_helper: Some(to_helper),
            inbox,
            compositor,
            identity,
            transport,
        };
        running.compositor.set_pane(pane, scale);
        running.send(&ToHelper::Embed {
            pane,
            scale,
            transport,
            // The rig comes from the desk's own server, which this process is the one that knows
            // how to find. Nothing about the show travels down the channel.
            desk: Some(DeskEndpoint {
                host: crate::server::address().ip().to_string(),
                port: crate::server::address().port(),
                user,
                // Named so a desk driving more than one renderer keeps a view per renderer, and so
                // this pane's camera is not the standalone visualizer's.
                target,
            }),
            surface_service,
        });
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
        running.compositor.resize(surface_size.0, surface_size.1);
        running.compositor.set_pane(pane, scale);
        running.send(&ToHelper::Pane { pane });
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

    /// Send the operator's picture settings on to the renderer, which owns this picture.
    pub(crate) fn send_picture(&self, picture: Picture) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.send(&ToHelper::Picture {
                atmosphere: picture.atmosphere,
                ambient: picture.ambient,
                quality: match picture.quality.as_str() {
                    "draft" => viz_helper::protocol::RenderQuality::Draft,
                    "standard" => viz_helper::protocol::RenderQuality::Standard,
                    "ultra" => viz_helper::protocol::RenderQuality::Ultra,
                    _ => viz_helper::protocol::RenderQuality::High,
                },
                exposure: picture.exposure,
                laser_brightness: picture.laser_brightness,
                lamp_fog_cloudiness: picture.lamp_fog_cloudiness,
                lamp_fog_turbulence: picture.lamp_fog_turbulence,
                laser_fog_cloudiness: picture.laser_fog_cloudiness,
                laser_fog_turbulence: picture.laser_fog_turbulence,
                show_labels: picture.show_labels,
                floor_grid: picture.floor_grid,
                background: picture.background,
                mode: match picture.mode.as_str() {
                    "top_down" => viz_helper::protocol::StageViewMode::TopDown,
                    "left_to_right" => viz_helper::protocol::StageViewMode::LeftToRight,
                    "right_to_left" => viz_helper::protocol::StageViewMode::RightToLeft,
                    "front_to_back" => viz_helper::protocol::StageViewMode::FrontToBack,
                    "back_to_front" => viz_helper::protocol::StageViewMode::BackToFront,
                    "lines_3d" => viz_helper::protocol::StageViewMode::Lines3d,
                    "simple_3d" => viz_helper::protocol::StageViewMode::Simple3d,
                    _ => viz_helper::protocol::StageViewMode::Full3d,
                },
                follow_preload: picture.follow_preload,
            });
        }
        Ok(())
    }

    /// Take what has arrived and draw. Driven from the desk's own loop, on the thread that owns
    /// the window, because that is where a surface may be presented.
    pub(crate) fn tick(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        let Some(running) = guard.as_mut() else {
            return Ok(());
        };
        let mut trouble = None;
        let mut ended = false;
        let mut picked: Vec<(Option<String>, bool)> = Vec::new();
        let mut latest_camera = None;
        loop {
            match running.inbox.try_recv() {
                Ok(FromRenderer::Surface {
                    handle,
                    width,
                    height,
                }) => {
                    // A surface that cannot be opened is not the end of the pane: the renderer
                    // replaces it on the next resize, and the last good picture stays up.
                    if let Err(detail) = running.adopt(handle, width, height) {
                        trouble = Some(detail);
                    }
                }
                Ok(FromRenderer::Frame {
                    width,
                    height,
                    rgba,
                }) => {
                    if let Err(detail) = running.compositor.accept_copy(width, height, &rgba) {
                        trouble = Some(detail);
                    }
                }
                Ok(FromRenderer::Presented(sample)) => {
                    let mut samples = self
                        .telemetry
                        .lock()
                        .map_err(|_| "the Stage pane benchmark telemetry")?;
                    if samples.len() >= 2_048 {
                        let overflow = samples.len() + 1 - 2_048;
                        samples.drain(..overflow);
                    }
                    samples.push(sample);
                }
                Ok(FromRenderer::Camera(camera)) => latest_camera = Some(camera),
                Ok(FromRenderer::Picked { fixture, additive }) => {
                    // Straight to the interface, which owns what is selected. The desk is only
                    // carrying the answer between the renderer that resolved the geometry and the
                    // one place allowed to decide what it means.
                    picked.push((fixture, additive));
                }
                Ok(FromRenderer::Trouble(detail)) => trouble = Some(detail),
                Ok(FromRenderer::Finished(detail)) => {
                    trouble = Some(detail);
                    ended = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
            }
        }
        if ended {
            // The renderer has gone. The supervisor would start another, but a fresh process has
            // not been greeted and has never been told where the pane is, so it would draw
            // nothing and the desk would hold a still picture of a rig that has since moved.
            // Taking the pane down instead is what lets the interface see it stop and go back to
            // the web renderer — which is the fallback existing for exactly this.
            running.compositor.clear_source();
            let _ = running.compositor.draw();
            running.helper.stop();
            *guard = None;
            self.drawing
                .store(false, std::sync::atomic::Ordering::Release);
            drop(guard);
            if let Some(detail) = trouble {
                *self.trouble.lock().map_err(|_| "the Stage pane")? = Some(detail);
            }
            return Ok(());
        }
        let result = running.compositor.draw();
        drop(guard);
        if let Some(camera) = latest_camera {
            *self.camera.lock().map_err(|_| "the Stage pane")? = Some(camera);
        }
        for (fixture, additive) in picked {
            self.picked
                .lock()
                .map_err(|_| "the Stage pane")?
                .push((fixture, additive));
        }
        if let Some(detail) = trouble.or_else(|| result.clone().err()) {
            *self.trouble.lock().map_err(|_| "the Stage pane")? = Some(detail);
        }
        result
    }

    /// What went wrong most recently, if anything has.
    /// Where the renderer's camera is, if it has said.
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

    pub(crate) fn close(&self) -> Result<(), String> {
        self.drawing
            .store(false, std::sync::atomic::Ordering::Release);
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.send(&ToHelper::Shutdown);
            // One empty frame before anything is torn down.
            //
            // A swapchain keeps showing the last thing presented to it, and the interface above is
            // transparent wherever it paints nothing — so a pane closed without this leaves its
            // final Stage on the window for good. What that looks like is every built-in screen
            // drawn over a Stage that is no longer there.
            running.compositor.clear_source();
            let _ = running.compositor.draw();
            running.helper.stop();
        }
        *guard = None;
        Ok(())
    }
}

impl Running {
    /// Take the surface the renderer announced.
    ///
    /// The message only says one is waiting; the right itself came through the rendezvous, because
    /// a mach port name means nothing outside the task holding it and so cannot travel down the
    /// channel the message did.
    fn adopt(
        &mut self,
        handle: SharedSurfaceHandle,
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
                .receive(SURFACE_PATIENCE)?
                .ok_or("the renderer announced a surface it never handed over")?;
            return self.compositor.adopt_shared_port(port, width, height);
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.compositor.adopt_shared(handle, width, height)
        }
    }

    /// A renderer that has died is not an error to send to: the desk carries on either way, and
    /// the next thing it sends will find a channel again or report that it did not.
    fn send(&mut self, message: &ToHelper) {
        let Some(to_helper) = self.to_helper.as_mut() else {
            return;
        };
        let Ok(payload) = encode(message) else {
            return;
        };
        if framing::write_frame(to_helper, &payload).is_err() {
            self.to_helper = None;
        }
    }
}

/// How long to wait for a right the renderer says it has already sent.
///
/// Short: the message announcing it is written after the right, so by the time this reads there is
/// something queued. The wait exists for the scheduling gap between two processes, not for work.
const SURFACE_PATIENCE: std::time::Duration = std::time::Duration::from_millis(500);

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

/// The reading thread: decode what the renderer says until the channel ends.
fn read_renderer(mut from_helper: impl std::io::Read, outbox: &Sender<FromRenderer>) {
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
                handle,
                width,
                height,
            }) => outbox.send(FromRenderer::Surface {
                handle,
                width,
                height,
            }),
            Ok(FromHelper::Frame {
                width,
                height,
                rgba,
            }) => outbox.send(FromRenderer::Frame {
                width,
                height,
                rgba,
            }),
            Ok(FromHelper::FramePresented {
                sequence,
                source_frame,
                source_input_epoch_micros,
                presented_epoch_micros,
                cpu_micros,
                acquire_micros,
                gpu_micros,
                instances,
                draw_calls,
                degraded,
                renderer,
                quality,
                follow_preload,
                width,
                height,
            }) => outbox.send(FromRenderer::Presented(FrameTelemetry {
                pane_id: String::new(),
                sequence,
                source_frame,
                source_input_epoch_micros,
                presented_epoch_micros,
                cpu_micros,
                acquire_micros,
                gpu_micros,
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
                follow_preload,
                width,
                height,
            })),
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
            Ok(FromHelper::Error { detail }) => outbox.send(FromRenderer::Trouble(detail)),
            Ok(FromHelper::Stopping { detail }) => outbox.send(FromRenderer::Finished(detail)),
            // The greeting is over; anything belonging to it arriving now is a stream out of step.
            Ok(FromHelper::Ready { .. } | FromHelper::Capabilities { .. }) => Ok(()),
            Err(detail) => outbox.send(FromRenderer::Finished(detail)),
        };
        if sent.is_err() {
            return;
        }
    }
}

/// How often the desk draws its pane, while it has one.
///
/// The surface is presented with vsync, so this only has to be at least as fast as the display;
/// the swapchain does the actual pacing and this loop blocks in `present` rather than spinning.
const TICK: std::time::Duration = std::time::Duration::from_millis(8);

/// How often the pump looks for a pane to start drawing, while there is none.
///
/// Deliberately far slower, and it costs the main thread nothing: with no pane there is nothing to
/// draw, and a task posted to the main thread every few milliseconds from startup competes with
/// the interface's own first paint on the one thread that can do either.
const IDLE: std::time::Duration = std::time::Duration::from_millis(250);

/// How long to wait for a posted frame before deciding the main thread is busy with something else.
const FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// Always yielded between frames, so the interface's own work is never starved by a renderer that
/// happens to be keeping up perfectly.
const MINIMUM_YIELD: std::time::Duration = std::time::Duration::from_millis(2);

/// Drive the pane from the thread that owns the window.
///
/// A surface may only be presented on the main thread on macOS, and the frame has to be drawn
/// whether or not the interface is doing anything, so the desk paces it rather than the web layer
/// asking for each frame across an IPC boundary.
///
/// Each frame is waited for before the next is posted. Posting on a fixed interval instead looks
/// equivalent and is not: a frame blocks in `present` until the display is ready, so posting
/// faster than the display retires them queues work on the one thread the interface also draws
/// on, without bound. What the operator sees is a beachball over a window that is busy drawing.
pub(crate) fn drive(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::Builder::new()
        .name("stage-pane".into())
        .spawn(move || {
            use tauri::Manager;
            loop {
                // Asked off the main thread, and without the lock a frame takes, so a desk with no
                // pane never reaches the interface's thread at all.
                let panes = handle.state::<StagePanes>().drawing_panes();
                if panes.is_empty() {
                    std::thread::sleep(IDLE);
                    continue;
                }
                let started = std::time::Instant::now();
                let (done, wait) = std::sync::mpsc::channel();
                let handle = handle.clone();
                let posted = handle.clone().run_on_main_thread(move || {
                    for pane in panes {
                        let _ = pane.tick();
                    }
                    // Tells this thread the main thread is free again, so the next frame is not
                    // stacked on top of one still waiting for the display.
                    let _ = done.send(());
                });
                if posted.is_err() {
                    // The application is going away, which is the only reason posting fails.
                    return;
                }
                if wait.recv_timeout(FRAME_TIMEOUT).is_err() {
                    // The main thread is busy with something else entirely. Backing off is better
                    // than adding to it.
                    std::thread::sleep(IDLE);
                    continue;
                }
                // Whatever is left of the frame after drawing it, and never nothing: a frame that
                // took longer than the interval still yields the thread before the next.
                std::thread::sleep(TICK.saturating_sub(started.elapsed()).max(MINIMUM_YIELD));
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
    user: String,
) -> Result<(), String> {
    if live_3d && visualizer.is_open()? {
        return Err(
            "Only one live 3D Stage is supported. Close the Visualizer or configure this screen as Stage - 2D."
                .to_owned(),
        );
    }
    let key = format!("{}:{pane_id}", window.label());
    panes.open(
        key,
        live_3d,
        Arc::new(window),
        (geometry.surface_width, geometry.surface_height),
        geometry.rect(),
        geometry.scale,
        user,
    )
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
pub(crate) fn close_stage_pane(
    window: tauri::WebviewWindow,
    panes: tauri::State<'_, StagePanes>,
    pane_id: String,
) -> Result<(), String> {
    panes.close(&format!("{}:{pane_id}", window.label()))
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
    let pane = if let Some(pane_id) = pane_id {
        panes.pane(&format!("{}:{pane_id}", window.label()))?
    } else {
        panes.live_3d_pane()?
    };
    let Some(pane) = pane else {
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

/// The operator's picture settings for the pane, which belong to the renderer drawing it.
///
/// Named as the web layer names them, for the reason [`PaneGeometry`] is.
#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Picture {
    atmosphere: f32,
    ambient: f32,
    quality: String,
    exposure: f32,
    laser_brightness: f32,
    lamp_fog_cloudiness: f32,
    lamp_fog_turbulence: f32,
    laser_fog_cloudiness: f32,
    laser_fog_turbulence: f32,
    show_labels: bool,
    floor_grid: bool,
    /// The colour behind the rig, linear RGB.
    background: [f32; 3],
    /// Which way the Stage is being looked at, as the web layer names it.
    mode: String,
    follow_preload: bool,
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
    panes: tauri::State<'_, StagePanes>,
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    pan: Option<f32>,
    tilt: Option<f32>,
    distance: Option<f32>,
) -> Result<(), String> {
    let Some(pane) = panes.live_3d_pane()? else {
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
    panes: tauri::State<'_, StagePanes>,
) -> Result<Vec<FrameTelemetry>, String> {
    if std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT").is_none() {
        return Ok(Vec::new());
    }
    panes.take_benchmark_samples()
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
