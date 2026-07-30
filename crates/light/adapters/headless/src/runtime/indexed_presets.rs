use super::*;
use light_application::{ActionError, ActionErrorKind};
use light_wire::v2::{preload_values as preload_wire, programming as programming_wire};

struct ResolvedIndexedPreset {
    fixture_ids: Vec<Uuid>,
    attribute: String,
    semantic_id: String,
}

pub(super) fn programming_action(
    state: &AppState,
    session: SessionId,
    action: programming_wire::ProgrammingValuesAction,
) -> Result<programming_wire::ProgrammingValuesAction, ActionError> {
    let programming_wire::ProgrammingValuesAction::ApplyIndexedPreset {
        expected_selection_revision,
        attribute,
        targets,
    } = action
    else {
        return Ok(action);
    };
    let resolved = resolve(
        state,
        session,
        expected_selection_revision,
        attribute,
        &targets,
    )?;
    Ok(programming_wire::ProgrammingValuesAction::ApplyIntent {
        fixture_ids: resolved.fixture_ids,
        group_id: None,
        attribute: resolved.attribute,
        operation: programming_wire::ProgrammingValueOperation::AbsoluteSet {
            value: programming_wire::ProgrammingAttributeValue::Discrete(resolved.semantic_id),
        },
        undo_group: None,
        timing: programming_wire::ProgrammingValueTiming::default(),
    })
}

pub(super) fn preload_action(
    state: &AppState,
    session: SessionId,
    action: preload_wire::ProgrammingPreloadValuesAction,
) -> Result<preload_wire::ProgrammingPreloadValuesAction, ActionError> {
    let preload_wire::ProgrammingPreloadValuesAction::ApplyIndexedPreset {
        expected_selection_revision,
        attribute,
        targets,
    } = action
    else {
        return Ok(action);
    };
    let resolved = resolve(
        state,
        session,
        expected_selection_revision,
        attribute,
        &targets,
    )?;
    Ok(preload_wire::ProgrammingPreloadValuesAction::ApplyIntent {
        fixture_ids: resolved.fixture_ids,
        group_id: None,
        attribute: resolved.attribute,
        operation: preload_wire::ProgrammingPreloadValueOperation::AbsoluteSet {
            value: preload_wire::ProgrammingPreloadAttributeValue::Discrete(resolved.semantic_id),
        },
        undo_group: None,
        timing: preload_wire::ProgrammingPreloadValueTiming::default(),
    })
}

fn resolve(
    state: &AppState,
    session: SessionId,
    expected_selection_revision: u64,
    attribute: String,
    targets: &[programming_wire::ProgrammingIndexedPresetTarget],
) -> Result<ResolvedIndexedPreset, ActionError> {
    if targets.is_empty() {
        return Err(invalid(
            "Indexed Preset requires at least one fixture target",
        ));
    }
    let interaction = state
        .programming
        .programmers()
        .interaction_state(session)
        .ok_or_else(|| invalid("Programmer interaction authority is unavailable"))?;
    if interaction.selection.revision != expected_selection_revision {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!(
                "Indexed Preset expected selection revision {expected_selection_revision}, but the current revision is {}",
                interaction.selection.revision
            ),
        )
        .at_related_revision(interaction.selection.revision));
    }
    let selected = interaction
        .selection
        .selected
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let snapshot = state.output.snapshot();
    let mut fixture_ids = Vec::with_capacity(targets.len());
    let mut seen = HashSet::new();
    let mut compatible: Option<(String, String, &'static str)> = None;
    for target in targets {
        if !seen.insert((target.fixture_id, target.function_id)) {
            return Err(invalid(
                "Indexed Preset contains a duplicate fixture target",
            ));
        }
        let fixture_id = light_core::FixtureId(target.fixture_id);
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| {
                fixture.fixture_id == fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == fixture_id)
            })
            .ok_or_else(|| invalid("Indexed Preset fixture is not in the active patch"))?;
        if !selected.contains(&fixture_id) && !selected.contains(&fixture.fixture_id) {
            return Err(invalid(
                "Indexed Preset target is no longer in the current selection",
            ));
        }
        let profile = fixture
            .definition
            .profile_snapshot
            .as_deref()
            .ok_or_else(|| invalid("Indexed Preset fixture has no embedded profile"))?;
        if profile.revision != target.expected_profile_revision {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Indexed Preset fixture profile changed while the modal was open",
            )
            .at_related_revision(u64::from(profile.revision)));
        }
        let mode = fixture
            .definition
            .mode_id
            .and_then(|mode_id| profile.mode(mode_id))
            .ok_or_else(|| invalid("Indexed Preset fixture mode is unavailable"))?;
        let (channel, function) = mode
            .channels
            .iter()
            .find_map(|channel| {
                channel
                    .functions
                    .iter()
                    .find(|function| function.id == target.function_id)
                    .map(|function| (channel, function))
            })
            .ok_or_else(|| invalid("Indexed Preset function is no longer available"))?;
        let owner = super::profile_head_owner(fixture, mode, channel.head_id).map_err(invalid)?;
        if owner != fixture_id || function.attribute.0 != attribute {
            return Err(invalid(
                "Indexed Preset function does not belong to the requested fixture and attribute",
            ));
        }
        let current = match &function.behavior {
            light_fixture::ChannelFunctionBehavior::Fixed {
                semantic_id, label, ..
            } => (semantic_id.clone(), label.clone(), "fixed"),
            light_fixture::ChannelFunctionBehavior::Indexed {
                semantic_id, label, ..
            } => (semantic_id.clone(), label.clone(), "indexed"),
            _ => {
                return Err(invalid(
                    "Indexed Preset target must be a fixed or indexed function",
                ));
            }
        };
        if let Some(expected) = &compatible {
            if expected != &current {
                return Err(invalid(
                    "Indexed Preset targets do not have compatible semantic meaning",
                ));
            }
        } else {
            compatible = Some(current);
        }
        fixture_ids.push(target.fixture_id);
    }
    let (semantic_id, _, _) =
        compatible.ok_or_else(|| invalid("Indexed Preset has no compatible function"))?;
    Ok(ResolvedIndexedPreset {
        fixture_ids,
        attribute,
        semantic_id,
    })
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
