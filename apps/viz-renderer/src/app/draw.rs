//! Drawing one frame of the window: what is measured, what is put on screen, and what the
//! overlay says over it.
//!
//! The renderer draws the picture; this decides what to hand it — the view the operator is on,
//! the scene the session holds, and the status surface built from both — and what to do with the
//! result, including the capture and benchmark runs that exit after it.

use super::{Application, Measured, is_external_camera_target, paced_interval};
use crate::session::Session;
use crate::settings::Preferences;
use crate::ui::QuickSettings;
use crate::ui::{self, StatusModel};
use std::time::{Duration, Instant};
use viz_render::Overlay;
use viz_render::{RenderError, Renderer, ResolvedCamera};
use viz_scene::{Camera, ConnectionState};
use winit::event_loop::ActiveEventLoop;

impl Application {
    pub(super) fn draw(&mut self, event_loop: &ActiveEventLoop) {
        let Some((now, delta, width, height)) = self.begin_frame(event_loop) else {
            return;
        };
        let Some(mut session) = self.session.take() else {
            return;
        };
        self.draw_session(&mut session, event_loop, now, delta, width, height);
        self.session = Some(session);
    }

    fn draw_session(
        &mut self,
        session: &mut Session,
        event_loop: &ActiveEventLoop,
        now: Instant,
        delta: f32,
        width: f32,
        height: f32,
    ) {
        session.pump(now);
        let preserve_external_override =
            is_external_camera_target(self.options.embed, session.source_view.mode)
                && self.external_camera.local_override();
        adopt_view(
            session,
            &mut self.camera,
            &mut self.adopted_source_view,
            &mut self.requested_view,
            &mut self.camera_is_local,
            &mut self.framed_revision,
            self.options.zoom,
            preserve_external_override,
        );
        let external_camera_target =
            is_external_camera_target(self.options.embed, session.source_view.mode);
        if external_camera_target {
            let incoming = session
                .values
                .external_camera
                .as_ref()
                .map(|camera| (camera.as_camera(), camera.stale || !camera.patched));
            if let Some(camera) = self.external_camera.observe(incoming) {
                self.camera.adopt(&camera);
            }
            if self.external_camera.has_pose() {
                self.camera_is_local = true;
            }
        }
        let camera_control = if external_camera_target {
            self.external_camera.status()
        } else {
            ui::DmxCameraControlStatus::None
        };
        let mut view = session.effective_view(&self.preferences);
        if self.camera_is_local {
            view.camera = self.camera.camera(&view.camera, view.mode);
        }
        let splash = splash_state(&session.connection, session.scene.fixtures.is_empty());
        if splash.is_some() {
            view.theme = viz_scene::Theme::LightOnDark;
        }

        session.pump(Instant::now());
        let atmosphere = self.preferences.atmosphere.resolve();
        let waiting_for_dmx = session.waiting_for_dmx();
        let live_beams = live_beam_count(session);
        let selection = Self::selection_summary(session, self.selected);
        let mut values = std::mem::take(&mut session.values);
        if !self.preferences.show_selection {
            values.selected_fixtures.clear();
        }
        values.atmosphere = atmosphere;
        let since_last_frame = now
            .saturating_duration_since(self.last_persistence)
            .as_secs_f32()
            .min(0.25);
        self.last_persistence = now;
        values.apply_persistence(&self.preferences.persistence, since_last_frame);
        values.apply_physical_motion(since_last_frame);

        self.overlay.clear();
        self.hotspots.clear();
        let time = if self.options.capture.is_some() {
            self.presented_frames as f32 / 60.0
        } else {
            now.duration_since(self.epoch).as_secs_f32()
        };
        self.lasers.run(&session.scene, &mut values, time);
        let laser_fault = self.lasers.fault(&values);
        self.effects.run(&session.scene, &mut values, time);
        let effect_fault = self.effects.fault(&values);
        let notice = self
            .snapshots
            .notice()
            .map(|(message, failure)| (message.to_owned(), failure))
            .or_else(|| {
                self.lasting_failure
                    .as_ref()
                    .map(|failure| (failure.clone(), true))
            })
            .or_else(|| laser_fault.map(|fault| (fault, true)))
            .or_else(|| effect_fault.map(|fault| (fault, true)));

        let result = self.render_frame(
            session,
            event_loop,
            now,
            delta,
            width,
            height,
            &values,
            &view,
            splash,
            atmosphere,
            waiting_for_dmx,
            live_beams,
            &selection,
            notice,
            camera_control,
            time,
        );
        session.values = values;
        let Some(result) = result else {
            return;
        };
        record_frame(
            result,
            session,
            &mut self.stats,
            &mut self.presented_frames,
            &mut self.frame_interval,
            &mut self.next_frame,
        );
        if self.options.verify_only && self.presented_frames >= 2 {
            event_loop.exit();
        }
        if self.options.benchmark_seconds.is_none() {
            return;
        }
        let measured = Measured {
            latency: session.latency_percentiles(),
            dmx_hz: session.input_rate_hz(),
            fixtures: session.scene.fixtures.len(),
            emitters: session.scene.emitters.len(),
            show: session.diagnostics.show_identity.clone(),
            inputs: session.diagnostics.inputs.clone(),
            scene_ready: !session.scene.emitters.is_empty(),
        };
        self.record_benchmark_frame(&measured, delta, now, event_loop);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_frame(
        &mut self,
        session: &Session,
        event_loop: &ActiveEventLoop,
        now: Instant,
        delta: f32,
        width: f32,
        height: f32,
        values: &viz_scene::SceneValues,
        view: &viz_scene::ViewConfiguration,
        splash: Option<ui::SplashState>,
        atmosphere: viz_scene::Atmosphere,
        waiting_for_dmx: bool,
        live_beams: u32,
        selection: &Option<String>,
        notice: Option<(String, bool)>,
        camera_control: ui::DmxCameraControlStatus,
        time: f32,
    ) -> Option<Result<viz_render::FrameStats, RenderError>> {
        let renderer = self.renderer.as_mut()?;
        renderer.set_crowd_amount(self.preferences.crowd_amount);
        let model = status_model(
            session,
            &self.preferences,
            &self.stats,
            view,
            live_beams,
            atmosphere,
            selection,
            waiting_for_dmx,
            renderer,
            self.frames_per_second,
            notice,
            camera_control,
        );
        build_overlay(
            &mut self.overlay,
            &mut self.hotspots,
            &self.quick_settings,
            &self.preferences,
            &model,
            session,
            values,
            view,
            splash,
            self.preferences.overlays_hidden,
            width,
            height,
        );
        let redraw_state = crate::redraw::RedrawState::new(
            session.scene.revision,
            values,
            view,
            (width as u32, height as u32),
            &self.overlay.quads,
        );
        let time_driven =
            crate::redraw::is_time_driven(values, view, &self.preferences.persistence);
        let forced = self.options.capture.is_some()
            || self.options.verify_only
            || self.options.benchmark_seconds.is_some();
        if !forced && !self.redraw_gate.should_draw(redraw_state, time_driven) {
            self.next_frame = now + Duration::from_millis(50);
            return None;
        }
        renderer.observe_frame_interval(view.quality, (delta * 1_000_000.0) as u64);
        if let Some(path) = self.options.capture.clone()
            && self.presented_frames + 1 >= u64::from(self.options.capture_frames)
        {
            write_capture(renderer, session, values, view, &self.overlay, time, &path);
            event_loop.exit();
            return None;
        }
        Some(renderer.render(&session.scene, values, view, &self.overlay, time))
    }
}

/// What the status surface is told about this frame.
///
/// Built from the session and the operator's own settings rather than from the renderer, so a
/// plan view and a rendered view report the same numbers about the same show.
#[allow(clippy::too_many_arguments)]
fn status_model<'a>(
    session: &'a Session,
    preferences: &'a Preferences,
    stats: &'a viz_render::FrameStats,
    view: &viz_scene::ViewConfiguration,
    live_beams: u32,
    atmosphere: viz_scene::Atmosphere,
    selection: &'a Option<String>,
    waiting_for_dmx: bool,
    renderer: &viz_render::Renderer,
    frames_per_second: f32,
    notice: Option<(String, bool)>,
    camera_control: ui::DmxCameraControlStatus,
) -> StatusModel<'a> {
    StatusModel {
        connection: &session.connection,
        diagnostics: &session.diagnostics,
        universes: &session.diagnostics.universes,
        view_mode: view.mode,
        quality: view.quality,
        quality_is_local: preferences.quality_override.is_some(),
        theme: view.theme,
        fixtures: session.scene.fixtures.len(),
        emitters: session.scene.emitters.len(),
        lights: live_beams,
        frames_per_second,
        latency_p50_millis: session.latency_percentiles().0,
        latency_p95_millis: session.latency_percentiles().1,
        latency_max_millis: session.latency_percentiles().2,
        dmx_hz: session.input_rate_hz(),
        fog_percent: atmosphere.density * 100.0,
        ambient_percent: view.ambient * 100.0,
        degraded: stats.degraded,
        particle_reduction: (stats.particles_drawn < stats.particles_requested)
            .then_some((stats.particles_drawn, stats.particles_requested)),
        exposure: preferences.exposure,
        renderer: gpu_label(renderer),
        gpu_millis: stats.gpu_micros.map(|micros| micros as f32 / 1000.0),
        waiting_for_dmx,
        camera_control,
        selection: selection.clone(),
        notice,
    }
}

