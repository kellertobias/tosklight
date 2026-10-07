use super::*;
use light_application::DynamicPresetSourceIssueReason;
use light_core::AttributeValue;
use light_core::programming::{
    GroupFamilyAssignment, PositionIntent, ProgrammingComponent, ProgrammingOwner, ScalarIntent,
    TargetReference,
};
use light_dynamics::{
    DynamicDefinition, DynamicFamilyRepresentation, DynamicLaneBody, DynamicPresetGroupTemplate,
    DynamicStartRequest, DynamicTargetScope, DynamicValue, DynamicValueAddress, DynamicValueSource,
    MaxMinConfiguration, PeriodicFunction, ProgrammingLaneBody, ProgrammingLaneConfiguration,
    PwmShape, RankDirection, SpatialPosition, SpatialProjection, SpatialSelectionMapping,
    SpatialSelectionShape,
};
use light_programmer::{GroupFixtureSource, GroupReference, SelectionRule};
use std::sync::Arc;

#[test]
fn group_rank_change_refreshes_frozen_targets_without_touching_clocks_or_unrelated_sources() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let grouped = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let universal = pan_definition(&[a, b], universal_template(vec![10.0, 20.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let (live, [grouped_id, universal_id]) = running(&previous, [&grouped, &universal]);
    assert_eq!(table(&live, grouped_id), [(a, 0.0), (b, 100.0)].into());
    let live_before = live.snapshot();
    let live_tables = tables(&live);

    // Moving the fixtures swaps the spatial rank; targets and phases stay identical.
    let destination = snapshot(vec![spatial_group("front", &[a, b])], [(a, 10.0), (b, 0.0)]);
    let mut candidate = live.fork_for_cold_install();
    let before = candidate.preset_source_instances();
    let report =
        materialize_cold_preset_dependencies(&previous, &destination, &mut candidate).unwrap();

    assert_eq!(report.invalidated_instances, vec![grouped_id]);
    assert_eq!(report.installed_instances.len(), 2);
    assert!(report.missing_groups.is_empty());
    assert_eq!(table(&candidate, grouped_id), [(a, 100.0), (b, 0.0)].into());
    assert_eq!(
        table(&candidate, universal_id),
        [(a, 10.0), (b, 20.0)].into()
    );
    let after = candidate.preset_source_instances();
    let generation = |instances: &[DynamicInstancePresetSources], id| {
        instances
            .iter()
            .find(|instance| instance.instance_id == id)
            .unwrap()
            .dependency_generation
    };
    assert_ne!(
        generation(&before, grouped_id),
        generation(&after, grouped_id)
    );
    assert_eq!(
        generation(&before, universal_id),
        generation(&after, universal_id)
    );
    assert_eq!(continuity(&candidate), continuity(&live));
    assert_eq!(live.snapshot(), live_before);
    assert_eq!(tables(&live), live_tables);
}

#[test]
fn unchanged_destination_keeps_every_generation_and_retained_table() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let grouped = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let (live, _) = running(&previous, [&grouped]);
    let mut candidate = live.fork_for_cold_install();
    let report =
        materialize_cold_preset_dependencies(&previous, &previous, &mut candidate).unwrap();

    assert!(report.invalidated_instances.is_empty());
    assert_eq!(generations(&candidate), generations(&live));
    assert_eq!(tables(&candidate), tables(&live));
}

#[test]
fn group_referenced_only_by_a_retained_fallback_template_invalidates_its_instance() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let mut template = universal_template(vec![5.0, 6.0]);
    template.fallback = Some(Box::new(group_template("front", vec![0.0, 100.0])));
    let definition = pan_definition(&[a, b], template);
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let (live, [instance]) = running(&previous, [&definition]);

    let destination = snapshot(vec![spatial_group("front", &[a])], [(a, 0.0), (b, 10.0)]);
    let mut candidate = live.fork_for_cold_install();
    let report =
        materialize_cold_preset_dependencies(&previous, &destination, &mut candidate).unwrap();
    assert_eq!(report.invalidated_instances, vec![instance]);
    assert_eq!(continuity(&candidate), continuity(&live));
}

