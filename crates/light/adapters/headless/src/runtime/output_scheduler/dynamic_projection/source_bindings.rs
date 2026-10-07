//! Bind captured authored rows before they enter Dynamic pause/resume history. The controller
//! reconciliation plan supplies the exact enabling row, including Live Group expansion.
use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicProgrammerSourceLane, DynamicSourceBinding, DynamicSourceOrigin, DynamicSourceOrigins,
};
use light_core::programming::IntentError;
use light_dynamics::{
    DynamicPresetSourceBinding, DynamicValue, DynamicValueAddress, DynamicValueSourceResolver,
};

pub(super) fn bind_captured_sources(
    origins: &mut DynamicSourceOrigins,
    runtime: &light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    assignments: CapturedSourceAssignments<'_>,
) -> Result<(), IntentError> {
    if let Some(programmer) = assignments.programmer {
        bind_programmer_sources(origins, runtime, inputs, programmer)?;
    }
    bind_projected_sources(
        origins,
        runtime,
        assignments.cues,
        |index| {
            let row = inputs.cue_values.get(index).ok_or_else(|| {
                IntentError("Cue source index is outside its captured vector".into())
            })?;
            let light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. } = row.value
            else {
                return Err(IntentError("Cue source is not an authored On row".into()));
            };
            Ok(DynamicSourceOrigin::Cue {
                source: row.source.into(),
                temporary_kind: match row.source_key {
                    light_playback::CueDynamicSourceKey::Normal { .. } => None,
                    light_playback::CueDynamicSourceKey::Temporary { kind, .. } => {
                        Some(kind.into())
                    }
                },
                cue_id: row.authored_cue_id,
                instance_link,
                changed_at: row.changed_at,
                transition_ordinal: row.transition_ordinal,
            })
        },
        |origin| matches!(origin, DynamicSourceOrigin::Cue { .. }),
    )?;
    bind_projected_sources(
        origins,
        runtime,
        assignments.playbacks,
        |index| {
            let row = inputs.dynamic_playbacks.get(index).ok_or_else(|| {
                IntentError("Playback source index is outside its captured vector".into())
            })?;
            let identity = row
                .playback_identity
                .or_else(|| PlaybackIdentity::physical(row.playback_number).ok())
                .ok_or_else(|| {
                    IntentError("captured Dynamic Playback has no valid identity".into())
                })?;
            Ok(DynamicSourceOrigin::Playback {
                identity,
                activated_at: row.activated_at,
            })
        },
        |origin| matches!(origin, DynamicSourceOrigin::Playback { .. }),
    )?;
    Ok(())
}

fn bind_projected_sources(
    origins: &mut DynamicSourceOrigins,
    runtime: &light_dynamics::DynamicRuntime,
    assignments: &[super::super::dynamic_reconciliation::ReconciledSourceAssignment],
    resolve: impl Fn(usize) -> Result<DynamicSourceOrigin, IntentError>,
    owns: impl Fn(&DynamicSourceOrigin) -> bool,
) -> Result<(), IntentError> {
    let mut active =
        rustc_hash::FxHashSet::with_capacity_and_hasher(assignments.len(), Default::default());
    for row in assignments {
        let binding = DynamicSourceBinding::Authored {
            instance_id: row.instance_id,
            controller_id: row.controller_id,
            target: row.target,
            lane_id: row.lane_id,
        };
        origins.bind(binding, resolve(row.captured_index)?)?;
        active.insert(binding);
    }
    origins.retain_authored_bindings(|record| {
        !owns(&record.origin)
            || active.contains(&record.binding)
            || binding_is_releasing(record.binding, runtime)
    });
    Ok(())
}

fn binding_is_releasing(
    binding: DynamicSourceBinding,
    runtime: &light_dynamics::DynamicRuntime,
) -> bool {
    match binding {
        DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            ..
        } => runtime.source_scope_is_releasing(instance_id, controller_id),
        DynamicSourceBinding::StaticBaseline { .. } | DynamicSourceBinding::Fixed { .. } => false,
    }
}