/// Everything drawn over the picture: the splash when there is none, fixture labels, the
/// status surface, and Quick Settings.
#[allow(clippy::too_many_arguments)]
fn build_overlay(
    overlay: &mut Overlay,
    hotspots: &mut Vec<ui::HotspotRect>,
    quick_settings: &QuickSettings,
    preferences: &Preferences,
    model: &StatusModel<'_>,
    session: &Session,
    values: &viz_scene::SceneValues,
    view: &viz_scene::ViewConfiguration,
    splash: Option<ui::SplashState>,
    overlays_hidden: bool,
    width: f32,
    height: f32,
) {
    // Nothing to draw yet: say why, on a page of its own, instead of presenting an empty
    // stage with a status line the operator has to decode.
    if let Some(splash) = splash.clone() {
        if !overlays_hidden {
            ui::build_splash(overlay, &splash, width, height);
        }
    } else if !overlays_hidden {
        let camera = ResolvedCamera::resolve(
            &view.camera,
            view.mode,
            width / height.max(1.0),
            session.scene.bounds,
        );
        ui::build_fixture_labels(
            overlay,
            &session.scene,
            values,
            &camera,
            view,
            width,
            height,
        );
        *hotspots = ui::build_status(overlay, model, width, height);
        ui::build_quick_settings(overlay, quick_settings, preferences, model, width, height);
    }
}

