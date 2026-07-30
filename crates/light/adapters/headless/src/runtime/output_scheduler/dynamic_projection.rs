use super::*;
use std::sync::OnceLock;

type DynamicProgrammerValues = Vec<(Uuid, i16, light_dynamics::DynamicAddressValue)>;

#[derive(Default)]
pub(in crate::runtime) struct ProgrammerReconciliationCache {
    signature: Mutex<
        Option<(
            Arc<DynamicProgrammerValues>,
            Arc<light_engine::EngineSnapshot>,
        )>,
    >,
}

impl ProgrammerReconciliationCache {
    fn changed(
        &self,
        values: &Arc<DynamicProgrammerValues>,
        snapshot: &Arc<light_engine::EngineSnapshot>,
    ) -> bool {
        let mut signature = self.signature.lock();
        let changed = signature
            .as_ref()
            .is_none_or(|(previous_values, previous_snapshot)| {
                !Arc::ptr_eq(previous_values, values) || !Arc::ptr_eq(previous_snapshot, snapshot)
            });
        if changed {
            *signature = Some((Arc::clone(values), Arc::clone(snapshot)));
        }
        changed
    }
}

struct TickSources<'a> {
    engine: &'a Engine,
    values: OnceLock<HashMap<(FixtureId, AttributeKey), AttributeValue>>,
}

impl<'a> TickSources<'a> {
    fn new(engine: &'a Engine) -> Self {
        Self {
            engine,
            values: OnceLock::new(),
        }
    }

    fn values(&self) -> &HashMap<(FixtureId, AttributeKey), AttributeValue> {
        self.values.get_or_init(|| self.engine.resolved_values())
    }
}

impl light_dynamics::ScalarSourceResolver for TickSources<'_> {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32> {
        self.values()
            .get(&(target, attribute.clone()))
            .and_then(AttributeValue::normalized)
    }

    fn preset(
        &self,
        _preset_id: &str,
        _target: FixtureId,
        _attribute: &AttributeKey,
    ) -> Option<f32> {
        None
    }
}

pub(in crate::runtime) fn dynamic_contributions(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    apply_auto_off: bool,
) -> Vec<ContributionBatch> {
    dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        None,
        apply_auto_off,
    )
    .0
}

pub(in crate::runtime) fn dynamic_contributions_cached(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: &ProgrammerReconciliationCache,
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    light_dynamics::DynamicRuntimeSnapshot,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let (batches, _, _, runtime, samples) = dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        Some(programmer_reconciliation_cache),
        apply_auto_off,
    );
    (batches, runtime, samples)
}

pub(in crate::runtime) fn dynamic_projection(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) -> (
    Vec<ContributionBatch>,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let (batches, _, _, _, samples) = dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        None,
        false,
    );
    (batches, samples)
}

