use super::{ApiError, AppState, ServerShowPatchPorts, Session};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, PatchFixtureUpdateAction,
    PatchFixtureUpdateIntent, PatchFixturesCommand,
};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{FixtureFreezeState, FreezeFamily, FrozenFixtureTarget, FrozenPositionOutput};
use light_wire::v2::live_action::{
    FixtureFreezeActionOutcome, FixtureFreezeFamily, FixtureFreezeLiveActionRequest,
    FixtureFreezeOperation,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

const FREEZE_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Default)]
pub(super) struct FixtureFreezeHistory {
    entries: Arc<Mutex<HashMap<uuid::Uuid, Vec<FreezeHistoryEntry>>>>,
}

#[derive(Clone, Debug, PartialEq)]
struct FreezeHistoryEntry {
    show_id: light_core::ShowId,
    programmer_undo_depth: usize,
    previous: HashMap<FixtureId, FixtureFreezeState>,
}

impl FixtureFreezeHistory {
    fn record(&self, session: &Session, entry: FreezeHistoryEntry) {
        let mut all = self.entries.lock();
        let entries = all.entry(session.desk.id).or_default();
        entries.push(entry);
        if entries.len() > FREEZE_HISTORY_LIMIT {
            entries.remove(0);
        }
    }

    fn next(&self, session: &Session, programmer_undo_depth: usize) -> Option<FreezeHistoryEntry> {
        self.entries
            .lock()
            .get(&session.desk.id)
            .and_then(|entries| entries.last())
            .filter(|entry| programmer_undo_depth <= entry.programmer_undo_depth)
            .cloned()
    }

    fn finish(&self, session: &Session, entry: &FreezeHistoryEntry) {
        let key = session.desk.id;
        let mut all = self.entries.lock();
        let remove = all.get_mut(&key).is_some_and(|entries| {
            if entries.last().is_some_and(|latest| latest == entry) {
                entries.pop();
            }
            entries.is_empty()
        });
        if remove {
            all.remove(&key);
        }
    }
}

pub(super) fn toggle_selected(
    state: &AppState,
    session: &Session,
    request: &FixtureFreezeLiveActionRequest,
    context: &ActionContext,
) -> Result<FixtureFreezeActionOutcome, ApiError> {
    apply_selected(state, session, request, context, false)
}

pub(super) fn apply_selected_with_activation(
    state: &AppState,
    session: &Session,
    request: &FixtureFreezeLiveActionRequest,
    context: &ActionContext,
) -> Result<FixtureFreezeActionOutcome, ApiError> {
    apply_selected(state, session, request, context, true)
}

fn apply_selected(
    state: &AppState,
    session: &Session,
    request: &FixtureFreezeLiveActionRequest,
    context: &ActionContext,
    activation_held: bool,
) -> Result<FixtureFreezeActionOutcome, ApiError> {
    let fixture_ids = state
        .programming
        .selection(session.id)
        .map(|selection| selection.selected)
        .unwrap_or_default();
    if fixture_ids.is_empty() && state.active_show.current().is_none() {
        return Ok(quiet_outcome(0));
    }
    let show_id = state
        .active_show
        .current()
        .ok_or_else(|| ApiError::bad_request("Freeze requires an active show"))?
        .id;
    let families = request
        .families
        .iter()
        .copied()
        .map(domain_family)
        .collect::<Vec<_>>();

    // Live-control actions are last-write-wins. Re-read and reapply after a narrow revision race
    // instead of surfacing an object-editor conflict to the operator.
    for attempt in 0..3 {
        let ports = if activation_held {
            ServerShowPatchPorts::with_activation_held(state.clone())
        } else {
            ServerShowPatchPorts::new(state.clone())
        };
        let snapshot = state
            .active_show
            .patch_snapshot(context, show_id, &ports)
            .map_err(api_error)?;
        if fixture_ids.is_empty() {
            return Ok(quiet_outcome(snapshot.patch_revision.value()));
        }
        let Some(captured) =
            prepare_freeze_capture(state, &snapshot, &fixture_ids, &families, request.operation)?
        else {
            return Ok(quiet_outcome(snapshot.patch_revision.value()));
        };
        let (command, affected_fixtures, previous) = freeze_command(
            show_id,
            &snapshot,
            &captured,
            &fixture_ids,
            &families,
            request.operation,
        )?;
        let request_id = context
            .request_id
            .as_deref()
            .map(|request_id| format!("{request_id}:freeze:{attempt}"))
            .unwrap_or_else(|| format!("freeze:{}:{attempt}", uuid::Uuid::new_v4()));
        let action = ActionEnvelope {
            context: context
                .clone()
                .with_request_id(request_id)
                .with_expected_revision(snapshot.patch_revision.value()),
            command,
        };
        match state.active_show.patch_fixtures(action, &ports) {
            Ok(result) => {
                if result.changed {
                    let programmer_undo_depth = state
                        .programming
                        .undo_depth(session.id)
                        .ok_or_else(|| ApiError::bad_request("Freeze requires a Programmer"))?;
                    state.programming.clear_redo(session.id);
                    state.fixture_freeze_history.record(
                        session,
                        FreezeHistoryEntry {
                            show_id,
                            programmer_undo_depth,
                            previous,
                        },
                    );
                }
                return Ok(FixtureFreezeActionOutcome {
                    changed: result.changed,
                    patch_revision: result.change.patch_revision.value(),
                    affected_fixtures,
                });
            }
            Err(error) if error.kind == ActionErrorKind::Conflict && attempt < 2 => continue,
            Err(error) => return Err(api_error(error)),
        }
    }
    unreachable!("the bounded Freeze retry loop always returns")
}

