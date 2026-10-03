//! Stage-one pinning of one controller's lanes. Clocks, Random decisions, addresses and
//! legacy scalar values are fixed here so deferred typed completion never advances them again.
use super::*;

/// Frame-wide inputs shared by every lane pinned for one instance in this frame.
pub(super) struct LanePinning<'a> {
    pub(super) instance_id: Uuid,
    pub(super) cycle_duration_millis: u64,
    pub(super) output_interval_millis: u64,
    pub(super) sources: &'a dyn ScalarSourceResolver,
    pub(super) authored_sources: &'a dyn DynamicValueSourceResolver,
    pub(super) evaluator: &'a DynamicEvaluator<'a>,
    pub(super) frame: &'a SamplingFrame,
    pub(super) addresses: &'a [Option<FrameAddress>],
    pub(super) random_phases: &'a HashMap<Uuid, HashMap<FixtureId, f32>>,
    pub(super) holding: bool,
    pub(super) retain_held: bool,
}

/// Targets whose held Angle sources differ from the live definition during a synchronized
/// Resume; their old and live branches must stay zipped instead of being folded.
pub(super) fn preserve_angle_targets(
    instance: &DynamicInstance,
    frame: &SamplingFrame,
    controller: &DynamicController,
) -> Option<HashSet<FixtureId>> {
    frame.synchronized_resume_mix.map(|_| {
        let mut held_by_target = HashMap::<FixtureId, Vec<(Uuid, &DynamicSampleExpression)>>::new();
        for (key, expression) in &instance.synchronized_hold_values {
            if key.0 == controller.id && instance.synchronized_hold_angle_sources.contains(key) {
                held_by_target
                    .entry(key.1)
                    .or_default()
                    .push((key.2, expression));
            }
        }
        frame
            .targets
            .iter()
            .copied()
            .filter(|target| {
                !same_angle_sources(
                    instance,
                    &frame.definition,
                    controller.id,
                    *target,
                    held_by_target.get(target).map_or(&[], Vec::as_slice),
                )
            })
            .collect::<HashSet<_>>()
    })
}

/// Retain held values that this frame did not pin live, either unchanged while holding or as
/// the old side of the in-progress Resume transition.
pub(super) fn retain_held_samples(
    instance: &DynamicInstance,
    frame: &SamplingFrame,
    controller: &DynamicController,
    holding: bool,
    work: &mut PinnedController,
) {
    for (key, held) in &instance.synchronized_hold_values {
        if key.0 != controller.id
            || instance.unavailable_samples.contains_key(key)
            || work.ready_keys.contains(key)
        {
            continue;
        }
        let expression = if holding {
            held.clone()
        } else {
            DynamicSampleExpression::Transition {
                from: Some(Arc::new(held.clone())),
                to: None,
                progress: frame.synchronized_resume_mix.unwrap_or(0.0),
                reason: crate::DynamicTransitionReason::Resume {
                    occurrence_id: instance
                        .synchronized_resume_transition
                        .expect("resume transition")
                        .occurrence_id,
                },
            }
        };
        work.retained.push((*key, expression));
    }
}