/// Adopt the view the source is asking for, and frame a newly loaded scene once.
///
/// An ordinary operator camera holds until the source sends an authoritative view. The dedicated
/// external camera is the exception: once local control has been latched, only its explicit
/// release action returns ownership to DMX, so a coincident source-view update cannot steal it.
#[allow(clippy::too_many_arguments)]
fn adopt_view(
    session: &mut Session,
    camera: &mut viz_render::CameraControl,
    adopted: &mut u64,
    requested: &mut Option<viz_scene::ViewMode>,
    camera_is_local: &mut bool,
    framed_revision: &mut Option<u64>,
    zoom: Option<f32>,
    preserve_local_camera: bool,
) {
    // The source has said which way to look. An ordinary operator selection holds until then; an
    // authoritative view replaces it. A latched external-camera override is released only by its
    // named operator action, so it records the source revision without moving the camera.
    if *adopted != session.source_view_epoch {
        *adopted = session.source_view_epoch;
        if let Some(mode) = requested.take() {
            // The desk's stored view is noted rather than obeyed: this launch named a view,
            // and the next thing the desk actually says will replace it.
            session.source_view.mode = mode;
            session.source_view.camera = Camera::framed(mode, session.scene.framing_bounds());
        }
        if !preserve_local_camera {
            camera.adopt(&session.source_view.camera);
            *camera_is_local = false;
        }
        *framed_revision = Some(session.scene.revision);
    }
    // Frame a newly loaded scene once. An operator camera move takes over from then on.
    if !*camera_is_local
        && !session.scene.emitters.is_empty()
        && *framed_revision != Some(session.scene.revision)
    {
        *framed_revision = Some(session.scene.revision);
        session.source_view.camera =
            Camera::framed(session.source_view.mode, session.scene.framing_bounds());
        camera.adopt(&session.source_view.camera);
        if let Some(zoom) = zoom {
            camera.zoom(zoom);
            *camera_is_local = true;
        }
    }
}

