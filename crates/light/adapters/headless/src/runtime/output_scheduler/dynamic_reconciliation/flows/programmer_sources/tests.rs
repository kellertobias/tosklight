use super::*;
use light_dynamics::{DynamicAddressValue, DynamicRuntime, DynamicTargetBinding};
use std::sync::Arc;

type CapturedRow = (Uuid, i16, DynamicAddressValue);

fn definition(axes: &[&str], live_group: bool) -> DynamicDefinition {
    let lanes = axes.iter().enumerate().map(|(index, axis)| {
        let owner = if *axis == "focus" { "focus" } else { "angles" };
        serde_json::json!({
            "id": Uuid::from_u128(200 + index as u128),
            "speed_multiplier": {"numerator":1,"denominator":1}, "width":1.0,
            "programming": {"address": {"representation":{"kind":owner},"component":{"kind":axis}},
                "configuration":{"mode":"keyframes","configuration":{"points":[
                    {"position":0.0,"source":{"kind":"value","value":{"kind":"scalar","value":0.5}},"interpolation":"linear"},
                    {"position":0.5,"source":{"kind":"value","value":{"kind":"scalar","value":1.0}},"interpolation":"linear"}
                ],"size":1.0}}}
        })
    }).collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({
        "id":Uuid::from_u128(100), "pool_number":7, "revision":1, "name":"Authored lanes",
        "target_binding": if live_group { serde_json::json!({"type":"live_group","group_id":"front"}) }
            else { serde_json::json!({"type":"targetless"}) },
        "lanes":lanes,
        "phase":{"ordering":{"type":"selection"},"offset_degrees":0.0,"span_degrees":360.0,
            "block_size":1,"repeats":1,"wings":false,"anchors_degrees":[]},
        "speed":{"type":"fixed","duration_millis":1000}, "default_activation":"start_now"
    })).unwrap()
}

fn row(definition: &DynamicDefinition, target: FixtureId, lane: usize, order: u64) -> CapturedRow {
    let lane = &definition.lanes[lane];
    (
        Uuid::from_u128(10),
        1,
        DynamicAddressValue {
            fixture_id: target,
            attribute: lane.output_owner(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::from_u128(20),
                dynamic: light_dynamics::DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone()),
                    },
                },
                lane_id: lane.id,
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: Default::default(),
            },
            programmer_order: order,
            changed_at_millis: order,
        },
    )
}

fn plan_sources(
    definition: &DynamicDefinition,
    rows: &[CapturedRow],
    extra: &[CapturedRow],
    targets: &[FixtureId],
) -> Vec<(FixtureId, Uuid, usize)> {
    let (plans, _) = programmer_controller_plan(rows, extra);
    assert_eq!(plans.len(), 1);
    let desired = plans.values().next().unwrap();
    let selection = DynamicLaneSelection::for_recorded_values(
        desired.reference,
        definition,
        &desired.lane_rows,
    );
    let mut output = planned_source_rows(desired, definition, &selection, targets);
    output.sort_by_key(|(target, lane, index)| (target.0, *lane, *index));
    output
}

#[test]
fn each_lane_keeps_its_own_source_instead_of_the_latest_controller_edit() {
    let definition = definition(&["pan", "tilt", "focus"], false);
    let target = FixtureId::new();
    let rows = vec![
        row(&definition, target, 0, 1),
        row(&definition, target, 1, 2),
    ];
    let extra = vec![row(&definition, target, 2, 9)];
    assert_eq!(
        plan_sources(&definition, &rows, &extra, &[target]),
        vec![
            (target, definition.lanes[0].id, 0),
            (target, definition.lanes[1].id, 1),
            (target, definition.lanes[2].id, 2),
        ]
    );
}

#[test]
fn uniform_group_uses_per_lane_template_for_existing_and_new_members_and_stable_ties() {
    let definition = definition(&["pan", "tilt", "focus"], true);
    let [a, b, future] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let rows = vec![
        row(&definition, a, 0, 1),
        row(&definition, b, 0, 5),
        row(&definition, a, 1, 2),
        row(&definition, b, 2, 9),
    ];
    let sources = plan_sources(&definition, &rows, &[], &[a, b, future]);
    for target in [a, b, future] {
        assert!(sources.contains(&(target, definition.lanes[0].id, 1)));
        assert!(sources.contains(&(target, definition.lanes[1].id, 2)));
        assert!(sources.contains(&(target, definition.lanes[2].id, 3)));
    }
    let tied = vec![row(&definition, a, 0, 5), row(&definition, b, 0, 5)];
    let sources = plan_sources(&definition, &tied, &[], &[future]);
    assert!(sources.contains(&(future, definition.lanes[0].id, 0)));
}

