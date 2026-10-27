//! Holds the active provider, the displayed scene, and the measured health of the pipeline.
//!
//! A candidate scene is validated before it replaces the displayed one, and values from a
//! previous provider are dropped the moment a new provider takes over.

use crate::settings::Preferences;
use std::collections::VecDeque;
use std::time::Instant;
use viz_scene::{
    ConnectionState, ProviderDiagnostics, ProviderEvent, ProviderKind, Scene, SceneProvider,
    SceneValues, ViewConfiguration,
};

/// Rolling latency window used for the p50/p95/max readout.
const LATENCY_SAMPLES: usize = 240;

pub struct Session {
    provider: Box<dyn SceneProvider>,
    input_monitor: Option<crate::input_monitor::InputMonitor>,
    child_input_monitor: Option<crate::input_monitor::InputMonitor>,
    pub kind: ProviderKind,
    pub scene: Scene,
    pub values: SceneValues,
    /// The source's authoritative view; the local quality override is applied on top.
    pub source_view: ViewConfiguration,
    /// Counts the times the source has actually stated a view. The host adopts on a change of
    /// this rather than on the view differing, because the view also carries what the operator
    /// selected here — and an operator's own selection must not read as an instruction.
    pub source_view_epoch: u64,
    pub connection: ConnectionState,
    pub diagnostics: ProviderDiagnostics,
    pub epoch: Instant,
    latency: VecDeque<f32>,
    last_presented_input_micros: u64,
    last_value_frame: u64,
    last_value_instant: Option<Instant>,
    dmx_intervals: VecDeque<f32>,
    awaiting_snapshot: bool,
    lamp_clock: Instant,
    lamp_dirty: bool,
    pending_renderer_settings: Option<viz_scene::RendererSettingsUpdate>,
}

impl Session {
    pub fn new(provider: Box<dyn SceneProvider>, kind: ProviderKind, epoch: Instant) -> Self {
        Self::with_input_monitor(
            provider,
            kind,
            epoch,
            crate::input_monitor::InputMonitor::from_environment(),
        )
    }

    fn with_input_monitor(
        mut provider: Box<dyn SceneProvider>,
        kind: ProviderKind,
        epoch: Instant,
        input_monitor: Option<crate::input_monitor::InputMonitor>,
    ) -> Self {
        provider.set_input_monitoring(input_monitor.is_some());
        Self {
            input_monitor,
            child_input_monitor: None,
            provider,
            kind,
            scene: Scene::default(),
            values: SceneValues::default(),
            source_view: ViewConfiguration::default(),
            source_view_epoch: 0,
            connection: ConnectionState::Idle,
            diagnostics: ProviderDiagnostics::default(),
            epoch,
            latency: VecDeque::with_capacity(LATENCY_SAMPLES),
            last_presented_input_micros: 0,
            last_value_frame: 0,
            last_value_instant: None,
            dmx_intervals: VecDeque::with_capacity(LATENCY_SAMPLES),
            awaiting_snapshot: true,
            lamp_clock: epoch,
            lamp_dirty: false,
            pending_renderer_settings: None,
        }
    }

    /// Replace the active provider without ever merging two sources.
    pub fn replace_provider(&mut self, mut provider: Box<dyn SceneProvider>, kind: ProviderKind) {
        provider.set_input_monitoring(
            self.input_monitor.is_some() || self.child_input_monitor.is_some(),
        );
        self.provider.shutdown();
        self.provider = provider;
        self.kind = kind;
        // Values, diagnostics, and revisions belong to the previous provider and are dropped.
        self.values = SceneValues::default();
        self.lamp_dirty = false;
        self.diagnostics = ProviderDiagnostics::default();
        self.latency.clear();
        self.last_presented_input_micros = 0;
        self.dmx_intervals.clear();
        self.last_value_frame = 0;
        self.last_value_instant = None;
        self.awaiting_snapshot = true;
        self.connection = ConnectionState::Idle;
        self.source_view_epoch = 0;
        self.pending_renderer_settings = None;
    }