/// Write the `--capture` PNG, saying where it went or why it could not be written.
fn write_capture(
    renderer: &mut Renderer,
    session: &Session,
    values: &viz_scene::SceneValues,
    view: &viz_scene::ViewConfiguration,
    overlay: &Overlay,
    time: f32,
    path: &std::path::Path,
) {
    match renderer.capture(&session.scene, values, view, overlay, time) {
        Ok(image) => {
            let bytes = crate::png::encode_rgba(image.width, image.height, &image.rgba);
            match std::fs::write(path, bytes) {
                Ok(()) => println!(
                    "captured {}x{} to {}",
                    image.width,
                    image.height,
                    path.display()
                ),
                Err(error) => eprintln!("capture: {error}"),
            }
        }
        Err(error) => eprintln!("capture: {error}"),
    }
}

/// What a presented frame means for the pacing and for the connection.
///
/// A frame that could not be presented is not a failure of the show: a surface that is briefly
/// unavailable is skipped, and only a real device error is reported as one.
fn record_frame(
    result: Result<viz_render::FrameStats, RenderError>,
    session: &mut Session,
    stats: &mut viz_render::FrameStats,
    presented_frames: &mut u64,
    frame_interval: &mut Duration,
    next_frame: &mut Instant,
) {
    match result {
        Ok(frame) => {
            *stats = frame;
            *presented_frames += 1;
            let presented = Instant::now();
            session.record_presented(presented);
            *frame_interval =
                paced_interval(*frame_interval, Duration::from_micros(frame.acquire_micros));
            // The next frame is due a pace after this one was presented, not after it was
            // started, so the wait lands between the refreshes rather than inside one.
            *next_frame = presented + *frame_interval;
        }
        Err(RenderError::SkipFrame) => {}
        Err(error) => {
            session.connection = ConnectionState::Failed {
                boundary: "renderer".into(),
                detail: error.to_string(),
            };
        }
    }
}

impl Application {
    /// Fold this frame into the `--benchmark` run, and print the report when it is done.
    fn record_benchmark_frame(
        &mut self,
        measured: &Measured,
        delta: f32,
        now: Instant,
        event_loop: &ActiveEventLoop,
    ) {
        if self.benchmark.is_empty() {
            // Give the connection a moment to load the scene before measuring.
            if now.duration_since(self.epoch).as_secs_f32() > 2.0 && measured.scene_ready {
                self.start_benchmark();
            }
        } else if self.record_benchmark(delta, measured) {
            self.print_benchmark(measured);
            event_loop.exit();
        }
    }
}

