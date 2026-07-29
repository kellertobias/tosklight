use super::dynamic_projection::DynamicPlaybackControl;
use super::*;

pub(super) fn reconcile_dynamic_playbacks(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
) -> HashMap<Uuid, DynamicPlaybackControl> {
    let snapshot = engine.snapshot();
    let active = engine
        .active_dynamic_playbacks()
        .into_iter()
        .filter(|playback| playback.enabled)
        .collect::<Vec<_>>();
    let desired_ids = active
        .iter()
        .map(dynamic_playback_controller_id)
        .collect::<HashSet<_>>();
    release_stale_playback_controllers(dynamics, &snapshot, &desired_ids, now_millis);

    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut controls = HashMap::new();
    for active in active {
        let Some(playback) = dynamic_playback_definition(&snapshot, &active) else {
            continue;
        };
        let light_playback::PlaybackTarget::Dynamic { assignment } = &playback.target else {
            continue;
        };
        let controller_id = dynamic_playback_controller_id(&active);
        controls.insert(
            controller_id,
            DynamicPlaybackControl {
                identity: active.playback_identity.unwrap_or_else(|| {
                    PlaybackIdentity::physical(active.playback_number)
                        .expect("active physical Dynamic Playback number is validated")
                }),
                master: active.master,
                crossfade_non_intensity: assignment.crossfade_non_intensity,
                auto_off_full_control: assignment.auto_off_full_control,
                temporary_only: active.flash && active.flash_restore_off,
            },
        );
        let definition = assignment
            .dynamic
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*assignment.dynamic.embedded_fallback.definition).clone());
        let speed_multiplier = effective_dynamic_playback_speed(&definition, &active);
        if let Some((instance_id, _)) = dynamics.controller(controller_id) {
            let _ = dynamics.update_controller(
                controller_id,
                Some(active.size),
                Some(speed_multiplier),
                None,
            );
            let resume_policy = match assignment.resume_policy {
                light_playback::DynamicPlaybackResumePolicy::FollowDynamic => None,
                light_playback::DynamicPlaybackResumePolicy::ResumeFrozenPhase => {
                    Some(light_dynamics::ActivationPolicy::StartNow)
                }
                light_playback::DynamicPlaybackResumePolicy::RejoinSynchronizedPosition => {
                    Some(light_dynamics::ActivationPolicy::JoinSyncNow)
                }
                light_playback::DynamicPlaybackResumePolicy::ResumeOnNextBoundary => {
                    Some(light_dynamics::ActivationPolicy::NextBoundary)
                }
            };
            let _ = dynamics.set_controller_paused_with_resume(
                instance_id,
                controller_id,
                active.paused,
                now_millis,
                resume_policy,
            );
            continue;
        }
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let targets = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => targets.clone(),
            light_dynamics::DynamicTargetBinding::Targetless => match &assignment.target_scope {
                Some(light_playback::DynamicPlaybackTargetScope::LiveGroup { group_id }) => {
                    light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
                }
                Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets }) => {
                    targets.clone()
                }
                None => Vec::new(),
            },
        };
        let activated_at_millis =
            u64::try_from(active.activated_at.timestamp_millis()).unwrap_or_default();
        if dynamics
            .start(light_dynamics::DynamicStartRequest {
                definition_id: definition.id,
                controller: light_dynamics::DynamicController {
                    id: controller_id,
                    source: light_dynamics::DynamicControllerSource::Playback {
                        playback_number: active.playback_number,
                    },
                    priority: assignment.priority,
                    activated_at_millis,
                    size: active.size,
                    speed_multiplier,
                    phase_offset_degrees: 0.0,
                    paused: active.paused,
                },
                target_scope: light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                stage_positions: (*snapshot.dynamic_stage_positions).clone(),
                now_millis: activated_at_millis.min(now_millis),
                activation_delay_millis: 0,
                activation_duration_millis: playback.xfade_millis,
                activation_policy_override: assignment.activation_override,
                reuse_matching_targetless: false,
            })
            .is_err()
        {
            controls.remove(&controller_id);
        }
    }
    controls
}

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
    let address = match playback.playback_identity {
        Some(light_playback::PlaybackIdentity::Virtual(address)) => {
            (u128::from(address.page()) << 16) | u128::from(address.number().get())
        }
        _ => u128::from(playback.playback_number),
    };
    Uuid::from_u128(0x4459_4e41_4d49_432d_504c_4159_4241_434b ^ address)
}