struct FreezeCapturedOutput {
    /// Finalized output parameters (masters applied, Freeze held) of the captured frame.
    values: light_engine::FrameValues,
    native: HashMap<FixtureId, FrozenPositionOutput>,
    /// Alias clearing is command-time only; no channel search or hashing occurs at frame rate.
    position_aliases: HashMap<FixtureId, HashSet<AttributeKey>>,
    /// Multi-head roots with Master channels of their own, frozen with their heads.
    root_owners: HashSet<FixtureId>,
}

fn quiet_outcome(patch_revision: u64) -> FixtureFreezeActionOutcome {
    FixtureFreezeActionOutcome {
        changed: false,
        patch_revision,
        affected_fixtures: 0,
    }
}

/// Whether a multi-head root owns channels of its own: a profile head (such as the Master head)
/// that none of its logical heads stands for.
fn root_owns_channels(root: &light_fixture::PatchedFixture) -> bool {
    root.definition.heads.iter().any(|head| {
        !head.parameters.is_empty()
            && !root
                .logical_heads
                .iter()
                .any(|logical| logical.head_index == head.index)
    })
}

/// Multi-head roots with Master channels of their own in `snapshot`.
fn root_owners(snapshot: &light_engine::EngineSnapshot) -> HashSet<FixtureId> {
    snapshot
        .fixtures
        .iter()
        .filter(|fixture| root_owns_channels(fixture))
        .map(|fixture| fixture.fixture_id)
        .collect()
}

/// The owners of `root` a Freeze of `selected` acts on: the selected root or heads, a root's own
/// Master channels with its heads.
fn selected_owners(
    root: &light_application::PatchFixtureProjection,
    selected: &[FixtureId],
    root_owners: &HashSet<FixtureId>,
) -> HashSet<FixtureId> {
    let mut owners = HashSet::new();
    for &owner in selected {
        if owner == root.patch.fixture_id && !root.patch.logical_heads.is_empty() {
            // A root with its own Master channels (Pan, Tilt, Intensity) is frozen too.
            if root_owners.contains(&owner) {
                owners.insert(owner);
            }
            owners.extend(root.patch.logical_heads.iter().map(|head| head.fixture_id));
        } else if owner == root.patch.fixture_id
            || root
                .patch
                .logical_heads
                .iter()
                .any(|head| head.fixture_id == owner)
        {
            owners.insert(owner);
        }
    }
    // Every logical head selected (a Group or range expands the root to them) is the whole
    // fixture: a root with Master channels of its own is frozen with them.
    if root_owners.contains(&root.patch.fixture_id)
        && !root.patch.logical_heads.is_empty()
        && root
            .patch
            .logical_heads
            .iter()
            .all(|head| owners.contains(&head.fixture_id))
    {
        owners.insert(root.patch.fixture_id);
    }
    owners
}

