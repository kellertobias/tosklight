use super::*;
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;

mod family_live;
mod projected_sources;
mod snapshot_publication;
mod source_lifecycle;
mod transition_notifications;

#[test]
fn output_transaction_publishes_runtime_and_origins_together_and_reuses_unchanged_catalogue() {
    use crate::runtime::dynamic_source_origins::{
        DynamicProgrammerSourceLane, DynamicSourceBinding, DynamicSourceOrigin,
    };
    let origins = SharedDynamicSourceOrigins::default();
    let previous = origins.load_full();
    let owner = light_core::ProgrammerId::new();
    let link = Uuid::new_v4();
    let binding = DynamicSourceBinding::Authored {
        instance_id: Uuid::new_v4(),
        controller_id: programmer_dynamic_controller_id(owner, link),
        target: FixtureId::new(),
        lane_id: Uuid::new_v4(),
    };
    let origin = DynamicSourceOrigin::Programmer {
        programmer_id: owner,
        lane: DynamicProgrammerSourceLane::Preload,
        instance_link: link,
        changed_at_millis: 123,
        programmer_order: 7,
    };
    let mut runtime = DynamicRuntime::default();
    let before = runtime.snapshot();
    let mut scratch = DynamicOutputFrameScratch::default();
    let rejected: Result<(_, _), &str> = with_dynamic_source_transaction(
        &mut runtime,
        &mut scratch,
        &origins,
        |runtime, candidate| {
            runtime.set_global_paused(true, 123);
            candidate.bind(binding, origin.clone()).unwrap();
            Err::<(), _>("render rejected")
        },
    );
    assert_eq!(rejected.unwrap_err(), "render rejected");
    assert_eq!(runtime.snapshot(), before);
    assert!(Arc::ptr_eq(&previous, &origins.load_full()));
    assert!(origins.load().binding(&binding).is_none());

    let (id, committed) = with_dynamic_source_transaction(
        &mut runtime,
        &mut scratch,
        &origins,
        |runtime, candidate| {
            runtime.set_global_paused(true, 123);
            candidate.bind(binding, origin.clone())
        },
    )
    .unwrap();
    assert_eq!(committed.binding(&binding), Some(id));
    assert!(Arc::ptr_eq(&committed, &origins.load_full()));
    assert!(
        previous.binding(&binding).is_none(),
        "retained frame stays immutable"
    );
    assert_ne!(runtime.snapshot(), before);

    let (same_id, unchanged) =
        with_dynamic_source_transaction(&mut runtime, &mut scratch, &origins, |_, candidate| {
            candidate.bind(binding, origin.clone())
        })
        .unwrap();
    assert_eq!(same_id, id);
    assert!(Arc::ptr_eq(&committed, &unchanged));
    assert!(Arc::ptr_eq(&committed, &origins.load_full()));
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Retry wave",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    })).unwrap()
}

