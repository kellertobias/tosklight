use super::{
    GroupMasterPlaybackAddress, PlaybackTopologyAction, PlaybackTopologyCommand,
    PlaybackTopologyResolution,
    change::{PreparedTopology, changed_cue_list_history, changed_present, no_change},
    page_actions::{create_page, delete_page, rename_page, validated_page},
    stored::{
        Stored, cue_list_object_id, find_cue_list, find_group, find_macro, find_timecode, invalid,
        not_found, page_object_id, same_typed, stored_projection, validate_identity,
        validate_revision,
    },
    validation::validate_show,
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectKind, lossless_json};
use light_playback::{CueList, PlaybackAddress, PlaybackDefinition, PlaybackPage, PlaybackTarget};
use light_show::{PortableShowDocument, PortableShowObjectRedo, PortableShowObjectUndo};
use serde_json::Value;

pub(super) fn prepare(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    expected_show_revision: u64,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_show(document, command, expected_show_revision)?;
    match &command.action {
        PlaybackTopologyAction::SaveCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
            cue_list,
            raw_body,
        } => save_cue_list(
            document,
            command,
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_deref(),
            cue_list,
            raw_body,
        ),
        PlaybackTopologyAction::UndoCueList { .. } | PlaybackTopologyAction::RedoCueList { .. } => {
            Err(invalid("Cuelist history requires the history boundary"))
        }
        PlaybackTopologyAction::AssignPlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
            playback,
        } => configure(
            document,
            command,
            *address,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
            Some(playback),
            false,
        ),
        PlaybackTopologyAction::UpdatePlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
            playback,
        } => configure(
            document,
            command,
            *address,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
            Some(playback),
            true,
        ),
        PlaybackTopologyAction::ClearPlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
        } => configure(
            document,
            command,
            *address,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
            None,
            false,
        ),
        PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision,
            address,
        } => assign_group_master(
            document,
            command,
            group_object_id,
            *expected_group_revision,
            address,
        ),
        PlaybackTopologyAction::CreatePage {
            page,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => create_page(
            document,
            command,
            *page,
            name,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
        PlaybackTopologyAction::RenamePage {
            page,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => rename_page(
            document,
            command,
            *page,
            name,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
        PlaybackTopologyAction::DeletePage {
            page,
            expected_page_revision,
            expected_page_object_id,
        } => delete_page(
            document,
            command,
            *page,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
    }
}
fn configure(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: PlaybackAddress,
    expected_revision: u64,
    expected_object_id: Option<&str>,
    playback: Option<&PlaybackDefinition>,
    update: bool,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let stored = validated_page(
        document,
        address.page(),
        expected_revision,
        expected_object_id,
    )?;
    let existing = stored
        .as_ref()
        .and_then(|page| page.typed.assignments.get(&address.position()));
    if update && existing.is_none() {
        return Err(not_found("Playback is not assigned"));
    }
    if !update && playback.is_some() && existing.is_some() {
        return Err(super::stored::conflict(
            "Playback position is already assigned",
        ));
    }
    if let Some(playback) = playback {
        playback.validate().map_err(invalid)?;
        validate_macro_target(document, playback)?;
        validate_timecode_target(document, playback)?;
        if let PlaybackTarget::CueList { cue_list_id } = playback.target {
            if find_cue_list(document, cue_list_id)?.is_none() {
                return Err(not_found("Cuelist does not exist"));
            }
        }
        if let PlaybackTarget::Group { ref group_id, .. } = playback.target {
            if find_group(document, group_id)?.is_none() {
                return Err(not_found("Group does not exist"));
            }
        }
    }
    let mut desired = stored
        .as_ref()
        .map(|page| page.typed.clone())
        .unwrap_or(PlaybackPage {
            number: address.page(),
            name: format!("Page {}", address.page()),
            assignments: Default::default(),
        });
    if let Some(playback) = playback {
        desired
            .assignments
            .insert(address.position(), playback.clone());
    } else {
        desired.assignments.remove(&address.position());
    }
    let resolution = PlaybackTopologyResolution::Playback { address };
    if stored.as_ref().is_some_and(|page| page.typed == desired)
        || (stored.is_none() && playback.is_none())
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            stored
                .as_ref()
                .map(|page| stored_projection(ActiveShowObjectKind::PlaybackPage, page))
                .into_iter()
                .collect(),
        ));
    }
    let object_id = page_object_id(document, stored.as_ref(), address.page())?;
    changed_present(
        document,
        command,
        resolution,
        vec![(
            ActiveShowObjectKind::PlaybackPage,
            object_id,
            typed_body(stored.as_ref(), &desired)?,
        )],
        Vec::new(),
    )
}
fn assign_group_master(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    group_object_id: &str,
    expected_group_revision: u64,
    address: &GroupMasterPlaybackAddress,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let group =
        find_group(document, group_object_id)?.ok_or_else(|| not_found("Group does not exist"))?;
    validate_revision(
        Some(&group),
        expected_group_revision,
        "Group",
        document.revision().value(),
    )?;
    let page = validated_page(
        document,
        address.address.page(),
        address.expected_page_revision,
        address.expected_page_object_id.as_deref(),
    )?;
    let existing = page
        .as_ref()
        .and_then(|page| page.typed.assignments.get(&address.address.position()));
    let mut playback = existing.cloned().unwrap_or_else(|| {
        PlaybackDefinition::new_cue_list(
            &group.typed.name,
            light_core::CueListId(uuid::Uuid::nil()),
        )
    });
    playback.target = PlaybackTarget::Group {
        group_id: group_object_id.into(),
        initial_master: preserved_initial_master(
            existing.map(|playback| &playback.target),
            group_object_id,
        ),
    };
    if existing.is_none() {
        playback.name = group.typed.name.clone();
        if let Some(color) = &group.typed.color {
            playback.color = color.clone();
        }
        playback.presentation_icon = group.typed.icon.clone();
    }
    playback.reset_incompatible_layout();
    configure(
        document,
        command,
        address.address,
        address.expected_page_revision,
        address.expected_page_object_id.as_deref(),
        Some(&playback),
        existing.is_some(),
    )
}
pub(super) enum CueListHistory {
    Undo(PortableShowObjectUndo),
    Redo(PortableShowObjectRedo),
}
pub(super) fn prepare_cue_list_history(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    expected_show_revision: u64,
    history: CueListHistory,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_show(document, command, expected_show_revision)?;
    let (cue_list_id, expected_revision, expected_object_id, is_undo) = match &command.action {
        PlaybackTopologyAction::UndoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => (
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_str(),
            true,
        ),
        PlaybackTopologyAction::RedoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => (
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_str(),
            false,
        ),
        _ => return Err(invalid("expected a Cuelist history action")),
    };
    let stored =
        find_cue_list(document, cue_list_id)?.ok_or_else(|| not_found("Cuelist was not found"))?;
    validate_identity(
        Some(&stored),
        Some(expected_object_id),
        "Cuelist",
        document.revision().value(),
    )?;
    validate_revision(
        Some(&stored),
        expected_revision,
        "Cuelist",
        document.revision().value(),
    )?;
    let (key, body) = match &history {
        CueListHistory::Undo(value) => (value.key(), value.body()),
        CueListHistory::Redo(value) => (value.key(), value.body()),
    };
    if key.kind() != ActiveShowObjectKind::CueList.as_str() || key.id() != expected_object_id {
        return Err(invalid("Cuelist history identity changed"));
    }
    let restored: CueList = serde_json::from_value(body.clone()).map_err(invalid)?;
    if restored.id != cue_list_id {
        return Err(invalid("Cuelist history body has a different identity"));
    }
    restored.validate().map_err(invalid)?;
    let mut transaction = document.transaction();
    match history {
        CueListHistory::Undo(value) if is_undo => {
            transaction.undo_object(value);
        }
        CueListHistory::Redo(value) if !is_undo => {
            transaction.redo_object(value);
        }
        _ => return Err(invalid("Cuelist history direction changed")),
    }
    changed_cue_list_history(
        document,
        command,
        PlaybackTopologyResolution::CueList { cue_list_id },
        transaction,
        expected_object_id,
    )
}
fn save_cue_list(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    cue_list_id: light_core::CueListId,
    expected_revision: u64,
    expected_object_id: Option<&str>,
    cue_list: &CueList,
    raw_body: &Value,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    if cue_list.id != cue_list_id {
        return Err(invalid(
            "Cuelist identity does not match the requested identity",
        ));
    }
    let raw_cue_list = serde_json::from_value::<CueList>(raw_body.clone()).map_err(invalid)?;
    if !same_typed(&raw_cue_list, cue_list)? {
        return Err(invalid("Cuelist body does not match its typed candidate"));
    }
    cue_list.validate().map_err(invalid)?;
    validate_timecode_graph_for_cue_list(document, cue_list)?;
    let stored = find_cue_list(document, cue_list_id)?;
    validate_identity(
        stored.as_ref(),
        expected_object_id,
        "Cuelist",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_revision,
        "Cuelist",
        document.revision().value(),
    )?;
    let (cue_list, raw_body) = preserve_omitted_out_timing(stored.as_ref(), cue_list, raw_body)?;
    cue_list.validate().map_err(invalid)?;
    let resolution = PlaybackTopologyResolution::CueList { cue_list_id };
    if let Some(existing) = stored.as_ref()
        && same_typed(&existing.typed, &cue_list)?
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(ActiveShowObjectKind::CueList, existing)],
        ));
    }
    let object_id = cue_list_object_id(document, stored.as_ref(), cue_list_id)?;
    let body = cue_list_body(stored.as_ref(), &cue_list, &raw_body)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::CueList, object_id, body)],
        Vec::new(),
    )
}
fn validate_timecode_graph_for_cue_list(
    document: &PortableShowDocument,
    candidate: &CueList,
) -> Result<(), ActionError> {
    let mut cue_lists = document
        .objects_of_kind("cue_list")
        .filter_map(|object| serde_json::from_value::<CueList>(object.body().clone()).ok())
        .filter(|cue_list| cue_list.id != candidate.id)
        .collect::<Vec<_>>();
    cue_lists.push(candidate.clone());
    let timecodes = document
        .objects_of_kind("timecode")
        .map(|object| {
            serde_json::from_value::<light_playback::TimecodeDefinition>(object.body().clone())
                .map_err(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::validate_cue_timecode_graph(&cue_lists, &timecodes).map_err(invalid)
}
fn preserve_omitted_out_timing(
    stored: Option<&Stored<CueList>>,
    requested: &CueList,
    raw_body: &Value,
) -> Result<(CueList, Value), ActionError> {
    let Some(stored) = stored else {
        return Ok((requested.clone(), raw_body.clone()));
    };
    let mut normalized = requested.clone();
    let mut normalized_raw = raw_body.clone();
    let raw_cues = normalized_raw
        .get_mut("cues")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("Cuelist body cues must be an array"))?;
    for (requested_cue, raw_cue) in normalized.cues.iter_mut().zip(raw_cues.iter_mut()) {
        let Some(stored_cue) = stored
            .typed
            .cues
            .iter()
            .find(|candidate| candidate.id == requested_cue.id)
        else {
            continue;
        };
        let raw_cue = raw_cue
            .as_object_mut()
            .ok_or_else(|| invalid("Cuelist cues must be objects"))?;
        preserve_optional_millis(
            raw_cue,
            "out_fade_millis",
            &mut requested_cue.out_fade_millis,
            stored_cue.out_fade_millis,
        );
        preserve_optional_millis(
            raw_cue,
            "out_delay_millis",
            &mut requested_cue.out_delay_millis,
            stored_cue.out_delay_millis,
        );
    }
    Ok((normalized, normalized_raw))
}
fn preserve_optional_millis(
    raw_cue: &mut serde_json::Map<String, Value>,
    field: &str,
    requested: &mut Option<u64>,
    stored: Option<u64>,
) {
    if raw_cue.contains_key(field) {
        return;
    }
    *requested = stored;
    if let Some(value) = stored {
        raw_cue.insert(field.to_owned(), Value::from(value));
    }
}
fn validate_macro_target(
    document: &PortableShowDocument,
    playback: &PlaybackDefinition,
) -> Result<(), ActionError> {
    let PlaybackTarget::Macro { macro_id } = &playback.target else {
        return Ok(());
    };
    if find_macro(document, *macro_id)?.is_none() {
        return Err(not_found(format!("Macro {macro_id} does not exist")));
    }
    Ok(())
}
fn validate_timecode_target(
    document: &PortableShowDocument,
    playback: &PlaybackDefinition,
) -> Result<(), ActionError> {
    let PlaybackTarget::Timecode { timecode_id } = &playback.target else {
        return Ok(());
    };
    if find_timecode(document, *timecode_id)?.is_none() {
        return Err(not_found(format!(
            "Timecode {} does not exist",
            timecode_id.0
        )));
    }
    Ok(())
}
fn preserved_initial_master(target: Option<&PlaybackTarget>, group_id: &str) -> Option<f32> {
    match target {
        Some(PlaybackTarget::Group {
            group_id: existing_group_id,
            initial_master,
        }) if existing_group_id == group_id => *initial_master,
        _ => None,
    }
}
fn cue_list_body(
    stored: Option<&Stored<CueList>>,
    requested: &CueList,
    raw_body: &Value,
) -> Result<Value, ActionError> {
    let mut merged = lossless_json::merge_typed_request(
        stored.map(|value| &value.raw_body),
        stored.map(|value| &value.typed),
        raw_body,
        requested,
        requested,
    )
    .map_err(invalid)?;
    lossless_json::strip_zero_u64_echo(&mut merged, "chaser_xfade_millis");
    strip_inherited_out_timing_nulls(&mut merged);
    Ok(merged)
}
fn strip_inherited_out_timing_nulls(body: &mut Value) {
    let Some(cues) = body.get_mut("cues").and_then(Value::as_array_mut) else {
        return;
    };
    for cue in cues {
        let Some(cue) = cue.as_object_mut() else {
            continue;
        };
        for field in ["out_fade_millis", "out_delay_millis"] {
            if cue.get(field).is_some_and(Value::is_null) {
                cue.remove(field);
            }
        }
    }
}
fn typed_body<T: serde::Serialize>(
    stored: Option<&Stored<T>>,
    desired: &T,
) -> Result<Value, ActionError> {
    match stored {
        Some(stored) => {
            lossless_json::merge_typed(&stored.raw_body, &stored.typed, desired).map_err(invalid)
        }
        None => serde_json::to_value(desired).map_err(invalid),
    }
}
