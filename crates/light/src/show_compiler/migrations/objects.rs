use super::{ObjectUpdate, invalid_object};
use crate::{ActionError, lossless_json};
use light_output::OutputRoute;
use light_playback::{CueList, PlaybackDefinition, PlaybackTarget};
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowCandidate, PortableShowCandidateObject};
use serde_json::{Map, Value, json};

const LEGACY_SPEED_GROUPS_BPM: [f64; 5] = [120.0, 90.0, 60.0, 30.0, 15.0];

pub(super) fn collect(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<ObjectUpdate>, ActionError> {
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
    let group_master_levels = reconcile_group_master_levels(candidate)?;
    candidate
        .objects()
        .filter_map(|object| {
            if object.key().kind() == "dynamic" {
                migrate_dynamic_fallbacks(object, &presets, &groups).transpose()
            } else {
                migrate_with_group_masters(object, &group_master_levels).transpose()
            }
        })
        .collect()
}

fn migrate_dynamic_fallbacks(
    object: PortableShowCandidateObject<'_>,
    presets: &std::collections::HashMap<String, Preset>,
    groups: &std::collections::HashMap<String, GroupDefinition>,
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
    super::super::objects::hydrate_dynamic_preset_fallbacks(&mut definition, presets, groups);
    let canonical =
        serde_json::to_value(definition).map_err(|error| invalid_object(object, error))?;
    copy_dynamic_fallbacks(&canonical, &mut migrated);
    migrate_embedded_dynamic_phase_orderings(&mut migrated);
    Ok((migrated != *object.body()).then(|| ObjectUpdate::from_object(object, migrated)))
}

fn copy_dynamic_fallbacks(canonical: &Value, migrated: &mut Value) {
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
    migrate_with_group_masters(object, &std::collections::HashMap::new())
}

fn migrate_with_group_masters(
    object: PortableShowCandidateObject<'_>,
    group_master_levels: &std::collections::HashMap<String, f32>,
) -> Result<Option<ObjectUpdate>, ActionError> {
    let mut migrated = match object.key().kind() {
        "attribute_configuration" => object.body().clone(),
        "cue_list" => migrate_cue_list(object)?,
        "dynamic" => object.body().clone(),
        "group" => migrate_group(object)?,
        "playback" => migrate_playback(object, group_master_levels)?,
        "playback_page" => migrate_playback_page(object, group_master_levels)?,
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
        && value.get("spatial_mapping").is_none()
        && let Ok(definition) =
            serde_json::from_value::<light_dynamics::DynamicDefinition>(value.clone())
        && let Ok(canonical) = serde_json::to_value(definition)
        && let Some(mapping) = canonical.get("spatial_mapping")
        && let Some(body) = value.as_object_mut()
    {
        body.insert("spatial_mapping".into(), mapping.clone());
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
    for cue in cues.iter_mut() {
        if let Some(cue) = cue.as_object_mut() {
            cue.remove("phasers");
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
    body.remove("master");
    body.remove("playback_fader");
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
    let after = serde_json::to_value(preset).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &after);
    Ok(migrated)
}

fn migrate_playback(
    object: PortableShowCandidateObject<'_>,
    group_master_levels: &std::collections::HashMap<String, f32>,
) -> Result<Value, ActionError> {
    let mut playback = serde_json::from_value::<PlaybackDefinition>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&playback).map_err(|error| invalid_object(object, error))?;
    let playback_number = playback.number;
    migrate_targetless_dynamic_assignment(
        &mut playback,
        DynamicAssignmentIdentity::Physical(playback_number),
    )
    .map_err(|error| invalid_object(object, error))?;
    reconcile_playback_group_master(&mut playback, group_master_levels);
    let canonical =
        serde_json::to_value(playback).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &canonical);
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
        "footprint",
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

fn migrate_playback_page(
    object: PortableShowCandidateObject<'_>,
    group_master_levels: &std::collections::HashMap<String, f32>,
) -> Result<Value, ActionError> {
    let mut page = serde_json::from_value::<light_playback::PlaybackPage>(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let before = serde_json::to_value(&page).map_err(|error| invalid_object(object, error))?;
    let page_number = page.number;
    for (number, playback) in &mut page.virtual_playbacks {
        migrate_targetless_dynamic_assignment(
            playback,
            DynamicAssignmentIdentity::Virtual {
                page: page_number,
                number: *number,
            },
        )
        .map_err(|error| invalid_object(object, error))?;
        reconcile_playback_group_master(playback, group_master_levels);
    }
    let after = serde_json::to_value(page).map_err(|error| invalid_object(object, error))?;
    let mut migrated = object.body().clone();
    lossless_json::apply_delta(&mut migrated, &before, &after);
    Ok(migrated)
}

fn reconcile_playback_group_master(
    playback: &mut PlaybackDefinition,
    levels: &std::collections::HashMap<String, f32>,
) {
    let PlaybackTarget::Group {
        group_id,
        initial_master,
    } = &mut playback.target
    else {
        return;
    };
    if let Some(level) = levels.get(group_id) {
        *initial_master = Some(*level);
    }
}

/// Choose one portable compatibility seed per assigned Group.
///
/// Explicit Playback-owned seeds win over legacy Group-owned values. When several assignments
/// disagree, the lowest physical Playback number wins; virtual assignments follow, ordered by
/// page and then number. The chosen value is subsequently written into every assignment for that
/// Group so transfer to a desk without an output-runtime sidecar remains deterministic.
fn reconcile_group_master_levels(
    candidate: PortableShowCandidate<'_>,
) -> Result<std::collections::HashMap<String, f32>, ActionError> {
    let mut assignments = Vec::new();
    for object in candidate.objects_of_kind("playback") {
        let playback = serde_json::from_value::<PlaybackDefinition>(object.body().clone())
            .map_err(|error| invalid_object(object, error))?;
        if let PlaybackTarget::Group {
            group_id,
            initial_master,
        } = playback.target
        {
            assignments.push(((0_u8, 0_u8, playback.number), group_id, initial_master));
        }
    }
    for object in candidate.objects_of_kind("playback_page") {
        let page = serde_json::from_value::<light_playback::PlaybackPage>(object.body().clone())
            .map_err(|error| invalid_object(object, error))?;
        for playback in page.virtual_playbacks.into_values() {
            if let PlaybackTarget::Group {
                group_id,
                initial_master,
            } = playback.target
            {
                assignments.push((
                    (1_u8, page.number, playback.number),
                    group_id,
                    initial_master,
                ));
            }
        }
    }
    assignments.sort_by_key(|(address, _, _)| *address);

    let mut levels = std::collections::HashMap::new();
    let mut assigned_groups = std::collections::HashSet::new();
    for (_, group_id, initial_master) in &assignments {
        assigned_groups.insert(group_id.clone());
        if let Some(initial_master) = initial_master {
            if let Some(chosen) = levels.get(group_id) {
                if chosen != initial_master {
                    tracing::warn!(
                        %group_id,
                        chosen_level = *chosen,
                        ignored_level = *initial_master,
                        "reconciled divergent portable Group Master assignment levels by stable Playback address"
                    );
                }
            } else {
                levels.insert(group_id.clone(), *initial_master);
            }
        }
    }
    for object in candidate.objects_of_kind("group") {
        let group_id = object.key().id();
        if !assigned_groups.contains(group_id) {
            continue;
        }
        if let Some(value) = object.body().get("master") {
            let Some(level) = value.as_f64() else {
                tracing::warn!(
                    %group_id,
                    "ignored malformed retired Group-owned master during Playback-owned migration"
                );
                continue;
            };
            if !level.is_finite() || !(0.0..=1.0).contains(&level) {
                tracing::warn!(
                    %group_id,
                    "ignored out-of-range retired Group-owned master during Playback-owned migration"
                );
                continue;
            }
            let level = level as f32;
            if let Some(chosen) = levels.get(group_id) {
                if *chosen != level {
                    tracing::warn!(
                        %group_id,
                        chosen_level = *chosen,
                        ignored_legacy_level = level,
                        "reconciled a legacy Group-owned master against Playback-owned state"
                    );
                }
            } else {
                levels.insert(group_id.to_owned(), level);
            }
        }
    }
    Ok(levels)
}

#[derive(Clone, Copy)]
enum DynamicAssignmentIdentity {
    Physical(u16),
    Virtual { page: u8, number: u16 },
}

fn migrate_targetless_dynamic_assignment(
    playback: &mut PlaybackDefinition,
    identity: DynamicAssignmentIdentity,
) -> Result<bool, String> {
    let light_playback::PlaybackTarget::Dynamic { assignment } = &mut playback.target else {
        return Ok(false);
    };
    if !matches!(
        assignment
            .dynamic
            .embedded_fallback
            .definition
            .target_binding,
        light_dynamics::DynamicTargetBinding::Targetless
    ) {
        return Ok(false);
    }
    let Some(scope) = assignment.target_scope.take() else {
        return Err(
            "legacy targetless Dynamic Playback assignment has no stored target scope".into(),
        );
    };
    let target_binding = match scope {
        light_playback::DynamicPlaybackTargetScope::LiveGroup { group_id } => {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id }
        }
        light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets } => {
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets }
        }
    };
    let original = &assignment.dynamic.embedded_fallback.definition;
    let mut migrated = original.as_ref().clone();
    let name = match identity {
        DynamicAssignmentIdentity::Physical(number) => format!("physical:{number}"),
        DynamicAssignmentIdentity::Virtual { page, number } => {
            format!("virtual:{page}:{number}")
        }
    };
    migrated.id = uuid::Uuid::new_v5(&original.id, name.as_bytes());
    migrated.target_binding = target_binding;
    assignment.dynamic.dynamic_id = None;
    assignment.dynamic.embedded_fallback.definition = std::sync::Arc::new(migrated);
    Ok(true)
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
