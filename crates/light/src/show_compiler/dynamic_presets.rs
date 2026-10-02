use light_core::programming::ProgrammingOwner;
use light_dynamics::{
    DynamicDefinition, DynamicLaneBody, DynamicPresetFixtureTemplate, DynamicPresetGroupTemplate,
    DynamicPresetTemplate, DynamicRandomRange, DynamicValueSource, ProgrammingLaneConfiguration,
};
use light_programmer::Preset;
use std::{collections::HashMap, sync::Arc};
mod native;
pub(super) use native::NativeTemplateValidator;

/// Stage dependency retention before the live Preset disappears. Use the candidate
/// version when present, and the prior document for a delete/move. Normalized edits
/// must write dependent Dynamics even when only the Preset was explicitly touched.
pub(super) fn stage_retention(
    document: &light_show::PortableShowDocument,
    transaction: &mut light_show::PortableShowTransaction,
    preserved: Option<&light_show::PortableShowObjectKey>,
) -> Result<(), crate::ActionError> {
    if !transaction.fixture_profile_revisions_changed()
        && !transaction
            .changed_object_kinds()
            .any(|kind| matches!(kind, "preset" | "dynamic"))
    {
        return Ok(());
    }
    let mut presets = document
        .objects_of_kind("preset")
        .filter_map(|object| {
            serde_json::from_value::<Preset>(object.body().clone())
                .ok()
                .filter(|preset| preset.validate_programming().is_ok())
                .map(|preset| (object.key().id().to_owned(), preset))
        })
        .collect::<HashMap<_, _>>();
    let updates = {
        let candidate = document
            .candidate(transaction)
            .map_err(|error| super::invalid_candidate(error.to_string()))?;
        let native = NativeTemplateValidator::new(candidate);
        for object in candidate.objects_of_kind("preset") {
            if let Ok(preset) = serde_json::from_value::<Preset>(object.body().clone())
                && preset.validate_programming().is_ok()
            {
                presets.insert(object.key().id().to_owned(), preset);
            }
        }
        candidate
            .objects_of_kind("dynamic")
            .filter_map(|object| {
                if preserved.is_some_and(|key| key == object.key()) {
                    return None;
                }
                let mut dynamic =
                    serde_json::from_value::<DynamicDefinition>(object.body().clone()).ok()?;
                retain_templates(&mut dynamic, &presets, &|value| native.allows(value));
                let canonical = serde_json::to_value(dynamic).ok()?;
                let mut body = object.body().clone();
                copy_typed_source_retention(&canonical, &mut body);
                (body != *object.body()).then(|| (object.key().id().to_owned(), body))
            })
            .collect::<Vec<_>>()
    };
    for (id, body) in updates {
        transaction.put("dynamic", id, body);
    }
    Ok(())
}

/// Retain authored families, not fitted channels or a fixture-expanded snapshot.
pub(super) fn retain_templates(
    dynamic: &mut DynamicDefinition,
    presets: &HashMap<String, Preset>,
    verify_native: &dyn Fn(&light_core::AttributeValue) -> bool,
) {
    let mut cache = HashMap::<(String, ProgrammingOwner), Arc<DynamicPresetTemplate>>::new();
    let mut retain = |source: &mut DynamicValueSource| {
        let DynamicValueSource::Preset {
            preset_id,
            address,
            retained,
            ..
        } = source
        else {
            return;
        };
        let Some(preset) = presets.get(preset_id) else {
            return;
        };
        if preset.validate_programming().is_err() {
            return;
        }
        let key = (preset_id.clone(), address.owner());
        let template = cache
            .entry(key)
            .or_insert_with(|| Arc::new(capture(preset, address.owner())));
        // Invalid pool content stays repairable; a valid previous source is retained.
        if template.validate(address.owner()).is_ok() {
            let mut next = template.as_ref().clone();
            next.retain_fallback_verified(address, retained.as_deref(), verify_native);
            if retained.as_deref() != Some(&next) {
                *retained = Some(Arc::new(next));
            }
        }
    };
    for lane in &mut dynamic.lanes {
        let DynamicLaneBody::Programming(body) = &mut lane.body else {
            continue;
        };
        match &mut body.configuration {
            ProgrammingLaneConfiguration::Keyframes(config) => {
                for point in &mut config.points {
                    retain(&mut point.source);
                }
            }
            ProgrammingLaneConfiguration::MaxMin(config) => {
                retain(&mut config.minimum);
                retain(&mut config.maximum);
            }
            ProgrammingLaneConfiguration::MiddleAmplitude(config) => retain(&mut config.middle),
            ProgrammingLaneConfiguration::Random => {}
        }
    }
    for group in &mut dynamic.random_groups {
        if let DynamicRandomRange::Programming { low, high } = &mut group.range {
            retain(low);
            retain(high);
        }
    }
}

fn capture(preset: &Preset, owner: ProgrammingOwner) -> DynamicPresetTemplate {
    let key = owner.key();
    let mut groups = preset
        .group_values
        .iter()
        .filter_map(|(group_id, values)| {
            values.get(&key).map(|value| DynamicPresetGroupTemplate {
                group_id: group_id.clone(),
                value: value.clone(),
            })
        })
        .collect::<Vec<_>>();
    let mut fixtures = preset
        .values
        .iter()
        .filter_map(|(fixture_id, values)| {
            values.get(&key).map(|value| DynamicPresetFixtureTemplate {
                fixture_id: *fixture_id,
                value: value.clone(),
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    fixtures.sort_by_key(|value| value.fixture_id.0);
    DynamicPresetTemplate {
        universal: preset.universal_values.get(&key).cloned(),
        groups,
        fixtures,
        fallback: None,
    }
}

/// Patch only derived source retention fields. Unknown sibling JSON remains intact.
pub(super) fn copy_typed_source_retention(
    canonical: &serde_json::Value,
    stored: &mut serde_json::Value,
) {
    use serde_json::Value;
    match (canonical, stored) {
        (Value::Object(source), Value::Object(target)) => {
            if source.get("kind").and_then(Value::as_str) == Some("preset")
                && source.contains_key("address")
                && source.contains_key("last_valid_by_target")
            {
                for key in ["retained", "last_valid_by_target"] {
                    if let Some(value) = source.get(key) {
                        // An absent optional empty fallback is already canonical. Editing an
                        // unrelated Preset must not replace this source generation just to add [].
                        if key == "last_valid_by_target"
                            && !target.contains_key(key)
                            && value.as_array().is_some_and(Vec::is_empty)
                        {
                            continue;
                        }
                        target.insert(key.into(), value.clone());
                    }
                }
            } else {
                for (key, value) in source {
                    if let Some(target) = target.get_mut(key) {
                        copy_typed_source_retention(value, target);
                    }
                }
            }
        }
        (Value::Array(source), Value::Array(target)) => {
            for (source, target) in source.iter().zip(target) {
                copy_typed_source_retention(source, target);
            }
        }
        _ => {}
    }
}
