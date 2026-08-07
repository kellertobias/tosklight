//! Drawing the desk's Stage pane, with no window of this process's own.
//!
//! The desk-opened visualizer is a window: winit runs it, it presents to a swapchain, and it draws
//! its own overlays and status. This is the other shape the same renderer takes. The desk owns the
//! window, the pane is a rectangle inside it that the web layout decides, and everything around the
//! picture — menus, dialogs, the sheet — is the desk's web interface drawn above it. So there is no
//! window to create here, no event loop to run, and no overlay to draw: the desk's chrome is the
//! chrome.
//!
//! Being a separate process is still the point. A GPU driver can end a process, and the desk's
//! Programmer, playback and output engine must not be in that address space when it does.
//!
//! Two transports carry the result, chosen by the desk from what both sides announced:
//!
//! * [`FrameTransport::Shared`] — the helper draws into a surface the desk can also address, and
//!   nothing is copied. This is the intended path wherever the platform has one.
//! * [`FrameTransport::Copy`] — the pane is read back and sent through the pipe. Portable, and it
//!   costs a readback and a re-upload every frame, so it is the fallback rather than the plan.
//!
//! If neither is available the desk never starts this at all and keeps drawing the Stage with its
//! own web renderer, which is why nothing here has to cope with having no transport.

use crate::helper_source::{Embedding, HelperSource};
use std::time::{Duration, Instant};
use viz_desk::{DeskConnection, DeskProvider};
use viz_helper::protocol::{FrameTransport, PaneInput};
use viz_render::{Overlay, Renderer};
use viz_scene::{ProviderEvent, Scene, SceneProvider, SceneValues, ViewConfiguration};

/// How long to wait for the desk to say where its pane is before giving up.
///
/// The desk sends `Embed` immediately after the handshake, so this only elapses when the desk died
/// between starting this process and instructing it. Waiting forever would leave an invisible
/// process alive after the application that owns it has gone.
const EMBED_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest a frame may take before the loop yields, so a stalled GPU cannot spin a core.
const IDLE_SLEEP: Duration = Duration::from_millis(2);

/// Run the embedded pane until the desk's channel ends.
pub fn run(mut source: HelperSource) -> Result<(), String> {
    let embedding = wait_for_embedding(&mut source)?;
    let mut state = PaneState::new(&embedding)?;
    let epoch = Instant::now();

    /*
     * The rig comes from the desk's own server, through the same provider the standalone
     * visualizer uses. The channel carries what only the desk knows — where the pane is, how to
     * hand a picture back, what the operator did over it — and not the show, which the server
     * already serves to anything that asks.
     *
     * Without an endpoint the pane draws an empty stage, which is what a desk too old to send one
     * would produce and is a picture rather than a failure.
     */
    let mut rig: Option<Box<DeskProvider>> = embedding.desk.as_ref().map(|desk| {
        Box::new(DeskProvider::start(
            DeskConnection {
                host: desk.host.clone(),
                port: desk.port,
                user: desk.user.clone(),
                target: desk.target.clone(),
                // This renderer is inside the desk's own window, so it reads the desk's output
                // rather than waiting for it on a network that may carry nothing at all.
                values_from_desk_output: true,
                ..DeskConnection::default()
            },
            epoch,
        ))
    });

    loop {
        if let Some(rig) = rig.as_mut() {
            let events = rig.poll();
            drain(events, &mut state);
        }
        let events = source.poll();
        if drain(events, &mut state) {
            // The desk's channel ended. Its window has gone or it asked this to stop; either way
            // there is nothing left to draw for.
            return Ok(());
        }
        let Some(embedding) = source.embedding() else {
            return Ok(());
        };
        for input in source.take_input() {
            state.apply(input);
        }
        // The picture settings are the renderer's own, and the desk sends them rather than
        // applying them: it is not the one drawing this.
        if let Some((atmosphere, ambient)) = source.picture() {
            state.values.atmosphere = viz_scene::AtmospherePreference {
                amount: atmosphere.clamp(0.0, 1.0),
            }
            .resolve();
            state.view.ambient = ambient.clamp(0.0, 2.0);
        }
        state.resize_for(&embedding)?;
        match state.draw(epoch, &mut source) {
            Ok(()) => {}
            // A frame that failed is not fatal on its own: a surface can be lost while a display
            // is reconfigured and be there again next frame. The desk is told, and the loop keeps
            // going rather than taking the pane away over one bad frame.
            Err(detail) => source.report(&detail),
        }
        std::thread::sleep(IDLE_SLEEP);
    }
}

/// Block until the desk says where its pane is.
fn wait_for_embedding(source: &mut HelperSource) -> Result<Embedding, String> {
    let deadline = Instant::now() + EMBED_TIMEOUT;
    loop {
        let _ = source.poll();
        if let Some(embedding) = source.embedding() {
            return Ok(embedding);
        }
        if source.is_finished() {
            return Err("the desk's channel ended before it asked for a pane".to_owned());
        }
        if Instant::now() >= deadline {
            return Err("the desk never said where its Stage pane is".to_owned());
        }
        std::thread::sleep(IDLE_SLEEP);
    }
}

