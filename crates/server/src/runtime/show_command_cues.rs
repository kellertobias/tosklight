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
    Ok(vec![put_cue_list(source_object, source_list)?])
}

fn cross_cuelist_mutations(
    operation: &str,
    source_object: &light_show::VersionedObject,
    mut source_list: light_playback::CueList,
    position: usize,
    destination_object: &light_show::VersionedObject,
    destination_body: serde_json::Value,
) -> Result<Vec<light_application::ActiveShowObjectMutation>, String> {
    if operation == "MOVE" {
        if source_list.cues.len() == 1 {
            return Err(
                "cannot move the only Cue out of a Cuelist; delete the Cuelist from its configuration instead"
                    .into(),
            );
        }
        source_list.cues.remove(position);
        Ok(vec![
            put_cue_list(source_object, source_list)?,
            put_cue_list_body(destination_object, destination_body),
        ])
    } else {
        Ok(vec![put_cue_list_body(
            destination_object,
            destination_body,
        )])
    }
}

fn put_cue_list(
    object: &light_show::VersionedObject,
    list: light_playback::CueList,
) -> Result<light_application::ActiveShowObjectMutation, String> {
    Ok(put_active_show_object(
        light_application::ActiveShowObjectKind::CueList,
        object.id.clone(),
        object.revision,
        serde_json::to_value(list).map_err(|error| error.to_string())?,
    ))
}

fn put_cue_list_body(
    object: &light_show::VersionedObject,
    body: serde_json::Value,
) -> light_application::ActiveShowObjectMutation {
    put_active_show_object(
        light_application::ActiveShowObjectKind::CueList,
        object.id.clone(),
        object.revision,
        body,
    )
}

fn transferred_cue_list_body(
    source_body: &serde_json::Value,
    source_index: usize,
    before: &light_playback::Cue,
    after: &light_playback::Cue,
    destination: &light_playback::CueList,
) -> Result<serde_json::Value, String> {
    let mut body = serde_json::to_value(destination).map_err(|error| error.to_string())?;
    let source = source_body["cues"]
        .as_array()
        .and_then(|cues| {
            cues.iter()
                .find(|cue| cue["id"] == before.id.to_string())
                .or_else(|| cues.get(source_index))
        })
        .ok_or("stored source Cue is missing its typed identity")?;
    let transferred = light_application::lossless_json::merge_typed(source, before, after)
        .map_err(|error| error.to_string())?;
    let target = body["cues"]
        .as_array_mut()
        .and_then(|cues| {
            cues.iter_mut()
                .find(|cue| cue["id"] == after.id.to_string())
        })
        .ok_or("transferred Cue is missing from its destination")?;
    *target = transferred;
    Ok(body)
}

struct CueTransfer<'a> {
    store: &'a ShowStore,
    snapshot: &'a EngineSnapshot,
    operation: &'a str,
    mode: CueTransferMode,
    source: CommandPlaybackAddress,
    source_object: light_show::VersionedObject,
    source_list: light_playback::CueList,
    position: usize,
}

fn transfer_cue(
    mut transfer: CueTransfer<'_>,
    destination: CommandPlaybackAddress,
) -> Result<Vec<light_application::ActiveShowObjectMutation>, String> {
    let destination_number = destination
        .cue
        .ok_or("cue destination requires CUE <cue-number>")?;
    let source_cue = transfer.source_list.cues[transfer.position].clone();
    let mut cue = destination_cue(
        &transfer.source_list,
        transfer.position,
        destination_number,
        transfer.mode,
    )?;
    if transfer.operation == "COPY" {
        cue.id = Uuid::new_v4();
    }
    if destination.playback == transfer.source.playback {
        if transfer
            .source_list
            .cues
            .iter()
            .any(|item| item.number == destination_number)
        {
            return Err("destination cue already exists".into());
        }
        if transfer.operation == "MOVE" {
            transfer.source_list.cues.remove(transfer.position);
        }
        transfer.source_list.cues.push(cue.clone());
        transfer
            .source_list
            .cues
            .sort_by(|a, b| a.number.total_cmp(&b.number));
        let body = transferred_cue_list_body(
            &transfer.source_object.body,
            transfer.position,
            &source_cue,
            &cue,
            &transfer.source_list,
        )?;
        return Ok(vec![put_cue_list_body(&transfer.source_object, body)]);
    }
    let (_, destination_object, mut destination_list) =
        cue_list_for_playback(transfer.store, transfer.snapshot, destination.playback)?;
    if destination_list
        .cues
        .iter()
        .any(|item| item.number == destination_number)
    {
        return Err("destination cue already exists".into());
    }
    destination_list.cues.push(cue.clone());
    destination_list
        .cues
        .sort_by(|a, b| a.number.total_cmp(&b.number));
    let destination_body = transferred_cue_list_body(
        &transfer.source_object.body,
        transfer.position,
        &source_cue,
        &cue,
        &destination_list,
    )?;
    cross_cuelist_mutations(
        transfer.operation,
        &transfer.source_object,
        transfer.source_list,
        transfer.position,
        &destination_object,
        destination_body,
    )
}

pub(super) fn execute_cue_mutation(
    state: &AppState,
    operation: &str,
    transfer_mode: Option<CueTransferMode>,
    body: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let snapshot = state.engine.snapshot();
    let at = body.iter().position(|token| token == "AT");
    let source_tokens = at.map_or(body, |index| &body[..index]);
    let (source, used) = parse_playback_address(source_tokens, true, &snapshot)?;
    if used != source_tokens.len() {
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
    let mutations = if operation == "DELETE" {
        delete_cue(&source_object, source_list, position)?
    } else {
        let mode = transfer_mode.ok_or(
            "Cue MOVE/COPY requires an explicit PLAIN or STATUS choice after the operation",
        )?;
        let at = at.ok_or("MOVE and COPY require AT and a destination")?;
        let (destination, used) = parse_playback_address(&body[at + 1..], true, &snapshot)?;
        if used != body.len() - at - 1 {
            return Err("unexpected cue destination tokens".into());
        }
        transfer_cue(
            CueTransfer {
                store: &store,
                snapshot: &snapshot,
                operation,
                mode,
                source,
                source_object,
                source_list,
                position,
            },
            destination,
        )?
    };
    let action = active_show_object_action(context.clone(), entry.id, mutations);
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
