use super::*;
use light_core::{NativeColorBinding, NativeColorIdentity};
use light_dynamics::{
    DynamicAddressValue, DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicLaneBody, DynamicReference, DynamicRuntime,
    DynamicRuntimeError, DynamicSemanticValue, DynamicValue, DynamicValueAddress,
    DynamicValueSource, DynamicValueTiming, MaxMinConfiguration, PeriodicFunction,
    ProgrammingLaneBody, ProgrammingLaneConfiguration, PwmShape, Rational, SpatialPosition,
};
use light_programmer::{GroupDefinition, GroupFixtureSource, GroupReference, SelectionRule};
use std::sync::Arc;

type ProgrammerRow = (Uuid, i16, DynamicAddressValue);

#[test]
fn valid_destination_reuses_warm_transitions_without_touching_live() {
    let [a, b, c] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let dynamic = definition(serde_json::json!({"type": "live_group", "group_id": "front"}));
    let rows = [programmer_row(
        Uuid::new_v4(),
        a,
        &dynamic,
        Uuid::new_v4(),
        1.0,
        10,
    )];
    let live_snapshot = snapshot(&dynamic, vec![explicit_group("front", &[a, b])]);
    let mut live = runtime(&dynamic);
    reconcile_programmer_dynamics(&mut live, 10, &live_snapshot, &rows, &[]);
    let live_before = live.snapshot();

    let destination = snapshot(&dynamic, vec![explicit_group("front", &[b, c])]);
    let mut warm = live.fork_for_cold_install();
    reconcile_programmer_dynamics(&mut warm, 20, &destination, &rows, &[]);
    let mut cold = live.fork_for_cold_install();
    let report =
        reconcile_cold_dynamic_candidate(&mut cold, inputs(&destination, &rows, &[], 20)).unwrap();

    assert!(report.scope_requirements.is_empty());
    assert_eq!(comparable(&cold), comparable(&warm));
    let instance = &cold.snapshot().instances[0];
    assert_eq!(instance.targets, vec![b, c]);
    assert_eq!(instance.started_at_millis, 10);
    assert_eq!(live.snapshot(), live_before);
}

#[test]
fn absent_and_deliberately_empty_groups_are_passive_and_distinguishable() {
    let fixture = FixtureId::new();
    let dynamic = definition(serde_json::json!({"type": "live_group", "group_id": "front"}));
    let rows = [programmer_row(
        Uuid::new_v4(),
        fixture,
        &dynamic,
        Uuid::new_v4(),
        1.0,
        10,
    )];

    let absent = snapshot(&dynamic, Vec::new());
    let mut candidate = runtime(&dynamic);
    let missing =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&absent, &rows, &[], 10)).unwrap();
    assert_eq!(
        reasons(&missing.scope_requirements),
        vec![ColdDynamicScopeReason::MissingGroup {
            group_id: "front".into()
        }]
    );
    assert!(candidate.snapshot().instances.is_empty());

    let stored_empty = snapshot(&dynamic, vec![explicit_group("front", &[])]);
    let mut candidate = runtime(&dynamic);
    let empty =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&stored_empty, &rows, &[], 10))
            .unwrap();
    assert_eq!(
        reasons(&empty.scope_requirements),
        vec![ColdDynamicScopeReason::EmptyGroup {
            group_id: "front".into()
        }]
    );
    assert_ne!(missing.scope_requirements, empty.scope_requirements);

    // An already-running controller whose Group becomes deliberately empty keeps its clock
    // and instance, exactly as warm reconciliation retains it with no targets.
    let mut live = runtime(&dynamic);
    let populated = snapshot(&dynamic, vec![explicit_group("front", &[fixture])]);
    reconcile_programmer_dynamics(&mut live, 10, &populated, &rows, &[]);
    let mut candidate = live.fork_for_cold_install();
    let retained =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&stored_empty, &rows, &[], 30))
            .unwrap();
    assert_eq!(
        reasons(&retained.scope_requirements),
        vec![ColdDynamicScopeReason::EmptyGroup {
            group_id: "front".into()
        }]
    );
    let instance = &candidate.snapshot().instances[0];
    assert!(instance.targets.is_empty());
    assert_eq!(instance.started_at_millis, 10);
}

