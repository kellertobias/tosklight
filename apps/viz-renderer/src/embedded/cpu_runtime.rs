//! Stateful package scripts belong to the simulation clock, never to GPU visibility/ownership.
use std::time::{Duration, Instant};
use viz_scene::{PersistencePreference, Scene, SceneValues, ViewConfiguration};

pub(super) struct CpuRuntime {
    pub(super) lasers: crate::lasers::Lasers,
    physics: crate::physics::Physics,
    next: Instant,
    dirty: bool,
}

impl CpuRuntime {
    pub fn new(
        lasers: crate::lasers::Lasers,
        physics: crate::physics::Physics,
        now: Instant,
    ) -> Self {
        Self {
            lasers,
            physics,
            next: now,
            dirty: false,
        }
    }

    /// Every delivered authoritative value frame is an edge, even if another is already queued.
    /// Periodic evaluations also continue without values, so elapsed-time/global script state is
    /// identical while active, hidden or backpressured. No geometry/labels/GPU upload happens here.
    pub fn advance(
        &mut self,
        scene: &Scene,
        values: &mut SceneValues,
        view: &ViewConfiguration,
        time: f32,
        now: Instant,
        input_edge: bool,
    ) {
        if !input_edge && now < self.next {
            return;
        }
        self.next = now + Duration::from_secs_f64(1.0 / 60.0);
        let moving = values.is_time_driven(&PersistencePreference::default());
        // A settled body's time-driven reset can jump back to its authored pose without making
        // is_time_driven true on either side. Retain that visible transition until submission.
        let before: Vec<_> = values.physics_frames.iter().map(physics_pose).collect();
        self.lasers.run(scene, values, time);
        self.physics.run(
            scene,
            values,
            time,
            view.quality,
            view.physics_reset_generation,
        );
        self.dirty |= moving
            || values.is_time_driven(&PersistencePreference::default())
            || before
                .iter()
                .copied()
                .ne(values.physics_frames.iter().map(physics_pose));
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn succeeded(&mut self) {
        self.dirty = false;
    }
}

fn physics_pose(frame: &viz_scene::PhysicsFrame) -> ([f32; 3], [f32; 3], bool, bool) {
    (
        frame.position_offset,
        frame.velocity,
        frame.released,
        frame.settled,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedded::{adopt_provider_values, producer::Producer};
    use std::sync::Arc;
    use viz_scene::{EmitterKind, RenderQuality};

    fn runtime(now: Instant) -> CpuRuntime {
        CpuRuntime::new(
            crate::lasers::Lasers::new(None),
            crate::physics::Physics::new(),
            now,
        )
    }

    fn physics_scene() -> Scene {
        let id = uuid::Uuid::new_v4();
        Scene {
            physics_scenery: vec![viz_scene::PhysicsSceneryObject {
                fixture_instance_id: id,
                scenery: viz_scene::SceneryObject {
                    id,
                    name: "Hidden release".into(),
                    position: glam::Vec3::new(0.0, 10.0, 0.0),
                    rotation_degrees: glam::Vec3::ZERO,
                    size: glam::Vec3::new(6.0, 2.0, 0.1),
                    colour: [0.3; 3],
                    roughness: 0.8,
                    kind: viz_scene::SceneryKind::Curtain,
                    chords: 1,
                    detail: Default::default(),
                    position_master: None,
                    architecture: None,
                },
                program: viz_scene::PhysicsProgram {
                    script: Some(Arc::from(
                        "export function physics(input){return {version:1,action:input.dmx[0]>=192?'release':input.dmx[0]<=31?'reset':'hold'}}",
                    )),
                    script_key: 1,
                    result_version: 1,
                },
                body: viz_scene::PhysicsBody {
                    mass_kilograms: 10.0,
                    gravity_metres_per_second_squared: 9.80665,
                },
                constraints: viz_scene::PhysicsConstraints {
                    floor_y_metres: 0.0,
                    scenery_collision: false,
                    self_collision: false,
                },
            }],
            ..Scene::default()
        }
    }

    #[test]
    fn hidden_release_hold_reset_edges_are_resolved_before_reveal_without_gpu_slots() {
        let now = Instant::now();
        let scene = physics_scene();
        let mut cpu = runtime(now);
        let view = ViewConfiguration {
            quality: RenderQuality::Draft,
            ..Default::default()
        };
        let mut values = SceneValues::default();
        let mut producer = Producer::new(true, now);
        producer.set_visible(false, now);
        for (millis, slot) in [
            (0, 0),
            (100, 255),
            (101, 128),
            (2100, 128),
            (2200, 0),
            (2201, 128),
        ] {
            let mut incoming = SceneValues::default();
            incoming.resize_physics(1);
            incoming.physics_frames[0].slots = vec![slot];
            adopt_provider_values(&mut values, incoming, 1);
            let at = now + Duration::from_millis(millis);
            cpu.advance(&scene, &mut values, &view, millis as f32 / 1000.0, at, true);
            assert!(producer.begin(at).is_none());
            assert!(values.physics_frames[0].fault.is_none());
            match millis {
                101 => assert!(
                    values.physics_frames[0].released,
                    "release edge cannot disappear into following hold"
                ),
                2100 => assert!(values.physics_frames[0].settled),
                2201 => assert!(
                    !values.physics_frames[0].released,
                    "reset edge survives following hold"
                ),
                _ => {}
            }
        }
        assert!(
            cpu.dirty(),
            "settled/reset result is pending until a picture succeeds"
        );
    }

    #[test]
    fn time_driven_reset_of_settled_body_invalidates_static_picture() {
        let now = Instant::now();
        let mut scene = physics_scene();
        scene.physics_scenery[0].program.script = Some(Arc::from(
            "export function physics(input){return {version:1,action:input.time<2?'release':'reset'}}",
        ));
        scene.physics_scenery[0].program.script_key += 1;
        let mut cpu = runtime(now);
        let view = ViewConfiguration::default();
        let mut values = SceneValues::default();
        values.resize_physics(1);
        cpu.advance(&scene, &mut values, &view, 0.0, now, true);
        cpu.advance(
            &scene,
            &mut values,
            &view,
            1.9,
            now + Duration::from_millis(1900),
            false,
        );
        assert!(values.physics_frames[0].settled);
        cpu.succeeded();
        assert!(!cpu.dirty());
        cpu.advance(
            &scene,
            &mut values,
            &view,
            2.1,
            now + Duration::from_millis(2100),
            false,
        );
        assert!(!values.physics_frames[0].released);
        assert!(
            cpu.dirty(),
            "a reset cannot disappear merely because the old body was settled"
        );
    }

    #[test]
    fn hidden_stateful_laser_observes_each_delivered_edge_and_periodic_time() {
        let now = Instant::now();
        let mut scene = crate::demo::build_scene();
        let index = scene
            .emitters
            .iter()
            .position(|emitter| emitter.kind == EmitterKind::Laser)
            .unwrap();
        let optics = scene.emitters[index].laser.as_mut().unwrap();
        optics.script = Some(Arc::from(
            "let count=0; export function scan(input){if(input.dmx[0]===255)count++;return {points:[{x:Math.min(count/10,1),y:0,b:1,amount:100}]};}",
        ));
        optics.script_key = optics.script_key.wrapping_add(1001);
        let mut cpu = runtime(now);
        let view = ViewConfiguration::default();
        let mut values = SceneValues::default();
        let mut producer = Producer::new(true, now);
        producer.set_visible(false, now);
        for (millis, slot) in [(0, 0), (100, 255), (101, 0), (102, 255), (103, 0)] {
            let mut incoming = SceneValues::default();
            incoming.resize(scene.emitters.len());
            incoming.emitters[index].intensity = 1.0;
            incoming.laser_scans[index].slots = vec![slot];
            adopt_provider_values(&mut values, incoming, 0);
            let at = now + Duration::from_millis(millis);
            cpu.advance(&scene, &mut values, &view, millis as f32 / 1000.0, at, true);
            assert!(producer.begin(at).is_none());
        }
        assert_eq!(values.laser_scans[index].fault, None);
        assert!((values.laser_scans[index].points[0].x - 0.2).abs() < 0.001);
        cpu.advance(
            &scene,
            &mut values,
            &view,
            1.0,
            now + Duration::from_secs(1),
            false,
        );
        assert!((values.laser_scans[index].points[0].x - 0.2).abs() < 0.001);
        assert!(cpu.dirty());
    }
}
