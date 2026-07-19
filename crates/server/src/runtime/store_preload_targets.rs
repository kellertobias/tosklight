use super::*;

type PreloadGroupValues =
    HashMap<String, HashMap<light_core::AttributeKey, light_programmer::GroupProgrammerValue>>;

pub(super) struct PreparedPreloadPreset {
    pub(super) object_id: String,
    pub(super) body: serde_json::Value,
}

pub(super) fn prepare_preload_preset(
    store: &ShowStore,
    input: &PreloadStoreInput,
    fixtures: &[light_core::TimedValue],
    groups: &PreloadGroupValues,
) -> Result<PreparedPreloadPreset, ApiError> {
    let hinted_family = input.family.unwrap_or_else(|| {
        command_preset_family(&input.target_id).unwrap_or(light_programmer::PresetFamily::Mixed)
    });
    let address =
        light_programmer::PresetAddress::from_storage_key(&input.target_id, hinted_family)
            .map_err(ApiError::bad_request)?;
    if input.family.is_some_and(|family| family != address.family) {
        return Err(ApiError::bad_request(
            "preset family does not match its pool address",
        ));
    }
    let storage_key = address.storage_key();
    let mut preset = light_programmer::Preset {
        name: input
            .name
            .clone()
            .unwrap_or_else(|| format!("Preset {}", address.number)),
        family: address.family,
        number: address.number,
        ..Default::default()
    };
    for value in fixtures {
        preset
            .values
            .entry(value.fixture_id)
            .or_default()
            .insert(value.attribute.clone(), value.value.clone());
    }
    for (group_id, pending) in groups {
        let attributes = preset.group_values.entry(group_id.clone()).or_default();
        for (attribute, scoped) in pending {
            attributes.insert(attribute.clone(), scoped.value.clone());
        }
    }
    preset.retain_family_attributes();
    let existing = store
        .objects("preset")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == storage_key);
    let had_existing = existing.is_some();
    let mut merged = existing
        .as_ref()
        .map(decode_preset_object)
        .transpose()
        .map_err(ApiError::bad_request)?
        .map(|(_, preset)| preset)
        .unwrap_or_else(|| light_programmer::Preset {
            family: address.family,
            number: address.number,
            ..Default::default()
        });
    if input.family.is_none() && had_existing {
        preset.family = merged.family;
    }
    merged.store(
        preset,
        input
            .mode
            .unwrap_or(light_programmer::PresetStoreMode::Merge),
    );
    let body = existing
        .as_ref()
        .map_or_else(
            || serde_json::to_value(&merged),
            |object| serialize_preset_preserving_extensions(&object.body, &merged),
        )
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(PreparedPreloadPreset {
        object_id: storage_key,
        body,
    })
}

pub(super) fn store_preload_cue(
    store: &ShowStore,
    input: &PreloadStoreInput,
    fixtures: &[light_core::TimedValue],
    groups: &PreloadGroupValues,
    expected: u64,
) -> Result<u64, ApiError> {
    let object = store
        .objects("cue_list")
        .map_err(ApiError::store)?
        .into_iter()
        .find(|object| object.id == input.target_id)
        .ok_or_else(|| ApiError::not_found("Cuelist"))?;
    let mut cue_list: light_playback::CueList = serde_json::from_value(object.body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let number = input
        .cue_number
        .ok_or_else(|| ApiError::bad_request("cue_number is required for cue storage"))?;
    let index = cue_list
        .cues
        .iter()
        .position(|cue| cue.number == number)
        .unwrap_or_else(|| {
            cue_list.cues.push(light_playback::Cue::new(number));
            cue_list
                .cues
                .sort_by(|left, right| left.number.total_cmp(&right.number));
            cue_list
                .cues
                .iter()
                .position(|cue| cue.number == number)
                .expect("inserted cue exists")
        });
    let cue = &mut cue_list.cues[index];
    if let Some(name) = &input.name {
        cue.name.clone_from(name);
    }
    for value in fixtures {
        cue.changes.retain(|change| {
            change.fixture_id != value.fixture_id || change.attribute != value.attribute
        });
        cue.changes.push(light_playback::CueChange::set(
            value.fixture_id,
            value.attribute.clone(),
            value.value.clone(),
        ));
    }
    for (group_id, pending) in groups {
        for (attribute, scoped) in pending {
            cue.group_changes
                .retain(|change| change.group_id != *group_id || change.attribute != *attribute);
            cue.group_changes.push(light_playback::GroupCueChange {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: Some(scoped.value.clone()),
                automatic_restore: false,
                fade_millis: scoped.fade_millis,
                delay_millis: scoped.delay_millis,
            });
        }
    }
    store
        .put_object(
            "cue_list",
            &input.target_id,
            &serde_json::to_value(cue_list)
                .map_err(|error| ApiError::internal(error.to_string()))?,
            expected,
        )
        .map_err(ApiError::store)
}
