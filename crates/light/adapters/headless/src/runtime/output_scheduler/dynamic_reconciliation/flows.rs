use super::*;

struct DesiredProgrammerController<'a> {
    programmer_id: Uuid,
    priority: i16,
    activated_at_millis: u64,
    reference: &'a light_dynamics::DynamicReference,
    overrides: &'a light_dynamics::DynamicInstanceOverrides,
    timing: light_dynamics::DynamicValueTiming,
    targets: Vec<FixtureId>,
    target_ids: HashSet<FixtureId>,
}

struct DesiredCueController<'a> {
    controller_id: Uuid,
    instance_link: Uuid,
    cue_list_id: light_core::CueListId,
    priority: i16,
    activated_at_millis: u64,
    reference: &'a light_dynamics::DynamicReference,
    overrides: &'a light_dynamics::DynamicInstanceOverrides,
    timing: light_dynamics::DynamicValueTiming,
    targets: Vec<FixtureId>,
    target_ids: HashSet<FixtureId>,
}

pub(in crate::runtime::output_scheduler) fn reconcile_dynamic_playbacks(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    active: &[light_playback::ActiveDynamicPlayback],
) -> HashMap<Uuid, DynamicPlaybackControl> {
    let desired_ids = active
        .iter()
        .map(dynamic_playback_controller_id)
        .collect::<HashSet<_>>();
    release_stale_playback_controllers(dynamics, &snapshot, &desired_ids, now_millis);
    if active.is_empty() {
        return HashMap::new();
    }

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
        let (targets, inherited_spatial_mapping) = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                resolve_dynamic_group(group_id, &groups, snapshot).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
                (targets.clone(), None)
            }
            light_dynamics::DynamicTargetBinding::Targetless => match &assignment.target_scope {
                Some(light_playback::DynamicPlaybackTargetScope::LiveGroup { group_id }) => {
                    let targets = resolve_dynamic_group(group_id, &groups, snapshot)
                        .map(|(targets, _)| targets)
                        .unwrap_or_default();
                    (targets, None)
                }
                Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets }) => {
                    (targets.clone(), None)
                }
                None => (Vec::new(), None),
            },
        };
        if let Some((instance_id, _)) = dynamics.controller(controller_id) {
            let _ = dynamics.reconcile_instance_targets(
                instance_id,
                light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                snapshot.dynamic_stage_positions.as_ref(),
                inherited_spatial_mapping.as_ref(),
            );
            let _ = dynamics.update_controller(
                controller_id,
                Some(active.size),
                Some(speed_multiplier),
                None,
            );
            let resume_policy = activation_policy(assignment.resume_policy);
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
                inherited_spatial_mapping,
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

fn activation_policy(
    policy: light_playback::DynamicPlaybackResumePolicy,
) -> Option<light_dynamics::ActivationPolicy> {
    match policy {
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
    }
}

pub(in crate::runtime::output_scheduler) fn reconcile_programmer_dynamics(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) {
    let values = programmer_values.iter().chain(extra_values);
    let mut desired = HashMap::<Uuid, DesiredProgrammerController>::new();
    let mut off = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for (programmer_id, priority, stored) in values {
        match &stored.value {
            light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic,
                overrides,
                timing,
                ..
            } => {
                let controller =
                    desired
                        .entry(*instance_link)
                        .or_insert_with(|| DesiredProgrammerController {
                            programmer_id: *programmer_id,
                            priority: *priority,
                            activated_at_millis: stored.changed_at_millis,
                            reference: dynamic,
                            overrides,
                            timing: *timing,
                            targets: Vec::new(),
                            target_ids: HashSet::new(),
                        });
                if controller.target_ids.insert(stored.fixture_id) {
                    controller.targets.push(stored.fixture_id);
                }
            }
            light_dynamics::DynamicSemanticValue::DynamicOff {
                instance_link,
                timing,
            } => {
                off.entry(*instance_link).or_insert(*timing);
            }
            light_dynamics::DynamicSemanticValue::Static { .. }
            | light_dynamics::DynamicSemanticValue::FixAt { .. }
            | light_dynamics::DynamicSemanticValue::Release => {}
        }
    }
    for controller_id in off.keys() {
        desired.remove(controller_id);
    }

    release_programmer_off(dynamics, now_millis, &off);
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
    if desired.is_empty() {
        return;
    }

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
    for (controller_id, desired) in desired {
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        let (targets, inherited_spatial_mapping) = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                resolve_dynamic_group(group_id, &groups, snapshot).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
                (targets.clone(), None)
            }
            light_dynamics::DynamicTargetBinding::Targetless => (desired.targets, None),
        };
        if let Some((instance_id, _)) = dynamics.controller(controller_id) {
            let _ = dynamics.reconcile_instance_targets(
                instance_id,
                light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                snapshot.dynamic_stage_positions.as_ref(),
                inherited_spatial_mapping.as_ref(),
            );
            let _ = dynamics.update_controller(
                controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
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
                ordered_targets: targets,
            },
            stage_positions: (*snapshot.dynamic_stage_positions).clone(),
            inherited_spatial_mapping,
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: true,
        });
    }
}

fn release_programmer_off(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    off: &HashMap<Uuid, light_dynamics::DynamicValueTiming>,
) {
    for (controller_id, timing) in off {
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
}

pub(in crate::runtime::output_scheduler) fn reconcile_cue_dynamics(
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    snapshot: &light_engine::EngineSnapshot,
    cue_values: &[light_playback::ActiveCueDynamicValue],
) {
    let mut desired = Vec::<DesiredCueController>::new();
    let mut release_timings = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for stored in cue_values {
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
        } = &stored.value
        else {
            continue;
        };
        let controller_id = cue_dynamic_controller_id(stored.cue_list_id, *instance_link);
        if let Some(controller) = desired
            .iter_mut()
            .find(|candidate| candidate.controller_id == controller_id)
        {
            if controller.target_ids.insert(stored.fixture_id) {
                controller.targets.push(stored.fixture_id);
            }
            continue;
        }
        desired.push(DesiredCueController {
            controller_id,
            instance_link: *instance_link,
            cue_list_id: stored.cue_list_id,
            priority: stored.priority,
            activated_at_millis: stored.changed_at_millis,
            reference: dynamic,
            overrides,
            timing: *timing,
            targets: vec![stored.fixture_id],
            target_ids: HashSet::from([stored.fixture_id]),
        });
    }

    let desired_ids = desired
        .iter()
        .map(|controller| controller.controller_id)
        .collect::<HashSet<_>>();
    release_inactive_cue_controllers(dynamics, &desired_ids, &release_timings, now_millis);
    if desired.is_empty() {
        return;
    }

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
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        let (targets, inherited_spatial_mapping) = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                resolve_dynamic_group(group_id, &groups, snapshot).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => {
                (targets.clone(), None)
            }
            light_dynamics::DynamicTargetBinding::Targetless => (desired.targets, None),
        };
        if let Some((instance_id, _)) = dynamics.controller(desired.controller_id) {
            let _ = dynamics.reconcile_instance_targets(
                instance_id,
                light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                snapshot.dynamic_stage_positions.as_ref(),
                inherited_spatial_mapping.as_ref(),
            );
            let _ = dynamics.update_controller(
                desired.controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
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
            inherited_spatial_mapping,
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: false,
        });
    }
}