fn prepare_freeze_capture(
    state: &AppState,
    patch: &light_application::PatchSnapshot,
    selected: &[FixtureId],
    families: &[FreezeFamily],
    operation: FixtureFreezeOperation,
) -> Result<Option<FreezeCapturedOutput>, ApiError> {
    let engine_snapshot = state.output.snapshot();
    let root_owners = root_owners(&engine_snapshot);
    let accepted = state.output.latest_visualization_frame().filter(|frame| {
        Arc::ptr_eq(&engine_snapshot, &frame.source_snapshot)
            && frame.scope.show_id == Some(patch.show_id.0)
    });
    let mut native = HashMap::new();
    let mut position_aliases = HashMap::new();
    let mut need_values = false;
    for root in &patch.fixtures {
        let owners = selected_owners(root, selected, &root_owners);
        for owner in owners {
            let previous = root.patch.freeze.targets.get(&owner);
            let full = previous.is_some_and(|target| target.full);
            let position = full
                || previous.is_some_and(|target| target.families.contains(&FreezeFamily::Position));
            let adds = if families.is_empty() {
                !matches!(operation, FixtureFreezeOperation::Unfreeze) && !full
            } else if full {
                matches!(operation, FixtureFreezeOperation::Toggle)
            } else {
                !matches!(operation, FixtureFreezeOperation::Unfreeze)
                    && !previous.is_some_and(|target| {
                        families
                            .iter()
                            .all(|family| target.families.contains(family))
                    })
            };
            need_values |= adds;
            let wants_position = families.is_empty() || families.contains(&FreezeFamily::Position);
            let captures_position = adds && wants_position && (!position || full);
            if wants_position
                && let Some(previous) = previous.and_then(|target| target.position_native.as_ref())
            {
                native.insert(owner, previous.clone());
            }
            if captures_position
                && state
                    .output
                    .engine()
                    .position_has_native_controls(&engine_snapshot, owner)
            {
                let Some(frame) = accepted.as_ref() else {
                    return Ok(None);
                };
                let Some(output) = state.output.engine().position_freeze_from_physical(
                    frame.generation,
                    &frame.physical,
                    owner,
                ) else {
                    return Ok(None);
                };
                native.insert(owner, output);
            }
            if let Some(output) = native.get(&owner)
                && let Some(fixture) = engine_snapshot
                    .fixtures
                    .iter()
                    .find(|fixture| fixture.fixture_id == root.patch.fixture_id)
                && let Some(profile) = fixture.definition.profile_snapshot.as_deref()
                && let Some(mode) = fixture.definition.mode_id.and_then(|id| profile.mode(id))
            {
                let controls: HashSet<_> = output
                    .instances
                    .iter()
                    .flat_map(|instance| instance.controls.iter().map(|control| control.channel_id))
                    .collect();
                let mut aliases = HashSet::new();
                for channel in mode
                    .channels
                    .iter()
                    .filter(|channel| controls.contains(&channel.id))
                {
                    aliases.insert(channel.attribute.clone());
                    aliases.insert(channel.fixture_attribute.clone());
                    aliases.insert(light_fixture::FixtureMode::control_action_attribute(
                        channel.id,
                    ));
                    aliases.extend(
                        channel
                            .functions
                            .iter()
                            .map(|function| function.attribute.clone()),
                    );
                }
                position_aliases.insert(owner, aliases);
            }
        }
    }
    // Existing non-physical/legacy capture remains available before a scheduler publication.
    // A new compiled Position hold above never reaches this fallback without an accepted frame.
    let values = if let Some(frame) = accepted {
        frame.values.clone()
    } else if need_values {
        let rendered = state
            .output
            .engine()
            .render(state.output.render_options())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        rendered.resolved_values.clone()
    } else {
        // Removal/idempotent commands need no physical sample. Reuse only an immutable empty
        // observation for the scalar carrier; no clocks, fitting or new output are requested.
        return Ok(Some(FreezeCapturedOutput {
            values: light_engine::FrameValues::empty(),
            native,
            position_aliases,
            root_owners,
        }));
    };
    Ok(Some(FreezeCapturedOutput {
        values,
        native,
        position_aliases,
        root_owners,
    }))
}