fn bind_programmer_sources(
    origins: &mut DynamicSourceOrigins,
    runtime: &light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    assignments: &[super::super::dynamic_reconciliation::ReconciledSourceAssignment],
) -> Result<(), IntentError> {
    let mut active =
        rustc_hash::FxHashSet::with_capacity_and_hasher(assignments.len(), Default::default());
    for assignment in assignments {
        let binding = DynamicSourceBinding::Authored {
            instance_id: assignment.instance_id,
            controller_id: assignment.controller_id,
            target: assignment.target,
            lane_id: assignment.lane_id,
        };
        // Legacy/test callers without captured sidecars remain explicitly unattributed. An
        // absent row must not reuse an old binding merely because its runtime ID is unchanged.
        let Some(row) = inputs
            .programmer_rows
            .and_then(|rows| rows.get(assignment.captured_index))
        else {
            origins.unbind(&binding);
            continue;
        };
        let Some((programmer_id, _, value)) =
            inputs.programmer_values.get(assignment.captured_index)
        else {
            return Err(IntentError(
                "Dynamic source row is outside the captured value vector".into(),
            ));
        };
        let light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. } = value.value
        else {
            return Err(IntentError(
                "Dynamic authored source does not refer to an On row".into(),
            ));
        };
        if row.programmer_id.0 != *programmer_id
            || row.changed_at_millis != value.changed_at_millis
            || row.programmer_order != value.programmer_order
        {
            return Err(IntentError(
                "Dynamic source sidecar does not match its captured row".into(),
            ));
        }
        origins.bind(
            binding,
            DynamicSourceOrigin::Programmer {
                programmer_id: row.programmer_id,
                lane: match row.lane {
                    light_engine::CapturedDynamicProgrammerLane::Live => {
                        DynamicProgrammerSourceLane::Live
                    }
                    light_engine::CapturedDynamicProgrammerLane::Preload => {
                        DynamicProgrammerSourceLane::Preload
                    }
                },
                instance_link,
                changed_at_millis: row.changed_at_millis,
                programmer_order: row.programmer_order,
            },
        )?;
        active.insert(binding);
    }
    // Drop obsolete live lookups, retaining immutable records referenced by held leaves.
    // Cold checkpoint pruning can reclaim records once neither binding nor history uses them.
    origins.retain_authored_bindings(|record| {
        !matches!(record.origin, DynamicSourceOrigin::Programmer { .. })
            || active.contains(&record.binding)
            || binding_is_releasing(record.binding, runtime)
    });
    Ok(())
}

/// A release can finish during sampling. Retire its lookup after that sample, retaining the
/// immutable record for any surviving history. Compare compact controller projections first;
/// unchanged frames allocate no membership set and never walk expression history.
pub(super) fn retire_removed_controllers(
    origins: &mut DynamicSourceOrigins,
    before: &light_dynamics::DynamicRuntimeSnapshot,
    after: &light_dynamics::DynamicRuntimeSnapshot,
) {
    let unchanged = before.instances.len() == after.instances.len()
        && before
            .instances
            .iter()
            .zip(&after.instances)
            .all(|(left, right)| {
                left.id == right.id
                    && left.controllers.len() == right.controllers.len()
                    && left
                        .controllers
                        .iter()
                        .zip(&right.controllers)
                        .all(|(left, right)| left.id == right.id)
            });
    if unchanged {
        return;
    }
    let active = after
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .controllers
                .iter()
                .map(move |controller| (instance.id, controller.id))
        })
        .collect::<HashSet<_>>();
    origins.retain_bindings_by_key(|binding, _| match *binding {
        DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            ..
        } => active.contains(&(instance_id, controller_id)),
        DynamicSourceBinding::StaticBaseline { .. } | DynamicSourceBinding::Fixed { .. } => true,
    });
}

/// The production scalar path uses the same fallible sampler and authored identity hook as
/// typed lanes. Typed Current/adoption and static evidence are installed with family output;
/// this adapter deliberately leaves unavailable semantic sources unknown.
pub(super) struct AuthoredDynamicSources<'a>(pub &'a DynamicSourceOrigins);

impl DynamicValueSourceResolver for AuthoredDynamicSources<'_> {
    fn authored_occurrence(
        &self,
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    ) -> Option<light_dynamics::DynamicSourceOccurrenceId> {
        self.0.binding(&DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            target,
            lane_id,
        })
    }
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

