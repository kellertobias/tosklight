use super::{ApiError, AppState, ServerShowPatchPorts, Session};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, PatchFixtureUpdateAction,
    PatchFixtureUpdateIntent, PatchFixturesCommand,
};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::{FixtureFreezeState, FreezeFamily, FrozenFixtureTarget};
use light_wire::v2::live_action::{
    FixtureFreezeActionOutcome, FixtureFreezeFamily, FixtureFreezeLiveActionRequest,
};
use std::collections::{HashMap, HashSet};

pub(super) fn toggle_selected(
    state: &AppState,
    session: &Session,
    request: &FixtureFreezeLiveActionRequest,
    context: &ActionContext,
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
        let ports = ServerShowPatchPorts::new(state.clone());
        let snapshot = state
            .active_show
            .patch_snapshot(context, show_id, &ports)
            .map_err(api_error)?;
        let rendered = state
            .output
            .engine()
            .render(state.output.render_options())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let (command, affected_fixtures) =
            freeze_command(show_id, &snapshot, &rendered, &fixture_ids, &families)?;
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
) -> Result<(PatchFixturesCommand, usize), ApiError> {
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
        for fixture_id in selected {
            affected.insert(fixture_id);
            toggle_target(&mut freeze, fixture_id, &families, rendered);
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
    ))
}

fn toggle_target(
    freeze: &mut FixtureFreezeState,
    fixture_id: FixtureId,
    families: &[FreezeFamily],
    rendered: &light_engine::RenderResult,
) {
    if families.is_empty() {
        if freeze
            .targets
            .get(&fixture_id)
            .is_some_and(|target| target.full)
        {
            freeze.targets.remove(&fixture_id);
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
    target.full = false;
    let removing = families
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
