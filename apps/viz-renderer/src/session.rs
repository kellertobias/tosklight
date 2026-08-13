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
    last_value_frame: u64,
    last_value_instant: Option<Instant>,
    dmx_intervals: VecDeque<f32>,
    awaiting_snapshot: bool,
}

impl Session {
    pub fn new(provider: Box<dyn SceneProvider>, kind: ProviderKind, epoch: Instant) -> Self {
        Self {
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
            last_value_frame: 0,
            last_value_instant: None,
            dmx_intervals: VecDeque::with_capacity(LATENCY_SAMPLES),
            awaiting_snapshot: true,
        }
    }

    /// Replace the active provider without ever merging two sources.
    pub fn replace_provider(&mut self, provider: Box<dyn SceneProvider>, kind: ProviderKind) {
        self.provider.shutdown();
        self.provider = provider;
        self.kind = kind;
        // Values, diagnostics, and revisions belong to the previous provider and are dropped.
        self.values = SceneValues::default();
        self.diagnostics = ProviderDiagnostics::default();
        self.latency.clear();
        self.dmx_intervals.clear();
        self.last_value_frame = 0;
        self.last_value_instant = None;
        self.awaiting_snapshot = true;
        self.connection = ConnectionState::Idle;
        self.source_view_epoch = 0;
    }

    pub fn capabilities(&self) -> viz_scene::ProviderCapabilities {
        self.provider.capabilities()
    }

    pub fn request_resync(&mut self) {
        self.awaiting_snapshot = true;
        self.provider.request_resync();
    }

    /// Drain the provider and fold its events into the displayed state.
    pub fn pump(&mut self, now: Instant) {
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
                ProviderEvent::Diagnostics(diagnostics) => self.diagnostics = *diagnostics,
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
    }

    fn accept_values(&mut self, mut values: SceneValues, now: Instant) {
        values.resize(self.scene.emitters.len());
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

    /// Record the packet-to-visible latency for the frame just presented.
    pub fn record_presented(&mut self, presented: Instant) {
        if self.values.newest_input_micros == 0 {
            return;
        }
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
mod tests {
    use super::*;
    use crate::demo::DemoProvider;
    use viz_scene::{EmitterInstance, EmitterKind, EmitterLayoutCells, MotionAxis};

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
                instance_id: viz_scene::uuid::Uuid::from_u128(*id),
                fixture_id: viz_scene::uuid::Uuid::from_u128(*id),
                name: format!("fixture {id}"),
                number: None,
                position: viz_scene::glam::Vec3::ZERO,
                rotation_degrees: viz_scene::glam::Vec3::ZERO,
                bracket_degrees: 0.0,
                shaper_degrees: None,
                installed_colour: [1.0; 3],
                installed_shaper_angles_degrees: [0.0; 4],
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
}