    pub fn capabilities(&self) -> viz_scene::ProviderCapabilities {
        self.provider.capabilities()
    }

    pub fn request_resync(&mut self) {
        self.awaiting_snapshot = true;
        self.provider.request_resync();
    }

    pub fn update_renderer_settings(&mut self, intent: viz_scene::RendererSettingsIntent) {
        self.provider.update_renderer_settings(intent);
    }

    pub fn take_renderer_settings(&mut self) -> Option<viz_scene::RendererSettingsUpdate> {
        self.pending_renderer_settings.take()
    }

    /// A visible CPU lamp change stays owed even when it settles before the next drawable.
    pub(crate) fn lamp_picture_dirty(&self) -> bool {
        self.lamp_dirty
    }

    /// A skipped/failed admission cannot acknowledge the corrected picture. This is submission
    /// bookkeeping, not evidence of compositor completion or actual display presentation.
    pub(crate) fn observe_lamp_submission(&mut self, successful: bool) {
        if successful {
            self.lamp_dirty = false;
        }
    }

    /// Drain the provider and fold its events into the displayed state.
    pub fn pump(&mut self, now: Instant) {
        self.lamp_dirty |= self.values.advance_lamp_response(
            &self.scene,
            now.saturating_duration_since(self.lamp_clock).as_secs_f64(),
        );
        self.lamp_clock = now;
        for event in self.provider.poll() {
            match event {
                ProviderEvent::Connection(state) => self.connection = state,
                ProviderEvent::Snapshot { scene, view } => {
                    // The candidate is only displayed once it is internally consistent.
                    match validate(&scene) {
                        Ok(()) => {
                            let mut scene = *scene;
                            scene.recompute_bounds();
                            let emitters = scene.emitters.len();
                            self.scene = scene;
                            self.values = SceneValues::default();
                            self.latency.clear();
                            self.last_presented_input_micros = 0;
                            self.values.resize(emitters);
                            if let Some(view) = view {
                                self.source_view = view;
                                self.source_view_epoch += 1;
                            }
                            self.awaiting_snapshot = false;
                        }
                        Err(reason) => {
                            self.connection = ConnectionState::Failed {
                                boundary: "scene snapshot".into(),
                                detail: reason,
                            };
                        }
                    }
                }
                ProviderEvent::SceneDelta(scene) => {
                    if self.awaiting_snapshot {
                        continue;
                    }
                    // A structural change to the show that is already displayed. The candidate is
                    // validated exactly as a snapshot is, and a bad one leaves the good scene
                    // alone; a valid one keeps every head's live value that still has a head.
                    if validate(&scene).is_ok() {
                        let mut scene = *scene;
                        scene.recompute_bounds();
                        self.values.carry_over(&self.scene, &scene);
                        self.scene = scene;
                    }
                }
                ProviderEvent::Values(values) => {
                    if self.awaiting_snapshot {
                        continue;
                    }
                    self.accept_values(*values, now);
                }
                ProviderEvent::View(view) => {
                    self.source_view = view;
                    self.source_view_epoch += 1;
                }
                ProviderEvent::RendererSettings(settings) => {
                    self.pending_renderer_settings = Some(settings);
                }
                ProviderEvent::Diagnostics(diagnostics) => {
                    if !self.awaiting_snapshot {
                        self.values.newest_input_micros = self
                            .values
                            .newest_input_micros
                            .max(diagnostics.newest_input_micros);
                    }
                    self.diagnostics = *diagnostics;
                }
                ProviderEvent::RendererSettings(_) => {}
                ProviderEvent::ResyncRequired { reason } => {
                    self.awaiting_snapshot = true;
                    self.connection = ConnectionState::Stale {
                        endpoint: self.diagnostics.endpoint.clone(),
                        reason,
                    };
                    self.provider.request_resync();
                }
            }
        }
        self.publish_input_monitor(now);
    }

