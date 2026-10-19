//! Live may advance during private restoration; its captured data remains isolated.
use super::*;
use light_core::{
    AttributeKey, AttributeValue, FixtureId, ManualClock, SessionId,
    programming::{PositionIntent, TargetReference},
};
use light_dynamics::{
    DynamicDefinition, DynamicRuntime, DynamicRuntimeSample, DynamicRuntimeSnapshot,
    DynamicSemanticValue,
};
use light_programmer::{DynamicProgrammerValueMutation, ProgrammerRegistry};
use std::{cell::RefCell, sync::mpsc};

type Callback = Box<dyn FnOnce(&DynamicRuntimeSnapshot)>;
struct Hook {
    runtime: usize,
    callback: Callback,
}
thread_local! { static CAPTURE_HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) }; }
fn identity(output: &OutputResource) -> usize {
    Arc::as_ptr(&output.dynamics) as usize
}
/// The hook is absent from production and belongs to this thread and this exact instance.
pub(super) fn after_capture(output: &OutputResource, portable: &DynamicRuntimeSnapshot) {
    let callback = CAPTURE_HOOK.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|hook| hook.runtime == identity(output))
        {
            slot.take().map(|hook| hook.callback)
        } else {
            None
        }
    });
    if let Some(callback) = callback {
        callback(portable);
    }
}
fn with_hook<T>(output: &OutputResource, callback: Callback, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<Hook>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CAPTURE_HOOK.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CAPTURE_HOOK.with(|slot| {
        slot.replace(Some(Hook {
            runtime: identity(output),
            callback,
        }))
    }));
    run()
}