/// Apply what the provider produced. Returns true once the channel has ended.
fn drain(events: Vec<ProviderEvent>, state: &mut PaneState) -> bool {
    let mut finished = false;
    for event in events {
        match event {
            ProviderEvent::Snapshot { scene, view } => {
                state.scene = *scene;
                if let Some(view) = view {
                    state.view = view;
                }
            }
            ProviderEvent::SceneDelta(scene) => state.scene = *scene,
            ProviderEvent::Values(values) => state.values = *values,
            ProviderEvent::View(view) => state.view = view,
            ProviderEvent::Connection(connection) => {
                finished |= matches!(connection, viz_scene::ConnectionState::Failed { .. });
            }
            ProviderEvent::Diagnostics(_) | ProviderEvent::ResyncRequired { .. } => {}
        }
    }
    finished
}

/// Everything the pane draws with, and the texture it draws into.
struct PaneState {
    renderer: Renderer,
    scene: Scene,
    values: SceneValues,
    view: ViewConfiguration,
    overlay: Overlay,
    transport: FrameTransport,
    /// Physical pixel size of the pane, which the desk's layout and display scale decide.
    size: (u32, u32),
    /// The surface handed to the desk, while the transport is a shared one.
    shared: Option<viz_surface::SharedSurface>,
    /// Where to hand the desk each surface, while the transport shares one.
    surface_service: Option<String>,
    /// Camera intent accumulated from forwarded input.
    orbit: (f32, f32),
    zoom: f32,
}

impl PaneState {
    fn new(embedding: &Embedding) -> Result<Self, String> {
        let size = pane_pixels(embedding);
        let renderer = Renderer::headless(size.0, size.1)?;
        Ok(Self {
            renderer,
            scene: Scene::default(),
            values: SceneValues::default(),
            view: ViewConfiguration::default(),
            overlay: Overlay::default(),
            transport: embedding.transport,
            size,
            shared: None,
            surface_service: embedding.surface_service.clone(),
            orbit: (0.0, 0.0),
            zoom: 0.0,
        })
    }

    /// Follow the pane the desk's layout reports.
    ///
    /// A shared surface is a fixed size, so a resize replaces it and the desk is told — which is
    /// why the handle is announced on every change rather than once at the start.
    fn resize_for(&mut self, embedding: &Embedding) -> Result<(), String> {
        let size = pane_pixels(embedding);
        if size == self.size {
            return Ok(());
        }
        self.size = size;
        self.renderer.resize(size.0, size.1);
        // A shared surface is a fixed size, so the next frame builds and announces a new one.
        self.shared = None;
        Ok(())
    }

    fn apply(&mut self, input: PaneInput) {
        match input {
            PaneInput::Orbit { dx, dy } => {
                self.orbit.0 += dx;
                self.orbit.1 += dy;
            }
            // Panning the Stage is the same gesture applied to the camera target, which the
            // camera model expresses as an orbit around a moved centre.
            PaneInput::Pan { dx, dy } => {
                self.orbit.0 += dx * 0.25;
                self.orbit.1 += dy * 0.25;
            }
            PaneInput::Zoom { amount } => self.zoom += amount,
        }
    }

    fn draw(&mut self, epoch: Instant, source: &mut HelperSource) -> Result<(), String> {
        let time = epoch.elapsed().as_secs_f32();
        match self.transport {
            FrameTransport::Shared => self.draw_shared(time, source),
            FrameTransport::Copy => self.draw_copy(time, source),
        }
    }

