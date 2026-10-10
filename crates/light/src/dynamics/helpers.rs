use super::*;

pub(super) fn validate_fix_at_targets(
    snapshot: &EngineSnapshot,
    targets: &[FixtureId],
    attribute: &AttributeKey,
) -> Result<(), ActionError> {
    let mut unsupported = 0_usize;
    let mut discrete = 0_usize;
    for target in targets {
        let Some(fixture) = snapshot.fixtures.iter().find(|fixture| {
            fixture.fixture_id == *target
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == *target)
        }) else {
            unsupported += 1;
            continue;
        };
        let head_index = fixture
            .logical_heads
            .iter()
            .find(|head| head.fixture_id == *target)
            .map(|head| head.head_index);
        let heads = fixture
            .definition
            .heads
            .iter()
            .filter(|head| fixture.fixture_id == *target || Some(head.index) == head_index)
            .collect::<Vec<_>>();
        if !heads
            .iter()
            .flat_map(|head| &head.parameters)
            .any(|parameter| parameter.attribute == *attribute)
        {
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
                heads.iter().any(|head| {
                    let profile_head_id = fixture
                        .logical_heads
                        .iter()
                        .find(|patched| patched.head_index == head.index)
                        .and_then(|patched| patched.profile_head_id)
                        .or_else(|| mode.heads.get(usize::from(head.index)).map(|head| head.id));
                    let matching = mode
                        .channels
                        .iter()
                        .filter(|channel| {
                            Some(channel.head_id) == profile_head_id
                                && channel.attribute == *attribute
                        })
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
                })
            });
        // A root addresses all its heads; a scalar sibling cannot make a discrete head scalar.
        if profile_discrete
            || heads.iter().any(|head| {
                let matching = head
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.attribute == *attribute)
                    .collect::<Vec<_>>();
                !matching.is_empty()
                    && matching
                        .iter()
                        .all(|parameter| !parameter.capabilities.is_empty())
            })
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
        // A multi-head fixture's heads carry their own identities: a head's value is checked
        // against what that head carries, and the fixture's own identity against every head.
        let supported = snapshot.fixtures.iter().any(|fixture| {
            let head_index = fixture
                .logical_heads
                .iter()
                .find(|head| head.fixture_id == target.fixture_id)
                .map(|head| head.head_index);
            if fixture.fixture_id != target.fixture_id && head_index.is_none() {
                return false;
            }
            // The whole-colour target is no fixture channel: the engine resolves it through
            // each head's colour engine, and a head without one reports itself unsupported.
            target.attribute == AttributeKey::color()
                || fixture
                    .definition
                    .heads
                    .iter()
                    .filter(|head| {
                        fixture.fixture_id == target.fixture_id || Some(head.index) == head_index
                    })
                    .flat_map(|head| &head.parameters)
                    .any(|parameter| {
                        parameter.attribute == target.attribute
                            // The semantic Position owner is carried by the Pan/Tilt channels.
                            || (target.attribute
                                == light_core::programming::ProgrammingOwner::Position.key()
                                && matches!(&*parameter.attribute.0, "pan" | "tilt"))
                    })
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
}

pub(super) fn identity(context: &ActionContext) -> Result<DynamicsIdentity, ActionError> {
    // A Dynamic runs as the desk's operator rather than anonymously, so a live session is still
    // required.
    Ok(DynamicsIdentity {
        session: context
            .session_id
            .map(SessionId)
            .ok_or_else(|| ActionError::new(ActionErrorKind::Unauthorized, "session required"))?,
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

/// A targetless Dynamic, no explicit targets and nothing selected: it has nothing to run on.
pub(super) fn nothing_to_target(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    snapshot: &EngineSnapshot,
    dynamic_id: Uuid,
    explicit: &[FixtureId],
) -> Result<bool, ActionError> {
    let definition = definition(snapshot, dynamic_id)?;
    Ok(
        matches!(definition.target_binding, DynamicTargetBinding::Targetless)
            && explicit.is_empty()
            && programmers
                .selection(session)
                .is_none_or(|selection| selection.selected.is_empty()),
    )
}

/// The fixtures such a Dynamic selects instead of starting: every fixture with a channel one of
/// its lanes drives. Refused when no fixture in the show can run it.
pub(super) fn capable_fixtures(
    snapshot: &EngineSnapshot,
    dynamic_id: Uuid,
) -> Result<Vec<FixtureId>, ActionError> {
    let definition = definition(snapshot, dynamic_id)?;
    let capable = snapshot
        .fixtures
        .iter()
        .filter(|fixture| {
            fixture
                .definition
                .heads
                .iter()
                .flat_map(|head| &head.parameters)
                .any(|parameter| {
                    definition
                        .lanes
                        .iter()
                        .any(|lane| lane.drives_attribute(&parameter.attribute))
                })
        })
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    if capable.is_empty() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "no fixture in the show can run this Dynamic",
        ));
    }
    Ok(capable)
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
    // A targetless Dynamic never falls back to every fixture: with nothing selected the start
    // paths select the capable fixtures first (`capable_selection`).
    let targets = if !explicit.is_empty() {
        explicit.to_vec()
    } else {
        programmers
            .selection(session)
            .map(|selection| selection.selected)
            .unwrap_or_default()
    };
    if targets.is_empty() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Dynamic target scope is empty",
        ));
    }
    Ok((targets, None))
}

