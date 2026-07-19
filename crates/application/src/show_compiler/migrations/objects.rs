use super::{ObjectUpdate, invalid_object};
use crate::{ActionError, lossless_json};
use light_output::OutputRoute;
use light_playback::{CueList, PlaybackDefinition};
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowCandidate, PortableShowCandidateObject};
use serde_json::{Map, Value};

const LEGACY_SPEED_GROUPS_BPM: [f64; 5] = [120.0, 90.0, 60.0, 30.0, 15.0];

pub(super) fn collect(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<ObjectUpdate>, ActionError> {
    candidate
        .objects()
        .filter_map(|object| migrate(object).transpose())
        .collect()
}

fn migrate(object: PortableShowCandidateObject<'_>) -> Result<Option<ObjectUpdate>, ActionError> {
    let migrated = match object.key().kind() {
        "cue_list" => migrate_cue_list(object)?,
        "group" => migrate_group(object)?,
        "playback" => migrate_playback(object)?,
        "preset" => migrate_preset(object)?,
        "route" => migrate_route(object)?,
        _ => return Ok(None),
    };
    Ok((migrated != *object.body()).then(|| ObjectUpdate::from_object(object, migrated)))
}

fn migrate_cue_list(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let mut cue_list = serde_json::from_value::<CueList>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let missing_cue_ids = missing_cue_ids(object);
    cue_list.migrate_legacy_chaser_xfade(&LEGACY_SPEED_GROUPS_BPM);

    let mut migrated = object.body().clone();
    let body = required_object_mut(&mut migrated, object)?;
    body.remove("chaser_xfade_millis");
    body.insert(
        "chaser_xfade_percent".into(),
        serde_json::to_value(cue_list.chaser_xfade_percent)
            .map_err(|error| invalid_object(object, error))?,
    );
    let cues = body
        .get_mut("cues")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid_object(object, "cues must be an array"))?;
    for index in missing_cue_ids {
        let cue = cues
            .get_mut(index)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid_object(object, format!("cue {index} must be an object")))?;
        cue.insert(
            "id".into(),
            Value::String(cue_list.cues[index].id.to_string()),
        );
    }
    Ok(migrated)
}

fn missing_cue_ids(object: PortableShowCandidateObject<'_>) -> Vec<usize> {
    object
        .body()
        .get("cues")
        .and_then(Value::as_array)
        .map(|cues| {
            cues.iter()
                .enumerate()
                .filter_map(|(index, cue)| {
                    cue.get("id")
                        .and_then(Value::as_str)
                        .is_none()
                        .then_some(index)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn migrate_group(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&group).map_err(|error| invalid_object(object, error))?;
    group.id = object.key().id().to_owned();
    let after = serde_json::to_value(group).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &after);
    Ok(migrated)
}

fn migrate_preset(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let mut preset = serde_json::from_value::<Preset>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&preset).map_err(|error| invalid_object(object, error))?;
    preset
        .reconcile_address(object.key().id())
        .map_err(|error| invalid_object(object, error))?;
    let after = serde_json::to_value(preset).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &after);
    Ok(migrated)
}

fn migrate_playback(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let playback = serde_json::from_value::<PlaybackDefinition>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let canonical =
        serde_json::to_value(playback).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    let body = required_object_mut(&mut migrated, object)?;
    let canonical = canonical_object(&canonical, object)?;
    for field in [
        "number",
        "name",
        "target",
        "buttons",
        "button_count",
        "fader",
        "has_fader",
        "go_activates",
        "auto_off",
        "xfade_millis",
        "color",
        "flash_release",
        "protect_from_swap",
        "presentation_icon",
        "presentation_image",
    ] {
        if !body.contains_key(field)
            && let Some(value) = canonical.get(field)
        {
            body.insert(field.into(), value.clone());
        }
    }
    if body.get("fader").and_then(Value::as_str) == Some("speed")
        && let Some(value) = canonical.get("fader")
    {
        body.insert("fader".into(), value.clone());
    }
    Ok(migrated)
}

fn migrate_route(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let destination_missing = object.body().get("destination").is_none();
    let delivery_mode_missing = object.body().get("delivery_mode").is_none();
    let mut route = serde_json::from_value::<OutputRoute>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    if destination_missing {
        route.destination = None;
    }
    if delivery_mode_missing {
        route.delivery_mode = Some(route.resolved_delivery_mode());
    }
    route
        .validate()
        .map_err(|error| invalid_object(object, error))?;

    let mut migrated = object.body().clone();
    let body = required_object_mut(&mut migrated, object)?;
    if destination_missing {
        body.insert("destination".into(), Value::Null);
    }
    if delivery_mode_missing {
        body.insert(
            "delivery_mode".into(),
            serde_json::to_value(route.delivery_mode)
                .map_err(|error| invalid_object(object, error))?,
        );
    }
    if !body.contains_key("minimum_slots") {
        body.insert("minimum_slots".into(), route.minimum_slots.into());
    }
    Ok(migrated)
}

fn required_object_mut<'a>(
    value: &'a mut Value,
    object: PortableShowCandidateObject<'_>,
) -> Result<&'a mut Map<String, Value>, ActionError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid_object(object, "body must be a JSON object"))
}

fn canonical_object<'a>(
    value: &'a Value,
    object: PortableShowCandidateObject<'_>,
) -> Result<&'a Map<String, Value>, ActionError> {
    value
        .as_object()
        .ok_or_else(|| invalid_object(object, "typed value must serialize as a JSON object"))
}