#[test]
fn failed_render_does_not_acknowledge_or_install_dynamic_reconciliation_and_retry_is_identical() {
    use crate::runtime::dynamic_snapshot_publication::RetainedFrameCapture;
    use std::{num::NonZeroUsize, time::Instant};
    let clock = Arc::new(light_core::ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = light_core::SessionId::new();
    programmers.start(session);
    let fixture = FixtureId::new();
    let definition = definition();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: 1,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: Default::default(),
            },
        }],
        None
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let before = runtime.snapshot();
    let dynamics = Mutex::new(runtime);
    let dynamic_snapshot = crate::runtime::DynamicSnapshotPublication::new(engine.snapshot());
    dynamic_snapshot
        .begin_retained_history(
            &mut dynamics.lock(),
            &engine.snapshot(),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    let capture_cursor = dynamic_snapshot.input_capture_cursor().unwrap();
    let selected_at = Instant::now();
    let origins = SharedDynamicSourceOrigins::default();
    let before_origins = origins.load_full();
    let groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
    }));
    let cache = ProgrammerReconciliationCache::default();
    let rate = AtomicU16::new(40);
    clock.advance_millis(125);
    let frame = RetainedFrameCapture::select(
        engine.prepare_output_frame(Default::default()),
        &dynamic_snapshot,
        selected_at,
    );
    engine.clear_programmer_transitions();
    let mut rejected_value = None;
    let failed = dynamic_output_frame(
        &engine,
        &frame,
        frame.retained(),
        &[],
        &dynamics,
        &dynamic_snapshot,
        &origins,
        &groups,
        &rate,
        &cache,
        &LiveFamilyAdapters::default(),
        legacy(|batches| {
            assert_eq!(batches.len(), 1);
            rejected_value = Some(batches[0].samples()[0].value().value.clone());
            engine.render_prepared(&frame, batches)
        }),
    );
    assert!(matches!(failed, Err(EngineError::StalePreparedFrame)));
    assert_eq!(dynamics.lock().committed_sample_boundary(), None);
    assert_eq!(dynamics.lock().snapshot(), before);
    assert!(Arc::ptr_eq(&before_origins, &origins.load_full()));
    assert!(cache.changed(frame.dynamic_programmer_values(), &frame.snapshot()));
    assert!(
        dynamic_snapshot
            .input_captures_since(capture_cursor)
            .unwrap()
            .is_empty()
    );
    assert!(dynamic_snapshot.input_capture_due(selected_at));

    let retry = RetainedFrameCapture::select(
        engine.prepare_output_frame(Default::default()),
        &dynamic_snapshot,
        selected_at,
    );
    let committed = dynamic_output_frame(
        &engine,
        &retry,
        retry.retained(),
        &[],
        &dynamics,
        &dynamic_snapshot,
        &origins,
        &groups,
        &rate,
        &cache,
        &LiveFamilyAdapters::default(),
        legacy(|batches| {
            assert_eq!(
                Some(batches[0].samples()[0].value().value.clone()),
                rejected_value
            );
            engine.render_prepared(&retry, batches)
        }),
    )
    .unwrap();
    assert_eq!(committed.runtime.instances.len(), 1);
    assert_eq!(committed.samples.len(), 1);
    let boundary = committed
        .sample_boundary
        .expect("accepted sample has its own history anchor");
    assert_eq!(Some(boundary), dynamics.lock().committed_sample_boundary());
    let captures = dynamic_snapshot
        .input_captures_since(capture_cursor)
        .unwrap();
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].live_sample, Some(boundary));
    assert_eq!(Some(captures[0].controls), dynamics.lock().control_cursor());
    assert!(std::ptr::eq(captures[0].frame.as_ref(), &*retry));
    assert!(!dynamic_snapshot.input_capture_due(selected_at));
    assert_eq!(boundary.scope(), DynamicSampleScope::WholeRuntime);
    assert_eq!(
        boundary.sampled_at_millis(),
        retry.sampled_at().timestamp_millis() as u64
    );
    assert!(
        !committed.events.is_empty(),
        "only the successful start publishes Dynamic events"
    );
    assert!(!cache.changed(retry.dynamic_programmer_values(), &retry.snapshot()));
    assert_ne!(dynamics.lock().snapshot(), before);
    let mut occurrences = HashSet::new();
    committed.samples[0]
        .expression
        .visit_source_occurrences(&mut |id| {
            occurrences.insert(id);
        })
        .unwrap();
    assert_eq!(
        occurrences.len(),
        1,
        "production sampling binds the captured authored row before retaining it"
    );
    let id = *occurrences.iter().next().unwrap();
    let record = committed.origins.get(id).unwrap();
    assert!(matches!(
        record.origin,
        crate::runtime::dynamic_source_origins::DynamicSourceOrigin::Programmer {
            lane: crate::runtime::dynamic_source_origins::DynamicProgrammerSourceLane::Live,
            ..
        }
    ));
    crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint::capture(
        dynamics.lock().snapshot(),
        &committed.origins,
    )
    .unwrap();
}