const AT: i64 = 1_000_125;
const TARGET: FixtureId = FixtureId(Uuid::from_u128(501));
fn legacy_definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id":Uuid::from_u128(502),"pool_number":1,"revision":1,"name":"Concurrent observer",
        "target_binding":{"type":"targetless"},"lanes":[{
            "id":Uuid::from_u128(503),"attribute":"intensity","mode":"max_min",
            "keyframes":{"points":[{"position":0.,"source":{"type":"value","value":0.2},"interpolation":"linear"},{"position":0.5,"source":{"type":"value","value":0.8},"interpolation":"linear"}],"size":1.},
            "max_min":{"minimum":{"type":"current"},"maximum":{"type":"value","value":0.8},"function":"sinus","size":1.},
            "middle_amplitude":{"middle":{"type":"current"},"amplitude":0.3,"function":"sinus","size":1.},
            "speed_multiplier":{"numerator":1,"denominator":1},"width":1.}],
        "phase":{"ordering":{"type":"selection"},"offset_degrees":0.,"span_degrees":0.,"block_size":1,"repeats":1,"wings":false,"anchors_degrees":[]},
        "speed":{"type":"fixed","duration_millis":1000},"default_activation":"start_now"
    })).unwrap()
}
fn typed_definition() -> DynamicDefinition {
    use light_dynamics::*;
    let mut definition = legacy_definition();
    definition.id = Uuid::from_u128(504);
    definition.pool_number = 2;
    let lane = &mut definition.lanes[0];
    lane.id = Uuid::from_u128(505);
    let family = |reference| DynamicValueSource::Value {
        value: DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1., 2., 3.],
        )))),
    };
    lane.body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target { reference: None },
            component: None,
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: vec![
                DynamicKeyframe {
                    position: 0.,
                    source: family(TargetReference::Origin),
                    interpolation: ScalarInterpolation::Linear,
                },
                DynamicKeyframe {
                    position: 0.5,
                    source: family(TargetReference::Point {
                        point_id: Uuid::from_u128(506),
                    }),
                    interpolation: ScalarInterpolation::Linear,
                },
            ],
            size: 1.,
        }),
    });
    definition
}
fn fixture() -> OutputResource {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(AT - 125).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let output = super::super::publication_tests::output_with_programmers(programmers.clone());
    let definitions = [legacy_definition(), typed_definition()];
    output
        .replace_snapshot(EngineSnapshot {
            dynamics: definitions.to_vec().into(),
            ..Default::default()
        })
        .unwrap();
    let session = SessionId::new();
    programmers.start(session);
    programmers.set(
        session,
        TARGET,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.35),
    );
    programmers.set(
        session,
        TARGET,
        AttributeKey("position".into()),
        AttributeValue::Position(Arc::new(PositionIntent::angles(10., 20.))),
    );
    for (index, definition) in definitions.iter().enumerate() {
        assert!(programmers.apply_dynamic_values(
            session,
            &[DynamicProgrammerValueMutation::Set {
                fixture_id: TARGET,
                attribute: if index == 0 {
                    AttributeKey::intensity()
                } else {
                    AttributeKey("position".into())
                },
                value: DynamicSemanticValue::DynamicOn {
                    instance_link: Uuid::from_u128(510 + index as u128),
                    dynamic: light_dynamics::DynamicReference {
                        dynamic_id: Some(definition.id),
                        last_known_pool_number: definition.pool_number,
                        embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                            definition: Arc::new(definition.clone())
                        }
                    },
                    lane_id: definition.lanes[0].id,
                    overrides: light_dynamics::DynamicInstanceOverrides {
                        size: 1.,
                        speed_multiplier: light_dynamics::Rational::ONE,
                        phase_offset_degrees: 0.
                    },
                    timing: Default::default(),
                },
            }],
            None
        ));
    }
    // Warm authoritative sampling creates real controller/history and producer witnesses.
    output_scheduler::dynamic_projection(
        &output.engine,
        &output.dynamics,
        &output.speed_groups,
        &output.rate,
        &[],
    );
    clock.set(chrono::DateTime::from_timestamp_millis(AT).unwrap());
    output_scheduler::dynamic_projection(
        &output.engine,
        &output.dynamics,
        &output.speed_groups,
        &output.rate,
        &[],
    );
    output
}
type Projection = (
    light_engine::ResolvedValues,
    DynamicRuntimeSnapshot,
    Vec<DynamicRuntimeSample>,
);
/// Frozen old observer sequence, including the original Live guard through restore/expect.
fn old_projection(output: &OutputResource, captured: &DynamicRuntimeSnapshot) -> Projection {
    let snapshot = output.engine.snapshot();
    let mut private = DynamicRuntime::with_programming_contract_support(
        output.engine.supported_programming_contract(),
    );
    private
        .install_definitions(snapshot.dynamics.iter().cloned())
        .unwrap();
    let live = output.dynamics.lock();
    assert!(
        output.dynamics.try_lock().is_none(),
        "the old control retains Live during restore"
    );
    let portable = live.snapshot();
    assert_eq!(&portable, captured);
    private
        .restore_snapshot(portable)
        .expect("live Dynamic runtime snapshot remains restorable");
    drop(live);
    let private = Mutex::new(private);
    let (batches, samples) = output_scheduler::dynamic_projection(
        &output.engine,
        &private,
        &output.speed_groups,
        &output.rate,
        &[],
    );
    let runtime = private.lock().output_projection_snapshot();
    (
        output
            .engine
            .resolved_values_with_contribution_batches(&batches),
        runtime,
        samples,
    )
}
// Independent instances are stored in fresh HashMaps; their sample-vector interleaving
// is not a runtime ordering contract. Each real emission identity must remain unique.
fn canonical_samples(mut samples: Vec<DynamicRuntimeSample>) -> Vec<DynamicRuntimeSample> {
    let identity = |sample: &DynamicRuntimeSample| {
        (
            sample.instance_id,
            sample.controller_id,
            sample.target.0,
            sample.lane_id,
        )
    };
    samples.sort_by_key(identity);
    assert!(
        samples
            .windows(2)
            .all(|pair| identity(&pair[0]) != identity(&pair[1])),
        "the oracle must reject duplicate emission identities"
    );
    samples
}

