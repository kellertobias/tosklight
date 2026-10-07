use super::*;

/// Return the same normalized first command token used by execution, after removing valid timing
/// clauses. Transport adapters use this to enforce capability ownership without maintaining a
/// second, subtly different command parser.
pub(super) fn normalized_programmer_command_family(
    command_line: &str,
) -> Result<Option<String>, String> {
    tokenize_programmer_command(command_line).map(|(tokens, _)| tokens.into_iter().next())
}

pub(super) fn active_show_store(
    state: &AppState,
) -> Result<(ShowEntry, ActiveShowRepository), String> {
    let entry = state
        .active_show
        .current()
        .clone()
        .ok_or("no active show is loaded")?;
    // Show recovery: the command line neither reads nor writes the show that failed to load.
    state
        .active_show
        .ensure_content_writable()
        .map_err(|error| error.message)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(|error| error.to_string())?;
    Ok((entry, store))
}

pub(super) fn emit_command_object_changed(
    state: &AppState,
    entry: &ShowEntry,
    kind: &str,
    id: &str,
    revision: u64,
) {
    emit(
        state,
        "show_object_changed",
        serde_json::json!({"show_id":entry.id,"kind":kind,"id":id,"revision":revision}),
    );
}

pub(super) fn decode_preset_object(
    object: &light_show::VersionedObject,
) -> Result<(light_programmer::PresetAddress, light_programmer::Preset), String> {
    let mut preset: light_programmer::Preset = serde_json::from_value(object.body.clone())
        .map_err(|error| format!("invalid stored preset: {error}"))?;
    let address = preset.reconcile_address(&object.id)?;
    Ok((address, preset))
}

pub(super) fn load_command_preset(
    state: &AppState,
    requested_address: light_programmer::PresetAddress,
) -> Result<light_programmer::Preset, String> {
    let (_, store) = active_show_store(state)?;
    let requested = requested_address.storage_key();
    let object = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == requested
                || decode_preset_object(object)
                    .is_ok_and(|(address, _)| address == requested_address)
        })
        .ok_or_else(|| format!("preset {requested} does not exist"))?;
    decode_preset_object(&object).map(|(_, preset)| preset)
}

pub(super) fn serialize_preset_preserving_extensions(
    original: &serde_json::Value,
    preset: &light_programmer::Preset,
) -> Result<serde_json::Value, serde_json::Error> {
    let before = serde_json::from_value::<light_programmer::Preset>(original.clone())?;
    light_application::lossless_json::merge_typed(original, &before, preset)
}

pub(super) fn apply_command_preset(
    state: &AppState,
    session: &Session,
    id: &str,
    selected: &[light_core::FixtureId],
) -> Result<(), String> {
    let (_, store) = active_show_store(state)?;
    let requested_address = light_programmer::PresetAddress::parse(id)?;
    let object = store
        .objects("preset")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.id == id
                || decode_preset_object(object)
                    .is_ok_and(|(address, _)| address == requested_address)
        })
        .ok_or_else(|| format!("preset {id} does not exist"))?;
    let (_, stored_preset) = decode_preset_object(&object)?;
    let preset = super::programmer_aim_command::resolve_aim_preset(state, &stored_preset)?;
    let groups = state
        .output
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let current_expression = state
        .programming
        .get(session.id)
        .and_then(|programmer| programmer.selection_expression);
    let programmer_fade_millis = state.installation.configuration().programmer_fade_millis;
    let live_group_targets = current_expression
        .as_ref()
        .map(light_programmer::SelectionExpression::live_group_owners)
        .unwrap_or_default();
    if !command_preset_has_values(
        &preset,
        selected,
        &live_group_targets,
        &groups,
        state.output.supported_programming_contract(),
    )? {
        return Ok(());
    }
    let has_position_intent = preset_has_position_intent(&preset);
    if (preset.family == light_programmer::PresetFamily::Position || has_position_intent)
        && state.output.supported_programming_contract()
            >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
    {
        return apply_position_preset(
            state,
            session,
            id,
            selected,
            &preset,
            &groups,
            programmer_fade_millis,
        );
    }
    apply_selected_fixture_values(
        state,
        session,
        selected,
        &preset,
        &groups,
        &live_group_targets,
        programmer_fade_millis,
    );
    apply_live_group_values(
        state,
        session,
        &preset,
        live_group_targets,
        programmer_fade_millis,
    );
    state.programming.set_modes(
        session.id,
        None,
        None,
        None,
        Some(Some(format!("preset:{id}"))),
    );
    Ok(())
}

fn preset_has_position_intent(preset: &light_programmer::Preset) -> bool {
    preset
        .values
        .values()
        .chain(preset.group_values.values())
        .chain(std::iter::once(&preset.universal_values))
        .flat_map(|values| values.values())
        .any(|value| {
            value.programming_owner() == Some(light_core::programming::ProgrammingOwner::Position)
        })
}