/// How many beams are actually putting light in the room this frame.
///
/// Counted from the semantic values, not from GPU lights, so a plan view reports the same number
/// of live beams as the rendered view does.
fn live_beam_count(session: &Session) -> u32 {
    session
        .scene
        .emitters
        .iter()
        .zip(session.values.emitters.iter())
        .filter(|(emitter, value)| {
            emitter.kind == viz_scene::EmitterKind::Beam && value.visible_intensity() > 0.004
        })
        .count() as u32
}

impl Application {
    /// The per-frame housekeeping that happens whether or not there is anything to draw: the
    /// frame clock, held movement keys, and whatever the snapshot workers finished.
    ///
    /// Returns `None` when this frame is a scripted snapshot run that has just finished, which
    /// ends the frame rather than drawing one.
    fn begin_frame(&mut self, event_loop: &ActiveEventLoop) -> Option<(Instant, f32, f32, f32)> {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        // Hold the pace even when this frame returns early, so a session that is still connecting
        // cannot turn the loop into a spin.
        self.next_frame = now + self.pace();
        if delta > 0.0 {
            if self.frame_intervals.len() == 240 {
                self.frame_intervals.remove(0);
            }
            self.frame_intervals.push(delta);
            let mean: f32 =
                self.frame_intervals.iter().sum::<f32>() / self.frame_intervals.len() as f32;
            self.frames_per_second = if mean > 0.0 { 1.0 / mean } else { 0.0 };
        }

        // Held movement keys are applied once per frame so speed follows frame time.
        let mode = self.view_mode();
        self.apply_walk(mode, delta.clamp(0.0, 0.1));
        let (width, height) = self.surface_size();

        // Collect whatever the snapshot workers finished, and keep the panel's list in step with
        // what is actually on disk.
        self.snapshots.pump();
        if self.options.snapshot && self.run_scripted_snapshot(event_loop) {
            return None;
        }
        if self.quick_settings.open {
            self.quick_settings.snapshots = self.snapshots.rows().to_vec();
            self.quick_settings.snapshot_folder = self.snapshots.folder().display().to_string();
        }
        Some((now, delta, width, height))
    }
}

/// Whether this frame is a splash rather than a picture, and what it should say.
///
/// Only ever when there is no rig to draw: a scene with fixtures in it is the picture, however
/// the connection is doing, and its troubles belong on the status bar where they do not cover the
/// stage.
pub(super) fn splash_state(connection: &ConnectionState, empty: bool) -> Option<ui::SplashState> {
    if !empty {
        return None;
    }
    match connection {
        ConnectionState::WaitingForShow { .. } => Some(ui::SplashState::NoShow),
        ConnectionState::Idle
        | ConnectionState::Resolving { .. }
        | ConnectionState::Authenticating { .. }
        | ConnectionState::LoadingScene { .. } => {
            Some(ui::SplashState::Loading(connection.summary()))
        }
        ConnectionState::Failed { boundary, detail } => {
            Some(ui::SplashState::Failed(format!("{boundary}: {detail}")))
        }
        // A stale connection still has its last scene; an empty one has nothing to be stale about.
        ConnectionState::Stale { reason, .. } => Some(ui::SplashState::Failed(reason.clone())),
        // Connected with no fixtures is a show that genuinely has none. The status bar already
        // says so, and it says it without hiding the stage.
        ConnectionState::Connected { .. } => None,
    }
}

/// What is drawing, and how hard: the adapter, its backend, and the anti-aliasing it gave us.
///
/// An adapter with no multisampling still draws; it says so rather than claiming an edge quality
/// it is not delivering.
pub(super) fn gpu_label(renderer: &Renderer) -> String {
    let samples = renderer.samples();
    if samples > 1 {
        format!(
            "{} ({}, {samples}\u{d7} MSAA)",
            renderer.adapter_name(),
            renderer.backend()
        )
    } else {
        format!(
            "{} ({}, no MSAA)",
            renderer.adapter_name(),
            renderer.backend()
        )
    }
}