    /// A renderer-owned Editor keeps its pipe across provider/session replacements.
    pub(crate) fn set_child_input_monitor(
        &mut self,
        monitor: Option<crate::input_monitor::InputMonitor>,
    ) {
        let was_enabled = self.input_monitor.is_some() || self.child_input_monitor.is_some();
        self.child_input_monitor = monitor;
        let enabled = self.input_monitor.is_some() || self.child_input_monitor.is_some();
        if was_enabled != enabled {
            self.provider.set_input_monitoring(enabled);
        }
    }

    fn publish_input_monitor(&mut self, now: Instant) {
        let parent_due = self
            .input_monitor
            .as_mut()
            .is_some_and(|monitor| monitor.due(now));
        let child_due = self
            .child_input_monitor
            .as_mut()
            .is_some_and(|monitor| monitor.due(now));
        if (parent_due || child_due)
            && !self.awaiting_snapshot
            && self.connection.is_connected()
            && let Some(snapshot) = self.provider.input_monitor_snapshot()
        {
            if parent_due && let Some(monitor) = &self.input_monitor {
                monitor.publish(snapshot.clone());
            }
            if child_due && let Some(monitor) = &self.child_input_monitor {
                monitor.publish(snapshot);
            }
        }
    }

    fn accept_values(&mut self, mut values: SceneValues, now: Instant) {
        values.resize(self.scene.emitters.len());
        values.retain_visual_motion_runtime_from(&self.values);
        self.lamp_dirty |= values.advance_lamp_response(&self.scene, 0.0);
        values.take_calibrated_runtime_from(&mut self.values);
        values.retain_physics_runtime_from(&self.values, self.scene.physics_scenery.len());
        if values.frame > self.last_value_frame {
            if let Some(previous) = self.last_value_instant {
                let interval = now.duration_since(previous).as_secs_f32();
                if interval > 0.0 {
                    push_sample(&mut self.dmx_intervals, interval);
                }
            }
            self.last_value_instant = Some(now);
            self.last_value_frame = values.frame;
        }
        if values.newest_input_micros == 0 {
            // A provider without network input has no packet timestamp; stamp arrival so the
            // latency readout still measures provider-to-visible time honestly.
            values.newest_input_micros = now.duration_since(self.epoch).as_micros() as u64;
        }
        self.values = values;
    }

    /// Record each new receipt's packet-to-visible latency once, on its first presentation.
    /// Rendering a held look without another receipt is not another input latency sample.
    pub fn record_presented(&mut self, presented: Instant) {
        let input_micros = self.values.newest_input_micros;
        if input_micros == 0 || input_micros <= self.last_presented_input_micros {
            return;
        }
        self.last_presented_input_micros = input_micros;
        let presented_micros = presented.duration_since(self.epoch).as_micros() as u64;
        let latency =
            presented_micros.saturating_sub(self.values.newest_input_micros) as f32 / 1000.0;
        push_sample(&mut self.latency, latency);
    }

    pub fn latency_percentiles(&self) -> (f32, f32, f32) {
        if self.latency.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let mut sorted: Vec<f32> = self.latency.iter().copied().collect();
        sorted.sort_by(f32::total_cmp);
        let percentile = |fraction: f32| {
            let index = ((sorted.len() - 1) as f32 * fraction).round() as usize;
            sorted[index]
        };
        (
            percentile(0.5),
            percentile(0.95),
            *sorted.last().expect("non-empty"),
        )
    }

    pub fn input_rate_hz(&self) -> f32 {
        if self.dmx_intervals.is_empty() {
            return 0.0;
        }
        let mean: f32 = self.dmx_intervals.iter().sum::<f32>() / self.dmx_intervals.len() as f32;
        if mean <= 0.0 { 0.0 } else { 1.0 / mean }
    }

    /// True while a scene is displayed but no input frame has arrived.
    pub fn waiting_for_dmx(&self) -> bool {
        !self.scene.emitters.is_empty() && self.values.newest_input_micros == 0
    }