struct Release(Option<mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn early_unlock_keeps_captured_projection_while_live_pause_hold_and_off_advance() {
    for off in [false, true] {
        let output = fixture();
        let before = output.dynamic_runtime_snapshot();
        assert_eq!(before.instances.len(), 2);
        assert!(
            before
                .instances
                .iter()
                .all(|instance| !instance.last_sample_values.is_empty())
        );
        std::thread::scope(|scope| {
            let (captured_tx, captured_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let mut release = Release(Some(release_tx));
            let observer = &output;
            let thread = scope.spawn(move || {
                with_hook(
                    observer,
                    Box::new(move |portable| {
                        captured_tx.send(portable.clone()).unwrap();
                        release_rx.recv().unwrap();
                    }),
                    || observer.visualization_dynamic_projection(&[], false),
                )
            });
            let captured = captured_rx.recv().unwrap();
            assert_eq!(captured, before);
            let historical = |snapshot: &DynamicRuntimeSnapshot| {
                snapshot
                    .instances
                    .iter()
                    .flat_map(|instance| {
                        instance
                            .expression_tape
                            .iter()
                            .flat_map(|tape| {
                                tape.roots.iter().map(|root| {
                                    light_dynamics::DynamicSampleExpression::Retained {
                                        tape: Arc::clone(tape),
                                        root: *root,
                                    }
                                    .operation_provenance()
                                    .unwrap()
                                    .handles()
                                    .iter()
                                    .map(|handle| handle.is_historical())
                                    .collect::<Vec<_>>()
                                })
                            })
                            .flatten()
                    })
                    .collect::<Vec<_>>()
            };
            let witnesses = historical(&captured);
            assert!(
                !witnesses.is_empty(),
                "race must carry real typed operation witnesses"
            );
            assert!(witnesses.iter().all(|historical| !historical));
            assert!(
                output.dynamics.try_lock().is_some(),
                "owned portable capture has released Live before restore"
            );
            let expected = old_projection(&output, &captured);
            assert!(!expected.2.is_empty());
            assert!(!expected.0.is_empty());
            let mut live = output
                .dynamics
                .try_lock()
                .expect("mutator acquires Live while observer is stopped before restore");
            for instance in &captured.instances {
                let controller = instance.controllers[0].id;
                if off {
                    live.off_controller(instance.id, controller, AT as u64, 0, 0)
                        .unwrap();
                } else {
                    live.set_controller_paused(instance.id, controller, true, AT as u64)
                        .unwrap();
                    live.set_controller_paused_with_resume(
                        instance.id,
                        controller,
                        false,
                        AT as u64,
                        Some(light_dynamics::ActivationPolicy::JoinSyncNow),
                    )
                    .unwrap();
                    live.set_controller_paused(instance.id, controller, true, AT as u64)
                        .unwrap();
                }
            }
            let changed = live.snapshot();
            drop(live);
            assert_ne!(changed, captured);
            if off {
                assert!(changed.instances.is_empty());
            } else {
                assert!(
                    changed
                        .instances
                        .iter()
                        .all(|instance| instance.controllers[0].paused
                            && !instance.synchronized_hold_values.is_empty())
                );
            }
            let checkpoint = output.dynamic_source_checkpoint().unwrap();
            assert_eq!(checkpoint.runtime, changed);
            checkpoint.clone().restore().unwrap();
            release.0.take().unwrap().send(()).unwrap();
            let actual = thread.join().unwrap();
            assert_eq!(
                actual.0, expected.0,
                "complete resolved values follow the captured checkpoint"
            );
            assert_eq!(
                actual.1, expected.1,
                "complete runtime snapshot follows the captured checkpoint"
            );
            assert_eq!(
                canonical_samples(actual.2),
                canonical_samples(expected.2),
                "complete samples match by unique emission identity across independent instances"
            );
            assert_eq!(
                historical(&captured),
                witnesses,
                "restoring a private shared tape cannot mark the captured witnesses historical"
            );
            assert_eq!(
                output.dynamic_runtime_snapshot(),
                changed,
                "observer never overwrites concurrent Live changes"
            );
            assert_eq!(
                output.dynamic_source_checkpoint().unwrap(),
                checkpoint,
                "runtime/catalogue pair remains coherent"
            );
        });
    }
}
#[test]
fn capture_hook_is_thread_and_instance_scoped_and_restored_after_panic() {
    let first = fixture();
    let other = fixture();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_hook(
            &first,
            Box::new(move |_| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }),
            || {
                other.visualization_dynamic_projection(&[], false);
                std::thread::scope(|scope| {
                    scope
                        .spawn(|| first.visualization_dynamic_projection(&[], false))
                        .join()
                        .unwrap();
                });
                assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
                panic!("hook restoration before invocation");
            },
        )
    }));
    assert!(result.is_err());
    first.visualization_dynamic_projection(&[], false);
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    let counter = hits.clone();
    with_hook(
        &first,
        Box::new(move |_| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
        || first.visualization_dynamic_projection(&[], false),
    );
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
}
