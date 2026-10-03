//! Restore a portable checkpoint, validating every retained instance before replacing Live state.
use super::*;
use std::collections::HashSet;

type SampleValues = HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>;

/// The retained sample rows of one stored instance, moved out of its snapshot.
struct RetainedSampleRows {
    expression_tape: Option<Arc<crate::RetainedExpressionTape>>,
    last_sample_values: Vec<DynamicHeldSampleSnapshot>,
    synchronized_hold_values: Vec<DynamicHeldSampleSnapshot>,
}

/// Validated held samples and their derived angle and capability state.
struct RestoredSamples {
    last_sample_values: SampleValues,
    synchronized_hold_values: SampleValues,
    synchronized_hold_angle_sources: HashSet<(Uuid, FixtureId, Uuid)>,
    unavailable_samples: native_capability::UnavailableSamples,
}

/// The controller and lane scope retained samples must stay within.
struct RetainedSampleScope<'a> {
    definition: &'a DynamicDefinition,
    targets: &'a [FixtureId],
    controllers: &'a HashMap<Uuid, DynamicController>,
    lane_selections: &'a HashMap<Uuid, CompiledLaneSelection>,
}

impl DynamicRuntime {
    pub fn restore_snapshot(
        &mut self,
        snapshot: DynamicRuntimeSnapshot,
    ) -> Result<(), DynamicRuntimeError> {
        assert!(
            self.output_frame_undo.is_none(),
            "snapshot restore is outside an output transaction"
        );
        let mut instances = HashMap::new();
        let mut bound_instances = HashMap::new();
        for stored in snapshot.instances {
            let id = stored.id;
            let instance = self.restore_instance(stored, &mut bound_instances)?;
            instances.insert(id, instance);
        }
        self.instances = instances;
        self.bound_instances = bound_instances;
        self.global_paused = snapshot.global_paused;
        // A portable checkpoint cannot certify a retained in-process input capture.
        self.sample_boundary = None;
        if let Some(journal) = &mut self.control_recording {
            journal.reset();
        }
        Ok(())
    }