#[test]
fn invalid_existing_group_fails_with_resolver_context_while_warm_still_tolerates_it() {
    let fixture = FixtureId::new();
    let dynamic = definition(serde_json::json!({"type": "live_group", "group_id": "front"}));
    let rows = [programmer_row(
        Uuid::new_v4(),
        fixture,
        &dynamic,
        Uuid::new_v4(),
        1.0,
        10,
    )];
    let destination = snapshot(&dynamic, vec![reference_group("front", "ghost")]);

    let mut candidate = runtime(&dynamic);
    let error =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&destination, &rows, &[], 10))
            .unwrap_err();
    let [failure] = error.failures.as_slice() else {
        panic!("expected one failure: {error:?}")
    };
    assert_eq!(
        failure.operation,
        ColdDynamicReconciliationOperation::GroupResolution
    );
    assert_eq!(
        failure.context.flow,
        ColdDynamicReconciliationFlow::Programmer
    );
    let ColdDynamicReconciliationCause::InvalidGroup { group_id, resolver } = &failure.cause else {
        panic!("expected an invalid Group: {failure:?}")
    };
    assert_eq!(group_id, "front");
    assert!(
        resolver.contains("ghost"),
        "resolver context is preserved: {resolver}"
    );
    assert!(error.scope_requirements.is_empty());

    let mut warm = runtime(&dynamic);
    reconcile_programmer_dynamics(&mut warm, 10, &destination, &rows, &[]);
    assert!(warm.snapshot().instances.is_empty());
}

#[test]
fn start_still_rejects_invalid_controller_values_when_the_scope_is_empty() {
    let dynamic = definition(serde_json::json!({"type": "live_group", "group_id": "front"}));
    let rows = [programmer_row(
        Uuid::new_v4(),
        FixtureId::new(),
        &dynamic,
        Uuid::new_v4(),
        -1.0,
        10,
    )];
    let destination = snapshot(&dynamic, Vec::new());
    let mut candidate = runtime(&dynamic);
    let error =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&destination, &rows, &[], 10))
            .unwrap_err();

    assert_eq!(
        operations_and_causes(&error),
        vec![(
            ColdDynamicReconciliationOperation::Start,
            ColdDynamicReconciliationCause::Runtime(DynamicRuntimeError::InvalidController)
        )]
    );
    // The absent Group itself remains a passive requirement, not the failure.
    assert_eq!(
        reasons(&error.scope_requirements),
        vec![ColdDynamicScopeReason::MissingGroup {
            group_id: "front".into()
        }]
    );
}

#[test]
fn invalid_embedded_fallback_definition_fails() {
    let mut fallback = definition(serde_json::json!({"type": "targetless"}));
    fallback.revision = 0;
    let rows = [programmer_row(
        Uuid::new_v4(),
        FixtureId::new(),
        &fallback,
        Uuid::new_v4(),
        1.0,
        10,
    )];
    // The destination registry no longer contains the referenced Dynamic.
    let destination = light_engine::EngineSnapshot::default();
    let mut candidate = DynamicRuntime::default();
    let error =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&destination, &rows, &[], 10))
            .unwrap_err();

    let [failure] = error.failures.as_slice() else {
        panic!("expected one failure: {error:?}")
    };
    assert_eq!(
        failure.operation,
        ColdDynamicReconciliationOperation::FallbackDefinition
    );
    assert!(matches!(
        failure.cause,
        ColdDynamicReconciliationCause::Runtime(DynamicRuntimeError::InvalidDefinition(_))
    ));
}