#[cfg(test)]
mod fixed_tests {
    use super::*;
    use crate::runtime::dynamic_source_origins::{DynamicFixedSource, DynamicFixedStamp};
    use light_core::{AttributeValue, ProgrammerId, programming::ProgrammingOwner};
    use light_dynamics::{
        DynamicController, DynamicControllerSource, DynamicDefinition, DynamicRuntime,
        DynamicSemanticValue, DynamicStartRequest, DynamicTargetScope,
    };

    #[test]
    fn authored_controller_retirement_preserves_fixed_until_explicit_fixed_reconciliation() {
        let definition: DynamicDefinition = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Retire source",
            "target_binding": {"type": "targetless"},
            "lanes": [{
                "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
                "keyframes": {"points": [
                    {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                    {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
                ]},
                "max_min": {"minimum": {"type": "value", "value": 0.0}, "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
                "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
                "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
            }],
            "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0, "span_degrees": 0.0,
                "block_size": 1, "repeats": 1, "wings": false, "anchors_degrees": []},
            "speed": {"type": "fixed", "duration_millis": 1000}, "default_activation": "start_now"
        })).unwrap();
        let target = FixtureId::new();
        let programmer_id = ProgrammerId::new();
        let link = Uuid::new_v4();
        let controller_id = light_dynamics::programmer_dynamic_controller_id(programmer_id, link);
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let instance_id = runtime
            .start(DynamicStartRequest {
                definition_id: definition.id,
                controller: DynamicController {
                    id: controller_id,
                    source: DynamicControllerSource::Programmer {
                        programmer_id: programmer_id.0,
                        instance_link: Some(link),
                    },
                    priority: 100,
                    activated_at_millis: 1_000,
                    size: 1.0,
                    speed_multiplier: 1.0,
                    phase_offset_degrees: 0.0,
                    paused: false,
                },
                target_scope: DynamicTargetScope {
                    ordered_targets: vec![target],
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: 1_000,
                activation_delay_millis: 0,
                activation_duration_millis: 0,
                activation_policy_override: None,
                reuse_matching_targetless: false,
            })
            .unwrap();
        let mut origins = DynamicSourceOrigins::default();
        let authored = DynamicSourceBinding::Authored {
            instance_id,
            controller_id,
            target,
            lane_id: definition.lanes[0].id,
        };
        let authored_occurrence = origins
            .bind(
                authored,
                DynamicSourceOrigin::Programmer {
                    programmer_id,
                    lane: DynamicProgrammerSourceLane::Live,
                    instance_link: link,
                    changed_at_millis: 1_000,
                    programmer_order: 1,
                },
            )
            .unwrap();
        let fixed = DynamicSourceBinding::Fixed {
            source: DynamicFixedSource::Programmer {
                programmer_id,
                lane: DynamicProgrammerSourceLane::Live,
            },
            target,
            owner: ProgrammingOwner::Focus,
            component: None,
        };
        let fixed_occurrence = origins
            .bind(
                fixed,
                DynamicSourceOrigin::Fixed {
                    stamp: DynamicFixedStamp::Programmer {
                        changed_at_millis: 1_000,
                        programmer_order: 2,
                    },
                    priority: 100,
                    value: DynamicSemanticValue::Static {
                        value: AttributeValue::Normalized(0.4),
                        timing: Default::default(),
                    },
                },
            )
            .unwrap();
        let published = origins.clone();
        let before = runtime.output_projection_snapshot();
        assert!(
            runtime
                .off_controller(instance_id, controller_id, 1_100, 0, 0)
                .unwrap()
        );
        retire_removed_controllers(&mut origins, &before, &runtime.output_projection_snapshot());
        assert_eq!(origins.binding(&authored), None);
        assert_eq!(origins.binding(&fixed), Some(fixed_occurrence));
        assert!(origins.get(authored_occurrence).is_some());
        assert!(origins.get(fixed_occurrence).is_some());
        assert_eq!(published.binding(&authored), Some(authored_occurrence));
        assert_eq!(published.binding(&fixed), Some(fixed_occurrence));
        origins
            .reconcile_captured_programming_fixed_sources(&[], Some(&[]), &[])
            .unwrap();
        assert_eq!(origins.binding(&fixed), None);
        assert!(origins.get(fixed_occurrence).is_some());
        assert_eq!(published.binding(&fixed), Some(fixed_occurrence));
    }
}