    /// Draw straight into the surface the desk samples.
    fn draw_shared(&mut self, time: f32, source: &mut HelperSource) -> Result<(), String> {
        if self.shared.is_none() {
            let surface = viz_surface::create(self.renderer.device(), self.size.0, self.size.1)
                .map_err(|error| error.to_string())?;
            // The right first, then the message that says one is waiting: the desk reads the
            // rendezvous when the message arrives, and a right already queued is one it finds
            // rather than one it waits for.
            #[cfg(target_os = "macos")]
            {
                let service = self
                    .surface_service
                    .as_deref()
                    .ok_or("the desk asked for a shared surface without saying where to send it")?;
                viz_surface::rendezvous::send_port(service, surface.mach_port())?;
            }
            source.send_surface(surface.handle(), self.size.0, self.size.1);
            self.shared = Some(surface);
        }
        let Some(surface) = self.shared.as_ref() else {
            return Ok(());
        };
        self.renderer
            .render_into(
                surface.view(),
                &self.scene,
                &self.values,
                &self.view,
                &self.overlay,
                time,
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Draw into a private texture, read it back, and send the pixels.
    fn draw_copy(&mut self, time: f32, source: &mut HelperSource) -> Result<(), String> {
        let image = self
            .renderer
            .capture(&self.scene, &self.values, &self.view, &self.overlay, time)
            .map_err(|error| error.to_string())?;
        source.send_frame(image.width, image.height, image.rgba);
        Ok(())
    }
}

/// The pane in the physical pixels a texture is measured in.
///
/// Clamped to at least one pixel: a collapsed pane is a layout mid-flight, not a reason to fail,
/// and a zero-sized texture is refused by every backend.
///
/// The copy transport is clamped further. A frame goes through the channel as RGBA, and the
/// channel refuses anything past [`viz_helper::framing::MAX_FRAME`] — so a Stage pane filling a 4K
/// display would be a frame nobody could send, every frame, silently. Rendering it slightly
/// smaller and letting the desk scale it up is a softer picture; not rendering it is a black pane.
/// A shared surface never travels through the channel and is never clamped.
fn pane_pixels(embedding: &Embedding) -> (u32, u32) {
    let scale = if embedding.scale > 0.0 {
        embedding.scale
    } else {
        1.0
    };
    let width = (embedding.pane.width * scale).round().max(1.0) as u32;
    let height = (embedding.pane.height * scale).round().max(1.0) as u32;
    let (width, height) = (width.min(16_384), height.min(16_384));
    if embedding.transport == FrameTransport::Shared {
        return (width, height);
    }
    fits_in_one_frame(width, height)
}

/// Shrink a pane, keeping its shape, until its pixels fit in one channel frame.
fn fits_in_one_frame(width: u32, height: u32) -> (u32, u32) {
    // The pixels plus the few bytes of message around them. Left generous rather than exact: the
    // point is to stay clear of the limit, not to reach it.
    const BUDGET: u64 = (viz_helper::framing::MAX_FRAME as u64) - 4_096;
    let pixels = u64::from(width) * u64::from(height) * 4;
    if pixels <= BUDGET {
        return (width, height);
    }
    let ratio = (BUDGET as f64 / pixels as f64).sqrt();
    let width = ((f64::from(width) * ratio).floor() as u32).max(1);
    let height = ((f64::from(height) * ratio).floor() as u32).max(1);
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_helper::pane::PaneRect;

    fn embedding(width: f32, height: f32, scale: f32) -> Embedding {
        Embedding {
            pane: PaneRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            scale,
            transport: FrameTransport::Copy,
            desk: None,
            surface_service: None,
        }
    }

    /// The desk works in points and the texture in pixels. Getting this wrong is a pane that is
    /// half the size it should be on every Retina display, which looks like a blurry render rather
    /// than an arithmetic mistake.
    #[test]
    fn the_pane_is_sized_in_the_display_s_own_pixels() {
        assert_eq!(pane_pixels(&embedding(640.0, 360.0, 2.0)), (1_280, 720));
        assert_eq!(pane_pixels(&embedding(640.0, 360.0, 1.0)), (640, 360));
        assert_eq!(pane_pixels(&embedding(100.5, 50.4, 1.5)), (151, 76));
    }

    /// The copy transport puts every frame through the channel, and a Stage pane filling a 4K
    /// display is larger than the channel accepts. Sending nothing at all would be a black pane
    /// on exactly the machines with the most pixels to fill.
    #[test]
    fn a_copied_pane_is_kept_inside_what_the_channel_carries() {
        let four_k = Embedding {
            pane: PaneRect {
                x: 0.0,
                y: 0.0,
                width: 3_840.0,
                height: 2_160.0,
            },
            scale: 1.0,
            transport: FrameTransport::Copy,
            desk: None,
            surface_service: None,
        };
        let (width, height) = pane_pixels(&four_k);
        assert!(
            u64::from(width) * u64::from(height) * 4 < viz_helper::framing::MAX_FRAME as u64,
            "{width}x{height} still fits in one frame"
        );
        // The shape is kept, so the desk scales it up rather than stretching it.
        let aspect = f64::from(width) / f64::from(height);
        assert!((aspect - 3_840.0 / 2_160.0).abs() < 0.01, "{aspect}");

        // A shared surface never travels through the channel, so it is never shrunk.
        let shared = Embedding {
            transport: FrameTransport::Shared,
            ..four_k.clone()
        };
        assert_eq!(pane_pixels(&shared), (3_840, 2_160));
    }

    /// A layout that has not run yet reports nothing, which must not become a zero-sized texture.
    #[test]
    fn a_collapsed_pane_still_asks_for_a_texture_a_backend_accepts() {
        assert_eq!(pane_pixels(&embedding(0.0, 0.0, 2.0)), (1, 1));
        assert_eq!(
            pane_pixels(&embedding(640.0, 360.0, 0.0)),
            (640, 360),
            "a scale nobody set is one, not a pane with no pixels"
        );
    }
}
