use super::dynamic_projection::DynamicPlaybackControl;
use super::*;

mod flows;

pub(super) use flows::*;

fn release_stale_playback_controllers(
    dynamics: &mut light_dynamics::DynamicRuntime,
    snapshot: &light_engine::EngineSnapshot,
    desired_ids: &HashSet<Uuid>,
    now_millis: u64,
) {
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Playback { .. }
        ) && !desired_ids.contains(&controller.id)
        {
            let release_millis = match controller.source {
                light_dynamics::DynamicControllerSource::Playback { playback_number } => snapshot
                    .playbacks
                    .iter()
                    .find(|playback| playback.number == playback_number)
                    .map_or(0, |playback| playback.xfade_millis),
                _ => 0,
            };
            let _ =
                dynamics.off_controller(instance_id, controller.id, now_millis, 0, release_millis);
        }
    }
}

fn dynamic_playback_definition<'a>(
    snapshot: &'a light_engine::EngineSnapshot,
    active: &light_playback::ActiveDynamicPlayback,
) -> Option<&'a light_playback::PlaybackDefinition> {
    match active.playback_identity {
        Some(light_playback::PlaybackIdentity::Virtual(address)) => snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.virtual_playbacks.get(&address.number().get())),
        _ => snapshot
            .playbacks
            .iter()
            .find(|playback| playback.number == active.playback_number),
    }
}

fn effective_dynamic_playback_speed(
    definition: &light_dynamics::DynamicDefinition,
    active: &light_playback::ActiveDynamicPlayback,
) -> f32 {
    let local = active.local_speed_multiplier.factor();
    let learned = active.learned_duration_millis.and_then(|learned| {
        let light_dynamics::DynamicSpeed::Fixed { duration_millis } = &definition.speed else {
            return None;
        };
        Some(*duration_millis as f64 / learned.max(1) as f64)
    });
    (local * learned.unwrap_or(1.0)).clamp(f64::EPSILON, 1_024.0) as f32
}

fn dynamic_playback_controller_id(playback: &light_playback::ActiveDynamicPlayback) -> Uuid {
    if let Some(target_id) = playback.dynamic_id {
        return light_playback::dynamic_playback_controller_id(target_id);
    }
    let address = match playback.playback_identity {
        Some(light_playback::PlaybackIdentity::Virtual(address)) => {
            (u128::from(address.page()) << 16) | u128::from(address.number().get())
        }
        _ => u128::from(playback.playback_number),
    };
    Uuid::from_u128(0x4459_4e41_4d49_432d_504c_4159_4241_434b ^ address)
}

fn resolve_dynamic_group(
    group_id: &str,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    snapshot: &light_engine::EngineSnapshot,
) -> Option<(
    Vec<light_core::FixtureId>,
    Option<light_dynamics::SpatialSelectionMapping>,
)> {
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(fixture_id, position)| {
            (
                *fixture_id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect();
    light_programmer::resolve_group_spatial(group_id, groups, &positions)
        .ok()
        .map(|resolved| (resolved.source_order, resolved.effective_mapping))
}

fn release_inactive_cue_controllers(
    dynamics: &mut light_dynamics::DynamicRuntime,
    desired_ids: &HashSet<Uuid>,
    release_timings: &HashMap<Uuid, light_dynamics::DynamicValueTiming>,
    now_millis: u64,
) {
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Cue { .. }
        ) && !desired_ids.contains(&controller.id)
        {
            let timing = release_timings
                .get(&controller.id)
                .copied()
                .unwrap_or_default();
            let _ = dynamics.off_controller(
                instance_id,
                controller.id,
                now_millis,
                timing.delay_millis.unwrap_or_default(),
                timing.fade_millis.unwrap_or_default(),
            );
        }
    }
}