    fn restore_instance(
        &self,
        mut stored: DynamicInstanceSnapshot,
        bound_instances: &mut HashMap<Uuid, Uuid>,
    ) -> Result<DynamicInstance, DynamicRuntimeError> {
        if let Some(tape) = &mut stored.expression_tape
            && !tape.operation_emissions().is_empty()
        {
            Arc::make_mut(tape).restore_operation_emissions();
        }
        let retained_lanes = self.compile_programming_lanes(&stored.definition)?;
        // Validate the historical owner first, even when the current pool has changed.
        let retained_presets = preset_values::PresetValues::compile(
            stored.preset_source_values,
            &retained_lanes,
            &stored.targets,
            true,
        )?;
        validate_stored_instance(
            &stored.targets,
            &stored.controllers,
            &stored.phase_by_target,
            &stored.phase_by_lane_target,
            &stored.controller_transitions,
        )?;
        let installed = (!self.definitions_pinned)
            .then(|| self.definitions.get(&stored.definition.id))
            .flatten();
        let definition = installed
            .cloned()
            .unwrap_or_else(|| Arc::new(stored.definition.clone()));
        let bound = !matches!(definition.target_binding, DynamicTargetBinding::Targetless);
        let programming_lanes = if installed.is_some() {
            self.compiled_lanes[&definition.id].clone()
        } else {
            retained_lanes
        };
        let preset_values = preset_values::PresetValues::compile(
            retained_presets.retained,
            &programming_lanes,
            &stored.targets,
            false,
        )?;
        if bound && bound_instances.insert(definition.id, stored.id).is_some() {
            return Err(DynamicRuntimeError::InvalidSnapshot(
                "a target-bound Dynamic has multiple singleton instances".into(),
            ));
        }
        let stored_controllers = stored.controllers.clone();
        let controllers = stored
            .controllers
            .into_iter()
            .map(|controller| (controller.id, controller))
            .collect();
        let controller_transitions =
            restore_controller_transitions(&stored_controllers, stored.controller_transitions)?;
        let random_streams = restore_random_streams(stored.random_streams);
        let phase_by_lane_target = restore_phase_by_lane_target(
            &stored.definition,
            &stored.phase_by_target,
            &stored.phase_by_lane_target,
        );
        let lane_selections = restore_lane_selections(
            stored.lane_selections,
            &controllers,
            &stored.definition,
            self.supported_programming_contract,
        )?;
        let samples = self.restore_retained_samples(
            stored.id,
            RetainedSampleRows {
                expression_tape: stored.expression_tape,
                last_sample_values: stored.last_sample_values,
                synchronized_hold_values: stored.synchronized_hold_values,
            },
            RetainedSampleScope {
                definition: &stored.definition,
                targets: &stored.targets,
                controllers: &controllers,
                lane_selections: &lane_selections,
            },
        )?;
        Ok(DynamicInstance {
            id: stored.id,
            definition,
            programming_lanes,
            preset_values,
            preset_dependency_generation: Uuid::new_v4(),
            targets: stored.targets,
            phase_by_lane_target,
            lane_selections,
            controllers,
            controller_transitions,
            started_at_millis: stored.started_at_millis,
            paused_at_millis: stored.paused_at_millis,
            paused_elapsed_millis: stored.paused_elapsed_millis,
            activation_policy: stored.activation_policy,
            pending_until_millis: stored.pending_until_millis,
            speed_paused_at_millis: stored.speed_paused_at_millis,
            speed_paused_elapsed_millis: stored.speed_paused_elapsed_millis,
            random_streams,
            completed: stored.completed,
            synchronized_hold_elapsed_millis: stored.synchronized_hold_elapsed_millis,
            synchronized_hold_captured: stored.synchronized_hold_captured
                || !samples.synchronized_hold_values.is_empty(),
            last_synchronized_elapsed_millis: stored.last_synchronized_elapsed_millis,
            synchronized_resume_transition: stored.synchronized_resume_transition,
            last_sample_values: samples.last_sample_values,
            synchronized_hold_angle_sources: samples.synchronized_hold_angle_sources,
            synchronized_hold_values: samples.synchronized_hold_values,
            unavailable_samples: samples.unavailable_samples,
            frame_addresses: None,
        })
    }

    fn restore_retained_samples(
        &self,
        instance_id: Uuid,
        rows: RetainedSampleRows,
        scope: RetainedSampleScope<'_>,
    ) -> Result<RestoredSamples, DynamicRuntimeError> {
        let prepared_tape = prepare_sample_tape(rows.expression_tape.as_ref())?;
        validate_retained_operations(
            prepared_tape.as_ref(),
            instance_id,
            [&rows.last_sample_values, &rows.synchronized_hold_values],
        )?;
        let mut last_sample_values = sample_values_from_snapshot(
            rows.last_sample_values,
            scope.definition,
            self.supported_programming_contract,
            prepared_tape.as_ref(),
        )?;
        let mut synchronized_hold_values = sample_values_from_snapshot(
            rows.synchronized_hold_values,
            scope.definition,
            self.supported_programming_contract,
            prepared_tape.as_ref(),
        )?;
        let targets = scope.targets.iter().copied().collect::<HashSet<_>>();
        for values in [&mut last_sample_values, &mut synchronized_hold_values] {
            // Historical Off left obsolete controller checkpoints behind. They have
            // no output owner and are discarded; active out-of-scope rows are invalid.
            values.retain(|(controller, _, _), _| scope.controllers.contains_key(controller));
            if values
                .iter()
                .any(|(key @ (controller, target, _), expression)| {
                    !targets.contains(target)
                        || scope
                            .lane_selections
                            .get(controller)
                            .is_some_and(|selection| {
                                !selection.allows_retained(
                                    scope.definition,
                                    *key,
                                    expression,
                                    values,
                                )
                            })
                })
            {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "held Dynamic exceeds its controller target/lane scope".into(),
                ));
            }
        }
        self.validate_retained_expressions(
            last_sample_values
                .values()
                .chain(synchronized_hold_values.values()),
            prepared_tape.as_ref(),
        )?;
        let unavailable_samples = self.unavailable_retained_samples(
            last_sample_values
                .iter()
                .chain(synchronized_hold_values.iter()),
        )?;
        let synchronized_hold_angle_sources =
            held_angle_sources(&synchronized_hold_values, prepared_tape.as_ref());
        Ok(RestoredSamples {
            last_sample_values,
            synchronized_hold_values,
            synchronized_hold_angle_sources,
            unavailable_samples,
        })
    }
}

