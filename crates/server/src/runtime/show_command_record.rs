use super::*;

fn record_operation(body: &mut &[String]) -> RecordOperation {
    match body.first().map(String::as_str) {
        Some("+") => {
            *body = &body[1..];
            RecordOperation::Merge
        }
        Some("-") => {
            *body = &body[1..];
            RecordOperation::Subtract
        }
        _ => RecordOperation::Overwrite,
    }
}

fn group_from_programmer(
    id: &str,
    existing: Option<light_programmer::GroupDefinition>,
    existing_membership: Vec<light_core::FixtureId>,
    programmer: &light_programmer::ProgrammerState,
    operation: RecordOperation,
) -> light_programmer::GroupDefinition {
    let mut group = existing.unwrap_or_else(|| light_programmer::GroupDefinition {
        id: id.to_owned(),
        name: format!("Group {id}"),
        ..Default::default()
    });
    group.derived_from = None;
    group.frozen_from = None;
    match operation {
        RecordOperation::Overwrite => {
            group.fixtures = programmer.selected.clone();
            match programmer.selection_expression.clone() {
                Some(light_programmer::SelectionExpression::LiveGroup { group_id, rule })
                    if group_id != id =>
                {
                    group.derived_from = Some(light_programmer::DerivedGroup {
                        source_group_id: group_id,
                        rule,
                    });
                }
                Some(light_programmer::SelectionExpression::FrozenGroup {
                    group_id,
                    source_revision,
                }) if group_id != id => {
                    group.frozen_from = Some(light_programmer::FrozenGroup {
                        source_group_id: group_id,
                        source_revision,
                        captured_at: chrono::Utc::now(),
                    });
                }
                _ => {}
            }
        }
        RecordOperation::Merge => {
            group.fixtures = existing_membership;
            for fixture in &programmer.selected {
                if !group.fixtures.contains(fixture) {
                    group.fixtures.push(*fixture);
                }
            }
        }
        RecordOperation::Subtract => {
            group.fixtures = existing_membership;
            group
                .fixtures
                .retain(|fixture| !programmer.selected.contains(fixture));
        }
    }
    group
}

fn prevent_derived_group_cycle(
    mut group: light_programmer::GroupDefinition,
    id: &str,
    snapshot: &EngineSnapshot,
    programmer: &light_programmer::ProgrammerState,
) -> light_programmer::GroupDefinition {
    if group.derived_from.is_none() {
        return group;
    }
    let mut groups = snapshot
        .groups
        .iter()
        .cloned()
        .map(|candidate| (candidate.id.clone(), candidate))
        .collect::<HashMap<_, _>>();
    groups.insert(id.to_owned(), group.clone());
    if light_programmer::resolve_group(id, &groups).is_err() {
        group.derived_from = None;
        group.frozen_from = None;
        group.fixtures = programmer.selected.clone();
    }
    group
}

fn delete_empty_group(
    snapshot: &EngineSnapshot,
    id: &str,
    existing: Option<&light_show::VersionedObject>,
) -> Result<light_application::ActiveShowObjectMutation, String> {
    let Some(existing) = existing else {
        return Err(format!("group {id} does not exist"));
    };
    if let Some(dependent) = snapshot.groups.iter().find(|group| {
        group
            .derived_from
            .as_ref()
            .is_some_and(|derived| derived.source_group_id == id)
    }) {
        return Err(format!(
            "cannot delete group {id}; derived group {} depends on it",
            dependent.id
        ));
    }
    Ok(delete_active_show_object(
        light_application::ActiveShowObjectKind::Group,
        existing.id.clone(),
        existing.revision,
    ))
}