/// Applies a Position (or position-owning) preset through the semantic selection planner.
fn apply_position_preset(
    state: &AppState,
    session: &Session,
    id: &str,
    selected: &[light_core::FixtureId],
    preset: &light_programmer::Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    programmer_fade_millis: u64,
) -> Result<(), String> {
    let mut selection = state
        .programming
        .selection(session.id)
        .ok_or("programmer does not exist")?;
    selection.selected = selected.to_vec();
    let snapshot = state.output.snapshot();
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(id, point)| {
            (
                *id,
                light_dynamics::Position3d {
                    x: f64::from(point.x),
                    y: f64::from(point.y),
                    z: f64::from(point.z),
                },
            )
        })
        .collect();
    let mutations = light_application::plan_preset_selection_values(
        &selection,
        preset,
        groups,
        &positions,
        programmer_fade_millis,
    )
    .map_err(|error| error.message)?;
    if mutations.is_empty() {
        return Ok(());
    }
    super::programmer_aim_command::apply_position_mutations(
        state,
        session,
        &mutations,
        Some(format!("preset:{id}")),
    )
}

fn apply_selected_fixture_values(
    state: &AppState,
    session: &Session,
    selected: &[light_core::FixtureId],
    preset: &light_programmer::Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    live_group_targets: &[String],
    programmer_fade_millis: u64,
) {
    for fixture in selected {
        // A universal colour reaches every selected fixture; the preset's own per-fixture and
        // Group values follow and win.
        if live_group_targets.is_empty() {
            for (attribute, value) in &preset.universal_values {
                state.programming.set_faded_with_timing(
                    session.id,
                    *fixture,
                    attribute.clone(),
                    value.clone(),
                    Some(programmer_fade_millis),
                    None,
                );
            }
        }
        if let Some(attributes) = preset.values.get(fixture) {
            for (attribute, value) in attributes {
                state.programming.set_faded_with_timing(
                    session.id,
                    *fixture,
                    attribute.clone(),
                    value.clone(),
                    Some(programmer_fade_millis),
                    None,
                );
            }
        }
        for (group_id, attributes) in preset
            .group_values
            .iter()
            .filter(|(group_id, _)| !live_group_targets.contains(group_id))
        {
            if light_programmer::resolve_group(group_id, groups)
                .is_ok_and(|members| members.contains(fixture))
            {
                for (attribute, value) in attributes {
                    state.programming.set_faded_with_timing(
                        session.id,
                        *fixture,
                        attribute.clone(),
                        value.clone(),
                        Some(programmer_fade_millis),
                        None,
                    );
                }
            }
        }
    }
}

fn apply_live_group_values(
    state: &AppState,
    session: &Session,
    preset: &light_programmer::Preset,
    live_group_targets: Vec<String>,
    programmer_fade_millis: u64,
) {
    for group_id in live_group_targets {
        for (attribute, value) in &preset.universal_values {
            state.programming.set_group_faded_with_timing(
                session.id,
                group_id.clone(),
                attribute.clone(),
                value.clone(),
                Some(programmer_fade_millis),
                None,
            );
        }
        let Some(attributes) = preset.group_values.get(&group_id) else {
            continue;
        };
        for (attribute, value) in attributes {
            state.programming.set_group_faded_with_timing(
                session.id,
                group_id.clone(),
                attribute.clone(),
                value.clone(),
                Some(programmer_fade_millis),
                None,
            );
        }
    }
}

pub(super) fn command_preset_address(
    tokens: &[String],
) -> Result<light_programmer::PresetAddress, String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <preset-type> . <preset-number>".into());
    }
    light_programmer::PresetAddress::parse(&format!("{}.{}", tokens[0], tokens[2]))
}

pub(super) fn command_preset_family(id: &str) -> Result<light_programmer::PresetFamily, String> {
    Ok(light_programmer::PresetAddress::parse(id)?.family)
}

/// Applicability is planned before explicit-address commands mutate selection.
pub(super) fn command_preset_has_values(
    preset: &light_programmer::Preset,
    selected: &[light_core::FixtureId],
    live_groups: &[String],
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    supported_contract: u16,
) -> Result<bool, String> {
    let mut applicable = false;
    let mut inspect = |value: &light_core::AttributeValue| -> Result<(), String> {
        let required = value.required_programming_contract();
        if required > supported_contract {
            return Err(format!(
                "Preset requires programming contract {required}; this runtime supports {supported_contract}"
            ));
        }
        applicable = true;
        Ok(())
    };
    if !selected.is_empty() || live_groups.iter().any(|id| groups.contains_key(id)) {
        for value in preset.universal_values.values() {
            inspect(value)?;
        }
    }
    for fixture in selected {
        if let Some(values) = preset.values.get(fixture) {
            for value in values.values() {
                inspect(value)?;
            }
        }
    }
    for (group_id, values) in &preset.group_values {
        let reaches_target = if live_groups.contains(group_id) {
            groups.contains_key(group_id)
        } else {
            light_programmer::resolve_group(group_id, groups)
                .is_ok_and(|members| selected.iter().any(|fixture| members.contains(fixture)))
        };
        if reaches_target {
            for value in values.values() {
                inspect(value)?;
            }
        }
    }
    Ok(applicable)
}