fn validate_stored_instance(
    targets: &[FixtureId],
    controllers: &[DynamicController],
    phase_by_target: &[(FixtureId, f32)],
    phase_by_lane_target: &[(Uuid, FixtureId, f32)],
    controller_transitions: &[DynamicControllerTransitionSnapshot],
) -> Result<(), DynamicRuntimeError> {
    if targets.is_empty()
        || controllers.is_empty()
        || phase_by_target.iter().any(|(_, phase)| !phase.is_finite())
        || phase_by_lane_target
            .iter()
            .any(|(_, _, phase)| !phase.is_finite())
    {
        return Err(DynamicRuntimeError::InvalidSnapshot(
            "instances require targets, controllers, and finite phase and sample values".into(),
        ));
    }
    for controller in controllers {
        validate_controller(controller)?;
    }
    for transition in controller_transitions {
        if let Some(gate) = transition.output_gate {
            gate.validate()?;
            if !controllers
                .iter()
                .any(|controller| controller.id == transition.controller_id)
            {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "a Dynamic output gate requires its retained controller".into(),
                ));
            }
        }
    }
    Ok(())
}

fn restore_controller_transitions(
    stored_controllers: &[DynamicController],
    stored_transitions: Vec<DynamicControllerTransitionSnapshot>,
) -> Result<HashMap<Uuid, DynamicControllerTransitionSnapshot>, DynamicRuntimeError> {
    Ok(if stored_transitions.is_empty() {
        stored_controllers
            .iter()
            .map(|controller| {
                (
                    controller.id,
                    DynamicControllerTransitionSnapshot {
                        controller_id: controller.id,
                        activation_started_at_millis: controller.activated_at_millis,
                        ..Default::default()
                    },
                )
            })
            .collect()
    } else {
        let transitions = stored_transitions
            .into_iter()
            .map(|transition| (transition.controller_id, transition))
            .collect::<HashMap<_, _>>();
        if stored_controllers
            .iter()
            .any(|controller| !transitions.contains_key(&controller.id))
        {
            return Err(DynamicRuntimeError::InvalidSnapshot(
                "every Dynamic controller requires transition state".into(),
            ));
        }
        transitions
    })
}

fn restore_random_streams(
    streams: Vec<DynamicRandomStreamSnapshot>,
) -> HashMap<(Uuid, FixtureId), RandomStreamState> {
    streams
        .into_iter()
        .map(|stream| {
            (
                (stream.group_id, stream.target),
                RandomStreamState {
                    last_elapsed_millis: stream.last_elapsed_millis,
                    next_decision_index: stream.next_decision_index,
                    active: stream.active.map(|pulse| RandomPulse {
                        started_at_millis: pulse.started_at_millis,
                        duration_millis: pulse.duration_millis,
                    }),
                },
            )
        })
        .collect()
}

/// Expand legacy per-target phases to every lane, then apply the per-lane phases over them.
fn restore_phase_by_lane_target(
    definition: &DynamicDefinition,
    phase_by_target: &[(FixtureId, f32)],
    phase_by_lane_target: &[(Uuid, FixtureId, f32)],
) -> HashMap<(Uuid, FixtureId), f32> {
    let mut phases = definition
        .lanes
        .iter()
        .flat_map(|lane| {
            phase_by_target
                .iter()
                .map(move |(target, phase)| ((lane.id, *target), *phase))
        })
        .collect::<HashMap<_, _>>();
    phases.extend(
        phase_by_lane_target
            .iter()
            .map(|(lane_id, target, phase)| ((*lane_id, *target), *phase)),
    );
    phases
}
