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
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use viz_helper::handshake::{HelperIdentity, greet_helper};
use viz_helper::pane::PaneRect;
use viz_helper::protocol::{
    FrameTransport, FromHelper, PaneInput, SharedSurfaceHandle, ToHelper, decode, encode,
    supported_transports,
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
}

struct Running {
    helper: SupervisedHelper,
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
    pub(crate) fn open<T>(
        &self,
        window: Arc<T>,
        surface_size: (u32, u32),
        pane: PaneRect,
        scale: f32,
    ) -> Result<(), String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        self.close()?;
        let program = crate::visualizer::helper_binary()?;
        // `--embed` implies `--helper` and opens no window: the desk owns the window, and this
        // process only ever draws the rectangle it is given.
        let mut helper = SupervisedHelper::new(program, vec!["--embed".to_owned()]);
        helper.start()?;
        let (mut to_helper, mut from_helper) = helper
            .take_channel()
            .ok_or("the renderer started without a channel")?;
        let identity = match greet_helper(&mut to_helper, &mut from_helper, "ToskLight Stage") {
            Ok(identity) => identity,
            Err(error) => {
                helper.stop();
                return Err(error.to_string());
            }
        };
        // What both sides can do decides what happens next — including nothing at all, which is
        // the desk keeping its web renderer rather than an error anybody has to handle.
        let Some(transport) = identity.embeddable_with(&desk_transports()) else {
            helper.stop();
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
        });
        *self.inner.lock().map_err(|_| "the Stage pane")? = Some(running);
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

    /// Take what has arrived and draw. Driven from the desk's own loop, on the thread that owns
    /// the window, because that is where a surface may be presented.
    pub(crate) fn tick(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        let Some(running) = guard.as_mut() else {
            return Ok(());
        };
        let mut trouble = None;
        let mut ended = false;
        loop {
            match running.inbox.try_recv() {
                Ok(FromRenderer::Surface {
                    handle,
                    width,
                    height,
                }) => {
                    if let Err(detail) = running.compositor.adopt_shared(handle, width, height) {
                        // A surface that cannot be opened is not the end of the pane: the renderer
                        // replaces it on the next resize, and the last good picture stays up.
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
            drop(guard);
            if let Some(detail) = trouble {
                *self.trouble.lock().map_err(|_| "the Stage pane")? = Some(detail);
            }
            return Ok(());
        }
        let result = running.compositor.draw();
        drop(guard);
        if let Some(detail) = trouble.or_else(|| result.clone().err()) {
            *self.trouble.lock().map_err(|_| "the Stage pane")? = Some(detail);
        }
        result
    }

    /// What went wrong most recently, if anything has.
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

    pub(crate) fn close(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().map_err(|_| "the Stage pane")?;
        if let Some(running) = guard.as_mut() {
            running.send(&ToHelper::Shutdown);
            running.helper.stop();
        }
        *guard = None;
        Ok(())
    }
}

impl Running {
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

/// What the desk can receive on this platform.
///
/// Read from the same place the renderer reads its own list, so the two halves cannot end up with
/// different ideas about what the platform offers.
fn desk_transports() -> Vec<FrameTransport> {
    let mut transports = supported_transports();
    // The renderer's list is what its build can produce; the desk's is what this build can take.
    // They are the same list except where the desk has no shared-surface support compiled in at
    // all, which is what this removes rather than assumes.
    if !viz_surface::is_supported() {
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

/// How often the desk draws its pane.
///
/// The surface is presented with vsync, so this only has to be at least as fast as the display;
/// the swapchain does the actual pacing and this loop blocks in `present` rather than spinning.
const TICK: std::time::Duration = std::time::Duration::from_millis(8);

/// Drive the pane from the thread that owns the window.
///
/// A surface may only be presented on the main thread on macOS, and the frame has to be drawn
/// whether or not the interface is doing anything, so the desk paces it rather than the web layer
/// asking for each frame across an IPC boundary.
pub(crate) fn drive(app: &tauri::AppHandle) {
    let handle = app.clone();
    std::thread::Builder::new()
        .name("stage-pane".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TICK);
                let handle = handle.clone();
                let posted = handle.clone().run_on_main_thread(move || {
                    use tauri::Manager;
                    let pane = handle.state::<StagePane>();
                    let _ = pane.tick();
                });
                if posted.is_err() {
                    // The application is going away, which is the only reason posting fails.
                    return;
                }
            }
        })
        .ok();
}

/// Whether the desk can draw the Stage itself, or has to keep using its web renderer.
#[tauri::command]
pub(crate) fn stage_pane_available() -> bool {
    embedding_possible()
}

/// The pane the interface laid out, in the points the web layout works in.
#[derive(Clone, Copy, serde::Deserialize)]
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
    app: tauri::AppHandle,
    pane: tauri::State<'_, StagePane>,
    geometry: PaneGeometry,
) -> Result<(), String> {
    use tauri::Manager;
    let window = app
        .get_window("main")
        .ok_or("the desk's window is not open")?;
    pane.open(
        Arc::new(window),
        (geometry.surface_width, geometry.surface_height),
        geometry.rect(),
        geometry.scale,
    )
}

#[tauri::command]
pub(crate) fn set_stage_pane(
    pane: tauri::State<'_, StagePane>,
    geometry: PaneGeometry,
) -> Result<(), String> {
    pane.set_pane(
        (geometry.surface_width, geometry.surface_height),
        geometry.rect(),
        geometry.scale,
    )
}

#[tauri::command]
pub(crate) fn close_stage_pane(pane: tauri::State<'_, StagePane>) -> Result<(), String> {
    pane.close()
}

/// What the operator did over the pane, already coalesced by the web layer into one step.
#[tauri::command]
pub(crate) fn stage_pane_input(
    pane: tauri::State<'_, StagePane>,
    gesture: String,
    x: f32,
    y: f32,
) -> Result<(), String> {
    let input = match gesture.as_str() {
        "orbit" => PaneInput::Orbit { dx: x, dy: y },
        "pan" => PaneInput::Pan { dx: x, dy: y },
        "zoom" => PaneInput::Zoom { amount: y },
        other => return Err(format!("no such pane gesture: {other}")),
    };
    pane.send_input(input)
}

/// What is drawing the pane and how it reaches the desk, plus whatever last went wrong.
///
/// One call rather than two so the interface cannot show a renderer and a failure that were true
/// at different moments.
#[tauri::command]
pub(crate) fn stage_pane_status(
    pane: tauri::State<'_, StagePane>,
) -> Result<(Option<String>, Option<String>), String> {
    Ok((pane.description()?, pane.trouble()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desk must never announce a transport it has no code to receive. Getting this wrong is a
    /// negotiation that succeeds and a pane that stays black.
    #[test]
    fn the_desk_offers_only_what_it_can_receive() {
        let transports = desk_transports();
        assert!(
            transports.contains(&FrameTransport::Copy),
            "copying needs nothing from the platform and is always available"
        );
        assert_eq!(
            transports.contains(&FrameTransport::Shared),
            viz_surface::is_supported(),
            "a shared transport is offered exactly where one is compiled in"
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
}
