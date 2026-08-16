//! Drawing the desk's Stage pane, with no window of this process's own.
//!
//! The desk-opened visualizer is a window: winit runs it, it presents to a swapchain, and it draws
//! its own overlays and status. This is the other shape the same renderer takes. The desk owns the
//! window, the pane is a rectangle inside it that the web layout decides, and everything around the
//! picture — menus, dialogs, the sheet — is the desk's web interface drawn above it. So there is no
//! window to create here and no application chrome to draw. Fixture labels remain part of the
//! picture itself and use the renderer's small screen-space overlay.
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
use viz_render::{Overlay, Renderer, ResolvedCamera};
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
    let mut state = match PaneState::new(&embedding) {
        Ok(state) => state,
        Err(detail) => {
            source.report(&detail);
            return Ok(());
        }
    };
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
    let mut rig = embedded_desk_provider(&embedding, epoch);

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
        let mut moved = false;
        for input in source.take_input() {
            if let Some((fixture, additive)) = state.apply(input) {
                source.send_picked(fixture, additive);
            } else {
                moved = true;
            }
        }
        // Only when it changed, and only from here: the camera the desk shows on its encoders is
        // the one the renderer is actually using, however it came to be there.
        if moved {
            source.send_camera(&state.view.camera);
        }
        // What the operator has selected, which the renderer draws and never decides.
        if let Some(fixtures) = source.selection() {
            state.values.selected_fixtures = fixtures
                .iter()
                .filter_map(|id| uuid::Uuid::parse_str(id).ok())
                .collect();
        }
        // The picture settings are the renderer's own, and the desk sends them rather than
        // applying them: it is not the one drawing this.
        if let Some(viz_helper::protocol::ToHelper::Picture {
            atmosphere,
            ambient,
            quality,
            exposure,
            laser_brightness,
            lamp_fog_cloudiness,
            lamp_fog_turbulence,
            laser_fog_cloudiness,
            laser_fog_turbulence,
            show_labels,
            floor_grid,
            show_beam_guides,
            background,
            mode,
            follow_preload,
        }) = source.picture()
        {
            state.values.atmosphere = viz_scene::AtmospherePreference {
                amount: atmosphere.clamp(0.0, 1.0),
            }
            .resolve();
            state.view.ambient = ambient.clamp(0.0, 2.0);
            state.view.exposure = exposure.clamp(0.05, 4.0);
            state.view.laser_brightness = laser_brightness.clamp(0.0, 4.0);
            state.view.fog_variation = viz_scene::FogVariation {
                lamp_cloudiness: lamp_fog_cloudiness.clamp(0.0, 1.0),
                lamp_turbulence: lamp_fog_turbulence.clamp(0.0, 1.0),
                laser_cloudiness: laser_fog_cloudiness.clamp(0.0, 1.0),
                laser_turbulence: laser_fog_turbulence.clamp(0.0, 1.0),
            };
            state.view.show_labels = *show_labels;
            state.view.floor_grid = *floor_grid;
            state.view.show_aim_guides = *show_beam_guides;
            state.view.background = Some(background.map(|channel| channel.clamp(0.0, 1.0)));
            // The provider is what lays the preload over the live picture, so the flag goes to it
            // rather than being kept here: the values are decoded there, and a second opinion about
            // whether to overlay would eventually disagree with the first.
            if state.following_preload != *follow_preload {
                state.following_preload = *follow_preload;
                if let Some(rig) = rig.as_mut() {
                    rig.follow_preload(*follow_preload);
                }
            }
            let was = state.view.mode;
            state.view.mode = match mode {
                viz_helper::protocol::StageViewMode::TopDown => viz_scene::ViewMode::TopDown,
                viz_helper::protocol::StageViewMode::LeftToRight => {
                    viz_scene::ViewMode::LeftToRight
                }
                viz_helper::protocol::StageViewMode::RightToLeft => {
                    viz_scene::ViewMode::RightToLeft
                }
                viz_helper::protocol::StageViewMode::FrontToBack => {
                    viz_scene::ViewMode::FrontToBack
                }
                viz_helper::protocol::StageViewMode::BackToFront => {
                    viz_scene::ViewMode::BackToFront
                }
                viz_helper::protocol::StageViewMode::Lines3d => viz_scene::ViewMode::Lines3d,
                viz_helper::protocol::StageViewMode::Simple3d => viz_scene::ViewMode::Simple3d,
                viz_helper::protocol::StageViewMode::Full3d => viz_scene::ViewMode::Full3d,
            };
            /*
             * Choosing a view frames the rig in it. This is the whole of what "viewed from" means:
             * an operator asking for the plan from stage left is asking to stand at stage left, not
             * to have the same picture relabelled — and the camera they had was aimed for the view
             * they were looking at, which this no longer is.
             *
             * It overrides a camera the operator moved by hand, deliberately. That camera belonged
             * to the view they have just left. Within a view their own aim is theirs and nothing
             * takes it back.
             */
            if state.view.mode != was {
                state.view.camera =
                    viz_scene::Camera::framed(state.view.mode, state.frame_bounds());
                state.camera_is_local = false;
            }
            state.view.quality = match quality {
                viz_helper::protocol::RenderQuality::Draft => viz_scene::RenderQuality::Draft,
                viz_helper::protocol::RenderQuality::Standard => viz_scene::RenderQuality::Standard,
                viz_helper::protocol::RenderQuality::High => viz_scene::RenderQuality::High,
                viz_helper::protocol::RenderQuality::Ultra => viz_scene::RenderQuality::Ultra,
            };
        }
        state.resize_for(&embedding)?;
        match state.draw(epoch, &mut source) {
            Ok(()) => {}
            // A frame that failed is not fatal on its own: a surface can be lost while a display
            // is reconfigured and be there again next frame. The desk is told, and the loop keeps
            // going rather than taking the pane away over one bad frame.
            Err(detail) => {
                source.report(&detail);
            }
        }
        std::thread::sleep(IDLE_SLEEP);
    }
}