fn clear_captured_position_aliases(
    captured: &FreezeCapturedOutput,
    owner: FixtureId,
    target: &mut FrozenFixtureTarget,
) {
    if target.position_native.is_none() {
        return;
    }
    let semantic = light_core::programming::ProgrammingOwner::Position.key();
    target.values.retain(|attribute, _| {
        *attribute != semantic
            && captured
                .position_aliases
                .get(&owner)
                .is_none_or(|aliases| !aliases.contains(attribute))
    });
}

fn domain_family(family: FixtureFreezeFamily) -> FreezeFamily {
    match family {
        FixtureFreezeFamily::Intensity => FreezeFamily::Intensity,
        FixtureFreezeFamily::Color => FreezeFamily::Color,
        FixtureFreezeFamily::Position => FreezeFamily::Position,
        FixtureFreezeFamily::Beam => FreezeFamily::Beam,
    }
}

fn freeze_command(
    show_id: light_core::ShowId,
    snapshot: &light_application::PatchSnapshot,
    captured: &FreezeCapturedOutput,
    fixture_ids: &[FixtureId],
    families: &[FreezeFamily],
    operation: FixtureFreezeOperation,
) -> Result<
    (
        PatchFixturesCommand,
        usize,
        HashMap<FixtureId, FixtureFreezeState>,
    ),
    ApiError,