fn cue_dynamic_row(
    definition: &DynamicDefinition,
    fixture_id: FixtureId,
    link: Uuid,
    source_key: light_playback::CueDynamicSourceKey,
) -> light_playback::ActiveCueDynamicValue {
    let source = source_key.source();
    light_playback::ActiveCueDynamicValue {
        source,
        source_key,
        output_enabled: true,
        sequence_master: 1.0,
        snap_sequence_master: 1.0,
        playback_number: source.playback_number,
        cue_list_id: source.cue_list_id,
        current_cue_id: Uuid::new_v4(),
        authored_cue_id: Uuid::new_v4(),
        priority: 0,
        changed_at: chrono::DateTime::from_timestamp_millis(1000).unwrap(),
        transition_ordinal: 7,
        changed_at_millis: 1000,
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::DynamicOn {
            instance_link: link,
            lane_id: definition.lanes[0].id,
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            overrides: DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: Default::default(),
        },
    }
}

#[test]
fn normal_and_two_temporary_kinds_of_same_cue_dynamic_have_independent_controllers() {
    use light_playback::{CueDynamicSourceKey as Key, SequenceMasterSource, TemporaryPlaybackKind};
    let definition = definition();
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    let source = SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    };
    let temporary = SequenceMasterSource {
        temporary: true,
        ..source
    };
    let keys = [
        Key::Normal { source },
        Key::Temporary {
            source: temporary,
            kind: TemporaryPlaybackKind::TempButton,
        },
        Key::Temporary {
            source: temporary,
            kind: TemporaryPlaybackKind::TempFader,
        },
    ];
    let mut rows = keys.map(|key| cue_dynamic_row(&definition, fixture, link, key));
    let snapshot = light_engine::EngineSnapshot {
        dynamics: vec![definition.clone()].into(),
        ..Default::default()
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    reconcile_cue_dynamics(&mut runtime, 1000, &snapshot, &rows);
    assert_eq!(runtime.snapshot().instances.len(), 3);
    let normal_instance = runtime.controller(keys[0].controller_id(link)).unwrap().0;
    let fader_instance = runtime.controller(keys[2].controller_id(link)).unwrap().0;

    // Off belongs to one concrete temporary source, even though all three share a pool link.
    rows[1].value = DynamicSemanticValue::DynamicOff {
        instance_link: link,
        timing: Default::default(),
    };
    rows[0].current_cue_id = Uuid::new_v4();
    rows[0].changed_at_millis = 1300;
    rows[0].changed_at = chrono::DateTime::from_timestamp_millis(1300).unwrap();
    rows[0].transition_ordinal += 1;
    reconcile_cue_dynamics(&mut runtime, 1300, &snapshot, &rows);
    assert_eq!(
        runtime.controller(keys[0].controller_id(link)).unwrap().0,
        normal_instance
    );
    assert_eq!(
        runtime.controller(keys[2].controller_id(link)).unwrap().0,
        fader_instance
    );
    assert!(runtime.controller(keys[1].controller_id(link)).is_none());
    assert!(
        runtime
            .snapshot()
            .instances
            .iter()
            .all(|instance| instance.started_at_millis == 1000)
    );
}

