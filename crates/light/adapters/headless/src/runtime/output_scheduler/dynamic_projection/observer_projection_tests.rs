//! Differential oracle for the discarded second observer-runtime snapshot.
use super::*;
use light_dynamics::{DynamicDefinition, DynamicRuntime, DynamicSemanticValue, DynamicValueTiming};
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};

const START: i64 = 1_000_000;
fn definition(current: bool) -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::from_u128(11), "pool_number": 1, "revision": 1, "name": "Observer wave",
        "color": null, "icon": null, "target_binding": {"type":"targetless"},
        "lanes": [{"id":Uuid::from_u128(12),"attribute":"intensity","mode":"max_min",
            "keyframes":{"points":[{"position":0.,"source":{"type":"value","value":0.2},"interpolation":"linear"},{"position":0.5,"source":{"type":"value","value":0.8},"interpolation":"linear"}],"size":1.},
            "max_min":{"minimum":if current {serde_json::json!({"type":"current"})} else {serde_json::json!({"type":"value","value":0.2})},
                "maximum":{"type":"value","value":0.8},"function":"sinus","size":1.},
            "middle_amplitude":{"middle":{"type":"current"},"amplitude":0.3,"function":"sinus","size":1.},
            "speed_multiplier":{"numerator":1,"denominator":1},"width":1.,"random_group_id":null}],
        "random_groups":[],"phase":{"ordering":{"type":"selection"},"offset_degrees":0.,"span_degrees":0.,"block_size":1,"repeats":1,"wings":false,"anchors_degrees":[]},
        "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
    })).unwrap()
}
struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    clock: Arc<light_core::ManualClock>,
    session: light_core::SessionId,
    target: FixtureId,
    definition: DynamicDefinition,
    old: Mutex<DynamicRuntime>,
    new: Mutex<DynamicRuntime>,
    speed: Mutex<[light_control::speed::SpeedGroupController; 5]>,
}
impl Rig {
    fn new(current: bool) -> Self {
        let clock = Arc::new(light_core::ManualClock::new(
            chrono::DateTime::from_timestamp_millis(START).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let engine = Engine::new(programmers.clone());
        let definition = definition(current);
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                dynamics: vec![definition.clone()].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        let runtime = || {
            let mut runtime = DynamicRuntime::default();
            runtime.derive_instance_ids_from(Uuid::from_u128(13));
            runtime.install_definitions([definition.clone()]).unwrap();
            Mutex::new(runtime)
        };
        let old = runtime();
        let new = runtime();
        let session = light_core::SessionId::new();
        programmers.start(session);
        Self {
            engine,
            programmers,
            clock,
            session,
            target: FixtureId(Uuid::from_u128(14)),
            definition,
            old,
            new,
            speed: Mutex::new(std::array::from_fn(|_| {
                light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
            })),
        }
    }
    fn set(&self, value: DynamicSemanticValue) {
        assert!(self.programmers.apply_dynamic_values(
            self.session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: self.target,
                attribute: AttributeKey::intensity(),
                value
            }],
            None
        ));
    }
    fn activate(&self) {
        self.set(DynamicSemanticValue::DynamicOn {
            instance_link: Uuid::from_u128(15),
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(self.definition.id),
                last_known_pool_number: 1,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(self.definition.clone()),
                },
            },
            lane_id: self.definition.lanes[0].id,
            overrides: light_dynamics::DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: light_dynamics::Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: Default::default(),
        });
    }
    fn at(
        &self,
        offset: i64,
        extras: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    ) -> (usize, usize) {
        self.clock
            .set(chrono::DateTime::from_timestamp_millis(START + offset).unwrap());
        // Match the observer's fresh runtime/install/portable restore sequence on every read.
        let portable = self.old.lock().snapshot();
        for runtime in [&self.old, &self.new] {
            let mut fresh = DynamicRuntime::default();
            fresh.derive_instance_ids_from(Uuid::from_u128(13));
            fresh
                .install_definitions(self.engine.snapshot().dynamics.iter().cloned())
                .unwrap();
            fresh.restore_snapshot(portable.clone()).unwrap();
            *runtime.lock() = fresh;
        }
        let rate = AtomicU16::new(40);
        // Exact old observer sequence: pair, then a second snapshot of the private runtime.
        let (old_batches, old_samples) =
            dynamic_projection(&self.engine, &self.old, &self.speed, &rate, extras);
        let old_snapshot = self.old.lock().output_projection_snapshot();
        let (new_batches, new_snapshot, new_samples) =
            observer_dynamic_projection(&self.engine, &self.new, &self.speed, &rate, extras);
        assert_eq!(
            new_snapshot, old_snapshot,
            "complete runtime snapshot at {offset}"
        );
        assert_eq!(
            new_snapshot,
            self.new.lock().output_projection_snapshot(),
            "returned snapshot is the actual post-call state"
        );
        assert_eq!(
            new_samples, old_samples,
            "full expressions/rank/activation/address at {offset}"
        );
        // Include batch boundaries, replacements/exclusions and all source/value metadata.
        assert_eq!(format!("{new_batches:?}"), format!("{old_batches:?}"));
        assert_eq!(
            self.engine
                .resolved_values_with_contribution_batches(&new_batches),
            self.engine
                .resolved_values_with_contribution_batches(&old_batches)
        );
        (new_snapshot.instances.len(), new_samples.len())
    }
}
#[test]
fn observer_reuses_complete_idle_and_paused_snapshots() {
    let rig = Rig::new(false);
    assert_eq!(rig.at(0, &[]), (0, 0));
    assert_eq!(rig.at(250, &[]), (0, 0));
    rig.engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    assert_eq!(rig.at(500, &[]), (0, 0));
    assert!(rig.new.lock().output_projection_snapshot().global_paused);
    assert_eq!(rig.at(750, &[]), (0, 0));
}
#[test]
fn observer_active_pause_hotedit_and_release_keep_exact_post_sampling_state() {
    let mut rig = Rig::new(false);
    rig.activate();
    assert_eq!(rig.at(0, &[]), (1, 1));
    assert_eq!(rig.at(125, &[]), (1, 1));
    rig.engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    rig.at(250, &[]);
    rig.at(500, &[]);
    rig.engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            false,
        ))
        .unwrap();
    rig.at(750, &[]);
    rig.definition.revision += 1;
    rig.definition.name = "Edited observer wave".into();
    rig.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            dynamics: vec![rig.definition.clone()].into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    rig.activate();
    rig.at(1000, &[]);
    rig.set(DynamicSemanticValue::DynamicOff {
        instance_link: Uuid::from_u128(15),
        timing: DynamicValueTiming {
            fade_millis: Some(1000),
            delay_millis: Some(100),
        },
    });
    for at in [1100, 1500, 2099, 2100, 2200, 2500] {
        rig.at(at, &[]);
    }
}
#[test]
fn observer_current_and_extra_pending_static_keep_values_and_runtime_equal() {
    let rig = Rig::new(true);
    rig.programmers.set(
        rig.session,
        rig.target,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.35),
    );
    rig.activate();
    let extras = vec![(
        Uuid::from_u128(16),
        200,
        light_dynamics::DynamicAddressValue {
            fixture_id: rig.target,
            attribute: AttributeKey::intensity(),
            programmer_order: 1,
            changed_at_millis: START as u64,
            value: DynamicSemanticValue::Static {
                value: AttributeValue::Normalized(0.65),
                timing: DynamicValueTiming {
                    fade_millis: Some(500),
                    delay_millis: None,
                },
            },
        },
    )];
    for at in [0, 125, 250, 500, 1000, 1250] {
        rig.at(at, &extras);
    }
}
