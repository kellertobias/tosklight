use super::{ObjectUpdate, invalid_object};
use crate::{ActionError, lossless_json};
use light_output::OutputRoute;
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowCandidate, PortableShowCandidateObject};
use serde_json::{Map, Value, json};

const LEGACY_SPEED_GROUPS_BPM: [f64; 5] = [120.0, 90.0, 60.0, 30.0, 15.0];

pub(super) fn collect(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<ObjectUpdate>, ActionError> {
    for object in candidate.objects() {
        validate_contract(object)?;
    }
    let presets = candidate
        .objects_of_kind("preset")
        .map(|object| {
            let mut body = object.body().clone();
            super::attribute_aliases::migrate(object, &mut body)?;
            serde_json::from_value::<Preset>(body)
                .map(|preset| (object.key().id().to_owned(), preset))
                .map_err(|error| invalid_object(object, error))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    let groups = candidate
        .objects_of_kind("group")
        .map(|object| {
            let mut body = object.body().clone();
            super::attribute_aliases::migrate(object, &mut body)?;
            serde_json::from_value::<GroupDefinition>(body)
                .map(|mut group| {
                    group.id = object.key().id().to_owned();
                    (group.id.clone(), group)
                })
                .map_err(|error| invalid_object(object, error))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    let native = super::super::dynamic_presets::NativeTemplateValidator::new(candidate);
    candidate
        .objects()
        .filter_map(|object| {
            if object.key().kind() == "dynamic" {
                migrate_dynamic_fallbacks(object, &presets, &groups, &|value| native.allows(value))
                    .transpose()
            } else {
                migrate(object).transpose()
            }
        })
        .collect()
}

fn validate_contract(object: PortableShowCandidateObject<'_>) -> Result<(), ActionError> {
    light_show::validate_playback_object(object.key().kind(), object.key().id(), object.body())
        .map_err(|error| invalid_object(object, error))
}

fn migrate_dynamic_fallbacks(
    object: PortableShowCandidateObject<'_>,
    presets: &std::collections::HashMap<String, Preset>,
    groups: &std::collections::HashMap<String, GroupDefinition>,
    verify_native: &dyn Fn(&light_core::AttributeValue) -> bool,
) -> Result<Option<ObjectUpdate>, ActionError> {
    let mut migrated = object.body().clone();
    super::attribute_aliases::migrate(object, &mut migrated)?;
    let Ok(mut definition) =
        serde_json::from_value::<light_dynamics::DynamicDefinition>(migrated.clone())
    else {
        // Invalid Dynamics remain portable, repairable show content. Compilation already skips
        // them, so a best-effort fallback migration must not make the entire show unloadable.
        return Ok(None);
    };
    super::super::objects::hydrate_dynamic_preset_fallbacks(
        &mut definition,
        presets,
        groups,
        verify_native,
    );
    let canonical =
        serde_json::to_value(definition).map_err(|error| invalid_object(object, error))?;
    copy_dynamic_fallbacks(&canonical, &mut migrated);
    migrate_embedded_dynamic_phase_orderings(&mut migrated);
    Ok((migrated != *object.body()).then(|| ObjectUpdate::from_object(object, migrated)))
}

fn copy_dynamic_fallbacks(canonical: &Value, migrated: &mut Value) {
    super::super::dynamic_presets::copy_typed_source_retention(canonical, migrated);
    let lane_count = canonical
        .pointer("/lanes")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    for lane_index in 0..lane_count {
        let point_count = canonical
            .pointer(&format!("/lanes/{lane_index}/keyframes/points"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        for point_index in 0..point_count {
            copy_scalar_fallback(
                canonical,
                migrated,
                &format!("/lanes/{lane_index}/keyframes/points/{point_index}/source"),
            );
        }
        for path in [
            format!("/lanes/{lane_index}/max_min/minimum"),
            format!("/lanes/{lane_index}/max_min/maximum"),
            format!("/lanes/{lane_index}/middle_amplitude/middle"),
        ] {
            copy_scalar_fallback(canonical, migrated, &path);
        }
    }
    let group_count = canonical
        .pointer("/random_groups")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    for group_index in 0..group_count {
        for path in [
            format!("/random_groups/{group_index}/low"),
            format!("/random_groups/{group_index}/high"),
        ] {
            copy_scalar_fallback(canonical, migrated, &path);
        }
    }
}

fn copy_scalar_fallback(canonical: &Value, migrated: &mut Value, path: &str) {
    let Some(fallbacks) = canonical.pointer(&format!("{path}/last_valid_by_target")) else {
        return;
    };
    let Some(source) = migrated.pointer_mut(path).and_then(Value::as_object_mut) else {
        return;
    };
    source.insert("last_valid_by_target".into(), fallbacks.clone());
}

pub(super) fn migrate(
    object: PortableShowCandidateObject<'_>,
) -> Result<Option<ObjectUpdate>, ActionError> {
    validate_contract(object)?;
    let mut migrated = match object.key().kind() {
        "attribute_configuration" => object.body().clone(),
        "cue_list" => migrate_cue_list(object)?,
        "dynamic" => object.body().clone(),
        "group" => migrate_group(object)?,
        "playback_page" => object.body().clone(),
        "patched_fixture" => super::stage_position::migrate(object.body()),
        "preset" => migrate_preset(object)?,
        "route" => migrate_route(object)?,
        "stage_layout" => migrate_stage_layout(object)?,
        _ => return Ok(None),
    };
    super::attribute_aliases::migrate(object, &mut migrated)?;
    migrate_embedded_dynamic_phase_orderings(&mut migrated);
    Ok((migrated != *object.body()).then(|| ObjectUpdate::from_object(object, migrated)))
}

/// Writes inferred definition-wide mappings into every typed Dynamic definition nested in a
/// portable object while leaving unknown sibling fields and legacy phase ordering untouched.
///
/// The `DynamicDefinition` deserializer is the compatibility oracle used by runtime snapshots,
/// Programmer/Preload state, and session undo/redo state. Portable objects need this additional
/// lossless write-through because their raw JSON bodies deliberately preserve unknown fields.
fn migrate_embedded_dynamic_phase_orderings(value: &mut Value) {
    if looks_like_dynamic_definition(value)
        && let Ok(definition) =
            serde_json::from_value::<light_dynamics::DynamicDefinition>(value.clone())
        && let Ok(canonical) = serde_json::to_value(definition)
        && let Some(body) = value.as_object_mut()
    {
        if !body.contains_key("spatial_mapping")
            && let Some(mapping) = canonical.get("spatial_mapping")
        {
            body.insert("spatial_mapping".into(), mapping.clone());
        }
        // Preserve unknown lane fields while writing the explicit Current partner into raw
        // portable objects and embedded fallback definitions, including Undo/Preload bodies.
        if let (Some(Value::Array(stored)), Some(Value::Array(lanes))) =
            (body.get_mut("lanes"), canonical.get("lanes"))
        {
            let old = std::mem::take(stored);
            *stored = lanes
                .iter()
                .map(|lane| {
                    old.iter()
                        .find(|candidate| candidate.get("id") == lane.get("id"))
                        .unwrap_or(lane)
                        .clone()
                })
                .collect();
        }
    }

    match value {
        Value::Array(values) => {
            for value in values {
                migrate_embedded_dynamic_phase_orderings(value);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                migrate_embedded_dynamic_phase_orderings(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn looks_like_dynamic_definition(value: &Value) -> bool {
    let Some(body) = value.as_object() else {
        return false;
    };
    [
        "id",
        "pool_number",
        "revision",
        "name",
        "target_binding",
        "lanes",
        "phase",
        "speed",
        "default_activation",
    ]
    .iter()
    .all(|field| body.contains_key(*field))
}

fn migrate_stage_layout(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let mut layout = serde_json::from_value::<crate::StageLayout>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let is_legacy = layout.positions_2d_config.is_none();
    let automatic = layout.effective_positions_2d_config().provenance
        == crate::StagePositions2dProvenance::Automatic;
    if is_legacy && automatic {
        let projection = layout.effective_positions_2d_config().projection;
        layout.regenerate_positions_2d(projection);
    } else if is_legacy {
        layout.mark_positions_2d_manual();
    }

    let mut migrated = object.body().clone();
    let body = required_object_mut(&mut migrated, object)?;
    if is_legacy && automatic {
        body.insert(
            "positions".into(),
            serde_json::to_value(&layout.positions)
                .map_err(|error| invalid_object(object, error))?,
        );
    }
    body.insert(
        "positions2dConfig".into(),
        serde_json::to_value(layout.effective_positions_2d_config())
            .map_err(|error| invalid_object(object, error))?,
    );
    Ok(migrated)
}

fn migrate_cue_list(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let missing_cue_ids = missing_cue_ids(object);
    let mut migrated = object.body().clone();
    migrate_cue_numbers(object, &mut migrated)?;
    let mut cue_list = serde_json::from_value::<CueList>(migrated.clone())
        .map_err(|error| invalid_object(object, error))?;
    cue_list.migrate_legacy_chaser_xfade(&LEGACY_SPEED_GROUPS_BPM);

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
    let mut cue_ids = std::collections::HashSet::with_capacity(cues.len());
    for (index, cue) in cues.iter_mut().enumerate() {
        if let Some(cue) = cue.as_object_mut() {
            cue.remove("phasers");
            if let Some(id) = cue
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                && !cue_ids.insert(id)
            {
                let replacement = uuid::Uuid::new_v5(
                    &cue_list.id.0,
                    format!("duplicate-cue-id:{id}:{index}").as_bytes(),
                );
                cue.insert("id".into(), Value::String(replacement.to_string()));
                cue_ids.insert(replacement);
            }
        }
    }
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
    // Persist the schema's explicit Cuelist-settings defaults for legacy Cuelists that predate a
    // field. Only absent keys are filled, so the migration is a one-time byte rewrite that stays
    // idempotent and preserves existing values and unknown extensions.
    let canonical =
        serde_json::to_value(&cue_list).map_err(|error| invalid_object(object, error))?;
    let canonical = canonical_object(&canonical, object)?;
    let body = required_object_mut(&mut migrated, object)?;
    for field in [
        "intensity_priority_mode",
        "wrap_mode",
        "restart_mode",
        "force_cue_timing",
        "disable_cue_timing",
        "auto_off_at_zero",
        "auto_off_flash_release",
        "speed_multiplier",
    ] {
        if !body.contains_key(field)
            && let Some(value) = canonical.get(field)
        {
            body.insert(field.into(), value.clone());
        }
    }
    Ok(migrated)
}

fn migrate_cue_numbers(
    object: PortableShowCandidateObject<'_>,
    migrated: &mut Value,
) -> Result<(), ActionError> {
    let cues = required_object_mut(migrated, object)?
        .get_mut("cues")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid_object(object, "cues must be an array"))?;
    let mut normalized = std::collections::HashMap::<String, String>::with_capacity(cues.len());
    let mut previous = None::<(String, light_playback::CueNumber)>;
    for (index, cue) in cues.iter_mut().enumerate() {
        let cue = cue
            .as_object_mut()
            .ok_or_else(|| invalid_object(object, format!("cue {index} must be an object")))?;
        let raw = cue
            .get("number")
            .ok_or_else(|| invalid_object(object, format!("cue {index} has no number")))?;
        let original = match raw {
            Value::String(number) => number.clone(),
            Value::Number(number) => number.to_string(),
            _ => {
                return Err(invalid_object(
                    object,
                    format!("cue {index} number must be a string or legacy number"),
                ));
            }
        };
        let number = match raw {
            Value::String(number) => number.parse::<light_playback::CueNumber>(),
            Value::Number(number) => number
                .as_f64()
                .ok_or_else(|| "legacy Cue number is outside the supported numeric range".into())
                .and_then(light_playback::CueNumber::try_from_legacy_f64),
            _ => unreachable!("number kind checked above"),
        }
        .map_err(|error| invalid_object(object, format!("Cue {original}: {error}")))?;
        let canonical = number.to_string();
        if let Some(previous) = normalized.insert(canonical.clone(), original.clone()) {
            return Err(invalid_object(
                object,
                format!(
                    "legacy Cue numbers {previous} and {original} both normalize to {canonical}; manually renumber one Cue before opening this show"
                ),
            ));
        }
        if let Some((previous_label, previous_number)) = previous.as_ref()
            && previous_number >= &number
        {
            return Err(invalid_object(
                object,
                format!(
                    "legacy Cue order would change when {previous_label} and {original} become canonical integer paths; manually renumber these Cues before opening this show"
                ),
            ));
        }
        previous = Some((original, number));
        cue.insert("number".into(), Value::String(canonical));
    }
    Ok(())
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
    group.id = object.key().id().to_owned();
    let canonical = serde_json::to_value(group).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    let body = required_object_mut(&mut migrated, object)?;
    let canonical = canonical_object(&canonical, object)?;
    if let Some(id) = canonical.get("id") {
        body.insert("id".into(), id.clone());
    }
    if body.get("source").is_none_or(Value::is_null) {
        let source = legacy_group_source(body);
        body.insert("source".into(), source);
    }
    // Persist the schema's explicit defaults for legacy Groups that predate a field, matching the
    // playback and route migrations. Only absent keys are filled, so the first load after an upgrade
    // rewrites the object once and every later load leaves it byte-identical, while existing values
    // and unknown extension fields are preserved.
    for field in [
        "color",
        "icon",
        "derived_from",
        "frozen_from",
        "programming",
    ] {
        if !body.contains_key(field)
            && let Some(value) = canonical.get(field)
        {
            body.insert(field.into(), value.clone());
        }
    }
    Ok(migrated)
}

fn legacy_group_source(body: &Map<String, Value>) -> Value {
    if let Some(derived) = body.get("derived_from").and_then(Value::as_object) {
        return json!({
            "type": "references",
            "references": [{
                "group_id": derived.get("source_group_id").cloned().unwrap_or(Value::Null),
                "rule": derived.get("rule").cloned().unwrap_or(Value::Null),
            }]
        });
    }
    json!({
        "type": "explicit",
        "fixture_ids": body.get("fixtures").cloned().unwrap_or_else(|| json!([])),
    })
}

fn migrate_preset(object: PortableShowCandidateObject<'_>) -> Result<Value, ActionError> {
    let mut preset = serde_json::from_value::<Preset>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&preset).map_err(|error| invalid_object(object, error))?;
    preset
        .reconcile_address(object.key().id())
        .map_err(|error| invalid_object(object, error))?;
    if preset.instance_id.is_none() {
        preset.instance_id = Some(uuid::Uuid::new_v4());
    }
    let after = serde_json::to_value(preset).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &after);
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