#[test]
fn per_target_angle_closure_uses_its_own_activation_and_current_partner_is_not_authored() {
    let definition = definition(&["pan", "tilt"], false);
    let [a, b, absent] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let rows = vec![row(&definition, a, 0, 1), row(&definition, b, 1, 8)];
    let sources = plan_sources(&definition, &rows, &[], &[a, b, absent]);
    assert_eq!(sources.len(), 4);
    for lane in &definition.lanes {
        assert!(sources.contains(&(a, lane.id, 0)));
        assert!(sources.contains(&(b, lane.id, 1)));
    }
    let one_axis = definition_for_current_partner();
    assert_eq!(one_axis.lanes.len(), 2);
    let rows = vec![row(&one_axis, a, 0, 1)];
    assert_eq!(
        plan_sources(&one_axis, &rows, &[], &[a]),
        vec![(a, one_axis.lanes[0].id, 0)]
    );
}

fn definition_for_current_partner() -> DynamicDefinition {
    definition(&["pan"], false)
}

#[test]
fn retained_off_binds_the_lower_on_and_a_later_lane_edit_selects_its_exact_capture() {
    let definition = definition(&["focus"], false);
    let target = FixtureId::new();
    let on = row(&definition, target, 0, 1);
    let mut off = on.clone();
    off.2.value = DynamicSemanticValue::DynamicOff {
        instance_link: Uuid::from_u128(20),
        timing: Default::default(),
    };
    off.2.programmer_order = 2;
    off.2.changed_at_millis = 2;
    assert_eq!(
        plan_sources(&definition, std::slice::from_ref(&on), &[off], &[target]),
        vec![(target, definition.lanes[0].id, 0)]
    );
    let newer = row(&definition, target, 0, 3);
    assert_eq!(
        plan_sources(&definition, &[on], &[newer], &[target]),
        vec![(target, definition.lanes[0].id, 1)]
    );
}

fn snapshot(definition: &DynamicDefinition, members: &[FixtureId]) -> light_engine::EngineSnapshot {
    light_engine::EngineSnapshot {
        dynamics: Arc::new(vec![definition.clone()]),
        groups: Arc::new(vec![light_programmer::GroupDefinition {
            id: "front".into(),
            name: "Front".into(),
            source: Some(light_programmer::GroupFixtureSource::Explicit {
                fixture_ids: members.to_vec(),
            }),
            ..Default::default()
        }]),
        dynamic_stage_positions: Arc::new(
            members
                .iter()
                .enumerate()
                .map(|(index, id)| {
                    (
                        *id,
                        light_dynamics::SpatialPosition {
                            x: index as f32,
                            y: 0.0,
                            z: 0.0,
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    }
}

#[test]
fn actual_reconciliation_refreshes_group_targets_without_restarting_the_source() {
    let definition = definition(&["pan", "tilt"], true);
    assert!(matches!(
        definition.target_binding,
        DynamicTargetBinding::LiveGroup { .. }
    ));
    let [a, b, future] = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let rows = vec![row(&definition, a, 0, 1), row(&definition, b, 1, 2)];
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let mut first = Vec::new();
    reconcile_programmer_dynamics_with_sources(
        &mut runtime,
        100,
        &snapshot(&definition, &[a, b]),
        &rows,
        &[],
        |source| first.push(source),
    );
    assert_eq!(first.len(), 4);
    let instance = first[0].instance_id;
    let before = runtime.snapshot();
    let mut next = Vec::new();
    reconcile_programmer_dynamics_with_sources(
        &mut runtime,
        200,
        &snapshot(&definition, &[b, future]),
        &rows,
        &[],
        |source| next.push(source),
    );
    assert_eq!(next.len(), 4);
    assert!(
        next.iter()
            .all(|source| source.instance_id == instance && source.target != a)
    );
    assert!(next.iter().any(|source| source.target == future
        && source.lane_id == definition.lanes[0].id
        && source.captured_index == 0));
    assert_eq!(
        runtime.snapshot().instances[0].started_at_millis,
        before.instances[0].started_at_millis
    );
}

/// TL-641: one row's selection resolved once per lane and re-keyed per target equals resolving
/// it for that target, for per-target and Live Group definitions alike.
#[test]
fn a_lane_selection_rekeyed_to_another_target_equals_its_own_resolution() {
    let targets = [FixtureId::new(), FixtureId::new()];
    for live_group in [false, true] {
        let definition = definition(&["pan", "tilt", "focus"], live_group);
        let (_, _, value) = row(&definition, targets[0], 0, 1);
        let DynamicSemanticValue::DynamicOn { dynamic, .. } = &value.value else {
            unreachable!()
        };
        for lane in &definition.lanes {
            let resolved = |target| {
                DynamicLaneSelection::for_recorded_values(
                    dynamic,
                    &definition,
                    &[(target, lane.id)],
                )
            };
            assert_eq!(
                retarget(resolved(targets[0]), targets[1]),
                resolved(targets[1])
            );
        }
    }
}