fn embedded_desk_provider(embedding: &Embedding, epoch: Instant) -> Option<Box<DeskProvider>> {
    embedding.desk.as_ref().map(|desk| {
        Box::new(DeskProvider::start(
            DeskConnection {
                host: desk.host.clone(),
                port: desk.port,
                user: desk.user.clone(),
                target: desk.target.clone(),
                // Embedded rendering reads the desk's output instead of waiting for network DMX.
                values_from_desk_output: true,
                ..DeskConnection::default()
            },
            epoch,
        ))
    })
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
                    state.adopt_desk_view(view);
                }
                state.frame_rig_if_unaimed();
            }
            ProviderEvent::SceneDelta(scene) => {
                state.scene = *scene;
                state.frame_rig_if_unaimed();
            }
            ProviderEvent::Values(values) => adopt_provider_values(
                &mut state.values,
                *values,
                state.scene.physics_scenery.len(),
            ),
            ProviderEvent::View(view) => state.adopt_desk_view(view),
            ProviderEvent::Connection(connection) => {
                finished |= matches!(connection, viz_scene::ConnectionState::Failed { .. });
            }
            ProviderEvent::Diagnostics(_)
            | ProviderEvent::RendererSettings(_)
            | ProviderEvent::ResyncRequired { .. } => {}
        }
    }
    finished
}

/// Adopt the desk's latest output frame without discarding pane-owned selection.
///
/// Selection arrives over the embedding channel rather than the scene provider. A provider value
/// frame therefore cannot authoritatively replace it, even though both happen to share the same
/// render-state structure.
fn adopt_provider_values(current: &mut SceneValues, mut next: SceneValues, physics_bodies: usize) {
    next.selected_fixtures = std::mem::take(&mut current.selected_fixtures);
    next.retain_visual_motion_runtime_from(current);
    next.retain_physics_runtime_from(current, physics_bodies);
    *current = next;
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
    /// Whether the operator is following their preload, which the provider lays over the live
    /// picture rather than replacing it with.
    following_preload: bool,
    /// True once the operator has moved the camera here, after which the desk's own view no longer
    /// takes it back. A camera that snapped home whenever the desk re-sent its view would be a
    /// camera nobody could aim.
    camera_is_local: bool,
    redraw_gate: crate::redraw::RedrawGate,
    last_tick: Instant,
    presented_frames: u64,
    physics: crate::physics::Physics,
}

