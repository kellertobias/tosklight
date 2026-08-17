use super::*;
use crate::runtime::command_http::speed_group_binding_command::{
    SpeedGroupBindingCommand, SpeedGroupBindingSource,
};

pub(super) fn execute_speed_group_binding(
    state: &AppState,
    context: &light_application::ActionContext,
    command: SpeedGroupBindingCommand,
) -> Result<usize, String> {
    let target = resolve_binding_target(state, context, command.source)?;
    let group = char::from(b'A' + command.group.index() as u8).to_string();
    match target {
        BindingTarget::CueList(cue_list_id) => bind_cue_list(state, context, cue_list_id, &group),
        BindingTarget::Dynamic(dynamic_id) => {
            bind_dynamic(state, context, dynamic_id, command.group)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingTarget {
    CueList(light_core::CueListId),
    Dynamic(Uuid),
}

fn resolve_binding_target(
    state: &AppState,
    context: &light_application::ActionContext,
    source: SpeedGroupBindingSource,
) -> Result<BindingTarget, String> {
    let snapshot = state.output.snapshot();
    match source {
        SpeedGroupBindingSource::CueList(number) => snapshot
            .playbacks
            .iter()
            .find(|playback| playback.number == number)
            .and_then(playback_binding_target)
            .filter(|target| matches!(target, BindingTarget::CueList(_)))
            .ok_or_else(|| format!("Cuelist {number} does not exist")),
        SpeedGroupBindingSource::Dynamic(number) => snapshot
            .dynamics
            .iter()
            .find(|dynamic| dynamic.pool_number == number)
            .map(|dynamic| BindingTarget::Dynamic(dynamic.id))
            .ok_or_else(|| format!("Dynamic {number} does not exist")),
        SpeedGroupBindingSource::Playback(address) => {
            let playback = super::set_commands::existing_assignment(state, context, address)?
                .ok_or("the addressed Playback is unassigned")?;
            playback_binding_target(&playback).ok_or_else(|| {
                "the addressed Playback must contain a Cuelist or stored Dynamic".into()
            })
        }
    }
}

fn playback_binding_target(playback: &light_playback::PlaybackDefinition) -> Option<BindingTarget> {
    match &playback.target {
        light_playback::PlaybackTarget::CueList { cue_list_id } => {
            Some(BindingTarget::CueList(*cue_list_id))
        }
        light_playback::PlaybackTarget::Dynamic { assignment } => {
            assignment.dynamic.dynamic_id.map(BindingTarget::Dynamic)
        }
        _ => None,
    }
}

fn bind_cue_list(
    state: &AppState,
    context: &light_application::ActionContext,
    cue_list_id: light_core::CueListId,
    group: &str,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let object = store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            serde_json::from_value::<light_playback::CueList>(object.body.clone())
                .is_ok_and(|cue_list| cue_list.id == cue_list_id)
        })
        .ok_or("the addressed Cuelist is not stored in the active show")?;
    let mut body = object.body.clone();
    let map = body
        .as_object_mut()
        .ok_or("the stored Cuelist body is invalid")?;
    if map.get("speed_group").and_then(serde_json::Value::as_str) == Some(group) {
        return Ok(0);
    }
    map.insert(
        "speed_group".into(),
        serde_json::Value::String(group.into()),
    );
    let cue_list: light_playback::CueList =
        serde_json::from_value(body.clone()).map_err(|error| error.to_string())?;
    cue_list.validate()?;
    commit_binding(
        state,
        context,
        &entry,
        light_application::ActiveShowObjectKind::CueList,
        object.id,
        object.revision,
        body,
    )
}

fn bind_dynamic(
    state: &AppState,
    context: &light_application::ActionContext,
    dynamic_id: Uuid,
    group: light_application::SpeedGroupId,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let object = store
        .objects("dynamic")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == dynamic_id.to_string())
        .ok_or("the addressed Dynamic is not stored in the active show")?;
    let mut definition: light_dynamics::DynamicDefinition =
        serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())?;
    let target_group = dynamic_speed_group(group);
    let beats_per_cycle = match definition.speed {
        light_dynamics::DynamicSpeed::SpeedGroup {
            group: current,
            beats_per_cycle,
        } => {
            if current == target_group {
                return Ok(0);
            }
            beats_per_cycle
        }
        light_dynamics::DynamicSpeed::Fixed { .. } => light_dynamics::Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    definition.speed = light_dynamics::DynamicSpeed::SpeedGroup {
        group: target_group,
        beats_per_cycle,
    };
    definition.revision = definition
        .revision
        .checked_add(1)
        .ok_or("Dynamic revision is exhausted")?;
    light_dynamics::validate_definition(&definition).map_err(|error| error.to_string())?;
    let mut body = object.body.clone();
    let map = body
        .as_object_mut()
        .ok_or("the stored Dynamic body is invalid")?;
    map.insert(
        "speed".into(),
        serde_json::to_value(&definition.speed).map_err(|error| error.to_string())?,
    );
    map.insert("revision".into(), definition.revision.into());
    commit_binding(
        state,
        context,
        &entry,
        light_application::ActiveShowObjectKind::Dynamic,
        object.id,
        object.revision,
        body,
    )
}

fn dynamic_speed_group(group: light_application::SpeedGroupId) -> light_dynamics::SpeedGroup {
    match group.one_based() {
        1 => light_dynamics::SpeedGroup::A,
        2 => light_dynamics::SpeedGroup::B,
        3 => light_dynamics::SpeedGroup::C,
        4 => light_dynamics::SpeedGroup::D,
        5 => light_dynamics::SpeedGroup::E,
        _ => unreachable!("SpeedGroupId guarantees 1-5"),
    }
}

fn commit_binding(
    state: &AppState,
    context: &light_application::ActionContext,
    entry: &ShowEntry,
    kind: light_application::ActiveShowObjectKind,
    object_id: String,
    expected_revision: u64,
    body: serde_json::Value,
) -> Result<usize, String> {
    let mutation = put_active_show_object(kind, object_id, expected_revision, body)
        .map_err(|error| error.message)?;
    let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    for change in &result.changes {
        emit_command_object_changed(
            state,
            entry,
            change.kind.as_str(),
            &change.object_id,
            change.object_revision,
        );
    }
    Ok(result.changes.len())
}
