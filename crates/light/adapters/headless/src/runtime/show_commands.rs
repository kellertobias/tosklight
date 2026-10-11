use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RecordOperation {
    Overwrite,
    Merge,
    Subtract,
}

fn merge_recorded_cue(
    cue: &mut light_playback::Cue,
    incoming: light_playback::Cue,
    operation: RecordOperation,
) {
    match operation {
        RecordOperation::Overwrite => {
            cue.changes = incoming.changes;
            cue.group_changes = incoming.group_changes;
            cue.fade_millis = incoming.fade_millis;
            cue.delay_millis = incoming.delay_millis;
            cue.out_fade_millis = incoming.out_fade_millis;
            cue.out_delay_millis = incoming.out_delay_millis;
            cue.trigger = incoming.trigger;
            cue.cue_only = incoming.cue_only;
        }
        RecordOperation::Merge => {
            for change in incoming.changes {
                cue.changes.retain(|existing| {
                    existing.fixture_id != change.fixture_id
                        || existing.attribute != change.attribute
                });
                cue.changes.push(change);
            }
            for change in incoming.group_changes {
                cue.group_changes.retain(|existing| {
                    existing.group_id != change.group_id || existing.attribute != change.attribute
                });
                cue.group_changes.push(change);
            }
        }
        RecordOperation::Subtract => {
            cue.changes.retain(|existing| {
                !incoming.changes.iter().any(|remove| {
                    existing.fixture_id == remove.fixture_id
                        && existing.attribute == remove.attribute
                })
            });
            cue.group_changes.retain(|existing| {
                !incoming.group_changes.iter().any(|remove| {
                    existing.group_id == remove.group_id && existing.attribute == remove.attribute
                })
            });
        }
    }
}

fn update_recorded_cue_list(
    object: &light_show::VersionedObject,
    mut list: light_playback::CueList,
    programmer: &light_programmer::ProgrammerState,
    requested: Option<light_playback::CueNumber>,
    timing: CommandTiming,
    operation: RecordOperation,
) -> Result<light_application::ActiveShowObjectMutation, String> {
    let number = requested.unwrap_or_else(|| {
        list.cues.last().map_or_else(
            || light_playback::CueNumber::from(1_u8),
            |cue| cue.number.next_whole(),
        )
    });
    if let Some(position) = list.cues.iter().position(|cue| cue.number == number) {
        if operation == RecordOperation::Subtract
            && programmer.values.is_empty()
            && programmer.group_values.is_empty()
        {
            list.cues.remove(position);
            if list.cues.is_empty() {
                return Err(
                    "cannot delete the only Cue; delete the Cuelist from its configuration instead"
                        .into(),
                );
            }
        } else {
            merge_recorded_cue(
                &mut list.cues[position],
                programmer_cue(programmer, number.clone(), timing),
                operation,
            );
        }
    } else if operation == RecordOperation::Subtract {
        return Err(format!("Cue {number} does not exist"));
    } else {
        list.cues.push(programmer_cue(programmer, number, timing));
    }
    list.cues
        .sort_by(|left, right| left.number.cmp(&right.number));
    put_active_show_object(
        light_application::ActiveShowObjectKind::CueList,
        object.id.clone(),
        object.revision,
        serde_json::to_value(list).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.message)
}

pub(super) fn store_cue_at(
    state: &AppState,
    session: &Session,
    playback: light_playback::PlaybackAddress,
    requested: Option<light_playback::CueNumber>,
    timing: CommandTiming,
    operation: RecordOperation,
    context: &light_application::ActionContext,
) -> Result<(), String> {
    let (entry, store) = active_show_store(state)?;
    let snapshot = state.output.snapshot();
    let programmer = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let programmer_is_empty = programmer.values.is_empty() && programmer.group_values.is_empty();
    if programmer_is_empty && operation != RecordOperation::Subtract {
        return Err("the programmer has no values to record".into());
    }
    if operation != RecordOperation::Overwrite && requested.is_none() {
        return Err("RECORD + and RECORD - require an explicit CUE target".into());
    }
    let (_, object, list) = cue_list_for_playback(&store, &snapshot, playback)?;
    let mutations = vec![update_recorded_cue_list(
        &object,
        list,
        &programmer,
        requested,
        timing,
        operation,
    )?];
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
    Ok(())
}