pub(super) fn dynamic_contributions_with_auto_off(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: Option<&ProgrammerReconciliationCache>,
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    Vec<PlaybackIdentity>,
    Vec<light_application::EventDraft>,
    light_dynamics::DynamicRuntimeSnapshot,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let now = engine.application_time();
    let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
    // Most ticks have no Dynamic source that needs an underlay value. Keep the whole-show
    // semantic projection lazy so the ordinary output path does not resolve every Playback,
    // Group, and Programmer contribution once here and then again during DMX rendering.
    // Dynamic sampling and candidate blending share this cache when either actually needs it.
    let sources = TickSources::new(engine);
    let speed_groups = speed_groups.lock();
    let speed_transports = std::array::from_fn(|index| {
        let snapshot = speed_groups[index].snapshot(now_millis);
        light_dynamics::DynamicSpeedTransport {
            effective_bpm: snapshot.effective_bpm,
            phase_origin_millis: snapshot.phase_origin_millis,
            phase_reference_millis: speed_groups[index].phase_reference_millis(now_millis),
            beat_phase: snapshot.beat_phase,
            phase_advancing: snapshot.phase_advancing,
        }
    });
    drop(speed_groups);
    let mut dynamics = dynamics.lock();
    let before_runtime = dynamics.output_projection_snapshot();
    let programmer_values = engine.dynamic_programmer_values();
    let cue_values = engine.active_cue_dynamic_values();
    let active_dynamic_playbacks = engine
        .active_dynamic_playbacks()
        .into_iter()
        .filter(|playback| playback.enabled)
        .collect::<Vec<_>>();
    let playback_paused = engine.playback_dynamics().paused;
    if dynamic_tick_is_idle(
        &before_runtime,
        &programmer_values,
        &cue_values,
        &active_dynamic_playbacks,
        extra_programmer_values,
        playback_paused,
    ) {
        return (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            before_runtime,
            Vec::new(),
        );
    }

    let rate = u64::from(rate.load(std::sync::atomic::Ordering::Relaxed).max(1));
    let interval = (1_000 / rate).max(1);
    let engine_snapshot = engine.snapshot();
    dynamics.set_global_paused(playback_paused, now_millis);
    let reconcile_programmer = !extra_programmer_values.is_empty()
        || programmer_reconciliation_cache
            .is_none_or(|cache| cache.changed(&programmer_values, &engine_snapshot));
    if reconcile_programmer {
        reconcile_programmer_dynamics(
            &mut dynamics,
            now_millis,
            &engine_snapshot,
            &programmer_values,
            extra_programmer_values,
        );
    }
    reconcile_cue_dynamics(&mut dynamics, now_millis, &engine_snapshot, &cue_values);
    let playback_controls = reconcile_dynamic_playbacks(
        &mut dynamics,
        now_millis,
        &engine_snapshot,
        &active_dynamic_playbacks,
    );
    let samples = dynamics.sample_all(now_millis, interval, &speed_transports, &sources);
    let after_runtime = dynamics.output_projection_snapshot();
    drop(dynamics);
    let dynamic_events = dynamic_transition_events(&before_runtime, &after_runtime, now_millis);
    let auto_offs = if apply_auto_off {
        let fully_controlled = fully_controlled_dynamic_playbacks(
            engine,
            &samples,
            &playback_controls,
            &after_runtime,
            &programmer_values,
            &cue_values,
            now,
        );
        engine.auto_off_fully_controlled_dynamic_playbacks_at(fully_controlled)
    } else {
        Vec::new()
    };

    let candidates = collect_dynamic_candidates(
        &programmer_values,
        &cue_values,
        extra_programmer_values,
        &samples,
        &playback_controls,
        &sources,
        now_millis,
    );
    if candidates.is_empty() {
        return (
            Vec::new(),
            auto_offs,
            dynamic_events,
            after_runtime,
            samples,
        );
    }
    (
        vec![ContributionBatch::new(candidates.into_iter().map(
            |((fixture_id, attribute), mut stack)| {
                stack.sort_by_key(|candidate| {
                    (
                        candidate.priority,
                        candidate.changed_at_millis,
                        candidate.stable_order,
                    )
                });
                let has_dynamic = stack.iter().any(|candidate| candidate.dynamic);
                let resolved = resolve_dynamic_stack(&stack, || {
                    sources
                        .values()
                        .get(&(fixture_id, attribute.clone()))
                        .cloned()
                });
                let candidate = stack
                    .last()
                    .expect("one Dynamic/FAT candidate exists for every stack");
                let merge_mode = if attribute.is_intensity() && !has_dynamic {
                    MergeMode::Htp
                } else {
                    MergeMode::Ltp
                };
                ContributionSample::independent(TimedValue {
                    fixture_id,
                    attribute,
                    value: resolved,
                    priority: candidate.priority,
                    changed_at: chrono::DateTime::from_timestamp_millis(
                        i64::try_from(candidate.changed_at_millis).unwrap_or(i64::MAX),
                    )
                    .unwrap_or(now),
                    programmer_order: candidate.stable_order.min(u128::from(u64::MAX)) as u64,
                    merge_mode,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            },
        ))],
        auto_offs,
        dynamic_events,
        after_runtime,
        samples,
    )
}

fn dynamic_tick_is_idle(
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    active_playbacks: &[light_playback::ActiveDynamicPlayback],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    playback_paused: bool,
) -> bool {
    dynamic_tick_is_idle_from_presence(
        runtime.instances.is_empty(),
        runtime.global_paused,
        playback_paused,
        !programmer_values.is_empty(),
        !cue_values.is_empty(),
        !active_playbacks.is_empty(),
        !extra_programmer_values.is_empty(),
    )
}

fn dynamic_tick_is_idle_from_presence(
    runtime_empty: bool,
    runtime_paused: bool,
    playback_paused: bool,
    has_programmer_values: bool,
    has_cue_values: bool,
    has_active_playbacks: bool,
    has_extra_programmer_values: bool,
) -> bool {
    runtime_empty
        && runtime_paused == playback_paused
        && !has_programmer_values
        && !has_cue_values
        && !has_active_playbacks
        && !has_extra_programmer_values
}

struct DynamicCandidate {
    value: AttributeValue,
    priority: i16,
    changed_at_millis: u64,
    stable_order: u128,
    activation_mix: f32,
    dynamic: bool,
}

fn resolve_dynamic_stack(
    stack: &[DynamicCandidate],
    resolve_underlay: impl FnOnce() -> Option<AttributeValue>,
) -> AttributeValue {
    let (first, remaining) = stack
        .split_first()
        .expect("one Dynamic/FAT candidate exists for every stack");
    let mut resolved = if first.activation_mix >= 1.0 {
        first.value.clone()
    } else {
        blend_attribute_value(
            resolve_underlay().unwrap_or_else(|| first.value.clone()),
            first.value.clone(),
            first.activation_mix,
        )
    };
    for candidate in remaining {
        resolved =
            blend_attribute_value(resolved, candidate.value.clone(), candidate.activation_mix);
    }
    resolved
}

fn collect_dynamic_candidates(
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    sources: &TickSources,
    now_millis: u64,
) -> HashMap<(FixtureId, AttributeKey), Vec<DynamicCandidate>> {
    let mut candidates = HashMap::<(FixtureId, AttributeKey), Vec<DynamicCandidate>>::new();
    let mut consider = |key: (FixtureId, AttributeKey), candidate: DynamicCandidate| {
        candidates.entry(key).or_default().push(candidate);
    };
    for sample in samples {
        let dynamic_value =
            playback_controls
                .get(&sample.controller_id)
                .map_or(sample.value, |control| {
                    if sample.attribute.is_intensity() {
                        sample.value * control.master
                    } else if control.crossfade_non_intensity {
                        sources
                            .current(sample.target, &sample.attribute)
                            .map_or(sample.value, |base| {
                                base + (sample.value - base) * control.master
                            })
                    } else {
                        sample.value
                    }
                });
        if playback_controls
            .get(&sample.controller_id)
            .is_some_and(|control| {
                control.master == 0.0
                    && !sample.attribute.is_intensity()
                    && !control.crossfade_non_intensity
            })
        {
            continue;
        }
        consider(
            (sample.target, sample.attribute.clone()),
            DynamicCandidate {
                value: AttributeValue::Normalized(dynamic_value),
                priority: sample.priority,
                changed_at_millis: sample.activated_at_millis,
                stable_order: sample.controller_id.as_u128(),
                activation_mix: sample.activation_mix,
                dynamic: true,
            },
        );
    }
    for (_, priority, stored) in programmer_values.iter().chain(extra_programmer_values) {
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            (stored.fixture_id, stored.attribute.clone()),
            DynamicCandidate {
                value,
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
                stable_order: u128::from(stored.programmer_order),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
    for stored in cue_values {
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            (stored.fixture_id, stored.attribute.clone()),
            DynamicCandidate {
                value,
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
                stable_order: stored.current_cue_id.as_u128(),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
    candidates
}

pub(in crate::runtime) fn dynamic_transition_events(
    before: &light_dynamics::DynamicRuntimeSnapshot,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    now_millis: u64,
) -> Vec<light_application::EventDraft> {
    let mut events = Vec::new();
    if before.global_paused != after.global_paused {
        events.push(light_application::EventDraft::dynamic_runtime_changed(
            None,
            light_application::DynamicRuntimeChange {
                kind: if after.global_paused {
                    light_application::DynamicRuntimeEventKind::Paused
                } else {
                    light_application::DynamicRuntimeEventKind::Resumed
                },
                dynamic_id: None,
                runtime_instance_id: None,
                controller_id: None,
                winning_controller_id: None,
                occurred_at_millis: now_millis,
                message: Some("global Dynamic transport".into()),
            },
        ));
    }
    let before_instances = before
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    let after_instances = after
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    append_current_instance_events(&mut events, after, &before_instances, now_millis);
    for instance in &before.instances {
        let after_instance = after_instances.get(&instance.id).copied();
        let after_controllers = after_instance
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        for transition in &instance.controller_transitions {
            if transition.release_started_at_millis.is_some()
                && !after_controllers.contains(&transition.controller_id)
            {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind: light_application::DynamicRuntimeEventKind::TransitionCompleted,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id: Some(transition.controller_id),
                        winning_controller_id: winning_controller(after_instance),
                        occurred_at_millis: now_millis,
                        message: None,
                    },
                ));
            }
        }
        let before_winner = winning_controller(Some(instance));
        let after_winner = winning_controller(after_instance);
        if before_winner != after_winner && after_winner.is_some() {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::ControllerWinnerChanged,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: after_winner,
                    winning_controller_id: after_winner,
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
    events
}

fn append_current_instance_events(
    events: &mut Vec<light_application::EventDraft>,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    before_instances: &HashMap<Uuid, &light_dynamics::DynamicInstanceSnapshot>,
    now_millis: u64,
) {
    for instance in &after.instances {
        let previous = before_instances.get(&instance.id).copied();
        if instance.completed && !previous.is_some_and(|previous| previous.completed) {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceOff,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(previous),
                    winning_controller_id: None,
                    occurred_at_millis: now_millis,
                    message: Some("one-shot completed".into()),
                },
            ));
        }
        let previous_controllers = previous
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        if previous.is_some_and(|previous| previous.completed) && !instance.completed {
            let controller_id = winning_controller(Some(instance));
            for kind in [
                light_application::DynamicRuntimeEventKind::InstanceStarted,
                if instance
                    .pending_until_millis
                    .is_some_and(|boundary| now_millis < boundary)
                {
                    light_application::DynamicRuntimeEventKind::InstancePending
                } else {
                    light_application::DynamicRuntimeEventKind::InstanceActive
                },
            ] {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id,
                        winning_controller_id: controller_id,
                        occurred_at_millis: now_millis,
                        message: Some("one-shot retriggered".into()),
                    },
                ));
            }
        }
        for controller in &instance.controllers {
            if instance.completed || previous.is_some_and(|previous| previous.completed) {
                continue;
            }
            if !previous_controllers.contains(&controller.id) {
                for kind in [
                    light_application::DynamicRuntimeEventKind::InstanceStarted,
                    if instance
                        .pending_until_millis
                        .is_some_and(|boundary| now_millis < boundary)
                    {
                        light_application::DynamicRuntimeEventKind::InstancePending
                    } else {
                        light_application::DynamicRuntimeEventKind::InstanceActive
                    },
                ] {
                    events.push(light_application::EventDraft::dynamic_runtime_changed(
                        None,
                        light_application::DynamicRuntimeChange {
                            kind,
                            dynamic_id: Some(instance.definition.id),
                            runtime_instance_id: Some(instance.id),
                            controller_id: Some(controller.id),
                            winning_controller_id: winning_controller(Some(instance)),
                            occurred_at_millis: now_millis,
                            message: None,
                        },
                    ));
                }
            }
        }
        if !instance.completed
            && previous.is_some_and(|previous| previous.pending_until_millis.is_some())
            && instance
                .pending_until_millis
                .is_none_or(|boundary| now_millis >= boundary)
        {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceActive,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(Some(instance)),
                    winning_controller_id: winning_controller(Some(instance)),
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
}

fn winning_controller(instance: Option<&light_dynamics::DynamicInstanceSnapshot>) -> Option<Uuid> {
    instance
        .filter(|instance| !instance.completed)
        .and_then(|instance| {
            instance
                .controllers
                .iter()
                .max_by_key(|controller| {
                    (
                        controller.priority,
                        controller.activated_at_millis,
                        controller.id,
                    )
                })
                .map(|controller| controller.id)
        })
}

fn authored_activation_mix(
    changed_at_millis: u64,
    timing: light_dynamics::DynamicValueTiming,
    now_millis: u64,
) -> f32 {
    let delay = timing.delay_millis.unwrap_or_default();
    if now_millis < changed_at_millis.saturating_add(delay) {
        return 0.0;
    }
    let fade = timing.fade_millis.unwrap_or_default();
    if fade == 0 {
        return 1.0;
    }
    (now_millis
        .saturating_sub(changed_at_millis)
        .saturating_sub(delay) as f32
        / fade as f32)
        .clamp(0.0, 1.0)
}

fn blend_attribute_value(
    underlying: AttributeValue,
    contribution: AttributeValue,
    mix: f32,
) -> AttributeValue {
    match (underlying.normalized(), contribution.normalized()) {
        (Some(underlying), Some(contribution)) => AttributeValue::Normalized(
            underlying + (contribution - underlying) * mix.clamp(0.0, 1.0),
        ),
        _ if mix >= 0.5 => contribution,
        _ => underlying,
    }
}

pub(in crate::runtime) struct DynamicPlaybackControl {
    pub(in crate::runtime) identity: PlaybackIdentity,
    pub(in crate::runtime) master: f32,
    pub(in crate::runtime) crossfade_non_intensity: bool,
    pub(in crate::runtime) auto_off_full_control: bool,
    pub(in crate::runtime) temporary_only: bool,
}

pub(in crate::runtime) fn fully_controlled_dynamic_playbacks(
    engine: &Engine,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<PlaybackIdentity> {
    if !controls
        .values()
        .any(|control| control.auto_off_full_control)
    {
        return Vec::new();
    }
    let persistent = engine
        .playback_contributions_at(now)
        .into_iter()
        .filter(|contribution| !contribution.source.temporary)
        .collect::<Vec<_>>();
    let mut addresses = HashMap::<
        PlaybackIdentity,
        Vec<(
            &light_dynamics::DynamicRuntimeSample,
            &DynamicPlaybackControl,
        )>,
    >::new();
    let controller_sources = runtime
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .controllers
                .iter()
                .map(|controller| (controller.id, controller.source.clone()))
        })
        .collect::<HashMap<_, _>>();
    let persistent_fat = persistent_fat_values(programmer_values, cue_values);
    for sample in samples {
        if let Some(control) = controls.get(&sample.controller_id)
            && control.auto_off_full_control
        {
            addresses
                .entry(control.identity)
                .or_default()
                .push((sample, control));
        }
    }
    addresses
        .into_iter()
        .filter_map(|(identity, target_samples)| {
            (!target_samples.is_empty()
                && target_samples.iter().all(|(sample, control)| {
                    let dynamic_value = if sample.attribute.is_intensity() {
                        sample.value * control.master
                    } else {
                        sample.value
                    };
                    persistent.iter().any(|candidate| {
                        candidate_playback_identity(candidate)
                            .is_some_and(|other| other != identity)
                            && candidate.value.fixture_id == sample.target
                            && candidate.value.attribute == sample.attribute
                            && persistent_playback_wins_dynamic(candidate, sample, dynamic_value)
                    }) || samples.iter().any(|candidate| {
                        candidate.controller_id != sample.controller_id
                            && candidate.target == sample.target
                            && candidate.attribute == sample.attribute
                            && candidate.activation_mix >= 1.0
                            && controller_sources
                                .get(&candidate.controller_id)
                                .is_some_and(|source| match source {
                                    light_dynamics::DynamicControllerSource::Playback {
                                        ..
                                    } => controls.get(&candidate.controller_id).is_some_and(
                                        |control| {
                                            control.identity != identity && !control.temporary_only
                                        },
                                    ),
                                    light_dynamics::DynamicControllerSource::Programmer {
                                        ..
                                    }
                                    | light_dynamics::DynamicControllerSource::Cue { .. } => true,
                                })
                            && persistent_dynamic_wins_dynamic(candidate, sample)
                    }) || persistent_fat.iter().any(|candidate| {
                        candidate.fixture_id == sample.target
                            && candidate.attribute == sample.attribute
                            && persistent_semantic_wins_dynamic(
                                candidate.priority,
                                candidate.changed_at_millis,
                                sample,
                            )
                    })
                }))
            .then_some(identity)
        })
        .collect()
}

