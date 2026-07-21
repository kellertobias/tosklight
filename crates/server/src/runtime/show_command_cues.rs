use super::*;

fn delete_cue(
    source_object: &light_show::VersionedObject,
    mut source_list: light_playback::CueList,
    position: usize,
) -> Result<Vec<light_application::ActiveShowObjectMutation>, String> {
    source_list.cues.remove(position);
    if source_list.cues.is_empty() {
        return Err(
            "cannot delete the only Cue; delete the Cuelist from its configuration instead".into(),
        );
    }
    let body = serde_json::to_value(source_list).map_err(|error| error.to_string())?;
    Ok(vec![put_active_show_object(
        light_application::ActiveShowObjectKind::CueList,
        source_object.id.clone(),
        source_object.revision,
        body,
    )])
}

pub(super) fn execute_cue_delete(
    state: &AppState,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let snapshot = state.engine.snapshot();
    let (source, used) = parse_playback_address(body, true, &snapshot)?;
    if used != body.len() {
        return Err("unexpected cue source tokens".into());
    }
    let source_cue = source.cue.ok_or("cue source requires CUE <cue-number>")?;
    let (_, source_object, source_list) =
        cue_list_for_playback(&store, &snapshot, source.playback)?;
    let position = source_list
        .cues
        .iter()
        .position(|cue| cue.number == source_cue)
        .ok_or_else(|| format!("cue {source_cue} does not exist"))?;
    let action = active_show_object_action(
        context.clone(),
        entry.id,
        delete_cue(&source_object, source_list, position)?,
    );
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    for change in result.changes {
        emit_command_object_changed(
            state,
            &entry,
            change.kind.as_str(),
            &change.object_id,
            change.object_revision,
        );
    }
    Ok(1)
}