#[test]
fn inconsistent_enabled_playback_rows_fail_and_empty_playback_scope_is_passive() {
    let dynamic = definition(serde_json::json!({"type": "targetless"}));
    let mut destination = snapshot(&dynamic, Vec::new());
    destination.playbacks = vec![
        dynamic_playback(&dynamic, 1, serde_json::json!(null)),
        dynamic_playback(
            &dynamic,
            2,
            serde_json::json!({"type": "frozen_targets", "targets": []}),
        ),
        serde_json::from_value(serde_json::json!({
            "number": 3, "name": "Cue list",
            "target": {"type": "cue_list", "cue_list_id": Uuid::new_v4()}
        }))
        .unwrap(),
    ]
    .into();
    let mut disabled_absent = active_playback(&dynamic, 9);
    disabled_absent.enabled = false;
    let playbacks = [
        active_playback(&dynamic, 1),
        active_playback(&dynamic, 2),
        active_playback(&dynamic, 3),
        active_playback(&dynamic, 4),
        disabled_absent,
    ];

    let mut candidate = runtime(&dynamic);
    let error = reconcile_cold_dynamic_candidate(
        &mut candidate,
        ColdDynamicReconciliationInputs {
            playbacks: &playbacks,
            ..inputs(&destination, &[], &[], 1_000)
        },
    )
    .unwrap_err();

    let mut failed = error
        .failures
        .iter()
        .map(|failure| {
            let light_dynamics::DynamicControllerSource::Playback {
                playback_number, ..
            } = failure.context.source
            else {
                panic!("expected a Playback context: {failure:?}")
            };
            (playback_number, failure.cause.clone())
        })
        .collect::<Vec<_>>();
    failed.sort_by_key(|(number, _)| *number);
    assert_eq!(
        failed,
        vec![
            (3, ColdDynamicReconciliationCause::NonDynamicPlaybackTarget),
            (4, ColdDynamicReconciliationCause::MissingPlaybackDefinition),
        ]
    );
    let mut passive = error
        .scope_requirements
        .iter()
        .map(|requirement| {
            (
                requirement.context.source.clone(),
                requirement.reason.clone(),
            )
        })
        .collect::<Vec<_>>();
    passive.sort_by_key(|(source, _)| format!("{source:?}"));
    assert_eq!(
        passive,
        [1, 2].map(|playback_number| (
            light_dynamics::DynamicControllerSource::physical_playback(playback_number),
            ColdDynamicScopeReason::EmptyScope
        ))
    );
}

#[test]
fn failed_candidate_leaves_live_unchanged_and_retry_preserves_activation_time() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let grouped = definition(serde_json::json!({"type": "live_group", "group_id": "front"}));
    let targetless = definition(serde_json::json!({"type": "targetless"}));
    let programmer = Uuid::new_v4();
    let running = programmer_row(programmer, a, &grouped, Uuid::new_v4(), 1.0, 10);
    let mut live_snapshot = snapshot(&grouped, vec![explicit_group("front", &[a])]);
    live_snapshot.dynamics = vec![grouped.clone(), targetless.clone()].into();
    let mut live = DynamicRuntime::default();
    live.install_definitions([grouped.clone(), targetless.clone()])
        .unwrap();
    reconcile_programmer_dynamics(
        &mut live,
        10,
        &live_snapshot,
        std::slice::from_ref(&running),
        &[],
    );
    let live_before = live.snapshot();

    let added = programmer_row(programmer, b, &targetless, Uuid::new_v4(), 1.0, 15);
    let rows = [running, added];
    let mut invalid = live_snapshot.clone();
    invalid.groups = vec![reference_group("front", "ghost")].into();
    let mut rejected = live.fork_for_cold_install();
    reconcile_cold_dynamic_candidate(&mut rejected, inputs(&invalid, &rows, &[], 40)).unwrap_err();
    assert_eq!(live.snapshot(), live_before);

    let mut first = live.fork_for_cold_install();
    reconcile_cold_dynamic_candidate(&mut first, inputs(&live_snapshot, &rows, &[], 40)).unwrap();
    let mut retry = live.fork_for_cold_install();
    reconcile_cold_dynamic_candidate(&mut retry, inputs(&live_snapshot, &rows, &[], 40)).unwrap();
    assert_eq!(live.snapshot(), live_before);
    for candidate in [&first, &retry] {
        let mut started = candidate
            .snapshot()
            .instances
            .iter()
            .map(|instance| (instance.definition.id, instance.started_at_millis))
            .collect::<Vec<_>>();
        started.sort();
        let mut expected = vec![(grouped.id, 10), (targetless.id, 15)];
        expected.sort();
        assert_eq!(started, expected);
    }
    assert_eq!(comparable(&first), comparable(&retry));
}

