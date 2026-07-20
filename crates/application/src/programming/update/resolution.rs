use super::{
    CueTargetRequest, ProgrammingUpdateTargetRequest, ResolvedCueTarget, UpdateError,
    resolve_cue_target,
};
use crate::{ActionError, ActionErrorKind};
use light_core::CueListId;
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowRevision};
use std::collections::HashMap;

pub(super) fn find_cue_list(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
) -> Result<(&PortableShowObject, CueList), ActionError> {
    let canonical = cue_list_id.0.to_string();
    if let Some(object) = document.object("cue_list", &canonical) {
        let cue_list = decode::<CueList>(object, "Cuelist")?;
        return (cue_list.id == cue_list_id)
            .then_some((object, cue_list))
            .ok_or_else(|| invalid("stored Cuelist identity does not match its object key"));
    }
    let mut found = None;
    for object in document.objects_of_kind("cue_list") {
        let cue_list = decode::<CueList>(object, "Cuelist")?;
        if cue_list.id == cue_list_id && found.replace((object, cue_list)).is_some() {
            return Err(invalid(
                "multiple stored Cuelists share the requested semantic identity",
            ));
        }
    }
    found.ok_or_else(|| missing(format!("Cuelist {}", cue_list_id.0)))
}

pub(super) fn find_preset<'a>(
    document: &'a PortableShowDocument,
    object_id: &str,
) -> Result<(&'a PortableShowObject, Preset), ActionError> {
    let object = document
        .object("preset", object_id)
        .ok_or_else(|| missing(format!("Preset {object_id}")))?;
    Ok((object, decode(object, "Preset")?))
}

pub(super) type StoredGroups<'a> = (
    &'a PortableShowObject,
    GroupDefinition,
    HashMap<String, GroupDefinition>,
);

pub(super) fn find_group<'a>(
    document: &'a PortableShowDocument,
    object_id: &str,
) -> Result<StoredGroups<'a>, ActionError> {
    let mut target = None;
    let groups = document
        .objects_of_kind("group")
        .map(|object| {
            let mut group = decode::<GroupDefinition>(object, "Group")?;
            group.id = object.key().id().to_owned();
            if group.id == object_id {
                target = Some((object, group.clone()));
            }
            Ok((group.id.clone(), group))
        })
        .collect::<Result<HashMap<_, _>, ActionError>>()?;
    let (object, group) = target.ok_or_else(|| missing(format!("Group {object_id}")))?;
    Ok((object, group, groups))
}

pub(super) fn resolve_requested_cue(
    request: &ProgrammingUpdateTargetRequest,
    active: &[super::ActiveCueContext],
) -> Result<ResolvedCueTarget, ActionError> {
    let ProgrammingUpdateTargetRequest::Cue {
        cue_list_id,
        playback_number,
        cue_id,
        cue_number,
        validate_active_context,
    } = request
    else {
        return Err(invalid("Update target is not a Cue"));
    };
    if *validate_active_context {
        return validate_active_cue(*cue_list_id, *playback_number, *cue_id, *cue_number, active);
    }
    let target = match (*cue_id, *playback_number) {
        (Some(cue_id), playback_number) => CueTargetRequest::Explicit(ResolvedCueTarget {
            cue_list_id: *cue_list_id,
            playback_number,
            cue_id,
            cue_number: cue_number.ok_or_else(|| invalid("explicit Cue requires cue_number"))?,
        }),
        (None, Some(playback_number)) => CueTargetRequest::ActivePlayback { playback_number },
        (None, None) => CueTargetRequest::PoolCueList {
            cue_list_id: *cue_list_id,
        },
    };
    resolve_cue_target(&target, active).map_err(update_error)
}

fn validate_active_cue(
    cue_list_id: CueListId,
    playback_number: Option<u16>,
    cue_id: Option<uuid::Uuid>,
    cue_number: Option<f64>,
    active: &[super::ActiveCueContext],
) -> Result<ResolvedCueTarget, ActionError> {
    let playback_number = playback_number
        .ok_or_else(|| invalid("live Cue Update target requires playback_number"))?;
    let context = active
        .iter()
        .find(|context| context.playback_number == playback_number)
        .ok_or_else(|| conflict("the touched playback is no longer active"))?;
    if context.cue_list_id != cue_list_id
        || cue_id.is_some_and(|cue_id| cue_id != context.cue_id)
        || cue_number.is_some_and(|cue_number| cue_number != context.cue_number)
    {
        return Err(conflict("the touched playback/Cue context changed"));
    }
    Ok(ResolvedCueTarget::from(context))
}

pub(super) fn validate_show_revision(
    document: &PortableShowDocument,
    expected: Option<PortableShowRevision>,
) -> Result<(), ActionError> {
    if expected.is_none_or(|expected| expected == document.revision()) {
        Ok(())
    } else {
        Err(conflict("stale active-show revision").at_related_revision(document.revision().value()))
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    object: &PortableShowObject,
    label: &str,
) -> Result<T, ActionError> {
    serde_json::from_value(object.body().clone())
        .map_err(|error| invalid(format!("invalid {label}: {error}")))
}

pub(super) fn update_error(error: UpdateError) -> ActionError {
    match error {
        UpdateError::StaleRevision { current, .. } => {
            conflict(error.to_string()).at_revision(current)
        }
        UpdateError::MissingTarget { .. } => missing(error.to_string()),
        UpdateError::AmbiguousPlaybackContext { .. } => conflict(error.to_string()),
        _ => invalid(error.to_string()),
    }
}

pub(super) fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn missing(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

pub(super) fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
