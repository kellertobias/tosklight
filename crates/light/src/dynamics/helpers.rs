use super::*;

pub(super) fn validate_fix_at_targets(
    snapshot: &EngineSnapshot,
    targets: &[FixtureId],
    attribute: &AttributeKey,
) -> Result<(), ActionError> {
    let mut unsupported = 0_usize;
    let mut discrete = 0_usize;
    for target in targets {
        let Some(fixture) = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == *target)
        else {
            unsupported += 1;
            continue;
        };
        let parameters = fixture
            .definition
            .heads
            .iter()
            .flat_map(|head| &head.parameters)
            .filter(|parameter| parameter.attribute == *attribute)
            .collect::<Vec<_>>();
        if parameters.is_empty() {
            unsupported += 1;
            continue;
        }
        let profile_discrete = fixture
            .definition
            .profile_snapshot
            .as_deref()
            .zip(fixture.definition.mode_id)
            .and_then(|(profile, mode_id)| profile.mode(mode_id))
            .is_some_and(|mode| {
                let matching = mode
                    .channels
                    .iter()
                    .filter(|channel| channel.attribute == *attribute)
                    .collect::<Vec<_>>();
                !matching.is_empty()
                    && matching.iter().all(|channel| {
                        !channel.functions.is_empty()
                            && channel.functions.iter().all(|function| {
                                !matches!(
                                    function.behavior,
                                    light_fixture::ChannelFunctionBehavior::Continuous { .. }
                                )
                            })
                    })
            });
        if profile_discrete
            || parameters
                .iter()
                .all(|parameter| !parameter.capabilities.is_empty())
        {
            discrete += 1;
        }
    }
    if unsupported > 0 {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            format!(
                "FixAT attribute {} is unsupported on {unsupported} of {} selected targets",
                attribute.0,
                targets.len(),
            ),
        ));
    }
    if discrete > 0 {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            format!(
                "FixAT requires a scalar attribute; {} is discrete on {discrete} of {} selected targets",
                attribute.0,
                targets.len(),
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_release_targets(
    snapshot: &EngineSnapshot,
    targets: &[light_programmer::ReleaseProgrammerFixtureValue],
) -> Result<(), ActionError> {
    for target in targets {
        let supported = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == target.fixture_id)
            .is_some_and(|fixture| {
                fixture
                    .definition
                    .heads
                    .iter()
                    .flat_map(|head| &head.parameters)
                    .any(|parameter| parameter.attribute == target.attribute)
            });
        if !supported {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                format!(
                    "RELEASE attribute {} is unsupported on fixture {}",
                    target.attribute.0, target.fixture_id.0
                ),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct DynamicsIdentity {
    pub(super) session: SessionId,
    pub(super) user: UserId,
}

pub(super) fn identity(context: &ActionContext) -> Result<DynamicsIdentity, ActionError> {
    Ok(DynamicsIdentity {
        session: context
            .session_id
            .map(SessionId)
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "session required"))?,
        user: context
            .user_id
            .map(UserId)
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "user required"))?,
    })
}

pub(super) fn definition(
    snapshot: &EngineSnapshot,
    id: Uuid,
) -> Result<&DynamicDefinition, ActionError> {
    snapshot
        .dynamics
        .iter()
        .find(|dynamic| dynamic.id == id)
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "Dynamic does not exist"))
}

pub(super) fn definition_and_targets<'a>(
    context: &ActionContext,
    ports: &dyn DynamicsPorts,
    programmers: &ProgrammerRegistry,
    session: SessionId,
    snapshot: &'a EngineSnapshot,
    dynamic_id: Uuid,
    explicit: &[FixtureId],
) -> Result<
    (
        &'a DynamicDefinition,
        Vec<FixtureId>,
        Option<SpatialSelectionMapping>,
    ),
    ActionError,
> {
    let result = definition(snapshot, dynamic_id).and_then(|definition| {
        resolve_targets(programmers, session, snapshot, definition, explicit)
            .map(|(targets, mapping)| (definition, targets, mapping))
    });
    if let Err(error) = &result {
        ports.publish_runtime_change(
            context,
            crate::DynamicRuntimeChange {
                kind: crate::DynamicRuntimeEventKind::FailedDependency,
                dynamic_id: Some(dynamic_id),
                runtime_instance_id: None,
                controller_id: None,
                winning_controller_id: None,
                occurred_at_millis: ports.now_millis(),
                message: Some(error.message.clone()),
            },
        );
    }
    result
}

pub(super) fn resolve_targets(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    snapshot: &EngineSnapshot,
    definition: &DynamicDefinition,
    explicit: &[FixtureId],
) -> Result<(Vec<FixtureId>, Option<SpatialSelectionMapping>), ActionError> {
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let bound = match &definition.target_binding {
        DynamicTargetBinding::LiveGroup { group_id } => {
            let positions = snapshot
                .dynamic_stage_positions
                .iter()
                .map(|(fixture_id, position)| {
                    (
                        *fixture_id,
                        Position3d {
                            x: f64::from(position.x),
                            y: f64::from(position.y),
                            z: f64::from(position.z),
                        },
                    )
                })
                .collect();
            let resolved = resolve_group_spatial(group_id, &groups, &positions)
                .map_err(|message| ActionError::new(ActionErrorKind::Invalid, message))?;
            Some((resolved.source_order, resolved.effective_mapping))
        }
        DynamicTargetBinding::FrozenTargets { targets } => Some((targets.clone(), None)),
        DynamicTargetBinding::Targetless => None,
    };
    if let Some((targets, inherited_spatial_mapping)) = bound {
        if targets.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "Dynamic target scope is empty",
            ));
        }
        return Ok((targets, inherited_spatial_mapping));
    }
    let targets = if !explicit.is_empty() {
        explicit.to_vec()
    } else if let Some(selection) = programmers.selection(session)
        && !selection.selected.is_empty()
    {
        selection.selected
    } else {
        snapshot
            .fixtures
            .iter()
            .map(|fixture| fixture.fixture_id)
            .collect()
    };
    if targets.is_empty() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Dynamic target scope is empty",
        ));
    }
    Ok((targets, None))
}