#[test]
fn removed_and_turned_off_controllers_follow_warm_release_rules() {
    let fixture = FixtureId::new();
    let dynamic = definition(serde_json::json!({"type": "targetless"}));
    let [removed_programmer, off_programmer] = [Uuid::new_v4(), Uuid::new_v4()];
    let link = Uuid::new_v4();
    let rows = [
        programmer_row(removed_programmer, fixture, &dynamic, link, 1.0, 10),
        programmer_row(off_programmer, fixture, &dynamic, link, 1.0, 10),
    ];
    let destination = snapshot(&dynamic, Vec::new());
    let mut live = runtime(&dynamic);
    reconcile_programmer_dynamics(&mut live, 10, &destination, &rows, &[]);
    assert_eq!(live.controllers().len(), 2);

    let mut off = rows[1].clone();
    off.2.programmer_order = 2;
    off.2.changed_at_millis = 20;
    off.2.value = DynamicSemanticValue::DynamicOff {
        instance_link: link,
        timing: DynamicValueTiming {
            fade_millis: Some(500),
            ..Default::default()
        },
    };
    let destination_rows = [off];
    let mut warm = live.fork_for_cold_install();
    reconcile_programmer_dynamics(&mut warm, 30, &destination, &destination_rows, &[]);
    let mut cold = live.fork_for_cold_install();
    let report = reconcile_cold_dynamic_candidate(
        &mut cold,
        inputs(&destination, &destination_rows, &[], 30),
    )
    .unwrap();

    assert!(report.scope_requirements.is_empty());
    assert_eq!(comparable(&cold), comparable(&warm));
    let off_id = light_dynamics::programmer_dynamic_controller_id(
        light_core::ProgrammerId(off_programmer),
        link,
    );
    assert!(
        cold.snapshot()
            .instances
            .iter()
            .flat_map(|instance| &instance.controller_transitions)
            .any(|transition| transition.controller_id == off_id
                && transition.release_started_at_millis == Some(30)
                && transition.release_duration_millis == 500)
    );
    assert_eq!(live.controllers().len(), 2);
}

#[test]
fn unavailable_native_capability_remains_passive() {
    let fixture = FixtureId::new();
    let dynamic = native_definition();
    let rows = [programmer_row(
        Uuid::new_v4(),
        fixture,
        &dynamic,
        Uuid::new_v4(),
        1.0,
        10,
    )];
    let destination = snapshot(&dynamic, Vec::new());
    // No original-model resolver is installed: the Direct Color lane compiles suspended.
    let mut candidate = runtime(&dynamic);
    let report =
        reconcile_cold_dynamic_candidate(&mut candidate, inputs(&destination, &rows, &[], 10))
            .unwrap();

    assert!(report.scope_requirements.is_empty());
    let instances = candidate.snapshot().instances;
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].targets, vec![fixture]);
}

fn inputs<'a>(
    snapshot: &'a light_engine::EngineSnapshot,
    programmer_values: &'a [ProgrammerRow],
    cue_values: &'a [light_playback::ActiveCueDynamicValue],
    captured_at_millis: u64,
) -> ColdDynamicReconciliationInputs<'a> {
    ColdDynamicReconciliationInputs {
        captured_at_millis,
        snapshot,
        programmer_values,
        extra_programmer_values: &[],
        cue_values,
        playbacks: &[],
        playback_paused: false,
    }
}

fn reasons(requirements: &[ColdDynamicScopeRequirement]) -> Vec<ColdDynamicScopeReason> {
    requirements
        .iter()
        .map(|requirement| requirement.reason.clone())
        .collect()
}

fn operations_and_causes(
    error: &ColdDynamicReconciliationError,
) -> Vec<(
    ColdDynamicReconciliationOperation,
    ColdDynamicReconciliationCause,
)> {
    error
        .failures
        .iter()
        .map(|failure| (failure.operation, failure.cause.clone()))
        .collect()
}

