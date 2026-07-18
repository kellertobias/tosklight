use super::*;

#[derive(Clone, Debug)]
pub(super) struct GeneratedProfilePreset {
    pub(super) semantic_id: String,
    pub(super) name: String,
    pub(super) family: String,
    pub(super) values: HashMap<
        light_core::FixtureId,
        HashMap<light_core::AttributeKey, light_core::AttributeValue>,
    >,
}

pub(super) fn generated_profile_preset_family(
    attribute: &light_core::AttributeKey,
) -> &'static str {
    match light_core::attribute_descriptor(attribute).family {
        light_core::AttributeClass::Intensity => "Intensity",
        light_core::AttributeClass::Position => "Position",
        light_core::AttributeClass::Color => "Color",
        light_core::AttributeClass::Beam | light_core::AttributeClass::Focus => "Beam",
        light_core::AttributeClass::Control | light_core::AttributeClass::Custom => "Mixed",
    }
}

pub(super) fn generated_profile_presets(
    snapshot: &EngineSnapshot,
    selected: &HashSet<light_core::FixtureId>,
) -> Result<Vec<GeneratedProfilePreset>, String> {
    let mut generated = BTreeMap::<(String, String), GeneratedProfilePreset>::new();
    for fixture in &snapshot.fixtures {
        let physical_selected = selected.contains(&fixture.fixture_id);
        if !physical_selected
            && !fixture
                .logical_heads
                .iter()
                .any(|head| selected.contains(&head.fixture_id))
        {
            continue;
        }
        let Some(profile) = fixture.definition.profile_snapshot.as_deref() else {
            continue;
        };
        let Some(mode) = fixture
            .definition
            .mode_id
            .and_then(|mode_id| profile.mode(mode_id))
        else {
            continue;
        };
        for channel in &mode.channels {
            let owner = profile_head_owner(fixture, mode, channel.head_id)?;
            if !physical_selected && !selected.contains(&owner) {
                continue;
            }
            for function in &channel.functions {
                let (semantic_id, label) = match &function.behavior {
                    light_fixture::ChannelFunctionBehavior::Fixed {
                        semantic_id, label, ..
                    }
                    | light_fixture::ChannelFunctionBehavior::Indexed {
                        semantic_id, label, ..
                    } => (semantic_id, label),
                    _ => continue,
                };
                let family = generated_profile_preset_family(&function.attribute).to_owned();
                let preset = generated
                    .entry((family.clone(), semantic_id.clone()))
                    .or_insert_with(|| GeneratedProfilePreset {
                        semantic_id: semantic_id.clone(),
                        name: label.clone(),
                        family,
                        values: HashMap::new(),
                    });
                if label < &preset.name {
                    preset.name.clone_from(label);
                }
                preset.values.entry(owner).or_default().insert(
                    function.attribute.clone(),
                    light_core::AttributeValue::Discrete(semantic_id.clone()),
                );
            }
        }
    }
    Ok(generated.into_values().collect())
}

pub(super) fn generate_profile_presets(
    state: &AppState,
    fixture_ids: Vec<light_core::FixtureId>,
) -> Result<serde_json::Value, String> {
    if fixture_ids.is_empty() {
        return Err("select at least one fixture before generating presets".into());
    }
    let generated =
        generated_profile_presets(&state.engine.snapshot(), &fixture_ids.into_iter().collect())?;
    if generated.is_empty() {
        return Err("the selected fixtures have no fixed or indexed values".into());
    }
    let (entry, store) = active_show_store(state)?;
    let existing = store.objects("preset").map_err(|error| error.to_string())?;
    let mut used = HashMap::<light_programmer::PresetFamily, HashSet<u32>>::new();
    for object in &existing {
        let (address, _) = decode_preset_object(object)?;
        used.entry(address.family)
            .or_default()
            .insert(address.number);
    }
    let mut ids = Vec::with_capacity(generated.len());
    let mut bodies = Vec::with_capacity(generated.len());
    let mut created = Vec::with_capacity(generated.len());
    for preset in generated {
        let family: light_programmer::PresetFamily =
            serde_json::from_value(serde_json::Value::String(preset.family.clone()))
                .map_err(|error| format!("invalid generated preset family: {error}"))?;
        let family_used = used.entry(family).or_default();
        let mut number = 1_u32;
        while family_used.contains(&number) {
            number += 1;
        }
        let address = light_programmer::PresetAddress::new(family, number)?;
        let storage_key = address.storage_key();
        family_used.insert(number);
        let mut body = serde_json::to_value(light_programmer::Preset {
            name: preset.name.clone(),
            family,
            number,
            values: preset.values,
            group_values: HashMap::new(),
        })
        .map_err(|error| error.to_string())?;
        body["generated_from_fixture_profile"] = serde_json::json!({
            "semantic_id":preset.semantic_id,
        });
        created.push(serde_json::json!({
            "address":address,
            "number":number,
            "name":preset.name,
            "family":preset.family,
        }));
        ids.push(storage_key);
        bodies.push(body);
    }
    let writes = ids
        .iter()
        .zip(&bodies)
        .map(|(id, body)| AtomicObjectWrite {
            kind: "preset",
            id,
            body,
            expected: 0,
        })
        .collect::<Vec<_>>();
    backup_show(state, &entry).map_err(|error| error.message)?;
    let revisions = store
        .mutate_objects_atomically(&writes, &[])
        .map_err(|error| error.to_string())?;
    refresh_command_show(state, &entry)?;
    for ((id, revision), item) in ids.iter().zip(revisions).zip(&created) {
        emit_command_object_changed(state, &entry, "preset", id, revision);
        emit(
            state,
            "preset_generated",
            serde_json::json!({"show_id":entry.id,"preset":item,"revision":revision}),
        );
    }
    Ok(serde_json::json!({"created":created}))
}