> {
    let mut families = families.to_vec();
    families.sort_by_key(|family| match family {
        FreezeFamily::Intensity => 0,
        FreezeFamily::Color => 1,
        FreezeFamily::Position => 2,
        FreezeFamily::Beam => 3,
    });
    families.dedup();
    let mut updates = Vec::new();
    let mut affected = HashSet::new();
    let mut previous = HashMap::new();
    for root in &snapshot.fixtures {
        let valid = std::iter::once(root.patch.fixture_id)
            .chain(root.patch.logical_heads.iter().map(|head| head.fixture_id))
            .collect::<HashSet<_>>();
        let selected = fixture_ids
            .iter()
            .copied()
            .filter(|fixture_id| valid.contains(fixture_id))
            .flat_map(|fixture_id| {
                if fixture_id == root.patch.fixture_id && !root.patch.logical_heads.is_empty() {
                    // A root with its own Master channels (Pan, Tilt, Intensity) is frozen too.
                    captured
                        .root_owners
                        .contains(&fixture_id)
                        .then_some(fixture_id)
                        .into_iter()
                        .chain(root.patch.logical_heads.iter().map(|head| head.fixture_id))
                        .collect::<Vec<_>>()
                } else {
                    vec![fixture_id]
                }
            })
            .scan(HashSet::new(), |seen, owner| {
                Some(seen.insert(owner).then_some(owner))
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut selected = selected;
        if captured.root_owners.contains(&root.patch.fixture_id)
            && !selected.contains(&root.patch.fixture_id)
            && !root.patch.logical_heads.is_empty()
            && root
                .patch
                .logical_heads
                .iter()
                .all(|head| selected.contains(&head.fixture_id))
        {
            selected.insert(0, root.patch.fixture_id);
        }
        if selected.is_empty() {
            continue;
        }
        let mut freeze = root.patch.freeze.clone();
        previous.insert(root.patch.fixture_id, freeze.clone());
        // A root frozen with its heads is one fixture, already counted through its heads.
        let heads_selected = root
            .patch
            .logical_heads
            .iter()
            .any(|head| selected.contains(&head.fixture_id));
        for fixture_id in selected {
            if fixture_id != root.patch.fixture_id || !heads_selected {
                affected.insert(fixture_id);
            }
            apply_target(&mut freeze, fixture_id, &families, captured, operation);
        }
        updates.push(PatchFixtureUpdateIntent {
            fixture_id: root.patch.fixture_id,
            expected_fixture_revision: root.fixture_revision,
            expected_show_revision: snapshot.show_revision,
            multipatch_instance_id: None,
            action: PatchFixtureUpdateAction::SetFreeze { freeze },
        });
    }
    if updates.is_empty()
        || fixture_ids.iter().any(|fixture_id| {
            !snapshot.fixtures.iter().any(|root| {
                root.patch.fixture_id == *fixture_id
                    || root
                        .patch
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == *fixture_id)
            })
        })
    {
        return Err(ApiError::bad_request(
            "Freeze target is not in the active patch",
        ));
    }
    Ok((
        PatchFixturesCommand {
            show_id,
            fixtures: Vec::new(),
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: updates,
        },
        affected.len(),
        previous,
    ))
}

fn apply_target(
    freeze: &mut FixtureFreezeState,
    fixture_id: FixtureId,
    families: &[FreezeFamily],
    captured: &FreezeCapturedOutput,
    operation: FixtureFreezeOperation,
) {
    if families.is_empty() {
        let frozen = freeze
            .targets
            .get(&fixture_id)
            .is_some_and(|target| target.full);
        let remove = matches!(operation, FixtureFreezeOperation::Unfreeze)
            || matches!(operation, FixtureFreezeOperation::Toggle) && frozen;
        if remove {
            freeze.targets.remove(&fixture_id);
            return;
        }
        if frozen {
            return;
        }
        freeze.targets.insert(
            fixture_id,
            FrozenFixtureTarget {
                position_native: captured.native.get(&fixture_id).cloned().or_else(|| {
                    freeze
                        .targets
                        .get(&fixture_id)
                        .and_then(|target| target.position_native.clone())
                }),
                full: true,
                families: Vec::new(),
                values: captured_values(captured, fixture_id, None),
            },
        );
        if let Some(target) = freeze.targets.get_mut(&fixture_id) {
            clear_captured_position_aliases(captured, fixture_id, target);
        }
        return;
    }

    let target = freeze.targets.entry(fixture_id).or_default();
    // A full Freeze already owns every attribute. The current persisted model cannot express
    // "full except this family", so explicit family Freeze/Unfreeze commands leave it intact.
    // Operators can first Unfreeze the fixture and then apply the desired partial Freeze.
    if target.full && !matches!(operation, FixtureFreezeOperation::Toggle) {
        return;
    }
    if target.full {
        // Toggle from full to selected families transfers only their holds. The old complete
        // scalar map must not keep unrelated attributes frozen after full ownership is removed.
        target.values.clear();
        target.position_native = None;
        target.families.clear();
    }
    target.full = false;
    let removing = matches!(operation, FixtureFreezeOperation::Unfreeze)
        || matches!(operation, FixtureFreezeOperation::Toggle)
            && families
                .iter()
                .all(|family| target.families.contains(family));
    if removing {
        target.families.retain(|family| !families.contains(family));
        target
            .values
            .retain(|attribute, _| !families.iter().any(|family| family.accepts(attribute)));
        if families.contains(&FreezeFamily::Position) {
            target.position_native = None;
        }
    } else {
        for family in families {
            if !target.families.contains(family) {
                target.families.push(*family);
            }
        }
        target
            .values
            .extend(captured_values(captured, fixture_id, Some(families)));
        if families.contains(&FreezeFamily::Position) {
            if let Some(native) = captured.native.get(&fixture_id) {
                target.position_native = Some(native.clone());
            }
            clear_captured_position_aliases(captured, fixture_id, target);
        }
    }
    if target.families.is_empty() {
        freeze.targets.remove(&fixture_id);
    }
}

pub(super) fn undo_latest(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
) -> Result<Option<bool>, ApiError> {
    let active_show_id = state.active_show.current().map(|show| show.id);
    let depth = state
        .programming
        .undo_depth(session.id)
        .ok_or_else(|| ApiError::bad_request("Undo requires a Programmer"))?;
    let Some(entry) = state.fixture_freeze_history.next(session, depth) else {
        return Ok(None);
    };
    if active_show_id != Some(entry.show_id) {
        return Ok(None);
    }
    for attempt in 0..3 {
        let ports = ServerShowPatchPorts::with_activation_held(state.clone());
        let snapshot = state
            .active_show
            .patch_snapshot(context, entry.show_id, &ports)
            .map_err(api_error)?;
        let mut updates = Vec::with_capacity(entry.previous.len());
        for (fixture_id, freeze) in &entry.previous {
            let fixture = snapshot
                .fixtures
                .iter()
                .find(|fixture| fixture.patch.fixture_id == *fixture_id)
                .ok_or_else(|| {
                    ApiError::conflict("A fixture changed since Freeze; Undo could not restore it")
                })?;
            updates.push(PatchFixtureUpdateIntent {
                fixture_id: *fixture_id,
                expected_fixture_revision: fixture.fixture_revision,
                expected_show_revision: snapshot.show_revision,
                multipatch_instance_id: None,
                action: PatchFixtureUpdateAction::SetFreeze {
                    freeze: freeze.clone(),
                },
            });
        }
        let request_id = context
            .request_id
            .as_deref()
            .map(|request_id| format!("{request_id}:freeze-undo:{attempt}"))
            .unwrap_or_else(|| format!("freeze-undo:{}:{attempt}", uuid::Uuid::new_v4()));
        let action = ActionEnvelope {
            context: context
                .clone()
                .with_request_id(request_id)
                .with_expected_revision(snapshot.patch_revision.value()),
            command: PatchFixturesCommand {
                show_id: entry.show_id,
                fixtures: Vec::new(),
                remove_fixture_ids: Vec::new(),
                placements: Vec::new(),
                vector_spreads: Vec::new(),
                fixture_updates: updates,
            },
        };
        match state.active_show.patch_fixtures(action, &ports) {
            Ok(result) => {
                state.fixture_freeze_history.finish(session, &entry);
                state.programming.clear_redo(session.id);
                return Ok(Some(result.changed));
            }
            Err(error) if error.kind == ActionErrorKind::Conflict && attempt < 2 => continue,
            Err(error) => return Err(api_error(error)),
        }
    }
    unreachable!("the bounded Freeze Undo retry loop always returns")
}

fn captured_values(
    captured: &FreezeCapturedOutput,
    fixture_id: FixtureId,
    families: Option<&[FreezeFamily]>,
) -> HashMap<AttributeKey, AttributeValue> {
    // Freeze holds the lamp's parameters before they become DMX (2026-10-05). Only resolved
    // parameters are captured, never visualization values (a virtual-dimmer lamp's visual
    // "intensity" is its colour's luminance, and visual colour channels are an approximation).
    // The captured values are finalized parameters, so a level is held as it was after the
    // masters, which never change it afterwards.
    captured
        .values
        .iter()
        .filter(|((owner, attribute), _)| {
            *owner == fixture_id
                && families
                    .is_none_or(|families| families.iter().any(|family| family.accepts(attribute)))
        })
        .map(|((_, attribute), value)| (attribute.clone(), value.clone()))
        .collect()
}

fn api_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
        ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}

