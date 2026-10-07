use super::tests::{group, live_group_dynamic, positions};
use super::*;
use light_dynamics::{
    DynamicAddressValue, DynamicControlCursor, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicOutputFrameScratch, DynamicReference, DynamicRuntime,
    DynamicSemanticValue, DynamicValueTiming, Rational, replay_dynamic_controls,
};
use std::num::NonZeroUsize;

type ProgrammerRow = (Uuid, i16, DynamicAddressValue);

enum Rows {
    Programmer(Vec<ProgrammerRow>),
    Cue(Vec<light_playback::ActiveCueDynamicValue>),
    Playback(Vec<light_playback::ActiveDynamicPlayback>),
}

impl Rows {
    fn reconcile(
        &self,
        runtime: &mut DynamicRuntime,
        now: u64,
        snapshot: &light_engine::EngineSnapshot,
    ) {
        match self {
            Self::Programmer(rows) => {
                reconcile_programmer_dynamics(runtime, now, snapshot, rows, &[])
            }
            Self::Cue(rows) => reconcile_cue_dynamics(runtime, now, snapshot, rows),
            Self::Playback(rows) => {
                reconcile_dynamic_playbacks(runtime, now, snapshot, rows);
            }
        }
    }

    fn edit(&mut self) {
        let value = match self {
            Self::Programmer(rows) => &mut rows[0].2.value,
            Self::Cue(rows) => &mut rows[0].value,
            Self::Playback(rows) => {
                rows[0].size = 0.4;
                rows[0].paused = true;
                return;
            }
        };
        let DynamicSemanticValue::DynamicOn { overrides, .. } = value else {
            unreachable!()
        };
        overrides.size = 0.4;
    }

    fn clear(&mut self) {
        match self {
            Self::Programmer(rows) => rows.clear(),
            Self::Cue(rows) => rows.clear(),
            Self::Playback(rows) => rows.clear(),
        }
    }
}

fn on(definition: &DynamicDefinition) -> DynamicSemanticValue {
    DynamicSemanticValue::DynamicOn {
        instance_link: Uuid::new_v4(),
        lane_id: definition.lanes[0].id,
        dynamic: DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        },
        overrides: DynamicInstanceOverrides {
            size: 1.,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.,
        },
        timing: Default::default(),
    }
}

fn setup(kind: usize) -> (light_engine::EngineSnapshot, Rows, DynamicRuntime) {
    let definition = live_group_dynamic();
    let fixture = FixtureId::new();
    let mut snapshot = light_engine::EngineSnapshot {
        dynamics: vec![definition.clone()].into(),
        groups: vec![group(&[fixture])].into(),
        dynamic_stage_positions: Arc::new(positions([(fixture, 0.)])),
        ..Default::default()
    };
    let rows = match kind {
        0 => Rows::Programmer(vec![(
            Uuid::new_v4(),
            1,
            DynamicAddressValue {
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: on(&definition),
                programmer_order: 1,
                changed_at_millis: 10,
            },
        )]),
        1 => {
            let source = light_playback::SequenceMasterSource {
                playback_number: Some(1),
                playback_identity: Some(PlaybackIdentity::physical(1).unwrap()),
                cue_list_id: light_core::CueListId::new(),
                temporary: false,
            };
            Rows::Cue(vec![light_playback::ActiveCueDynamicValue {
                source,
                source_key: light_playback::CueDynamicSourceKey::Normal { source },
                output_enabled: true,
                sequence_master: 1.,
                snap_sequence_master: 1.,
                playback_number: Some(1),
                cue_list_id: source.cue_list_id,
                authored_cue_id: Uuid::new_v4(),
                current_cue_id: Uuid::new_v4(),
                priority: 1,
                changed_at: chrono::DateTime::from_timestamp_millis(10).unwrap(),
                transition_ordinal: 1,
                changed_at_millis: 10,
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: on(&definition),
            }])
        }
        2 => {
            snapshot.playbacks = vec![serde_json::from_value(serde_json::json!({
                "number": 1, "name": "Recorded playback", "target": {"type": "dynamic", "assignment": {
                    "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": definition.pool_number,
                        "embedded_fallback": {"definition": definition}}
                }}
            })).unwrap()].into();
            Rows::Playback(vec![serde_json::from_value(serde_json::json!({
                "dynamic_id": definition.id, "playback_number": 1, "enabled": true, "paused": false,
                "activated_at": chrono::DateTime::from_timestamp_millis(10).unwrap()
            })).unwrap()])
        }
        _ => unreachable!(),
    };
    let mut runtime = DynamicRuntime::default();
    // Cold dependency installation precedes recording/forking on both branches.
    runtime.install_definitions([definition]).unwrap();
    (snapshot, rows, runtime)
}