#[test]
fn absent_group_is_passive_but_an_existing_malformed_group_fails_before_any_invalidation() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let definition = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let (live, [instance]) = running(&previous, [&definition]);
    let live_before = live.snapshot();

    // Deleting the Group is passive: the targets keep their last valid values.
    let absent = snapshot(Vec::new(), [(a, 0.0), (b, 10.0)]);
    let mut candidate = live.fork_for_cold_install();
    let report = materialize_cold_preset_dependencies(&previous, &absent, &mut candidate).unwrap();
    assert_eq!(
        report.missing_groups,
        vec![ColdPresetMissingGroup {
            instance_id: instance,
            group_id: "front".into()
        }]
    );
    assert_eq!(report.invalidated_instances, vec![instance]);
    assert_eq!(table(&candidate, instance), table(&live, instance));

    // An existing Group whose nested reference cannot resolve is a genuine failure.
    let malformed = snapshot(
        vec![reference_group("front", "ghost")],
        [(a, 0.0), (b, 10.0)],
    );
    let mut candidate = live.fork_for_cold_install();
    let error =
        materialize_cold_preset_dependencies(&previous, &malformed, &mut candidate).unwrap_err();
    let ColdPresetMaterializationError::InvalidGroup { group_id, resolver } = error else {
        panic!("expected an invalid Group, got {error:?}")
    };
    assert_eq!(group_id, "front");
    assert!(
        resolver.contains("ghost"),
        "resolver context is kept: {resolver}"
    );
    assert_eq!(generations(&candidate), generations(&live));
    assert_eq!(live.snapshot(), live_before);
}

#[test]
fn stale_member_rejects_the_whole_batch_without_partial_installation() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let first = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let second = pan_definition(&[a, b], group_template("front", vec![200.0, 300.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let (live, [first_id, _]) = running(&previous, [&first, &second]);
    let destination = snapshot(vec![spatial_group("front", &[a, b])], [(a, 10.0), (b, 0.0)]);

    let mut candidate = live.fork_for_cold_install();
    let (prepared, _) = prepare_cold_preset_batch(&previous, &destination, &mut candidate).unwrap();
    let tables_before = tables(&candidate);
    assert!(candidate.invalidate_preset_source_dependencies(first_id));
    assert_eq!(
        install_cold_preset_batch(&mut candidate, prepared),
        Err(ColdPresetMaterializationError::Stale)
    );
    assert_eq!(tables(&candidate), tables_before);
    assert_eq!(tables(&live), tables_before);
}

#[test]
fn genuine_compiler_failure_rejects_the_candidate() {
    let definition = pan_definition(
        &[FixtureId(Uuid::nil())],
        group_template("front", vec![0.0]),
    );
    let previous = snapshot(Vec::new(), []);
    let mut candidate = DynamicRuntime::default();
    candidate.install_definitions([definition.clone()]).unwrap();
    start(&mut candidate, &definition);
    let error =
        materialize_cold_preset_dependencies(&previous, &previous, &mut candidate).unwrap_err();
    assert!(
        matches!(error, ColdPresetMaterializationError::Compile(_)),
        "{error:?}"
    );
}

#[test]
fn incompatible_member_uses_verified_fallback_and_reports_passive_quality() {
    let [a, b, c] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let old = group_template("front", vec![-360.0, 0.0, 360.0]);
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0, 2.0, 3.0],
    )));
    let mut latest = DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: "front".into(),
            value: AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
                owner: ProgrammingOwner::Position,
                template: target,
                members: [(b.0, angles(vec![42.0]))].into(),
            })),
        }],
        ..Default::default()
    };
    latest.retain_fallback(&pan_address(), Some(&old));
    let definition = pan_definition(&[b, c], latest);
    let previous = snapshot(
        vec![spatial_group("front", &[a, b, c])],
        [(a, 0.0), (b, 1.0), (c, 2.0)],
    );
    let (live, [instance]) = running(&previous, [&definition]);
    let mut candidate = live.fork_for_cold_install();
    let report =
        materialize_cold_preset_dependencies(&previous, &previous, &mut candidate).unwrap();

    assert_eq!(table(&candidate, instance), [(b, 42.0), (c, 360.0)].into());
    let quality = report
        .source_quality
        .iter()
        .map(|quality| (quality.issue.target, quality.issue.reason.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        quality,
        vec![(c, DynamicPresetSourceIssueReason::IncompatibleSource)]
    );
}