impl LanePinning<'_> {
    pub(super) fn pin_controller_lanes(
        &self,
        instance: &mut DynamicInstance,
        work: &mut PinnedController,
        controller: &DynamicController,
        preserve_angle_targets: Option<&HashSet<FixtureId>>,
        random_envelopes: &mut HashMap<RandomKey, f32>,
        mut undo: Option<&mut transaction::OutputFrameUndo>,
    ) {
        let frame = self.frame;
        for (target_index, target) in frame.targets.iter().copied().enumerate() {
            let preserve_angle_branches =
                preserve_angle_targets.is_some_and(|targets| targets.contains(&target));
            for (lane_index, lane) in frame.definition.lanes.iter().enumerate() {
                self.pin_lane(
                    instance,
                    work,
                    controller,
                    (target_index, target),
                    (lane_index, lane),
                    preserve_angle_branches,
                    random_envelopes,
                    undo.as_deref_mut(),
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pin_lane(
        &self,
        instance: &mut DynamicInstance,
        work: &mut PinnedController,
        controller: &DynamicController,
        (target_index, target): (usize, FixtureId),
        (lane_index, lane): (usize, &crate::DynamicLane),
        preserve_angle_branches: bool,
        random_envelopes: &mut HashMap<RandomKey, f32>,
        undo: Option<&mut transaction::OutputFrameUndo>,
    ) {
        let frame = self.frame;
        let holding = self.holding;
        let retain_held = self.retain_held;
        if !instance.lane_is_active(controller.id, target, lane.id) {
            return;
        }
        let key = (controller.id, target, lane.id);
        if instance.unavailable_samples.contains_key(&key)
            && (holding || frame.synchronized_resume_mix.is_some_and(|mix| mix < 1.0))
        {
            return;
        }
        let cached_address = self
            .addresses
            .get(target_index * frame.definition.lanes.len() + lane_index)
            .copied()
            .flatten();
        if holding && let Some(held) = instance.synchronized_hold_values.get(&key) {
            work.lanes.push(PinnedLane {
                target,
                lane_index,
                address: expression_address(held, lane, cached_address),
                preserve_angle_branches,
                fresh: false,
                value: PinnedValue::Ready(held.clone()),
            });
            if retain_held {
                work.ready_keys.insert(key);
            }
            return;
        }
        if holding && instance.synchronized_hold_captured {
            return;
        }
        if instance
            .programming_lanes
            .get(&lane.id)
            .is_some_and(|lane| lane.unavailable_native_source().is_some())
        {
            return;
        }
        let phase = self
            .random_phases
            .get(&lane.id)
            .and_then(|phases| phases.get(&target))
            .or_else(|| instance.phase_by_lane_target.get(&(lane.id, target)))
            .copied()
            .unwrap_or(0.0)
            + controller.phase_offset_degrees;
        let random = self.lane_random_envelope(instance, lane, target, random_envelopes, undo);
        let authored_occurrence = self.authored_sources.authored_occurrence(
            self.instance_id,
            controller.id,
            target,
            lane.id,
        );
        let mut expression = if let Some(address) = angle_current_address(lane) {
            DynamicSampleExpression::AngleCurrent {
                address: Arc::new(address.clone()),
            }
        } else if instance.programming_lanes.contains_key(&lane.id) {
            work.typed_indices.push(work.lanes.len());
            work.lanes.push(PinnedLane {
                target,
                lane_index,
                address: cached_address,
                preserve_angle_branches,
                fresh: true,
                value: PinnedValue::Typed {
                    phase,
                    random_envelope: random,
                    authored_occurrence,
                },
            });
            return;
        } else {
            let Some(expression) =
                self.sample_legacy_scalar(controller, target, lane, phase, random)
            else {
                return;
            };
            expression
        };
        expression.bind_fresh_authored_occurrence(authored_occurrence);
        if let Some(expression) = held_or_live_value(
            instance,
            key,
            expression,
            frame.synchronized_resume_mix,
            preserve_angle_branches,
        ) {
            let address = expression_address(&expression, lane, cached_address);
            work.lanes.push(PinnedLane {
                target,
                lane_index,
                address,
                preserve_angle_branches,
                fresh: true,
                value: PinnedValue::Ready(expression),
            });
        }
        if retain_held {
            work.ready_keys.insert(key);
        }
    }

    /// Only a reached eligible live lane requests a decision. Shared group/target lanes and
    /// controllers receive that same cached envelope in both stages.
    fn lane_random_envelope(
        &self,
        instance: &mut DynamicInstance,
        lane: &crate::DynamicLane,
        target: FixtureId,
        random_envelopes: &mut HashMap<RandomKey, f32>,
        undo: Option<&mut transaction::OutputFrameUndo>,
    ) -> Option<f32> {
        let frame = self.frame;
        let instance_id = self.instance_id;
        lane.random_group_id.and_then(|group_id| {
            let key = (instance_id, group_id, target);
            if let Some(value) = random_envelopes.get(&key) {
                return Some(*value);
            }
            let group = frame
                .definition
                .random_groups
                .iter()
                .find(|group| group.id == group_id)?;
            if let Some(undo) = undo {
                undo.random(instance, (group_id, target));
            }
            let envelope = random_envelope(
                instance
                    .random_streams
                    .entry((group_id, target))
                    .or_default(),
                group,
                instance_id,
                target,
                frame.elapsed,
                random_group_speed_factor(&frame.definition, group_id),
                self.output_interval_millis,
            );
            random_envelopes.insert(key, envelope);
            Some(envelope)
        })
    }

    fn sample_legacy_scalar(
        &self,
        controller: &DynamicController,
        target: FixtureId,
        lane: &crate::DynamicLane,
        phase: f32,
        random: Option<f32>,
    ) -> Option<DynamicSampleExpression> {
        let observed = ObservedScalarSources {
            inner: self.sources,
            used_current: std::cell::Cell::new(false),
        };
        let value = self.evaluator.sample_lane(
            lane,
            DynamicEvaluationContext {
                instance_id: self.instance_id,
                target,
                elapsed_millis: self.frame.elapsed,
                cycle_duration_millis: self.cycle_duration_millis,
                phase_degrees: phase,
                output_interval_millis: self.output_interval_millis,
                random_envelope: random,
                sources: &observed,
            },
        )?;
        let value = if controller.size == 1.0 {
            value
        } else {
            observed
                .current(target, &lane.output_owner())
                .map_or(value, |base| base + (value - base) * controller.size)
        };
        Some(DynamicSampleExpression::LegacyScalar {
            attribute: lane.output_owner(),
            value,
            occurrence: None,
            dependency_occurrence: observed.used_current.get().then(|| {
                crate::DynamicSourceDependency::unknown(
                    observed.current_occurrence(target, &lane.output_owner()),
                )
            }),
        })
    }
}