#[test]
fn swap_masks_cue_output_without_restarting_its_clock_and_master_is_captured() {
    let definition = definition();
    let fixture = FixtureId::new();
    let link = Uuid::new_v4();
    let source = light_playback::SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    };
    let mut row = cue_dynamic_row(
        &definition,
        fixture,
        link,
        light_playback::CueDynamicSourceKey::Normal { source },
    );
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let engine = Engine::new(ProgrammerRegistry::default());
    let snapshot = Arc::new(light_engine::EngineSnapshot {
        dynamics: vec![definition].into(),
        ..Default::default()
    });
    let addresser = engine.frame_addresser();
    let programmers = Arc::new(vec![]);
    let speeds = capture_dynamic_speed_transports(
        &Mutex::new(std::array::from_fn(|_| {
            light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
        })),
        1000,
    );
    let mut run = |row: &light_playback::ActiveCueDynamicValue, millis| {
        let inputs = CapturedDynamicInputs {
            now: chrono::DateTime::from_timestamp_millis(millis).unwrap(),
            speed_transports: &speeds,
            rate: 40,
            snapshot: &snapshot,
            programmer_values: &programmers,
            programmer_rows: None,
            cue_values: std::slice::from_ref(row),
            dynamic_playbacks: &[],
            playback_paused: false,
            addresser: &addresser,
            extra_programmer_values: &[],
            programmer_reconciliation_cache: None,
            force_source_reconciliation: false,
        };
        let sources = TickSources::new(&engine);
        let sampled = sample_captured_dynamic_inputs(&mut runtime, &inputs, &sources);
        let projected = project_captured_dynamic_sample(&inputs, &sampled, &sources);
        (sampled, projected)
    };
    let (initial, _) = run(&row, 1000);
    let id = initial.after_runtime.instances[0].id;
    row.output_enabled = false;
    let (muted, output) = run(&row, 1250);
    assert!(muted.samples.is_empty());
    assert!(output.is_empty());
    assert_eq!(muted.after_runtime.instances[0].id, id);
    row.output_enabled = true;
    row.sequence_master = 0.4;
    let (resumed, output) = run(&row, 1500);
    assert_eq!(resumed.after_runtime.instances[0].id, id);
    assert_eq!(resumed.after_runtime.instances[0].started_at_millis, 1000);
    assert_eq!(resumed.samples[0].legacy().unwrap().value, 1.0);
    assert_eq!(output[0].samples()[0].value().value.normalized(), Some(0.4));

    // A suppressed or temporary FAT row must not be mistaken for permanent takeover.
    row.value = DynamicSemanticValue::FixAt {
        value: 1.0,
        timing: Default::default(),
    };
    row.output_enabled = false;
    assert!(persistent_fat_values(&[], std::slice::from_ref(&row)).is_empty());
    row.output_enabled = true;
    row.source.temporary = true;
    assert!(persistent_fat_values(&[], std::slice::from_ref(&row)).is_empty());
}

#[test]
fn cue_fat_arbitration_preserves_submillisecond_time_and_action_ordinal() {
    let definition = definition();
    let fixture = FixtureId::new();
    let source = light_playback::SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId::new(),
        temporary: false,
    };
    let mut older = cue_dynamic_row(
        &definition,
        fixture,
        Uuid::new_v4(),
        light_playback::CueDynamicSourceKey::Normal { source },
    );
    older.current_cue_id = Uuid::from_u128(u128::MAX);
    older.value = DynamicSemanticValue::FixAt {
        value: 0.2,
        timing: Default::default(),
    };
    older.transition_ordinal = 50;
    let mut newer = older.clone();
    newer.current_cue_id = Uuid::from_u128(1);
    newer.value = DynamicSemanticValue::FixAt {
        value: 0.8,
        timing: Default::default(),
    };
    newer.changed_at += chrono::Duration::microseconds(500);
    newer.transition_ordinal = 1;
    let engine = Engine::new(ProgrammerRegistry::default());
    let addresser = engine.frame_addresser();
    let sources = TickSources::new(&engine);
    let now = chrono::DateTime::from_timestamp_millis(2000).unwrap();
    let resolve = |rows: &[light_playback::ActiveCueDynamicValue]| {
        let candidates = collect_dynamic_candidates(
            &addresser,
            &[],
            rows,
            &[],
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &sources,
            2000,
        );
        dynamic_contribution_batch(candidates, &sources, now)
    };
    let output = resolve(&[newer.clone(), older.clone()]);
    assert_eq!(output.samples()[0].value().value.normalized(), Some(0.8));
    assert_eq!(output.samples()[0].value().changed_at, newer.changed_at);

    newer.changed_at = older.changed_at;
    newer.transition_ordinal = 51;
    for rows in [
        [older.clone(), newer.clone()],
        [newer.clone(), older.clone()],
    ] {
        let output = resolve(&rows);
        assert_eq!(output.samples()[0].value().value.normalized(), Some(0.8));
        assert_eq!(output.samples()[0].value().programmer_order, 51);
    }
}