    /// Apply the operator's local view settings on top of the source's view.
    ///
    /// Quality is an override — it replaces the source's choice only when set. Appearance,
    /// ambient light, exposure trim, laser brightness and plan labels are renderer-local
    /// throughout.
    pub fn effective_view(&self, preferences: &Preferences) -> ViewConfiguration {
        let mut view = self.source_view;
        if let Some(quality) = preferences.quality_override {
            view.quality = quality;
        }
        view.theme = preferences.theme;
        if let Some(background) = preferences.background {
            view.background = Some(background);
        }
        view.ambient = preferences.ambient;
        view.exposure = preferences.exposure;
        view.laser_brightness = preferences.laser_brightness;
        view.fog_variation = preferences.fog_variation;
        view.detailed_people = preferences.detailed_people;
        view.show_labels = preferences.show_labels;
        if let Some(floor_grid) = preferences.floor_grid {
            view.floor_grid = floor_grid;
        }
        view
    }

    pub fn shutdown(&mut self) {
        self.provider.shutdown();
    }
}

fn push_sample(window: &mut VecDeque<f32>, sample: f32) {
    if window.len() == LATENCY_SAMPLES {
        window.pop_front();
    }
    window.push_back(sample);
}

/// A candidate scene must reference only fixtures it carries, so a partially built snapshot can
/// never replace a good one.
fn validate(scene: &Scene) -> Result<(), String> {
    for (index, emitter) in scene.emitters.iter().enumerate() {
        if emitter.fixture_index as usize >= scene.fixtures.len() {
            return Err(format!(
                "emitter {index} references fixture {} of {}",
                emitter.fixture_index,
                scene.fixtures.len()
            ));
        }
        if emitter.cells.is_empty() {
            return Err(format!("emitter {index} has no cells"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "session/lamp_dirty_tests.rs"]
mod lamp_dirty_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::DemoProvider;
    use std::time::Duration;
    use viz_scene::{EmitterInstance, EmitterKind, EmitterLayoutCells, MotionAxis};

    #[test]
    fn effective_detailed_people_remains_raw_for_capture_from_draft_lines() {
        let mut session = session();
        session.source_view.quality = viz_scene::RenderQuality::Draft;
        session.source_view.mode = viz_scene::ViewMode::Lines3d;
        let mut preferences = Preferences::from_options(&crate::settings::Options::default());
        preferences.detailed_people = true;
        let view = session.effective_view(&preferences);
        assert!(view.detailed_people);
        assert_eq!(view.quality, viz_scene::RenderQuality::Draft);
        assert_eq!(view.mode, viz_scene::ViewMode::Lines3d);
        preferences.detailed_people = false;
        assert!(!session.effective_view(&preferences).detailed_people);
    }

    fn session() -> Session {
        Session::new(
            Box::new(DemoProvider::new()),
            ProviderKind::PlanningSoftware,
            Instant::now(),
        )
    }

    #[test]
    fn a_snapshot_populates_the_scene_and_sizes_the_value_array() {
        let mut session = session();
        session.pump(Instant::now());
        assert!(!session.scene.fixtures.is_empty());
        assert_eq!(session.values.emitters.len(), session.scene.emitters.len());
        assert!(session.connection.is_connected());
    }

    #[test]
    fn an_inconsistent_snapshot_is_refused_rather_than_displayed() {
        let mut scene = Scene::default();
        scene.emitters.push(EmitterInstance {
            fixture_index: 4,
            head_index: 0,
            label: "Orphan".into(),
            local_origin: viz_scene::glam::Vec3::ZERO,
            tilt_pivot: viz_scene::glam::Vec3::ZERO,
            local_orientation_degrees: viz_scene::glam::Vec3::ZERO,
            pan: None,
            tilt: Some(MotionAxis {
                axis: viz_scene::glam::Vec3::X,
                min_degrees: 0.0,
                max_degrees: 1.0,
            }),
            beam_angle_degrees: 10.0,
            field_angle_degrees: 20.0,
            optics: viz_scene::EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
        assert!(validate(&scene).is_err());
    }

    #[test]
    fn replacing_the_provider_drops_the_previous_values_and_diagnostics() {
        let mut session = session();
        session.pump(Instant::now());
        session.values.newest_input_micros = 42;
        session.replace_provider(
            Box::new(DemoProvider::new()),
            ProviderKind::PlanningSoftware,
        );
        assert_eq!(session.values.newest_input_micros, 0);
        assert!(session.values.emitters.is_empty());
        assert_eq!(session.connection, ConnectionState::Idle);
    }

    #[test]
    fn provider_values_do_not_reset_motion_advanced_by_the_window() {
        let mut session = session();
        session.pump(Instant::now());
        assert!(!session.values.emitters.is_empty());
        session.values.emitters[0].pan_motion.position_degrees = -12.0;
        session.values.emitters[0]
            .pan_motion
            .velocity_degrees_per_second = -40.0;
        let target = viz_scene::PhysicalMotionTarget::Position {
            degrees: 54.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 360.0,
        };
        let mut incoming = SceneValues::default();
        incoming.resize(session.scene.emitters.len());
        incoming.emitters[0].pan_motion.target = Some(target);

        session.accept_values(incoming, Instant::now());

        assert_eq!(
            session.values.emitters[0].pan_motion.position_degrees,
            -12.0
        );
        assert_eq!(
            session.values.emitters[0]
                .pan_motion
                .velocity_degrees_per_second,
            -40.0
        );
        assert_eq!(session.values.emitters[0].pan_motion.target, Some(target));
    }

    #[test]
    fn the_local_quality_override_replaces_the_source_setting() {
        let mut session = session();
        session.pump(Instant::now());
        let mut preferences =
            crate::settings::Preferences::from_options(&crate::settings::Options::default());
        assert_eq!(
            session.effective_view(&preferences).quality,
            session.source_view.quality
        );
        preferences.quality_override = Some(viz_scene::RenderQuality::Draft);
        assert_eq!(
            session.effective_view(&preferences).quality,
            viz_scene::RenderQuality::Draft
        );
    }

    #[test]
    fn local_rendering_and_feature_settings_reach_the_effective_view() {
        let mut session = session();
        session.pump(Instant::now());
        let mut preferences =
            crate::settings::Preferences::from_options(&crate::settings::Options::default());
        preferences.background = Some([0.02, 0.04, 0.08]);
        preferences.show_labels = false;
        preferences.floor_grid = Some(false);
        preferences.exposure = 1.75;

        let view = session.effective_view(&preferences);
        assert_eq!(view.background, Some([0.02, 0.04, 0.08]));
        assert!(!view.show_labels);
        assert!(!view.floor_grid);
        assert_eq!(view.exposure, 1.75);
    }

    /// A provider that hands out exactly what a test queued for it.
    struct ScriptedProvider {
        events: Vec<ProviderEvent>,
    }

    impl viz_scene::SceneProvider for ScriptedProvider {
        fn capabilities(&self) -> viz_scene::ProviderCapabilities {
            viz_scene::ProviderCapabilities {
                kind: ProviderKind::LightingDesk,
                available: true,
                unavailable_reason: None,
                default_host: "127.0.0.1".into(),
                default_port: 5000,
                uses_network_input: true,
            }
        }

        fn poll(&mut self) -> Vec<ProviderEvent> {
            std::mem::take(&mut self.events)
        }

        fn request_resync(&mut self) {}

        fn shutdown(&mut self) {}
    }

    fn one_fixture_scene(ids: &[u128]) -> Scene {
        let mut scene = Scene::default();
        for (index, id) in ids.iter().enumerate() {
            scene.fixtures.push(viz_scene::FixtureInstance {
                lamp_response: Default::default(),
                drawn_as_scenery: false,
                invisible: false,
                instance_id: viz_scene::uuid::Uuid::from_u128(*id),
                fixture_id: viz_scene::uuid::Uuid::from_u128(*id),
                name: format!("fixture {id}"),
                number: None,
                position: viz_scene::glam::Vec3::ZERO,
                rotation_degrees: viz_scene::glam::Vec3::ZERO,
                position_master: None,
                bracket_degrees: 0.0,
                bracket_hinge: None,
                shaper_degrees: None,
                installed_colour: [1.0; 3],
                installed_shaper_angles_degrees: [0.0; 4],
                installed_shaper_blades: [0.0; 4],
                body: viz_scene::FixtureBody {
                    size: viz_scene::glam::Vec3::splat(0.3),
                    kind: viz_scene::BodyKind::Lantern,
                },
                patched: true,
                address: None,
                model: None,
                fallback: None,
            });
            scene.emitters.push(EmitterInstance {
                fixture_index: index as u32,
                head_index: 0,
                label: "head".into(),
                local_origin: viz_scene::glam::Vec3::ZERO,
                tilt_pivot: viz_scene::glam::Vec3::ZERO,
                local_orientation_degrees: viz_scene::glam::Vec3::ZERO,
                pan: None,
                tilt: Some(MotionAxis {
                    axis: viz_scene::glam::Vec3::X,
                    min_degrees: 0.0,
                    max_degrees: 1.0,
                }),
                beam_angle_degrees: 10.0,
                field_angle_degrees: 20.0,
                optics: viz_scene::EmitterOptics::default(),
                kind: EmitterKind::Beam,
                cells: EmitterLayoutCells::single(),
                laser: None,
                effect: None,
                live_shaper_angle_roles: [false; 4],
                shaper_roles: [false; 4],
                live_shaper_rotation_role: false,
            });
        }
        scene
    }

    /// A fixture patched while the desk holds a look must not black out the rest of the rig.
    #[test]
    fn a_delta_keeps_the_live_values_of_every_head_that_still_exists() {
        let mut session = Session::new(
            Box::new(ScriptedProvider {
                events: vec![ProviderEvent::Snapshot {
                    scene: Box::new(one_fixture_scene(&[1, 2])),
                    view: None,
                }],
            }),
            ProviderKind::LightingDesk,
            Instant::now(),
        );
        session.pump(Instant::now());
        session.values.emitters[0].intensity = 0.4;
        session.values.emitters[1].intensity = 0.9;

        // The first fixture is unpatched away and a new one is added after the second.
        session.replace_provider(
            Box::new(ScriptedProvider {
                events: vec![ProviderEvent::SceneDelta(Box::new(one_fixture_scene(&[
                    2, 3,
                ])))],
            }),
            ProviderKind::LightingDesk,
        );
        // Replacing the provider is a fresh start; put the session back in the state a running
        // connection would be in, which is what the delta is applied to.
        session.awaiting_snapshot = false;
        session.scene = one_fixture_scene(&[1, 2]);
        session.values.resize(2);
        session.values.emitters[0].intensity = 0.4;
        session.values.emitters[1].intensity = 0.9;

        session.pump(Instant::now());
        assert_eq!(session.scene.fixtures.len(), 2);
        assert_eq!(session.values.emitters.len(), 2);
        assert_eq!(
            session.values.emitters[0].intensity, 0.9,
            "the fixture that survived kept its level"
        );
        assert_eq!(
            session.values.emitters[1].intensity, 0.0,
            "the new fixture starts dark"
        );
    }

    #[test]
    fn an_installed_gel_change_replaces_scene_colour_without_dropping_live_level() {
        let mut session = Session::new(
            Box::new(ScriptedProvider {
                events: vec![ProviderEvent::Snapshot {
                    scene: Box::new(one_fixture_scene(&[1])),
                    view: None,
                }],
            }),
            ProviderKind::LightingDesk,
            Instant::now(),
        );
        session.pump(Instant::now());
        session.values.emitters[0].intensity = 0.72;
        let mut changed = one_fixture_scene(&[1]);
        changed.revision = session.scene.revision + 1;
        changed.fixtures[0].installed_colour = [0.7, 0.04, 0.02];
        session.provider = Box::new(ScriptedProvider {
            events: vec![ProviderEvent::SceneDelta(Box::new(changed))],
        });

        session.pump(Instant::now());

        assert_eq!(
            session.scene.fixtures[0].installed_colour,
            [0.7, 0.04, 0.02]
        );
        assert_eq!(session.values.emitters[0].intensity, 0.72);
    }

    /// The delta path validates its candidate exactly as the snapshot path does.
    #[test]
    fn an_inconsistent_delta_leaves_the_displayed_scene_alone() {
        let mut session = Session::new(
            Box::new(ScriptedProvider {
                events: vec![ProviderEvent::Snapshot {
                    scene: Box::new(one_fixture_scene(&[1])),
                    view: None,
                }],
            }),
            ProviderKind::LightingDesk,
            Instant::now(),
        );
        session.pump(Instant::now());
        let mut broken = one_fixture_scene(&[1]);
        broken.emitters[0].fixture_index = 9;
        session.replace_provider(
            Box::new(ScriptedProvider {
                events: vec![ProviderEvent::SceneDelta(Box::new(broken))],
            }),
            ProviderKind::LightingDesk,
        );
        session.awaiting_snapshot = false;
        session.scene = one_fixture_scene(&[1]);
        session.pump(Instant::now());
        assert_eq!(session.scene.emitters[0].fixture_index, 0);
    }

    #[test]
    fn repeated_presentations_without_input_do_not_measure_held_look_age() {
        let epoch = Instant::now();
        let mut session = Session::new(
            Box::new(ScriptedProvider { events: Vec::new() }),
            ProviderKind::LightingDesk,
            epoch,
        );
        session.record_presented(epoch + Duration::from_secs(1));
        assert_eq!(session.latency_percentiles(), (0., 0., 0.));
        session.values.newest_input_micros = 1_000;
        session.record_presented(epoch + Duration::from_micros(2_000));
        session.record_presented(epoch + Duration::from_secs(20));
        assert_eq!(
            session.latency_percentiles(),
            (1., 1., 1.),
            "no new packet arrived"
        );
    }

    #[test]
    fn held_input_freshness_updates_latency_without_changing_values() {
        let epoch = Instant::now();
        let mut session = Session::new(
            Box::new(ScriptedProvider { events: Vec::new() }),
            ProviderKind::LightingDesk,
            epoch,
        );
        session.awaiting_snapshot = false;
        session.values.frame = 7;
        session.values.newest_input_micros = 1_000;
        session.record_presented(epoch + Duration::from_micros(2_000));
        assert_eq!(session.latency_percentiles(), (1., 1., 1.));
        // Internal helper diagnostics are serialized; old producers may omit this additive field.
        let diagnostics = serde_json::from_value(serde_json::json!({
            "endpoint": "", "resolved_address": "", "authenticated": false,
            "show_identity": "", "scene_revision": 0, "interface": "", "inputs": [],
            "universes": [], "preview_universes": [], "warnings": [],
            "newest_input_micros": 9_000,
        }))
        .unwrap();
        session.provider = Box::new(ScriptedProvider {
            events: vec![ProviderEvent::Diagnostics(Box::new(diagnostics))],
        });
        session.pump(epoch + Duration::from_micros(9_500));
        assert_eq!(session.values.frame, 7);
        assert_eq!(session.values.newest_input_micros, 9_000);
        session.record_presented(epoch + Duration::from_micros(10_000));
        assert_eq!(session.latency_percentiles(), (1., 1., 1.));
        session.record_presented(epoch + Duration::from_secs(20));
        assert_eq!(
            session.latency_percentiles(),
            (1., 1., 1.),
            "no receipt means no new latency sample"
        );
        let changed = SceneValues {
            frame: 8,
            newest_input_micros: 20_000,
            ..SceneValues::default()
        };
        session.provider = Box::new(ScriptedProvider {
            events: vec![ProviderEvent::Values(Box::new(changed))],
        });
        session.pump(epoch + Duration::from_micros(20_500));
        session.record_presented(epoch + Duration::from_micros(21_000));
        assert_eq!(session.values.frame, 8);
        assert_eq!(session.latency_percentiles(), (1., 1., 1.));
        session.replace_provider(
            Box::new(ScriptedProvider { events: Vec::new() }),
            ProviderKind::LightingDesk,
        );
        assert_eq!(session.latency_percentiles(), (0., 0., 0.));
        session.record_presented(epoch + Duration::from_secs(21));
        assert_eq!(session.latency_percentiles(), (0., 0., 0.));
    }

    #[test]
    fn latency_percentiles_are_ordered() {
        let mut session = session();
        for sample in [5.0_f32, 40.0, 12.0, 8.0, 90.0] {
            push_sample(&mut session.latency, sample);
        }
        let (p50, p95, max) = session.latency_percentiles();
        assert!(p50 <= p95);
        assert!(p95 <= max);
        assert_eq!(max, 90.0);
    }

    #[test]
    fn input_monitor_pump_opts_in_before_poll_and_never_publishes_unready_source() {
        use std::sync::{Arc, Mutex, mpsc};
        use viz_scene::input_monitor::{MonitorContext, MonitorSnapshot, ReceivedDmx};
        struct Probe(Arc<Mutex<Vec<&'static str>>>);
        impl SceneProvider for Probe {
            fn capabilities(&self) -> viz_scene::ProviderCapabilities {
                DemoProvider::new().capabilities()
            }
            fn set_input_monitoring(&mut self, enabled: bool) {
                assert!(enabled);
                self.0.lock().unwrap().push("enabled");
            }
            fn poll(&mut self) -> Vec<ProviderEvent> {
                self.0.lock().unwrap().push("poll");
                vec![]
            }
            fn request_resync(&mut self) {}
            fn shutdown(&mut self) {}
            fn input_monitor_snapshot(&self) -> Option<MonitorSnapshot> {
                self.0.lock().unwrap().push("snapshot");
                Some(MonitorSnapshot {
                    version: 1,
                    generated_millis: viz_scene::input_monitor::unix_millis(),
                    context: MonitorContext {
                        producer: "network_receiver".into(),
                        endpoint: "owned".into(),
                        show_id: None,
                        show_revision: 7,
                        mappings: vec![],
                    },
                    received: ReceivedDmx {
                        universes: vec![],
                        inputs: vec![],
                        warnings: vec![],
                    },
                })
            }
        }
        struct Writer(mpsc::Sender<Vec<u8>>);
        impl std::io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.send(bytes.to_vec()).unwrap();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let calls = Arc::new(Mutex::new(vec![]));
        let (writer, received) = mpsc::channel();
        let monitor = crate::input_monitor::InputMonitor::with_writer(Writer(writer));
        let mut session = Session::with_input_monitor(
            Box::new(Probe(calls.clone())),
            ProviderKind::LightingDesk,
            Instant::now(),
            Some(monitor),
        );
        session.pump(Instant::now());
        assert_eq!(*calls.lock().unwrap(), vec!["enabled", "poll"]);
        assert!(
            received.try_recv().is_err(),
            "unready source cannot publish a held previous snapshot"
        );
        session.awaiting_snapshot = false;
        session.connection = ConnectionState::Connected {
            endpoint: "owned".into(),
            revision: 7,
        };
        session.pump(Instant::now() + Duration::from_secs(1));
        let message = received.recv_timeout(Duration::from_secs(2)).unwrap();
        let snapshot: MonitorSnapshot = serde_json::from_slice(&message).unwrap();
        assert_eq!(snapshot.context.show_revision, 7);
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["enabled", "poll", "poll", "snapshot"]
        );
        session.connection = ConnectionState::Stale {
            endpoint: "owned".into(),
            reason: "disconnected".into(),
        };
        session.pump(Instant::now() + Duration::from_secs(2));
        assert!(received.try_recv().is_err());
    }
}