fn replay_since(
    live: &DynamicRuntime,
    pending: &mut DynamicRuntime,
    cursor: &mut DynamicControlCursor,
    at: u64,
) {
    let batch = live.controls_since(*cursor).unwrap().unwrap();
    assert!(!batch.is_empty());
    assert!(batch.operation_times().all(|time| time == at));
    let anchor = pending.committed_sample_boundary();
    replay_dynamic_controls(pending, &mut Default::default(), cursor, &batch, anchor).unwrap();
    assert_eq!(pending.snapshot(), live.snapshot());
}

#[test]
fn all_source_flows_record_delayed_starts_edits_and_releases_at_capture_time() {
    for kind in 0..3 {
        let (mut snapshot, mut rows, mut live) = setup(kind);
        let mut cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
        let mut pending = live.fork_for_pending_preview();
        rows.reconcile(&mut live, 100, &snapshot);
        assert_eq!(live.instance_count(), 1, "source kind {kind}");
        let id = live.snapshot().instances[0].id;
        assert_eq!(live.snapshot().instances[0].started_at_millis, 10);
        replay_since(&live, &mut pending, &mut cursor, 100);
        assert_eq!(pending.snapshot().instances[0].id, id);

        let unchanged = live.control_cursor();
        rows.reconcile(&mut live, 110, &snapshot);
        assert_eq!(
            live.control_cursor(),
            unchanged,
            "unchanged source kind {kind}"
        );
        assert!(live.controls_since(cursor).unwrap().unwrap().is_empty());

        rows.edit();
        let replacement = FixtureId::new();
        snapshot.groups = vec![group(&[replacement])].into();
        snapshot.dynamic_stage_positions = Arc::new(positions([(replacement, 5.)]));
        rows.reconcile(&mut live, 200, &snapshot);
        assert_eq!(live.snapshot().instances[0].id, id);
        assert_eq!(live.snapshot().instances[0].targets, vec![replacement]);
        assert_eq!(live.controllers()[0].1.size, 0.4);
        replay_since(&live, &mut pending, &mut cursor, 200);
        let unchanged = live.control_cursor();
        rows.reconcile(&mut live, 210, &snapshot);
        assert_eq!(live.control_cursor(), unchanged);

        rows.clear();
        rows.reconcile(&mut live, 300, &snapshot);
        assert_eq!(live.instance_count(), 0);
        replay_since(&live, &mut pending, &mut cursor, 300);
    }
}