fn cue_dynamic_controller_id(cue_list_id: light_core::CueListId, instance_link: Uuid) -> Uuid {
    Uuid::from_u128(
        instance_link.as_u128()
            ^ cue_list_id.0.as_u128().rotate_left(1)
            ^ 0x4355_452d_4459_4e41_4d49_432d_4354_524c,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_dynamics::{
        DynamicAddressValue, DynamicDefinition, DynamicDefinitionSnapshot,
        DynamicInstanceOverrides, DynamicReference, DynamicSemanticValue, DynamicValueTiming,
        Position3d, ProjectionPreset, RankDirection, Rational, SpatialPosition, SpatialProjection,
        SpatialSelectionMapping, SpatialSelectionShape,
    };
    use light_programmer::{GroupDefinition, GroupFixtureSource};
    use std::sync::Arc;

    #[test]
    fn programmer_live_group_reconciliation_uses_current_membership_and_positions() {
        let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
        let dynamic = live_group_dynamic();
        let instance_link = Uuid::new_v4();
        let programmer_id = Uuid::new_v4();
        let values = fixtures[..2]
            .iter()
            .copied()
            .map(|fixture_id| {
                (
                    programmer_id,
                    1,
                    DynamicAddressValue {
                        fixture_id,
                        attribute: AttributeKey::intensity(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link,
                            dynamic: DynamicReference {
                                dynamic_id: Some(dynamic.id),
                                last_known_pool_number: dynamic.pool_number,
                                embedded_fallback: DynamicDefinitionSnapshot {
                                    definition: Arc::new(dynamic.clone()),
                                },
                            },
                            lane_id: dynamic.lanes[0].id,
                            overrides: DynamicInstanceOverrides {
                                size: 1.0,
                                speed_multiplier: Rational::ONE,
                                phase_offset_degrees: 0.0,
                            },
                            timing: DynamicValueTiming::default(),
                        },
                        programmer_order: 1,
                        changed_at_millis: 10,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut snapshot = light_engine::EngineSnapshot {
            dynamics: Arc::new(vec![dynamic.clone()]),
            groups: Arc::new(vec![group(&fixtures[..2])]),
            dynamic_stage_positions: Arc::new(positions([(fixtures[0], 0.0), (fixtures[1], 10.0)])),
            ..Default::default()
        };
        let mut runtime = light_dynamics::DynamicRuntime::default();
        runtime.install_definitions([dynamic]).unwrap();

        reconcile_programmer_dynamics(&mut runtime, 10, &snapshot, &values, &[]);
        let first = runtime.snapshot();
        assert_eq!(first.instances.len(), 1);
        let instance_id = first.instances[0].id;
        assert_eq!(first.instances[0].targets, fixtures[..2]);

        snapshot.groups = Arc::new(vec![group(&fixtures[1..])]);
        snapshot.dynamic_stage_positions =
            Arc::new(positions([(fixtures[1], 10.0), (fixtures[2], 0.0)]));
        reconcile_programmer_dynamics(&mut runtime, 20, &snapshot, &values, &[]);

        let reconciled = runtime.snapshot();
        assert_eq!(reconciled.instances.len(), 1);
        assert_eq!(reconciled.instances[0].id, instance_id);
        assert_eq!(reconciled.instances[0].started_at_millis, 10);
        assert_eq!(
            reconciled.instances[0].targets,
            vec![fixtures[1], fixtures[2]]
        );
        let phases = reconciled.instances[0]
            .phase_by_lane_target
            .iter()
            .map(|(_, target, phase)| (*target, *phase))
            .collect::<HashMap<_, _>>();
        assert_eq!(phases[&fixtures[2]], 0.0);
        assert_eq!(phases[&fixtures[1]], 180.0);
    }

    fn group(fixtures: &[FixtureId]) -> GroupDefinition {
        GroupDefinition {
            id: "front".into(),
            name: "Front".into(),
            source: Some(GroupFixtureSource::Explicit {
                fixture_ids: fixtures.to_vec(),
            }),
            mapping: Some(SpatialSelectionMapping {
                projection: SpatialProjection::from_preset(
                    ProjectionPreset::Top,
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

    fn positions<const N: usize>(
        values: [(FixtureId, f32); N],
    ) -> HashMap<FixtureId, SpatialPosition> {
        values
            .into_iter()
            .map(|(fixture_id, x)| (fixture_id, SpatialPosition { x, y: 0.0, z: 0.0 }))
            .collect()
    }

    fn live_group_dynamic() -> DynamicDefinition {
        serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "pool_number": 7,
            "revision": 1,
            "name": "Live Group wave",
            "color": null,
            "icon": null,
            "target_binding": {"type": "live_group", "group_id": "front"},
            "lanes": [{
                "id": Uuid::new_v4(),
                "attribute": "intensity",
                "mode": "keyframes",
                "keyframes": {
                    "points": [
                        {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                        {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
                    ],
                    "size": 1.0
                },
                "max_min": {
                    "minimum": {"type": "value", "value": 0.0},
                    "maximum": {"type": "value", "value": 1.0},
                    "function": "sinus", "size": 1.0,
                    "pwm": {"attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                        "attack_interpolation": "linear", "decay_interpolation": "linear"}
                },
                "middle_amplitude": {
                    "middle": {"type": "current"}, "amplitude": 0.5,
                    "function": "sinus", "size": 1.0,
                    "pwm": {"attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                        "attack_interpolation": "linear", "decay_interpolation": "linear"}
                },
                "speed_multiplier": {"numerator": 1, "denominator": 1},
                "width": 1.0,
                "random_group_id": null
            }],
            "random_groups": [],
            "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
                "span_degrees": 360.0, "block_size": 1, "repeats": 1,
                "wings": false, "anchors_degrees": []},
            "speed": {"type": "fixed", "duration_millis": 1000},
            "default_activation": "start_now"
        }))
        .unwrap()
    }
}