fn running<const N: usize>(
    snapshot: &light_engine::EngineSnapshot,
    definitions: [&DynamicDefinition; N],
) -> (DynamicRuntime, [Uuid; N]) {
    let mut runtime = DynamicRuntime::default();
    runtime
        .install_definitions(definitions.iter().map(|definition| (*definition).clone()))
        .unwrap();
    let instances = definitions.map(|definition| start(&mut runtime, definition));
    materialize_cold_preset_dependencies(snapshot, snapshot, &mut runtime).unwrap();
    (runtime, instances)
}

fn start(runtime: &mut DynamicRuntime, definition: &DynamicDefinition) -> Uuid {
    let light_dynamics::DynamicTargetBinding::FrozenTargets { targets } =
        &definition.target_binding
    else {
        unreachable!("test Dynamics use frozen targets")
    };
    runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: Uuid::new_v4(),
                source: light_dynamics::DynamicControllerSource::physical_playback(1),
                priority: 100,
                activated_at_millis: 5,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: targets.clone(),
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 5,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap()
}

fn table(runtime: &DynamicRuntime, instance_id: Uuid) -> HashMap<FixtureId, f32> {
    runtime
        .preset_source_instances()
        .into_iter()
        .find(|instance| instance.instance_id == instance_id)
        .unwrap()
        .last_valid
        .into_iter()
        .flat_map(|record| record.values)
        .map(|fallback| {
            let DynamicValue::Scalar(value) = fallback.value else {
                panic!("Pan tables are scalar")
            };
            (fallback.target, value)
        })
        .collect()
}

fn tables(runtime: &DynamicRuntime) -> Vec<String> {
    runtime
        .preset_source_instances()
        .iter()
        .map(|instance| format!("{}|{:?}", instance.instance_id, instance.last_valid))
        .collect()
}

fn generations(runtime: &DynamicRuntime) -> Vec<(Uuid, Uuid)> {
    runtime
        .preset_source_instances()
        .iter()
        .map(|instance| (instance.instance_id, instance.dependency_generation))
        .collect()
}

/// Clocks, controllers, pause, phases and targets that dependency refresh must never reset.
fn continuity(runtime: &DynamicRuntime) -> Vec<String> {
    let mut instances = runtime
        .snapshot()
        .instances
        .into_iter()
        .map(|instance| {
            format!(
                "{}|{:?}|{:?}|{:?}|{:?}|{}|{:?}|{:?}",
                instance.id,
                instance.targets,
                instance.phase_by_lane_target,
                instance.controllers,
                instance.controller_transitions,
                instance.started_at_millis,
                instance.paused_at_millis,
                instance.lane_selections,
            )
        })
        .collect::<Vec<_>>();
    instances.sort();
    instances
}

fn snapshot<const N: usize>(
    groups: Vec<GroupDefinition>,
    positions: [(FixtureId, f32); N],
) -> light_engine::EngineSnapshot {
    light_engine::EngineSnapshot {
        groups: groups.into(),
        dynamic_stage_positions: Arc::new(
            positions
                .into_iter()
                .map(|(fixture, x)| (fixture, SpatialPosition { x, y: 0.0, z: 0.0 }))
                .collect(),
        ),
        ..Default::default()
    }
}

fn spatial_group(id: &str, fixtures: &[FixtureId]) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: id.into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: fixtures.to_vec(),
        }),
        mapping: Some(SpatialSelectionMapping {
            projection: SpatialProjection::from_preset(
                light_dynamics::ProjectionPreset::Top,
                Position3d::default(),
            ),
            shape: SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
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

fn angles(points: Vec<f32>) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: if points.len() == 1 {
            ScalarIntent::Value(points[0])
        } else {
            ScalarIntent::Spread(points)
        },
        tilt_degrees: ScalarIntent::Value(0.0),
    }))
}