/// Runtime state that must agree between warm and cold reconciliation. Instance IDs and the
/// per-reconciliation Preset dependency token are fresh random values and are excluded.
fn comparable(runtime: &DynamicRuntime) -> Vec<String> {
    let mut instances = runtime
        .snapshot()
        .instances
        .into_iter()
        .map(|instance| {
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{}|{:?}",
                instance.definition.id,
                instance.targets,
                instance.phase_by_lane_target,
                instance.controllers,
                instance.controller_transitions,
                instance.started_at_millis,
                instance.lane_selections,
            )
        })
        .collect::<Vec<_>>();
    instances.sort();
    instances
}

fn runtime(definition: &DynamicDefinition) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

fn snapshot(
    definition: &DynamicDefinition,
    groups: Vec<GroupDefinition>,
) -> light_engine::EngineSnapshot {
    light_engine::EngineSnapshot {
        dynamics: vec![definition.clone()].into(),
        groups: groups.into(),
        dynamic_stage_positions: Arc::new(HashMap::<FixtureId, SpatialPosition>::new()),
        ..Default::default()
    }
}

fn explicit_group(id: &str, fixtures: &[FixtureId]) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: id.into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: fixtures.to_vec(),
        }),
        ..Default::default()
    }
}

fn reference_group(id: &str, referenced: &str) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: id.into(),
        source: Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: referenced.into(),
                rule: SelectionRule::All,
            }],
        }),
        ..Default::default()
    }
}

fn programmer_row(
    programmer_id: Uuid,
    fixture_id: FixtureId,
    definition: &DynamicDefinition,
    instance_link: Uuid,
    size: f32,
    changed_at_millis: u64,
) -> ProgrammerRow {
    (
        programmer_id,
        1,
        DynamicAddressValue {
            fixture_id,
            attribute: AttributeKey::intensity(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone()),
                    },
                },
                lane_id: definition.lanes[0].id,
                overrides: DynamicInstanceOverrides {
                    size,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: DynamicValueTiming::default(),
            },
            programmer_order: 1,
            changed_at_millis,
        },
    )
}

fn dynamic_playback(
    definition: &DynamicDefinition,
    number: u16,
    target_scope: serde_json::Value,
) -> light_playback::PlaybackDefinition {
    serde_json::from_value(serde_json::json!({
        "number": number, "name": "Dynamic playback", "target": {"type": "dynamic", "assignment": {
            "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": definition.pool_number,
                "embedded_fallback": {"definition": definition}},
            "target_scope": target_scope
        }}
    }))
    .unwrap()
}

fn active_playback(
    definition: &DynamicDefinition,
    number: u16,
) -> light_playback::ActiveDynamicPlayback {
    serde_json::from_value(serde_json::json!({
        "dynamic_id": definition.id, "playback_number": number, "enabled": true, "paused": false,
        "activated_at": chrono::DateTime::from_timestamp_millis(1000).unwrap()
    }))
    .unwrap()
}

fn native_definition() -> DynamicDefinition {
    let mut definition = definition(serde_json::json!({"type": "targetless"}));
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: NativeColorIdentity {
                    profile_id: Uuid::from_u128(1),
                    profile_revision: 2,
                    profile_digest: "original".into(),
                    mode_id: Uuid::from_u128(2),
                    head_id: Uuid::from_u128(3),
                    path_id: Uuid::from_u128(4),
                    model_revision: 1,
                    native_layout_signature: "layout".into(),
                },
            },
            component: Some(light_core::programming::ProgrammingComponent::NativeColor(
                NativeColorBinding {
                    channel_id: Uuid::from_u128(5),
                    function_id: Uuid::from_u128(15),
                },
            )),
        },
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Value {
                value: DynamicValue::Native(0),
            },
            maximum: DynamicValueSource::Value {
                value: DynamicValue::Native(255),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.0,
            pwm: PwmShape::default(),
        }),
    });
    definition
}

fn definition(target_binding: serde_json::Value) -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Cold reconciliation",
        "target_binding": target_binding,
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
            "span_degrees": 360.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}