/// One logical authored Dynamic in the current Programmer. Runtime identity is scoped to the
/// Programmer; the stored link remains unchanged when an edit moves through Preload.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerDynamicController {
    pub authored_link: Uuid,
    pub controller_id: Uuid,
    pub dynamic_id: Option<Uuid>,
    pub targets: Vec<FixtureId>,
}

pub(super) fn effective_programmer_dynamic_values(
    state: &light_programmer::ProgrammerState,
) -> Vec<&light_dynamics::DynamicAddressValue> {
    light_dynamics::merge_dynamic_address_values(
        state
            .dynamic_values
            .iter()
            .chain(state.preload_dynamic_active.iter())
            .chain(
                (state.blind && state.preload_capture_programmer)
                    .then_some(state.preload_dynamic_pending.iter())
                    .into_iter()
                    .flatten(),
            ),
    )
}

/// Resolve all editable Dynamic controllers from the same effective authored layers used by
/// reconciliation. A committed Preload remains editable after GO returns input to Live.
pub fn effective_programmer_dynamic_controllers(
    state: &light_programmer::ProgrammerState,
) -> Vec<ProgrammerDynamicController> {
    let mut result = Vec::<ProgrammerDynamicController>::new();
    let mut indices = HashMap::<Uuid, (usize, &light_dynamics::DynamicAddressValue)>::new();
    for stored in effective_programmer_dynamic_values(state) {
        let DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            ..
        } = &stored.value
        else {
            continue;
        };
        let (index, latest) = indices.entry(*instance_link).or_insert_with(|| {
            let index = result.len();
            result.push(ProgrammerDynamicController {
                authored_link: *instance_link,
                controller_id: light_dynamics::programmer_dynamic_controller_id(
                    state.id,
                    *instance_link,
                ),
                dynamic_id: dynamic.dynamic_id,
                targets: Vec::new(),
            });
            (index, stored)
        });
        if light_dynamics::dynamic_address_edit_is_later(stored, latest) {
            result[*index].dynamic_id = dynamic.dynamic_id;
            *latest = stored;
        }
        if !result[*index].targets.contains(&stored.fixture_id) {
            result[*index].targets.push(stored.fixture_id);
        }
    }
    result.sort_by_key(|controller| controller.controller_id);
    result
}

/// Accept an authored link or its runtime UUID only when this Programmer actually owns its
/// effective authored rows. Never resolve a controller from another desk's runtime alone.
pub fn resolve_programmer_dynamic_controller(
    state: &light_programmer::ProgrammerState,
    authored_or_runtime_id: Uuid,
) -> Option<ProgrammerDynamicController> {
    let mut matches = effective_programmer_dynamic_controllers(state)
        .into_iter()
        .filter(|value| {
            value.authored_link == authored_or_runtime_id
                || value.controller_id == authored_or_runtime_id
        });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
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
    effective_programmer_dynamic_controllers(&state)
        .into_iter()
        .find_map(|controller| {
            (controller.dynamic_id == Some(dynamic_id)
                && controller
                    .targets
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    == target_set)
                .then_some(controller.controller_id)
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

pub(super) fn store_off(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    controller_id: Uuid,
    timing: DynamicValueTiming,
) -> Result<(), ActionError> {
    let state = programmers
        .get(session)
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable"))?;
    let controller =
        resolve_programmer_dynamic_controller(&state, controller_id).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                "Dynamic controller is not present in this Programmer",
            )
        })?;
    let mutations = effective_programmer_dynamic_values(&state)
        .into_iter()
        .filter(|stored| {
            matches!(
                stored.value,
                DynamicSemanticValue::DynamicOn { instance_link, .. }
                    if instance_link == controller.authored_link
            )
        })
        .map(|stored| DynamicProgrammerValueMutation::Set {
            fixture_id: stored.fixture_id,
            attribute: stored.attribute.clone(),
            value: DynamicSemanticValue::DynamicOff {
                instance_link: controller.authored_link,
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
