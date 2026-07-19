use super::{
    fixtures::{FixtureIdentityCatalog, top_level_profile_reference},
    locations::{
        add_direct_array_at, add_direct_map_keys, add_direct_value, add_fixture_array,
        add_fixture_array_at, add_fixture_map_keys, add_fixture_value,
        add_optional_direct_reference, direct_reference, id_descriptor, key_only_descriptor,
        primary_identity, scalar_id, value_location,
    },
};
use crate::selective_import::{ImportIdentityFormat, ImportObjectDescriptor};
use light_show::PortableShowObject;
use serde_json::Value;

pub(super) fn group_descriptor(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = id_descriptor(object)?;
    add_fixture_array(object.body(), "/fixtures", source, target, &mut descriptor)?;
    for pointer in [
        "/derived_from/source_group_id",
        "/frozen_from/source_group_id",
    ] {
        add_optional_direct_reference(object.body(), pointer, "group", &mut descriptor)?;
    }
    Ok(descriptor)
}

pub(super) fn preset_descriptor(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = ImportObjectDescriptor {
        identities: vec![primary_identity(
            object,
            "/number",
            ImportIdentityFormat::NumericSuffix,
        )?],
        ..ImportObjectDescriptor::default()
    };
    add_fixture_map_keys(object.body(), "/values", source, target, &mut descriptor)?;
    add_direct_map_keys(object.body(), "/group_values", "group", &mut descriptor)?;
    Ok(descriptor)
}

pub(super) fn cue_list_descriptor(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = id_descriptor(object)?;
    let Some(cues) = object.body().pointer("/cues").and_then(Value::as_array) else {
        return Ok(descriptor);
    };
    for (cue_index, cue) in cues.iter().enumerate() {
        for (change_index, change) in cue
            .pointer("/changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            add_fixture_value(
                change,
                "/fixture_id",
                format!("/cues/{cue_index}/changes/{change_index}/fixture_id"),
                source,
                target,
                &mut descriptor,
            )?;
        }
        for (change_index, change) in cue
            .pointer("/group_changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            add_direct_value(
                change,
                "/group_id",
                format!("/cues/{cue_index}/group_changes/{change_index}/group_id"),
                "group",
                &mut descriptor,
            )?;
        }
        for (phaser_index, phaser) in cue
            .pointer("/phasers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            add_fixture_array_at(
                phaser,
                "/fixture_ids",
                &format!("/cues/{cue_index}/phasers/{phaser_index}/fixture_ids"),
                source,
                target,
                &mut descriptor,
            )?;
            add_direct_array_at(
                phaser,
                "/group_ids",
                &format!("/cues/{cue_index}/phasers/{phaser_index}/group_ids"),
                "group",
                &mut descriptor,
            )?;
        }
    }
    Ok(descriptor)
}

pub(super) fn playback_descriptor(
    object: &PortableShowObject,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = ImportObjectDescriptor {
        identities: vec![primary_identity(
            object,
            "/number",
            ImportIdentityFormat::Full,
        )?],
        ..ImportObjectDescriptor::default()
    };
    match object
        .body()
        .pointer("/target/type")
        .and_then(Value::as_str)
    {
        Some("cue_list") => add_optional_direct_reference(
            object.body(),
            "/target/cue_list_id",
            "cue_list",
            &mut descriptor,
        )?,
        Some("group") => add_optional_direct_reference(
            object.body(),
            "/target/group_id",
            "group",
            &mut descriptor,
        )?,
        _ => {}
    }
    Ok(descriptor)
}

pub(super) fn playback_page_descriptor(
    object: &PortableShowObject,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = ImportObjectDescriptor {
        identities: vec![primary_identity(
            object,
            "/number",
            ImportIdentityFormat::Full,
        )?],
        ..ImportObjectDescriptor::default()
    };
    let Some(slots) = object.body().pointer("/slots").and_then(Value::as_object) else {
        return Ok(descriptor);
    };
    for (slot, playback) in slots {
        let id = scalar_id(playback)
            .ok_or_else(|| format!("playback page slot {slot} is not an identity"))?;
        descriptor.references.push(direct_reference(
            "playback",
            id,
            value_location(
                format!("/slots/{}", slot.replace('~', "~0").replace('/', "~1")),
                ImportIdentityFormat::Full,
            ),
        ));
    }
    Ok(descriptor)
}

pub(super) fn stage_layout_descriptor(
    object: &PortableShowObject,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = key_only_descriptor(object);
    for pointer in ["/positions", "/positions3d"] {
        add_fixture_map_keys(object.body(), pointer, source, target, &mut descriptor)?;
    }
    Ok(descriptor)
}

pub(super) fn control_mapping_descriptor(
    object: &PortableShowObject,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = key_only_descriptor(object);
    let action = object
        .body()
        .pointer("/action/type")
        .and_then(Value::as_str);
    if matches!(
        action,
        Some("cue_go" | "cue_back" | "cue_pause" | "cue_release")
    ) {
        add_optional_direct_reference(
            object.body(),
            "/action/cue_list_id",
            "cue_list",
            &mut descriptor,
        )?;
    }
    Ok(descriptor)
}

pub(super) fn dependency_list_descriptor(
    object: &PortableShowObject,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = id_descriptor(object)?;
    add_optional_direct_reference(object.body(), "/macro_id", "macro", &mut descriptor)?;
    let Some(dependencies) = object
        .body()
        .pointer("/dependencies")
        .and_then(Value::as_array)
    else {
        return Ok(descriptor);
    };
    for (index, dependency) in dependencies.iter().enumerate() {
        let kind = dependency
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("dependency {index} has no kind"))?;
        let id = dependency
            .get("id")
            .and_then(scalar_id)
            .ok_or_else(|| format!("dependency {index} has no id"))?;
        descriptor.references.push(direct_reference(
            kind,
            id,
            value_location(
                format!("/dependencies/{index}/id"),
                ImportIdentityFormat::Full,
            ),
        ));
    }
    Ok(descriptor)
}

pub(super) fn effect_descriptor(
    object: &PortableShowObject,
) -> Result<ImportObjectDescriptor, String> {
    let mut descriptor = id_descriptor(object)?;
    if let Some(reference) = top_level_profile_reference(object.body())? {
        descriptor.profile_references.push(reference);
    }
    Ok(descriptor)
}