pub(super) fn reconcile_programmer_dynamics(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) {
    struct DesiredController {
        programmer_id: Uuid,
        priority: i16,
        activated_at_millis: u64,
        reference: light_dynamics::DynamicReference,
        overrides: light_dynamics::DynamicInstanceOverrides,
        timing: light_dynamics::DynamicValueTiming,
        targets: Vec<FixtureId>,
    }

    let values = engine
        .dynamic_programmer_values()
        .into_iter()
        .chain(extra_values.iter().cloned());
    let mut desired = HashMap::<Uuid, DesiredController>::new();
    let mut off = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for (programmer_id, priority, stored) in values {
        match stored.value {
            light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic,
                overrides,
                timing,
                ..
            } => {
                let controller =
                    desired
                        .entry(instance_link)
                        .or_insert_with(|| DesiredController {
                            programmer_id,
                            priority,
                            activated_at_millis: stored.changed_at_millis,
                            reference: dynamic,
                            overrides,
                            timing,
                            targets: Vec::new(),
                        });
                if !controller.targets.contains(&stored.fixture_id) {
                    controller.targets.push(stored.fixture_id);
                }
            }
            light_dynamics::DynamicSemanticValue::DynamicOff {
                instance_link,
                timing,
            } => {
                off.entry(instance_link).or_insert(timing);
            }
            light_dynamics::DynamicSemanticValue::Static { .. }
            | light_dynamics::DynamicSemanticValue::FixAt { .. }
            | light_dynamics::DynamicSemanticValue::Release => {}
        }
    }
    for controller_id in off.keys() {
        desired.remove(controller_id);
    }

    for (controller_id, timing) in &off {
        if let Some((instance_id, _)) = dynamics.controller(*controller_id) {
            let _ = dynamics.off_controller(
                instance_id,
                *controller_id,
                now_millis,
                timing.delay_millis.unwrap_or_default(),
                timing.fade_millis.unwrap_or_default(),
            );
        }
    }
    let desired_ids = desired.keys().copied().collect::<HashSet<_>>();
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Programmer { .. }
        ) && !desired_ids.contains(&controller.id)
            && !off.contains_key(&controller.id)
        {
            let _ = dynamics.off_controller(instance_id, controller.id, now_millis, 0, 0);
        }
    }

    let snapshot = engine.snapshot();
    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    for (controller_id, desired) in desired {
        if dynamics.controller(controller_id).is_some() {
            let _ = dynamics.update_controller(
                controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
            );
            continue;
        }
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let _ = dynamics.start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: controller_id,
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: desired.programmer_id,
                },
                priority: desired.priority,
                activated_at_millis: desired.activated_at_millis,
                size: desired.overrides.size,
                speed_multiplier: desired.overrides.speed_multiplier.factor() as f32,
                phase_offset_degrees: desired.overrides.phase_offset_degrees,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: desired.targets,
            },
            stage_positions: (*snapshot.dynamic_stage_positions).clone(),
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: true,
        });
    }
}

pub(super) fn reconcile_cue_dynamics(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
) {
    struct DesiredController {
        controller_id: Uuid,
        instance_link: Uuid,
        cue_list_id: light_core::CueListId,
        priority: i16,
        activated_at_millis: u64,
        reference: light_dynamics::DynamicReference,
        overrides: light_dynamics::DynamicInstanceOverrides,
        timing: light_dynamics::DynamicValueTiming,
        targets: Vec<FixtureId>,
    }

    let mut desired = Vec::<DesiredController>::new();
    let mut release_timings = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for stored in engine.active_cue_dynamic_values() {
        if let light_dynamics::DynamicSemanticValue::DynamicOff {
            instance_link,
            timing,
        } = &stored.value
        {
            release_timings.insert(
                cue_dynamic_controller_id(stored.cue_list_id, *instance_link),
                *timing,
            );
            continue;
        }
        let light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            overrides,
            timing,
            ..
        } = stored.value
        else {
            continue;
        };
        let controller_id = cue_dynamic_controller_id(stored.cue_list_id, instance_link);
        if let Some(controller) = desired
            .iter_mut()
            .find(|candidate| candidate.controller_id == controller_id)
        {
            if !controller.targets.contains(&stored.fixture_id) {
                controller.targets.push(stored.fixture_id);
            }
            continue;
        }
        desired.push(DesiredController {
            controller_id,
            instance_link,
            cue_list_id: stored.cue_list_id,
            priority: stored.priority,
            activated_at_millis: stored.changed_at_millis,
            reference: dynamic,
            overrides,
            timing,
            targets: vec![stored.fixture_id],
        });
    }

    let desired_ids = desired
        .iter()
        .map(|controller| controller.controller_id)
        .collect::<HashSet<_>>();
    release_inactive_cue_controllers(dynamics, &desired_ids, &release_timings, now_millis);

    let snapshot = engine.snapshot();
    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    for desired in desired {
        if dynamics.controller(desired.controller_id).is_some() {
            let _ = dynamics.update_controller(
                desired.controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
            );
            continue;
        }
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let targets = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => targets.clone(),
            light_dynamics::DynamicTargetBinding::Targetless => desired.targets,
        };
        let _ = dynamics.start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: desired.controller_id,
                source: light_dynamics::DynamicControllerSource::Cue {
                    cue_list_id: desired.cue_list_id.0,
                    instance_link: desired.instance_link,
                },
                priority: desired.priority,
                activated_at_millis: desired.activated_at_millis,
                size: desired.overrides.size,
                speed_multiplier: desired.overrides.speed_multiplier.factor() as f32,
                phase_offset_degrees: desired.overrides.phase_offset_degrees,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: targets,
            },
            stage_positions: (*snapshot.dynamic_stage_positions).clone(),
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: false,
        });
    }
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
