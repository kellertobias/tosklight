use super::{
    CueTargetRequest, ProgrammingUpdateTargetRequest, ResolvedCueTarget, UpdateError,
    resolve_cue_target,
};
use crate::{ActionError, ActionErrorKind};
use light_core::CueListId;
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset, PresetAddress, PresetFamily};
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

// Explicit Update requests carry stored object identity, not a recall address.
pub(super) fn find_preset<'a>(
    document: &'a PortableShowDocument,
    object_id: &str,
) -> Result<(&'a PortableShowObject, Preset), ActionError> {
    let object = document
        .object("preset", object_id)
        .ok_or_else(|| missing(format!("Preset {object_id}")))?;
    Ok((object, decode(object, "Preset")?))
}

/// Resolve a trusted operator address to its unique persisted identity.
/// External Update requests continue to require an exact object key.
pub fn resolve_preset_update_address(
    document: &PortableShowDocument,
    address: PresetAddress,
) -> Result<String, ActionError> {
    let address = PresetAddress::new(address.family, address.number).map_err(invalid)?;
    find_recalled_preset(document, &address.storage_key())
        .map(|(object, _)| object.key().id().to_owned())
}

// Recalled Programmer context carries a typed pool address, including legacy aliases.
pub(super) fn find_recalled_preset<'a>(
    document: &'a PortableShowDocument,
    object_id: &str,
) -> Result<(&'a PortableShowObject, Preset), ActionError> {
    let exact = document.object("preset", object_id);
    let exact_preset = exact
        .map(|object| decode::<Preset>(object, "Preset"))
        .transpose()?;
    let family = exact_preset
        .as_ref()
        .map_or(PresetFamily::Mixed, |preset| preset.family);
    let requested = match PresetAddress::from_storage_key(object_id, family) {
        Ok(address) => address,
        // Preserve exact opaque legacy keys; they do not acquire numeric aliases.
        Err(_) => {
            return exact
                .zip(exact_preset)
                .ok_or_else(|| missing(format!("Preset {object_id}")));
        }
    };
    let mut found = None;
    for object in document.objects_of_kind("preset") {
        // Parse keys first: unrelated Presets need no body decode or value clone.
        if PresetAddress::from_storage_key(object.key().id(), requested.family).ok()
            != Some(requested)
        {
            continue;
        }
        let mut preset = decode::<Preset>(object, "Preset")?;
        let stored_number = preset.number;
        let address = preset
            .reconcile_address(object.key().id())
            .map_err(invalid)?;
        // Resolution is observational, including legacy number=0 bodies.
        preset.number = stored_number;
        if address != requested {
            continue;
        }
        if found.replace((object, preset)).is_some() {
            return Err(invalid(
                "multiple stored Presets share the requested address",
            ));
        }
    }
    found.ok_or_else(|| missing(format!("Preset {object_id}")))
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
        playback_address,
        cue_id,
        cue_number,
        validate_active_context,
    } = request
    else {
        return Err(invalid("Update target is not a Cue"));
    };
    if *validate_active_context {
        return validate_active_cue(
            *cue_list_id,
            *playback_address,
            *cue_id,
            cue_number.clone(),
            active,
        );
    }
    let target = match (*cue_id, *playback_address) {
        (Some(cue_id), playback_address) => CueTargetRequest::Explicit(ResolvedCueTarget {
            cue_list_id: *cue_list_id,
            playback_address,
            cue_id,
            cue_number: cue_number
                .clone()
                .ok_or_else(|| invalid("explicit Cue requires cue_number"))?,
        }),
        (None, Some(playback_address)) => CueTargetRequest::ActivePlayback { playback_address },
        (None, None) => CueTargetRequest::PoolCueList {
            cue_list_id: *cue_list_id,
        },
    };
    resolve_cue_target(&target, active).map_err(update_error)
}

fn validate_active_cue(
    cue_list_id: CueListId,
    playback_address: Option<light_playback::PlaybackAddress>,
    cue_id: Option<uuid::Uuid>,
    cue_number: Option<light_playback::CueNumber>,
    active: &[super::ActiveCueContext],
) -> Result<ResolvedCueTarget, ActionError> {
    let playback_address = playback_address
        .ok_or_else(|| invalid("live Cue Update target requires playback_address"))?;
    let context = active
        .iter()
        .find(|context| context.playback_address == playback_address)
        .ok_or_else(|| conflict("the touched playback is no longer active"))?;
    if context.cue_list_id != cue_list_id
        || cue_id.is_some_and(|cue_id| cue_id != context.cue_id)
        || cue_number
            .as_ref()
            .is_some_and(|cue_number| cue_number != &context.cue_number)
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
