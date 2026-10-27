//! Fixture mechanics follow wall time independently of whether the pane can submit a GPU frame.
use std::time::{Duration, Instant};
use viz_scene::{Scene, SceneValues};

pub(super) struct MechanicsClock {
    last: Instant,
    dirty: bool,
}

impl MechanicsClock {
    pub fn new(now: Instant) -> Self {
        Self {
            last: now,
            dirty: false,
        }
    }

    /// Cheap mechanics keep advancing while hidden/backpressured. A render deadline forces the
    /// remaining fraction of the CPU tick so labels and geometry see the current pose together.
    pub fn advance(&mut self, scene: &Scene, values: &mut SceneValues, now: Instant, force: bool) {
        let elapsed = now.saturating_duration_since(self.last);
        if !force && elapsed < Duration::from_millis(10) {
            return;
        }
        self.last = now;
        self.dirty |= values.advance_lamp_response(scene, elapsed.as_secs_f64());
        let moving = mechanics_moving(values);
        if moving || force {
            values.apply_physical_motion(elapsed.as_secs_f32().min(0.25));
            values.apply_calibrated_motion(scene, elapsed.as_secs_f32().min(0.25));
        }
        self.dirty |= moving;
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn succeeded(&mut self) {
        self.dirty = false;
    }
}

fn mechanics_moving(values: &SceneValues) -> bool {
    values
        .physical_positions
        .iter()
        .any(|position| position.axes.iter().any(|axis| axis.motion.is_moving()))
        || values.emitters.iter().any(|emitter| {
            emitter.pan_motion.is_moving()
                || emitter.tilt_motion.is_moving()
                || emitter.gobo_rotation_motion.is_moving()
                || emitter.prism_rotation_motion.is_moving()
                || emitter.gobo_wheel_motion.motion.is_moving()
                || emitter.colour_wheel_motion.motion.is_moving()
                || emitter
                    .gobo_wheels
                    .iter()
                    .chain(&emitter.prism_wheels)
                    .any(|wheel| {
                        wheel.rotation_motion.is_moving() || wheel.wheel_motion.motion.is_moving()
                    })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedded::{adopt_provider_values, producer::Producer};
    use viz_scene::PhysicalMotionTarget;

    #[test]
    fn hidden_thermal_clock_settles_without_gpu_credit_and_capture_stays_frozen() {
        let mut scene = crate::demo::build_scene();
        let fixture = scene.emitters[0].fixture_index as usize;
        scene.fixtures[fixture].lamp_response = viz_scene::LampResponseProfile::TungstenNominal;
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values.advance_lamp_response(&scene, 0.0);
        values.emitters[0].intensity = 1.0;
        let now = Instant::now();
        let mut clock = MechanicsClock::new(now);
        let mut producer = Producer::new(true, now);
        producer.set_visible(false, now);
        clock.advance(&scene, &mut values, now + Duration::from_millis(10), true);
        let frozen = viz_render::still::FrozenStill::new(
            &scene,
            &values,
            &Default::default(),
            0.01,
            Default::default(),
            viz_render::still::StillOrigin {
                pane_id: "thermal-test".into(),
                follow_preload: false,
            },
        )
        .unwrap();
        let captured = frozen.values().emitters[0].visible_intensity();
        assert!((0.3..0.4).contains(&captured));
        for tick in 2..=300 {
            if tick % 10 == 0 {
                let mut incoming = SceneValues::default();
                incoming.resize(scene.emitters.len());
                incoming.emitters[0].intensity = if tick < 20 { 1.0 } else { 0.0 };
                adopt_provider_values(&mut values, incoming, 0);
                values.advance_lamp_response(&scene, 0.0);
            }
            let at = now + Duration::from_millis(tick * 10);
            clock.advance(&scene, &mut values, at, false);
            assert!(producer.begin(at).is_none());
        }
        assert_eq!(values.emitters[0].visible_intensity(), 0.0);
        assert!(
            !values.emitters[0]
                .lamp_response
                .as_ref()
                .unwrap()
                .is_moving()
        );
        assert_eq!(frozen.values().emitters[0].visible_intensity(), captured);
        assert!(
            clock.dirty(),
            "the settled final picture remains owed until a successful submit"
        );
        clock.succeeded();
        clock.advance(&scene, &mut values, now + Duration::from_secs(4), false);
        assert!(
            !clock.dirty(),
            "settled lamp clocks stop producing dirty pictures"
        );
    }

    fn incoming() -> SceneValues {
        let mut values = SceneValues::default();
        values.resize(1);
        let emitter = &mut values.emitters[0];
        for (state, degrees) in [
            (&mut emitter.pan_motion, 54.0),
            (&mut emitter.tilt_motion, -36.0),
        ] {
            state.target = Some(PhysicalMotionTarget::Position {
                degrees,
                max_speed: 180.0,
                acceleration: 360.0,
                deceleration: 360.0,
            });
        }
        values
    }

    #[test]
    fn hidden_five_seconds_of_authoritative_pan_tilt_does_not_replay_on_reveal() {
        let scene = Scene::default();
        let now = Instant::now();
        let mut values = incoming();
        let mut clock = MechanicsClock::new(now);
        let mut producer = Producer::new(true, now);
        producer.set_visible(false, now);
        for tick in 1..=500 {
            let at = now + Duration::from_millis(tick * 10);
            // The 10 Hz desk owns the target; adopting it must preserve *advancing*, not frozen,
            // renderer mechanics. Provider-side home positions are deliberately zero each time.
            if tick % 10 == 0 {
                adopt_provider_values(&mut values, incoming(), 0);
            }
            clock.advance(&scene, &mut values, at, false);
            assert!(
                producer.begin(at).is_none(),
                "hidden mechanics never request a GPU slot"
            );
        }
        assert!((values.emitters[0].pan_motion.position_degrees - 54.0).abs() < 0.001);
        assert!((values.emitters[0].tilt_motion.position_degrees + 36.0).abs() < 0.001);
        let at = now + Duration::from_secs(5);
        assert!(producer.set_visible(true, at));
        assert!(producer.begin(at).is_some());
        clock.advance(&scene, &mut values, at, true);
        assert!(!values.emitters[0].pan_motion.is_moving());
        assert!(!values.emitters[0].tilt_motion.is_moving());
        assert!(clock.dirty(), "final settled pose still needs a picture");
    }

    #[test]
    fn backpressure_settling_keeps_dirty_until_success_not_merely_a_draw_attempt() {
        let scene = Scene::default();
        let now = Instant::now();
        let mut values = incoming();
        let mut clock = MechanicsClock::new(now);
        for tick in 1..=500 {
            clock.advance(
                &scene,
                &mut values,
                now + Duration::from_millis(tick * 10),
                false,
            );
        }
        assert!(clock.dirty());
        clock.advance(&scene, &mut values, now + Duration::from_secs(6), true);
        assert!(
            clock.dirty(),
            "a render attempt is not a successful picture"
        );
        clock.succeeded();
        assert!(!clock.dirty());
    }
}
