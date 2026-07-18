use super::*;

fn delete_cue(
    state: &AppState,
    entry: &ShowEntry,
    store: &ShowStore,
    source_object: light_show::VersionedObject,
    mut source_list: light_playback::CueList,
    position: usize,
) -> Result<usize, String> {
    source_list.cues.remove(position);
    if source_list.cues.is_empty() {
        return Err(
            "cannot delete the only Cue; delete the Cuelist from its configuration instead".into(),
        );
    }
    let revision = store
        .put_object(
            "cue_list",
            &source_object.id,
            &serde_json::to_value(source_list).map_err(|error| error.to_string())?,
            source_object.revision,
        )
        .map_err(|error| error.to_string())?;
    refresh_command_show(state, entry)?;
    emit_command_object_changed(state, entry, "cue_list", &source_object.id, revision);
    Ok(1)
}

fn persist_cross_cuelist_transfer(
    store: &ShowStore,
    operation: &str,
    source_object: &light_show::VersionedObject,
    mut source_list: light_playback::CueList,
    position: usize,
    destination_object: &light_show::VersionedObject,
    destination_list: light_playback::CueList,
) -> Result<(), String> {
    if operation == "MOVE" {
        if source_list.cues.len() == 1 {
            return Err(
                "cannot move the only Cue out of a Cuelist; delete the Cuelist from its configuration instead"
                    .into(),
            );
        }
        source_list.cues.remove(position);
        let source_body = serde_json::to_value(source_list).map_err(|error| error.to_string())?;
        let destination_body =
            serde_json::to_value(destination_list).map_err(|error| error.to_string())?;
        store
            .mutate_objects_atomically(
                &[
                    AtomicObjectWrite {
                        kind: "cue_list",
                        id: &source_object.id,
                        body: &source_body,
                        expected: source_object.revision,
                    },
                    AtomicObjectWrite {
                        kind: "cue_list",
                        id: &destination_object.id,
                        body: &destination_body,
                        expected: destination_object.revision,
                    },
                ],
                &[],
            )
            .map_err(|error| error.to_string())?;
    } else {
        store
            .put_object(
                "cue_list",
                &destination_object.id,
                &serde_json::to_value(destination_list).map_err(|error| error.to_string())?,
                destination_object.revision,
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
) -> Result<(), String> {
    let destination_number = destination
        .cue
        .ok_or("cue destination requires CUE <cue-number>")?;
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
        transfer.source_list.cues.push(cue);
        transfer
            .source_list
            .cues
            .sort_by(|a, b| a.number.total_cmp(&b.number));
        transfer
            .store
            .put_object(
                "cue_list",
                &transfer.source_object.id,
                &serde_json::to_value(transfer.source_list).map_err(|error| error.to_string())?,
                transfer.source_object.revision,
            )
            .map_err(|error| error.to_string())?;
        return Ok(());
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
    destination_list.cues.push(cue);
    destination_list
        .cues
        .sort_by(|a, b| a.number.total_cmp(&b.number));
    persist_cross_cuelist_transfer(
        transfer.store,
        transfer.operation,
        &transfer.source_object,
        transfer.source_list,
        transfer.position,
        &destination_object,
        destination_list,
    )
}

pub(super) fn execute_cue_mutation(
    state: &AppState,
    operation: &str,
    transfer_mode: Option<CueTransferMode>,
    body: &[String],
    entry: &ShowEntry,
    store: &ShowStore,
    snapshot: &EngineSnapshot,
) -> Result<usize, String> {
    let at = body.iter().position(|token| token == "AT");
    let source_tokens = at.map_or(body, |index| &body[..index]);
    let (source, used) = parse_playback_address(source_tokens, true, snapshot)?;
    if used != source_tokens.len() {
        return Err("unexpected cue source tokens".into());
    }
    let source_cue = source.cue.ok_or("cue source requires CUE <cue-number>")?;
    let (_, source_object, source_list) = cue_list_for_playback(store, snapshot, source.playback)?;
    let position = source_list
        .cues
        .iter()
        .position(|cue| cue.number == source_cue)
        .ok_or_else(|| format!("cue {source_cue} does not exist"))?;
    if operation == "DELETE" {
        return delete_cue(state, entry, store, source_object, source_list, position);
    }
    let mode = transfer_mode
        .ok_or("Cue MOVE/COPY requires an explicit PLAIN or STATUS choice after the operation")?;
    let at = at.ok_or("MOVE and COPY require AT and a destination")?;
    let (destination, used) = parse_playback_address(&body[at + 1..], true, snapshot)?;
    if used != body.len() - at - 1 {
        return Err("unexpected cue destination tokens".into());
    }
    transfer_cue(
        CueTransfer {
            store,
            snapshot,
            operation,
            mode,
            source,
            source_object,
            source_list,
            position,
        },
        destination,
    )?;
    refresh_command_show(state, entry)?;
    Ok(1)
}