impl PaneState {
    fn new(embedding: &Embedding) -> Result<Self, String> {
        let size = pane_pixels(embedding);
        let mut renderer = Renderer::headless(size.0, size.1)?;
        renderer.set_media_content_enabled(false);
        let target = embedding
            .desk
            .as_ref()
            .map_or("embedded", |desk| desk.target.as_str());
        let preferences = crate::settings::preferences_path(&crate::settings::Options::default());
        let physics_state_path = crate::physics::Physics::state_path(
            preferences.as_deref(),
            &format!("embedded-{target}"),
        );
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
            camera_is_local: false,
            following_preload: false,
            redraw_gate: crate::redraw::RedrawGate::default(),
            last_tick: Instant::now(),
            presented_frames: 0,
            physics: crate::physics::Physics::with_state_path(physics_state_path),
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

    fn rebuild_fixture_overlay(&mut self) {
        self.overlay.clear();
        let camera = ResolvedCamera::resolve(
            &self.view.camera,
            self.view.mode,
            self.size.0 as f32 / self.size.1.max(1) as f32,
            self.scene.bounds,
        );
        crate::ui::build_fixture_labels(
            &mut self.overlay,
            &self.scene,
            &self.values,
            &camera,
            &self.view,
            self.size.0 as f32,
            self.size.1 as f32,
        );
    }

    /// Move the camera the way the operator asked.
    ///
    /// The same gestures the desk's own 3D Stage uses, so an operator switching between the two
    /// renderers does not have to learn the pane twice: drag orbits about what is being looked at,
    /// the wheel moves toward it. What the pane adds is the middle and secondary buttons, which
    /// the desk's Stage has nowhere to put.
    /// Returns what the operator pointed at, when the gesture was a pick.
    fn apply(&mut self, input: PaneInput) -> Option<(Option<String>, bool)> {
        use glam::Vec3;
        if let PaneInput::Pick { x, y, additive } = input {
            return Some((self.pick_at(x, y), additive));
        }
        /*
         * A plan is a drawing seen square on, so the gestures that would turn it out of plan are
         * not offered at all. An operator who could orbit a 2D Stage would be one keystroke away
         * from a 2D Stage that is not 2D, with no way back to square except by eye — and the whole
         * point of choosing a side is that the answer is exact.
         *
         * What is left is what a plan can honestly do: slide it about, and zoom it. Zoom on an
         * orthographic camera is the size of the window onto the world rather than the distance to
         * it; moving the camera nearer changes nothing an orthographic projection can see.
         */
        if let PaneInput::Frame = input {
            self.frame_rig();
            return None;
        }
        if self.view.mode.is_plot() {
            return self.apply_to_plan(input);
        }
        self.camera_is_local = true;
        let camera = &mut self.view.camera;
        let to_eye = camera.position - camera.target;
        let distance = to_eye.length().max(0.01);
        let forward = (-to_eye).normalize_or_zero();
        let right = forward.cross(camera.up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        // A drag across the pane is a fixed sweep whatever the pane's size, which is what makes it
        // feel the same in a small pane and a large one.
        let radians_per_point = std::f32::consts::PI / 360.0;
        // Metres per point, scaled by how far away the subject is: a drag should move the picture
        // by about as much whether the camera is on top of the rig or across the room from it.
        let metres_per_point = distance * 0.0025;

        match input {
            PaneInput::Orbit { dx, dy } => {
                let yaw = glam::Quat::from_axis_angle(up, -dx * radians_per_point);
                // Pitch is clamped by rebuilding from the rotated vector rather than by tracking
                // an angle, so no amount of dragging can put the camera through its own up axis.
                let pitched = glam::Quat::from_axis_angle(right, -dy * radians_per_point);
                let rotated = (yaw * pitched) * to_eye;
                let level = Vec3::new(rotated.x, 0.0, rotated.z).length();
                if level > distance * 0.05 {
                    camera.position = camera.target + rotated;
                }
            }
            PaneInput::Pan { dx, dy } => {
                let shift = right * (-dx * metres_per_point) + up * (dy * metres_per_point);
                camera.position += shift;
                camera.target += shift;
            }
            PaneInput::Truck { dx, dy } => {
                // The camera alone, so the view turns as it walks.
                camera.position += right * (-dx * metres_per_point) + up * (dy * metres_per_point);
            }
            PaneInput::Fly {
                forward: ahead,
                right: across,
            } => {
                // Both move, so the view walks rather than turning — the camera keeps looking the
                // way it was pointed and arrives somewhere else.
                let step = forward * ahead + right * across;
                camera.position += step;
                camera.target += step;
            }
            PaneInput::Zoom { amount } => {
                // Proportional, so each notch covers the same fraction of the remaining distance
                // and the camera approaches the subject without ever reaching it.
                let scaled = (distance * (0.9_f32).powf(amount)).clamp(0.05, 5_000.0);
                camera.position = camera.target + to_eye.normalize_or_zero() * scaled;
            }
            // Both answered above, before the camera is touched: pointing at a fixture must not
            // move the view the operator is pointing with, and framing is not a camera gesture.
            PaneInput::Pick { .. } | PaneInput::Frame => {}
            PaneInput::Place {
                x,
                y,
                z,
                pan,
                tilt,
                distance: reach,
            } => {
                if let Some(x) = x {
                    camera.position.x = x;
                }
                if let Some(y) = y {
                    camera.position.y = y;
                }
                if let Some(z) = z {
                    camera.position.z = z;
                }
                // Pan and tilt aim the camera from where it now is, so an encoder can address the
                // two independently of the three positions above it.
                let reach = reach.unwrap_or(distance).max(0.05);
                if pan.is_some() || tilt.is_some() || reach != distance {
                    let aimed = camera.target - camera.position;
                    let current_pan = aimed.x.atan2(aimed.z);
                    let current_tilt = (aimed.y / aimed.length().max(0.001)).asin();
                    let pan = pan.map_or(current_pan, f32::to_radians);
                    let tilt = tilt
                        .map_or(current_tilt, f32::to_radians)
                        .clamp(-1.55, 1.55);
                    let direction =
                        Vec3::new(pan.sin() * tilt.cos(), tilt.sin(), pan.cos() * tilt.cos());
                    camera.target = camera.position + direction * reach;
                }
            }
        }
        None
    }

    /// Take what the desk's Visualizer view says, without letting it take the pane's own picture.
    ///
    /// That view addresses the standalone Visualizers a desk is driving. This pane is inside the
    /// desk's own window and is told what to draw through its own channel, so the two would
    /// otherwise fight over the same fields — the pane switching to the view an operator chose in
    /// its settings and being switched back a frame later by an instruction meant for a screen at
    /// the back of the room.
    fn adopt_desk_view(&mut self, view: ViewConfiguration) {
        let (mode, camera, background, floor_grid) = (
            self.view.mode,
            self.view.camera,
            self.view.background,
            self.view.floor_grid,
        );
        self.view = view;
        self.view.mode = mode;
        self.view.background = background;
        self.view.floor_grid = floor_grid;
        if self.camera_is_local {
            self.view.camera = camera;
        }
    }

    /// What a camera is framed against.
    ///
    /// The rig for the house view, which is placed relative to the front of the stage, and the whole
    /// scene for the plans, which have to get the room in frame.
    fn frame_bounds(&self) -> viz_scene::Aabb {
        if self.view.mode.is_orthographic() {
            self.scene.bounds
        } else {
            self.scene.rig_bounds()
        }
    }

    /// Put the camera where the view says it belongs, framing the whole rig.
    ///
    /// This is what Reset view does, and it deliberately gives the camera back: an operator asking
    /// to reset is asking to undo their own aiming, so the pane stops counting as locally aimed.
    fn frame_rig(&mut self) {
        self.camera_is_local = false;
        self.view.camera = viz_scene::Camera::framed(self.view.mode, self.frame_bounds());
    }

    /// Frame the rig, while the operator has not aimed this pane themselves.
    ///
    /// The picture settings and the rig arrive on separate channels and in no fixed order, so a
    /// view chosen before the show loaded was framed against an empty stage. This puts the rig in
    /// frame as soon as there is one, and stops the moment the operator takes the camera.
    fn frame_rig_if_unaimed(&mut self) {
        if self.camera_is_local || self.scene.bounds.is_empty() {
            return;
        }
        self.view.camera = viz_scene::Camera::framed(self.view.mode, self.frame_bounds());
    }

    /// The gestures a plan view has, which are sliding it about and zooming it.
    ///
    /// Orbit, truck and fly are dropped rather than reinterpreted: there is no sensible plan-view
    /// meaning for turning the camera, and quietly turning one gesture into another is worse than
    /// the gesture doing nothing. A pick still answers, because pointing at a fixture on a plan is
    /// exactly what a plan is for.
    fn apply_to_plan(&mut self, input: PaneInput) -> Option<(Option<String>, bool)> {
        let camera = &mut self.view.camera;
        // How much of the world one point of drag covers, from the size of the window onto it.
        // A drag moves the picture by the same amount on screen however far the plan is zoomed.
        let metres_per_point = camera.orthographic_size.max(0.1) * 0.0035;
        let (right, up) = viz_render::CameraControl::from_camera(camera).page_axes(self.view.mode);
        match input {
            PaneInput::Pan { dx, dy } | PaneInput::Truck { dx, dy } => {
                self.camera_is_local = true;
                let shift = right * (-dx * metres_per_point) + up * (dy * metres_per_point);
                camera.position += shift;
                camera.target += shift;
            }
            PaneInput::Zoom { amount } => {
                self.camera_is_local = true;
                // The half-height of what is on screen, in metres. Bounded so a plan can neither
                // be zoomed into a single fixture's paint nor out until the rig is one pixel.
                camera.orthographic_size =
                    (camera.orthographic_size * (0.9_f32).powf(amount)).clamp(0.2, 500.0);
            }
            PaneInput::Frame => self.frame_rig(),
            // A plan is square on by construction. Nothing here may turn it.
            PaneInput::Orbit { .. } | PaneInput::Fly { .. } => {}
            PaneInput::Pick { .. } | PaneInput::Place { .. } => {}
        }
        None
    }

    /// What is under a point in the pane's own logical coordinates.
    ///
    /// The renderer resolves the geometry and answers with a fixture; it does not decide what is
    /// selected. Selection is the desk's, and a renderer holding a second opinion about it would
    /// be a second answer to the one question an operator has to be able to trust.
    fn pick_at(&self, x: f32, y: f32) -> Option<String> {
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        let camera = viz_render::ResolvedCamera::resolve(
            &self.view.camera,
            self.view.mode,
            width / height.max(1.0),
            self.scene.bounds,
        );
        // The pane reports where the pointer was as a fraction of its own size, so no scale factor
        // has to agree across the channel: the renderer knows how many pixels it drew and nothing
        // else needs to.
        let ray = camera.ray_through(x * width, y * height, width, height);
        let reach = (self.view.camera.position - self.view.camera.target).length();
        match viz_render::pick(&self.scene, &ray, reach).element {
            viz_render::PickedElement::Fixture(index) => self
                .scene
                .fixtures
                .get(index)
                .map(|fixture| fixture.fixture_id.to_string()),
            // Scenery is not selectable, and an empty click clears the selection — both are the
            // same answer here: nothing.
            _ => None,
        }
    }

    fn draw(&mut self, epoch: Instant, source: &mut HelperSource) -> Result<(), String> {
        let now = Instant::now();
        let elapsed = now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(0.25);
        self.last_tick = now;
        self.values.apply_physical_motion(elapsed);
        let time = epoch.elapsed().as_secs_f32();
        self.physics.run(
            &self.scene,
            &mut self.values,
            time,
            self.view.quality,
            self.view.physics_reset_generation,
        );
        self.rebuild_fixture_overlay();
        let redraw_state = crate::redraw::RedrawState::new(
            self.scene.revision,
            0,
            &self.values,
            &self.view,
            self.size,
            &self.overlay.quads,
        );
        let persistence = viz_scene::PersistencePreference {
            decay_seconds: 0.0,
            ..viz_scene::PersistencePreference::default()
        };
        if !self.redraw_gate.should_draw(
            redraw_state,
            crate::redraw::is_time_driven(&self.values, &self.view, &persistence),
        ) {
            return Ok(());
        }
        let stats = match self.transport {
            FrameTransport::Shared => self.draw_shared(time, source),
            FrameTransport::Copy => self.draw_copy(time, source),
        }?;
        self.presented_frames = self.presented_frames.saturating_add(1);
        let presented_epoch_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_micros() as u64)
            .unwrap_or_default();
        let quality = match self.view.quality {
            viz_scene::RenderQuality::Draft => viz_helper::protocol::RenderQuality::Draft,
            viz_scene::RenderQuality::Standard => viz_helper::protocol::RenderQuality::Standard,
            viz_scene::RenderQuality::High => viz_helper::protocol::RenderQuality::High,
            viz_scene::RenderQuality::Ultra => viz_helper::protocol::RenderQuality::Ultra,
        };
        let renderer = format!(
            "{} ({}, {}x MSAA)",
            self.renderer.adapter_name(),
            self.renderer.backend(),
            self.renderer.samples()
        );
        source.send_frame_presented(
            self.presented_frames,
            self.values.frame,
            self.values.newest_input_micros,
            presented_epoch_micros,
            stats,
            renderer,
            quality,
            self.following_preload,
            self.size.0,
            self.size.1,
        );
        Ok(())
    }

    /// Draw straight into the surface the desk samples.
    fn draw_shared(
        &mut self,
        time: f32,
        source: &mut HelperSource,
    ) -> Result<viz_render::FrameStats, String> {
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
            return Ok(self.renderer.stats());
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
            .map_err(|error| error.to_string())
    }

    /// Draw into a private texture, read it back, and send the pixels.
    fn draw_copy(
        &mut self,
        time: f32,
        source: &mut HelperSource,
    ) -> Result<viz_render::FrameStats, String> {
        let image = self
            .renderer
            .capture(&self.scene, &self.values, &self.view, &self.overlay, time)
            .map_err(|error| error.to_string())?;
        source.send_frame(image.width, image.height, image.rgba);
        Ok(self.renderer.stats())
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

    #[test]
    fn live_value_frames_preserve_the_authoritative_pane_selection() {
        let selected = uuid::Uuid::new_v4();
        let mut current = SceneValues {
            selected_fixtures: [selected].into_iter().collect(),
            frame: 7,
            ..SceneValues::default()
        };
        let next = SceneValues {
            frame: 8,
            ..SceneValues::default()
        };

        adopt_provider_values(&mut current, next, 0);

        assert_eq!(current.frame, 8, "the new output frame is still adopted");
        assert_eq!(current.selected_fixtures, [selected].into_iter().collect());
    }

    #[test]
    fn live_value_frames_do_not_reset_motion_advanced_by_the_renderer() {
        let target = viz_scene::PhysicalMotionTarget::Position {
            degrees: 54.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 360.0,
        };
        let mut current = SceneValues::default();
        current.resize(1);
        current.emitters[0].pan_motion.position_degrees = 12.0;
        current.emitters[0].pan_motion.velocity_degrees_per_second = 40.0;
        let mut next = SceneValues::default();
        next.resize(1);
        next.emitters[0].pan_motion.target = Some(target);

        adopt_provider_values(&mut current, next, 0);

        assert_eq!(current.emitters[0].pan_motion.position_degrees, 12.0);
        assert_eq!(
            current.emitters[0].pan_motion.velocity_degrees_per_second,
            40.0
        );
        assert_eq!(current.emitters[0].pan_motion.target, Some(target));
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

    fn aimed() -> Embedding {
        embedding(640.0, 360.0, 1.0)
    }

    /// The camera has to end up somewhere an operator would predict. These are the properties that
    /// hold whatever the arithmetic is: orbiting keeps the distance, panning keeps the direction,
    /// trucking moves the eye and not the subject, and zoom never arrives at what it approaches.
    #[test]
    fn dragging_moves_the_camera_the_way_the_gesture_says() {
        let Ok(mut state) = PaneState::new(&aimed()) else {
            eprintln!("no GPU here; skipping the camera model");
            return;
        };
        let start = state.view.camera;
        let reach = |camera: &viz_scene::Camera| (camera.position - camera.target).length();

        state.apply(PaneInput::Orbit { dx: 40.0, dy: 0.0 });
        assert!(
            (reach(&state.view.camera) - reach(&start)).abs() < 0.001,
            "orbiting turns around the subject rather than approaching it"
        );
        assert!(state.view.camera.position.distance(start.position) > 0.01);
        assert_eq!(state.view.camera.target, start.target);

        let before = state.view.camera;
        state.apply(PaneInput::Pan { dx: 25.0, dy: 10.0 });
        let moved = state.view.camera;
        assert!(
            (moved.position - moved.target)
                .normalize()
                .distance((before.position - before.target).normalize())
                < 0.001,
            "panning slides the picture without turning it"
        );
        assert!(moved.target.distance(before.target) > 0.001);

        let before = state.view.camera;
        state.apply(PaneInput::Truck { dx: 25.0, dy: 0.0 });
        assert_eq!(
            state.view.camera.target, before.target,
            "trucking walks the camera and leaves the subject alone"
        );
        assert!(state.view.camera.position.distance(before.position) > 0.001);

        let before = state.view.camera;
        state.apply(PaneInput::Fly {
            forward: 2.0,
            right: 0.0,
        });
        let flown = state.view.camera;
        assert!(
            (flown.position - flown.target)
                .normalize()
                .distance((before.position - before.target).normalize())
                < 0.001,
            "flying walks the view without turning it"
        );
        assert!(
            (flown.position.distance(before.position) - 2.0).abs() < 0.01,
            "and by the metres it was asked for"
        );

        let before = reach(&state.view.camera);
        state.apply(PaneInput::Zoom { amount: 3.0 });
        let after = reach(&state.view.camera);
        assert!(after < before, "a positive notch moves in");
        assert!(after > 0.0, "and never arrives");
    }

    /// The camera an operator aimed is theirs. A desk re-sending its own view — which it does on
    /// every reconnection — must not take it back.
    #[test]
    fn aiming_the_camera_makes_it_the_operators() {
        let Ok(mut state) = PaneState::new(&aimed()) else {
            return;
        };
        assert!(!state.camera_is_local);
        state.apply(PaneInput::Orbit { dx: 10.0, dy: 0.0 });
        assert!(state.camera_is_local);
    }

    /// An encoder addresses the camera by number, and each number must be settable alone.
    #[test]
    fn the_camera_can_be_placed_one_number_at_a_time() {
        let Ok(mut state) = PaneState::new(&aimed()) else {
            return;
        };
        state.apply(PaneInput::Place {
            x: Some(4.0),
            y: None,
            z: None,
            pan: None,
            tilt: None,
            distance: None,
        });
        assert!((state.view.camera.position.x - 4.0).abs() < 0.001);

        let height = state.view.camera.position.y;
        state.apply(PaneInput::Place {
            x: None,
            y: None,
            z: None,
            pan: Some(90.0),
            tilt: Some(0.0),
            distance: Some(10.0),
        });
        assert!(
            (state.view.camera.position.y - height).abs() < 0.001,
            "aiming does not move the camera"
        );
        let aimed_at = state.view.camera.target - state.view.camera.position;
        assert!((aimed_at.length() - 10.0).abs() < 0.01);
        assert!(aimed_at.y.abs() < 0.01, "a zero tilt looks level");
        assert!(aimed_at.x > 0.0, "ninety degrees of pan looks along +X");
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