fn group_template(group_id: &str, pan: Vec<f32>) -> DynamicPresetTemplate {
    DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: group_id.into(),
            value: angles(pan),
        }],
        ..Default::default()
    }
}

fn universal_template(pan: Vec<f32>) -> DynamicPresetTemplate {
    DynamicPresetTemplate {
        universal: Some(angles(pan)),
        ..Default::default()
    }
}

fn pan_address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    }
}

fn pan_definition(targets: &[FixtureId], template: DynamicPresetTemplate) -> DynamicDefinition {
    let mut definition: DynamicDefinition = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Cold Preset",
        "target_binding": {"type": "frozen_targets", "targets": targets},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "pan", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"}
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
    }))
    .unwrap();
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: pan_address(),
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Value {
                value: DynamicValue::Scalar(0.0),
            },
            maximum: DynamicValueSource::Preset {
                preset_id: "3.1".into(),
                address: pan_address(),
                last_valid_by_target: Vec::new(),
                retained: Some(Arc::new(template)),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.0,
            pwm: PwmShape::default(),
        }),
    });
    definition
}

mod native_fallback;

#[test]
fn pending_materialization_prepares_new_instances_once_and_rolls_back_with_failed_output() {
    let target = FixtureId::new();
    let definition = pan_definition(&[target], universal_template(vec![45.0]));
    let show = snapshot(Vec::new(), [(target, 0.0)]);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = start(&mut runtime, &definition);
    let initial = runtime.snapshot();
    assert!(runtime.has_pending_preset_sources());
    let mut scratch = light_dynamics::DynamicOutputFrameScratch::default();
    let failed: Result<(), &str> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        let report = materialize_pending_preset_dependencies(&show, runtime).unwrap();
        assert_eq!(report.installed_instances, vec![instance]);
        assert_eq!(table(runtime, instance), [(target, 45.0)].into());
        assert!(!runtime.has_pending_preset_sources());
        Err("final encoding failed")
    });
    assert!(failed.is_err());
    assert_eq!(runtime.snapshot(), initial);
    assert!(runtime.has_pending_preset_sources());
    materialize_pending_preset_dependencies(&show, &mut runtime).unwrap();
    assert_eq!(table(&runtime, instance), [(target, 45.0)].into());
    let accepted = runtime.snapshot();
    let report = materialize_pending_preset_dependencies(&show, &mut runtime).unwrap();
    assert!(report.installed_instances.is_empty());
    assert_eq!(runtime.snapshot(), accepted);
}

#[test]
fn pending_materialization_does_not_recompile_unaffected_instances_or_repeat_missing_groups() {
    let target = FixtureId::new();
    let known = pan_definition(&[target], universal_template(vec![25.0]));
    let missing = pan_definition(&[target], group_template("not-stored", vec![60.0]));
    let show = snapshot(Vec::new(), [(target, 0.0)]);
    let mut runtime = DynamicRuntime::default();
    runtime
        .install_definitions([known.clone(), missing.clone()])
        .unwrap();
    let known_id = start(&mut runtime, &known);
    materialize_pending_preset_dependencies(&show, &mut runtime).unwrap();
    let original = runtime.preset_source_instances()[0].dependency_generation;
    let missing_id = start(&mut runtime, &missing);
    let report = materialize_pending_preset_dependencies(&show, &mut runtime).unwrap();
    assert_eq!(report.installed_instances, vec![missing_id]);
    assert_eq!(report.missing_groups.len(), 1);
    let report = materialize_pending_preset_dependencies(&show, &mut runtime).unwrap();
    assert!(report.installed_instances.is_empty());
    assert!(report.missing_groups.is_empty());
    assert_eq!(
        runtime
            .preset_source_instances()
            .iter()
            .find(|row| row.instance_id == known_id)
            .unwrap()
            .dependency_generation,
        original
    );
}
