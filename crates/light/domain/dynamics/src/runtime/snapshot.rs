//! Capture the runtime as a portable checkpoint or as the lean projection the output path reads.
use super::*;

impl DynamicRuntime {
    pub fn snapshot(&self) -> DynamicRuntimeSnapshot {
        let mut instances = self
            .instances
            .values()
            .map(|instance| {
                let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
                controllers.sort_by_key(|controller| controller.id);
                let mut controller_transitions = instance
                    .controller_transitions
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                controller_transitions.sort_by_key(|transition| transition.controller_id);
                let mut phase_by_lane_target = instance
                    .phase_by_lane_target
                    .iter()
                    .map(|((lane_id, target), phase)| (*lane_id, *target, *phase))
                    .collect::<Vec<_>>();
                phase_by_lane_target.sort_by_key(|(lane_id, target, _)| (*lane_id, target.0));
                let mut phase_by_target = if instance.definition.phase_spread_mode
                    == crate::DynamicPhaseSpreadMode::Uniform
                {
                    instance
                        .definition
                        .lanes
                        .first()
                        .map(|lane| {
                            instance
                                .targets
                                .iter()
                                .filter_map(|target| {
                                    instance
                                        .phase_by_lane_target
                                        .get(&(lane.id, *target))
                                        .map(|phase| (*target, *phase))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                phase_by_target.sort_by_key(|(target, _)| target.0);
                let mut random_streams = instance
                    .random_streams
                    .iter()
                    .map(|((group_id, target), stream)| DynamicRandomStreamSnapshot {
                        group_id: *group_id,
                        target: *target,
                        last_elapsed_millis: stream.last_elapsed_millis,
                        next_decision_index: stream.next_decision_index,
                        active: stream.active.map(|pulse| DynamicRandomPulseSnapshot {
                            started_at_millis: pulse.started_at_millis,
                            duration_millis: pulse.duration_millis,
                        }),
                    })
                    .collect::<Vec<_>>();
                random_streams.sort_by_key(|stream| (stream.group_id, stream.target.0));
                let (expression_tape, last_sample_values, synchronized_hold_values) =
                    sample_values_snapshot(
                        &instance.last_sample_values,
                        &instance.synchronized_hold_values,
                    );
                DynamicInstanceSnapshot {
                    id: instance.id,
                    definition: instance.definition.as_ref().clone(),
                    targets: instance.targets.clone(),
                    phase_by_target,
                    phase_by_lane_target,
                    controllers,
                    lane_selections: instance.lane_selection_snapshot(),
                    controller_transitions,
                    started_at_millis: instance.started_at_millis,
                    paused_at_millis: instance.paused_at_millis,
                    paused_elapsed_millis: instance.paused_elapsed_millis,
                    activation_policy: instance.activation_policy,
                    pending_until_millis: instance.pending_until_millis,
                    speed_paused_at_millis: instance.speed_paused_at_millis,
                    speed_paused_elapsed_millis: instance.speed_paused_elapsed_millis,
                    random_streams,
                    completed: instance.completed,
                    synchronized_hold_elapsed_millis: instance.synchronized_hold_elapsed_millis,
                    synchronized_hold_captured: instance.synchronized_hold_captured,
                    last_synchronized_elapsed_millis: instance.last_synchronized_elapsed_millis,
                    synchronized_resume_transition: instance.synchronized_resume_transition,
                    last_sample_values,
                    synchronized_hold_values,
                    expression_tape,
                    preset_source_values: instance.preset_values.retained.clone(),
                }
            })
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| instance.id);
        DynamicRuntimeSnapshot {
            global_paused: self.global_paused,
            instances,
        }
    }

    /// Snapshot only the runtime identity and controller state consumed on the output path.
    ///
    /// Output arbitration, transition events, auto-off, and the visualization Dynamic stack do
    /// not read phase maps, random streams, synchronized hold samples, or retained sample values.
    /// Omitting those persistence-only fields avoids cloning and sorting the largest runtime maps
    /// twice per output frame while leaving `snapshot()` and show persistence unchanged.
    pub fn output_projection_snapshot(&self) -> DynamicRuntimeSnapshot {
        let mut instances = self
            .instances
            .values()
            .map(|instance| {
                let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
                controllers.sort_by_key(|controller| controller.id);
                let mut controller_transitions = instance
                    .controller_transitions
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                controller_transitions.sort_by_key(|transition| transition.controller_id);
                DynamicInstanceSnapshot {
                    id: instance.id,
                    definition: instance.definition.as_ref().clone(),
                    targets: instance.targets.clone(),
                    phase_by_target: Vec::new(),
                    phase_by_lane_target: Vec::new(),
                    controllers,
                    lane_selections: instance.lane_selection_snapshot(),
                    controller_transitions,
                    started_at_millis: instance.started_at_millis,
                    paused_at_millis: instance.paused_at_millis,
                    paused_elapsed_millis: instance.paused_elapsed_millis,
                    activation_policy: instance.activation_policy,
                    pending_until_millis: instance.pending_until_millis,
                    speed_paused_at_millis: instance.speed_paused_at_millis,
                    speed_paused_elapsed_millis: instance.speed_paused_elapsed_millis,
                    random_streams: Vec::new(),
                    completed: instance.completed,
                    synchronized_hold_elapsed_millis: None,
                    synchronized_hold_captured: false,
                    last_synchronized_elapsed_millis: None,
                    synchronized_resume_transition: instance.synchronized_resume_transition,
                    last_sample_values: Vec::new(),
                    synchronized_hold_values: Vec::new(),
                    expression_tape: None,
                    preset_source_values: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| instance.id);
        DynamicRuntimeSnapshot {
            global_paused: self.global_paused,
            instances,
        }
    }
}