pub(super) fn matching_programmer_controller(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    dynamic_id: Uuid,
    targets: &[FixtureId],
) -> Option<Uuid> {
    let state = programmers.get(session)?;
    let target_set = targets
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut by_controller = HashMap::<Uuid, std::collections::HashSet<FixtureId>>::new();
    for value in state
        .dynamic_values
        .iter()
        .chain(state.preload_dynamic_pending.iter())
    {
        if let DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            ..
        } = &value.value
            && dynamic.dynamic_id == Some(dynamic_id)
        {
            by_controller
                .entry(*instance_link)
                .or_default()
                .insert(value.fixture_id);
        }
    }
    by_controller
        .into_iter()
        .find_map(|(controller, found)| (found == target_set).then_some(controller))
}

pub(super) fn controller_dynamic_id(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    controller_id: Uuid,
) -> Option<Uuid> {
    let state = programmers.get(session)?;
    state
        .dynamic_values
        .iter()
        .chain(state.preload_dynamic_pending.iter())
        .find_map(|stored| match &stored.value {
            DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic,
                ..
            } if *instance_link == controller_id => dynamic.dynamic_id,
            _ => None,
        })
}

pub(super) fn publish_start_events(
    context: &ActionContext,
    ports: &dyn DynamicsPorts,
    dynamic_id: Uuid,
    outcome: &DynamicStartOutcome,
    preload: bool,
) {
    let now = ports.now_millis();
    let change = |kind| crate::DynamicRuntimeChange {
        kind,
        dynamic_id: Some(dynamic_id),
        runtime_instance_id: Some(outcome.runtime_instance_id),
        controller_id: Some(outcome.controller_id),
        winning_controller_id: (!preload).then_some(outcome.controller_id),
        occurred_at_millis: now,
        message: preload.then(|| "staged in Preload".into()),
    };
    ports.publish_runtime_change(
        context,
        change(crate::DynamicRuntimeEventKind::InstanceStarted),
    );
    ports.publish_runtime_change(
        context,
        change(if preload {
            crate::DynamicRuntimeEventKind::InstancePending
        } else {
            crate::DynamicRuntimeEventKind::InstanceActive
        }),
    );
    if !preload {
        ports.publish_runtime_change(
            context,
            change(crate::DynamicRuntimeEventKind::ControllerWinnerChanged),
        );
    }
}

pub(super) fn publish_off_events(
    context: &ActionContext,
    ports: &dyn DynamicsPorts,
    dynamic_id: Option<Uuid>,
    outcome: &DynamicStartOutcome,
    timing: DynamicValueTiming,
    preload: bool,
) {
    let releasing =
        timing.delay_millis.unwrap_or_default() > 0 || timing.fade_millis.unwrap_or_default() > 0;
    ports.publish_runtime_change(
        context,
        crate::DynamicRuntimeChange {
            kind: if preload {
                crate::DynamicRuntimeEventKind::InstancePending
            } else if releasing {
                crate::DynamicRuntimeEventKind::InstanceRelease
            } else {
                crate::DynamicRuntimeEventKind::InstanceOff
            },
            dynamic_id,
            runtime_instance_id: Some(outcome.runtime_instance_id),
            controller_id: Some(outcome.controller_id),
            winning_controller_id: None,
            occurred_at_millis: ports.now_millis(),
            message: preload.then(|| "staged Dynamic Off in Preload".into()),
        },
    );
}

pub(super) fn programmer_preload_active(
    programmers: &ProgrammerRegistry,
    session: SessionId,
) -> bool {
    programmers
        .get(session)
        .is_some_and(|state| state.blind && state.preload_capture_programmer)
}

pub(super) fn controller_targets(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    controller_id: Uuid,
) -> Vec<FixtureId> {
    let Some(state) = programmers.get(session) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for stored in state
        .dynamic_values
        .iter()
        .chain(state.preload_dynamic_pending.iter())
    {
        if matches!(
            stored.value,
            DynamicSemanticValue::DynamicOn { instance_link, .. }
                if instance_link == controller_id
        ) && !targets.contains(&stored.fixture_id)
        {
            targets.push(stored.fixture_id);
        }
    }
    targets
}

pub(super) fn store_off(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    controller_id: Uuid,
    timing: DynamicValueTiming,
) -> Result<(), ActionError> {
    let state = programmers
        .get(session)
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable"))?;
    let preload = state.blind && state.preload_capture_programmer;
    let mutations = state
        .dynamic_values
        .iter()
        .chain(
            preload
                .then(|| state.preload_dynamic_pending.iter())
                .into_iter()
                .flatten(),
        )
        .filter(|stored| {
            matches!(
                stored.value,
                DynamicSemanticValue::DynamicOn { instance_link, .. }
                    if instance_link == controller_id
            )
        })
        .map(|stored| DynamicProgrammerValueMutation::Set {
            fixture_id: stored.fixture_id,
            attribute: stored.attribute.clone(),
            value: DynamicSemanticValue::DynamicOff {
                instance_link: controller_id,
                timing,
            },
        })
        .collect::<Vec<_>>();
    if mutations.is_empty() || !programmers.apply_dynamic_values(session, &mutations, None) {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            "Dynamic Off produced no Programmer change",
        ));
    }
    Ok(())
}
