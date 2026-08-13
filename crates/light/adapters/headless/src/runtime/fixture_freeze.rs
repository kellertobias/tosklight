use super::{ApiError, AppState, ServerShowPatchPorts, Session};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, PatchFixtureUpdateAction,
    PatchFixtureUpdateIntent, PatchFixturesCommand,
};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{FixtureFreezeState, FreezeFamily, FrozenFixtureTarget};
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
    entries: Arc<Mutex<HashMap<(light_core::UserId, uuid::Uuid), Vec<FreezeHistoryEntry>>>>,
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
        let entries = all.entry((session.user.id, session.desk.id)).or_default();
        entries.push(entry);
        if entries.len() > FREEZE_HISTORY_LIMIT {
            entries.remove(0);
        }
    }

    fn next(
        &self,
        session: &Session,
        programmer_undo_depth: usize,
    ) -> Option<FreezeHistoryEntry> {
        self.entries
            .lock()
            .get(&(session.user.id, session.desk.id))
            .and_then(|entries| entries.last())
            .filter(|entry| programmer_undo_depth <= entry.programmer_undo_depth)
            .cloned()
    }

    fn finish(&self, session: &Session, entry: &FreezeHistoryEntry) {
        let key = (session.user.id, session.desk.id);
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
    if fixture_ids.is_empty() {
        return Err(ApiError::bad_request(
            "Freeze requires at least one selected fixture",
        ));
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
        let rendered = state
            .output
            .engine()
            .render(state.output.render_options())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let (command, affected_fixtures, previous) = freeze_command(
            show_id,
            &snapshot,
            &rendered,
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
    rendered: &light_engine::RenderResult,
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
                    root.patch
                        .logical_heads
                        .iter()
                        .map(|head| head.fixture_id)
                        .collect::<Vec<_>>()
                } else {
                    vec![fixture_id]
                }
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }
        let mut freeze = root.patch.freeze.clone();
        previous.insert(root.patch.fixture_id, freeze.clone());
        for fixture_id in selected {
            affected.insert(fixture_id);
            apply_target(
                &mut freeze,
                fixture_id,
                &families,
                rendered,
                operation,
            );
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
    rendered: &light_engine::RenderResult,
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
                full: true,
                families: Vec::new(),
                values: captured_values(rendered, fixture_id, None),
            },
        );
        return;
    }

    let target = freeze.targets.entry(fixture_id).or_default();
    // A full Freeze already owns every attribute. The current persisted model cannot express
    // "full except this family", so explicit family Freeze/Unfreeze commands leave it intact.
    // Operators can first Unfreeze the fixture and then apply the desired partial Freeze.
    if target.full && !matches!(operation, FixtureFreezeOperation::Toggle) {
        return;
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
    } else {
        for family in families {
            if !target.families.contains(family) {
                target.families.push(*family);
            }
        }
        target
            .values
            .extend(captured_values(rendered, fixture_id, Some(families)));
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
    rendered: &light_engine::RenderResult,
    fixture_id: FixtureId,
    families: Option<&[FreezeFamily]>,
) -> HashMap<AttributeKey, AttributeValue> {
    rendered
        .resolved_values
        .iter()
        .chain(rendered.profile_visualization_values.iter())
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

pub(super) fn append_command_family(
    state: &AppState,
    session: &Session,
    digit: u8,
) -> bool {
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
    if !current
        .split_whitespace()
        .next()
        .is_some_and(|token| {
            token.eq_ignore_ascii_case("FREEZE") || token.eq_ignore_ascii_case("UNFREEZE")
        })
    {
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