pub(super) fn advance_command_mode(state: &AppState, session: &Session) -> bool {
    let current = state
        .programming
        .get(session.id)
        .map(|programmer| programmer.command_line)
        .unwrap_or_default();
    let current = current.trim();
    let next = if current
        .split_whitespace()
        .next()
        .is_some_and(|token| token.eq_ignore_ascii_case("FREEZE"))
    {
        format!("UNFREEZE{}", &current["FREEZE".len()..])
    } else {
        "FREEZE".to_owned()
    };
    state.programming.set_command_line(session.id, next)
}

pub(super) fn append_command_family(state: &AppState, session: &Session, digit: u8) -> bool {
    let family = match digit {
        1 => "INTENSITY",
        2 => "COLOR",
        3 => "POSITION",
        4 => "BEAM",
        _ => return false,
    };
    let current = state
        .programming
        .get(session.id)
        .map(|programmer| programmer.command_line)
        .unwrap_or_default();
    let current = current.trim();
    if !current.split_whitespace().next().is_some_and(|token| {
        token.eq_ignore_ascii_case("FREEZE") || token.eq_ignore_ascii_case("UNFREEZE")
    }) {
        return false;
    }
    if current
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case(family))
    {
        return true;
    }
    state
        .programming
        .set_command_line(session.id, format!("{current} {family}"))
}

#[cfg(test)]
mod native_capture_tests;