fn record_group(
    state: &AppState,
    session: &Session,
    body: &[String],
    operation: RecordOperation,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if body.len() != 2 {
        return Err("expected RECORD [ + | - ] GROUP <group-number>".into());
    }
    let id = &body[1];
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let snapshot = state.engine.snapshot();
    let (entry, store) = active_show_store(state)?;
    let existing = store
        .objects("group")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == *id);
    if operation == RecordOperation::Subtract && programmer.selected.is_empty() {
        let mutation = delete_empty_group(&snapshot, id, existing.as_ref())?;
        let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
        run_active_show_object_action_in_programming_interaction(state, action)
            .map_err(|error| error.message)?;
        return Ok(1);
    }
    let existing_group = existing
        .as_ref()
        .map(|object| {
            serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())
        })
        .transpose()?;
    if operation != RecordOperation::Overwrite && existing_group.is_none() {
        return Err(format!("group {id} does not exist"));
    }
    let membership = if existing_group.is_some() {
        let groups = snapshot
            .groups
            .iter()
            .cloned()
            .map(|group| (group.id.clone(), group))
            .collect();
        light_programmer::resolve_group(id, &groups)?
    } else {
        Vec::new()
    };
    let group = group_from_programmer(id, existing_group, membership, &programmer, operation);
    let group = prevent_derived_group_cycle(group, id, &snapshot, &programmer);
    let action = active_show_object_action(
        context.clone(),
        entry.id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Group,
            id.clone(),
            existing.as_ref().map_or(0, |object| object.revision),
            serde_json::to_value(group).map_err(|error| error.to_string())?,
        )],
    );
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    state.programmers.finish_selection_gesture(session.id);
    Ok(programmer.selected.len())
}

fn record_cue(
    state: &AppState,
    session: &Session,
    body: &[String],
    timing: CommandTiming,
    operation: RecordOperation,
    snapshot: &EngineSnapshot,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if body.first().is_some_and(|token| token == "CUE") {
        let show = state.active_show.read().clone().ok_or("no show is open")?;
        let playback = state
            .desk
            .lock()
            .selected_playback(session.desk.id, show.id)
            .map_err(|error| error.to_string())?
            .ok_or("no playback is selected; use RECORD SET <playback> CUE <cue>")?;
        store_cue_at(
            state,
            session,
            playback,
            Some(parse_command_cue_number(&body[1..])?),
            timing,
            operation,
            context,
        )?;
        return Ok(1);
    }
    let (address, used) = parse_playback_address(body, true, snapshot)?;
    if used != body.len() {
        return Err("unexpected tokens after cue target".into());
    }
    store_cue_at(
        state,
        session,
        address.playback,
        address.cue,
        timing,
        operation,
        context,
    )?;
    Ok(1)
}

fn record_preset(
    state: &AppState,
    session: &Session,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let address = command_preset_address(body)?;
    let id = address.storage_key();
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let preset = programmer_preset(&programmer, format!("Preset {id}"), address);
    if preset.values.is_empty() && preset.group_values.is_empty() {
        return Err("the programmer has no values to record".into());
    }
    let (entry, store) = active_show_store(state)?;
    let existing = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == id
                || decode_preset_object(object).is_ok_and(|(stored, _)| stored == address)
        });
    let storage_key = existing
        .as_ref()
        .map(|object| object.id.clone())
        .unwrap_or(id);
    let action = active_show_object_action(
        context.clone(),
        entry.id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Preset,
            storage_key,
            existing.as_ref().map_or(0, |object| object.revision),
            serde_json::to_value(preset).map_err(|error| error.to_string())?,
        )],
    );
    run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    Ok(1)
}

pub(super) fn execute_record_show_command(
    state: &AppState,
    session: &Session,
    mut body: &[String],
    timing: CommandTiming,
    snapshot: &EngineSnapshot,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let operation = record_operation(&mut body);
    if body.first().is_some_and(|token| token == "GROUP") {
        record_group(state, session, body, operation, context)
    } else if body
        .first()
        .is_some_and(|token| token == "CUE" || token == "SET")
    {
        record_cue(state, session, body, timing, operation, snapshot, context)
    } else if operation != RecordOperation::Overwrite {
        Err("RECORD + and RECORD - currently require GROUP or SET ... CUE targets".into())
    } else {
        record_preset(state, session, body, context)
    }
}