#[test]
fn all_source_flows_record_embedded_fallback_before_start_and_rollback_it() {
    for kind in 0..3 {
        let (mut snapshot, rows, mut live) = setup(kind);
        snapshot.dynamics = Vec::new().into();
        live.install_definitions([]).unwrap();
        let mut cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
        let mut pending = live.fork_for_pending_preview();
        let mut scratch = DynamicOutputFrameScratch::default();
        let rejected: Result<(), &str> =
            live.with_output_frame_transaction(&mut scratch, |runtime| {
                rows.reconcile(runtime, 100, &snapshot);
                assert_eq!(
                    runtime.instance_count(),
                    1,
                    "fallback starts source kind {kind}"
                );
                Err("reject output")
            });
        assert!(rejected.is_err());
        assert_eq!(live.control_cursor(), Some(cursor));
        assert_eq!(live.instance_count(), 0);
        rows.reconcile(&mut live, 100, &snapshot);
        // A prefix containing only insertion must not start anything, but must enable the
        // following Start on a branch that never had the deleted show definition.
        let batch = live.controls_since(cursor).unwrap().unwrap();
        assert!(batch.len() >= 2);
        replay_dynamic_controls(
            &mut pending,
            &mut scratch,
            &mut cursor,
            &batch.prefix(1).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(pending.instance_count(), 0);
        replay_since(&live, &mut pending, &mut cursor, 100);
        let unchanged = live.control_cursor();
        rows.reconcile(&mut live, 110, &snapshot);
        assert_eq!(live.control_cursor(), unchanged);
    }
}

#[test]
fn programmer_overlay_gate_records_once_and_replays_without_destructive_off() {
    let (snapshot, rows, mut live) = setup(0);
    let Rows::Programmer(normal) = rows else {
        unreachable!()
    };
    let mut cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
    let mut pending = live.fork_for_pending_preview();
    reconcile_programmer_dynamics(&mut live, 100, &snapshot, &normal, &[]);
    replay_since(&live, &mut pending, &mut cursor, 100);
    let mut overlay = normal[0].clone();
    let DynamicSemanticValue::DynamicOn { instance_link, .. } = overlay.2.value.clone() else {
        unreachable!()
    };
    overlay.2.value = DynamicSemanticValue::DynamicOff {
        instance_link,
        timing: DynamicValueTiming {
            fade_millis: Some(50),
            ..Default::default()
        },
    };
    overlay.2.programmer_order = 2;
    overlay.2.changed_at_millis = 15;
    reconcile_programmer_dynamics(&mut live, 200, &snapshot, &normal, &[overlay.clone()]);
    let stored = live.snapshot();
    let transition = &stored.instances[0].controller_transitions[0];
    assert_eq!(transition.release_started_at_millis, None);
    assert_eq!(transition.output_gate.unwrap().started_at_millis, 200);
    replay_since(&live, &mut pending, &mut cursor, 200);
    let unchanged = live.control_cursor();
    reconcile_programmer_dynamics(&mut live, 210, &snapshot, &normal, &[overlay]);
    assert_eq!(live.control_cursor(), unchanged);
    reconcile_programmer_dynamics(&mut live, 300, &snapshot, &normal, &[]);
    assert!(
        live.snapshot().instances[0].controller_transitions[0]
            .output_gate
            .is_none()
    );
    replay_since(&live, &mut pending, &mut cursor, 300);
}

#[test]
fn failed_output_transaction_discards_source_reconciliation_and_its_records() {
    for kind in 0..3 {
        let (snapshot, rows, mut live) = setup(kind);
        let cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
        let before = live.snapshot();
        let mut scratch = DynamicOutputFrameScratch::default();
        let rejected: Result<(), &str> =
            live.with_output_frame_transaction(&mut scratch, |runtime| {
                rows.reconcile(runtime, 100, &snapshot);
                assert_eq!(runtime.instance_count(), 1);
                assert_eq!(runtime.control_cursor(), Some(cursor));
                assert!(runtime.controls_since(cursor).unwrap().unwrap().is_empty());
                Err("reject output after reconciliation")
            });
        assert!(rejected.is_err());
        assert_eq!(live.snapshot(), before, "source kind {kind}");
        assert_eq!(live.control_cursor(), Some(cursor));
        assert!(live.controls_since(cursor).unwrap().unwrap().is_empty());
        let accepted: Result<(), &str> =
            live.with_output_frame_transaction(&mut scratch, |runtime| {
                rows.reconcile(runtime, 100, &snapshot);
                Ok(())
            });
        accepted.unwrap();
        assert!(
            live.controls_since(cursor)
                .unwrap()
                .unwrap()
                .operation_times()
                .all(|at| at == 100)
        );
        assert_eq!(live.instance_count(), 1);
    }
}