#[derive(Clone)]
struct PersistentFatValue {
    fixture_id: FixtureId,
    attribute: AttributeKey,
    priority: i16,
    changed_at_millis: u64,
}

fn persistent_fat_values(
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> Vec<PersistentFatValue> {
    let programmer = programmer_values
        .iter()
        .filter_map(|(_, priority, stored)| {
            matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute.clone(),
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    let cues = cue_values.iter().filter_map(|stored| {
        matches!(
            &stored.value,
            light_dynamics::DynamicSemanticValue::FixAt { .. }
        )
        .then_some(PersistentFatValue {
            fixture_id: stored.fixture_id,
            attribute: stored.attribute.clone(),
            priority: stored.priority,
            changed_at_millis: stored.changed_at_millis,
        })
    });
    programmer.chain(cues).collect()
}

fn candidate_playback_identity(
    candidate: &light_playback::PlaybackContribution,
) -> Option<PlaybackIdentity> {
    candidate.source.playback_identity.or_else(|| {
        candidate
            .source
            .playback_number
            .and_then(|number| PlaybackIdentity::physical(number).ok())
    })
}

fn persistent_dynamic_wins_dynamic(
    candidate: &light_dynamics::DynamicRuntimeSample,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (
        candidate.priority,
        candidate.activated_at_millis,
        candidate.controller_id,
    ) > (
        dynamic.priority,
        dynamic.activated_at_millis,
        dynamic.controller_id,
    )
}

fn persistent_semantic_wins_dynamic(
    priority: i16,
    changed_at_millis: u64,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (priority, changed_at_millis) > (dynamic.priority, dynamic.activated_at_millis)
}

fn persistent_playback_wins_dynamic(
    candidate: &light_playback::PlaybackContribution,
    dynamic: &light_dynamics::DynamicRuntimeSample,
    dynamic_value: f32,
) -> bool {
    if candidate.value.priority != dynamic.priority {
        return candidate.value.priority > dynamic.priority;
    }
    if candidate.value.merge_mode == MergeMode::Htp {
        return candidate.value.value.normalized().unwrap_or(0.0) > dynamic_value;
    }
    u64::try_from(candidate.value.changed_at.timestamp_millis()).unwrap_or_default()
        > dynamic.activated_at_millis
}

#[cfg(test)]
mod tick_source_tests {
    use super::*;
    use light_programmer::ProgrammerRegistry;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn tick_sources_defer_whole_show_resolution() {
        let engine = Engine::new(ProgrammerRegistry::default());
        let sources = TickSources::new(&engine);

        assert!(
            sources.values.get().is_none(),
            "constructing an output tick must not eagerly resolve the whole show"
        );
    }

    #[test]
    fn fully_active_first_candidate_does_not_resolve_the_underlay() {
        let underlay_requested = AtomicBool::new(false);
        let stack = [DynamicCandidate {
            value: AttributeValue::Normalized(0.75),
            priority: 10,
            changed_at_millis: 1,
            stable_order: 1,
            activation_mix: 1.0,
            dynamic: true,
        }];

        let resolved = resolve_dynamic_stack(&stack, || {
            underlay_requested.store(true, Ordering::Relaxed);
            Some(AttributeValue::Normalized(0.25))
        });

        assert_eq!(resolved, AttributeValue::Normalized(0.75));
        assert!(
            !underlay_requested.load(Ordering::Relaxed),
            "a fully active winning candidate makes the underlay irrelevant"
        );
    }

    #[test]
    fn fading_first_candidate_blends_from_the_underlay() {
        let stack = [DynamicCandidate {
            value: AttributeValue::Normalized(1.0),
            priority: 10,
            changed_at_millis: 1,
            stable_order: 1,
            activation_mix: 0.5,
            dynamic: true,
        }];

        let resolved = resolve_dynamic_stack(&stack, || Some(AttributeValue::Normalized(0.0)));

        assert_eq!(resolved, AttributeValue::Normalized(0.5));
    }

    #[test]
    fn only_a_completely_static_tick_is_idle() {
        assert!(dynamic_tick_is_idle_from_presence(
            true, false, false, false, false, false, false,
        ));

        for non_idle in [
            dynamic_tick_is_idle_from_presence(false, false, false, false, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, true, false, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, true, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, true, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, false, true, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, false, false, true),
        ] {
            assert!(
                !non_idle,
                "runtime, pause, Dynamic/FAT, Playback, and extra inputs must use the full path"
            );
        }
    }

    #[test]
    fn matching_paused_state_can_still_be_idle() {
        assert!(dynamic_tick_is_idle_from_presence(
            true, true, true, false, false, false, false,
        ));
    }
}
