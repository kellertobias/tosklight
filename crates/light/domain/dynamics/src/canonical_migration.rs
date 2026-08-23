use crate::{DynamicDefinition, DynamicKeyframe, ScalarSource, validate_definition};
use light_core::{
    AttributeKey, CanonicalAttributeTransform, canonical_attribute_migration_id,
    transform_canonical_normalized,
};
use std::collections::HashMap;

/// Migrates canonical attribute identities and values inside one valid Dynamic definition.
/// Invalid definitions remain repairable content and must be filtered by the caller before use.
pub fn migrate_canonical_attributes(definition: &mut DynamicDefinition) -> Result<bool, String> {
    validate_definition(definition).map_err(|error| error.to_string())?;
    let mut targets = HashMap::new();
    for lane in &definition.lanes {
        let source = &*lane.attribute.0;
        let target =
            canonical_attribute_migration_id(source).map_or(source, |migration| migration.0);
        if let Some(previous) = targets.insert(target.to_owned(), source.to_owned())
            && previous != source
        {
            return Err(format!(
                "Dynamic lanes store both legacy {source} and canonical {target} values"
            ));
        }
    }

    let random_groups = definition
        .random_groups
        .iter()
        .cloned()
        .map(|group| (group.id, group))
        .collect::<HashMap<_, _>>();
    let mut migrated_random_groups = HashMap::new();
    let mut additions = Vec::new();
    let mut changed = false;
    for lane in &mut definition.lanes {
        let Some((target, transform)) = canonical_attribute_migration_id(&lane.attribute.0) else {
            continue;
        };
        changed = true;
        let source = lane.attribute.clone();
        let target = AttributeKey(target.into());
        migrate_scalar_sources(&mut lane.keyframes.points, &source, &target, transform)?;
        migrate_scalar_source(&mut lane.max_min.minimum, &source, &target, transform)?;
        migrate_scalar_source(&mut lane.max_min.maximum, &source, &target, transform)?;
        migrate_scalar_source(
            &mut lane.middle_amplitude.middle,
            &source,
            &target,
            transform,
        )?;
        if transform == CanonicalAttributeTransform::InvertNormalized {
            lane.middle_amplitude.invert_waveform = !lane.middle_amplitude.invert_waveform;
        }
        if let Some(group_id) = lane.random_group_id {
            let key = (group_id, target.0.clone());
            let migrated_id = if let Some(id) = migrated_random_groups.get(&key) {
                *id
            } else {
                let mut group = random_groups.get(&group_id).cloned().ok_or_else(|| {
                    format!("Dynamic lane references missing random group {group_id}")
                })?;
                group.id =
                    uuid::Uuid::new_v5(&group.id, format!("canonical:{}", target.0).as_bytes());
                migrate_scalar_source(&mut group.low, &source, &target, transform)?;
                migrate_scalar_source(&mut group.high, &source, &target, transform)?;
                migrated_random_groups.insert(key, group.id);
                additions.push(group.clone());
                group.id
            };
            lane.random_group_id = Some(migrated_id);
        }
        lane.attribute = target;
    }
    definition.random_groups.extend(additions);
    Ok(changed)
}

fn migrate_scalar_sources(
    points: &mut [DynamicKeyframe],
    source: &AttributeKey,
    target: &AttributeKey,
    transform: CanonicalAttributeTransform,
) -> Result<(), String> {
    for point in points {
        migrate_scalar_source(&mut point.source, source, target, transform)?;
    }
    Ok(())
}

fn migrate_scalar_source(
    value: &mut ScalarSource,
    source: &AttributeKey,
    target: &AttributeKey,
    transform: CanonicalAttributeTransform,
) -> Result<(), String> {
    match value {
        ScalarSource::Current => Ok(()),
        ScalarSource::Value { value } => {
            *value = transform_canonical_normalized(*value, transform);
            Ok(())
        }
        ScalarSource::Preset {
            attribute,
            last_valid_by_target,
            ..
        } => {
            if attribute != source {
                return Err(format!(
                    "Dynamic Preset source {} does not match lane attribute {}",
                    attribute.0, source.0
                ));
            }
            *attribute = target.clone();
            for fallback in last_valid_by_target {
                fallback.value = transform_canonical_normalized(fallback.value, transform);
            }
            Ok(())
        }
    }
}
