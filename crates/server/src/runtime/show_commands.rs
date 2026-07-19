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
            cue.phasers = incoming.phasers;
            cue.fade_millis = incoming.fade_millis;
            cue.delay_millis = incoming.delay_millis;
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
    requested: Option<f64>,
    timing: CommandTiming,
    operation: RecordOperation,
) -> Result<light_application::ActiveShowObjectMutation, String> {
    let number =
        requested.unwrap_or_else(|| list.cues.last().map_or(1.0, |cue| cue.number.floor() + 1.0));
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
                programmer_cue(programmer, number, timing),
                operation,
            );
        }
    } else if operation == RecordOperation::Subtract {
        return Err(format!("Cue {number} does not exist"));
    } else {
        list.cues.push(programmer_cue(programmer, number, timing));
    }
    list.cues
        .sort_by(|left, right| left.number.total_cmp(&right.number));
    Ok(put_active_show_object(
        light_application::ActiveShowObjectKind::CueList,
        object.id.clone(),
        object.revision,
        serde_json::to_value(list).map_err(|error| error.to_string())?,
    ))
}

fn new_cue_list(
    playback: u16,
    cue: light_playback::Cue,
) -> (light_playback::CueList, light_playback::PlaybackDefinition) {
    let cue_list_id = light_core::CueListId::new();
    let list = light_playback::CueList {
        id: cue_list_id,
        name: format!("Cuelist {playback}"),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let definition = light_playback::PlaybackDefinition {
        number: playback,
        name: list.name.clone(),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [
            light_playback::PlaybackButtonAction::GoMinus,
            light_playback::PlaybackButtonAction::Go,
            light_playback::PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    (list, definition)
}

fn store_new_cue_list(
    playback: u16,
    cue: light_playback::Cue,
) -> Result<Vec<light_application::ActiveShowObjectMutation>, String> {
    let (list, definition) = new_cue_list(playback, cue);
    let list_id = list.id.0.to_string();
    let playback_id = playback.to_string();
    Ok(vec![
        put_active_show_object(
            light_application::ActiveShowObjectKind::CueList,
            list_id,
            0,
            serde_json::to_value(list).map_err(|error| error.to_string())?,
        ),
        put_active_show_object(
            light_application::ActiveShowObjectKind::Playback,
            playback_id,
            0,
            serde_json::to_value(definition).map_err(|error| error.to_string())?,
        ),
    ])
}

pub(super) fn store_cue_at(
    state: &AppState,
    session: &Session,
    playback: u16,
    requested: Option<f64>,
    timing: CommandTiming,
    operation: RecordOperation,
    context: &light_application::ActionContext,
) -> Result<(), String> {
    let _activation = state
        .activation_lock
        .clone()
        .try_lock_owned()
        .map_err(|_| "the active show is changing; retry Record".to_owned())?;
    let (entry, store) = active_show_store(state)?;
    let snapshot = state.engine.snapshot();
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let programmer_is_empty = programmer.values.is_empty() && programmer.group_values.is_empty();
    if programmer_is_empty && operation != RecordOperation::Subtract {
        return Err("the programmer has no values to record".into());
    }
    if operation != RecordOperation::Overwrite && requested.is_none() {
        return Err("RECORD + and RECORD - require an explicit CUE target".into());
    }
    let mutations = if let Some(definition) = snapshot
        .playbacks
        .iter()
        .find(|item| item.number == playback)
    {
        let (_, object, list) = cue_list_for_playback(&store, &snapshot, definition.number)?;
        vec![update_recorded_cue_list(
            &object,
            list,
            &programmer,
            requested,
            timing,
            operation,
        )?]
    } else {
        if operation == RecordOperation::Subtract {
            return Err(format!("Cuelist {playback} does not exist"));
        }
        if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback) {
            return Err("Cuelist number must be within 1-1000".into());
        }
        let number = requested.unwrap_or(1.0);
        store_new_cue_list(playback, programmer_cue(&programmer, number, timing))?
    };
    let action = active_show_object_action(context.clone(), entry.id, mutations);
    let result = run_active_show_object_action(state, action).map_err(|error| error.message)?;
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
