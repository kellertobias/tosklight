//! Scoped JSON migrations for durable Programmer state persisted by older desk versions.

/// Normalizes retired canonical identities in durable Programmer and Preload state. This stays a
/// scoped JSON migration instead of changing `AttributeKey` deserialization globally: fixture
/// profiles deliberately retain their fixture-facing source identity.
pub(super) fn migrate_retired_programmer_attributes(
    value: &mut serde_json::Value,
) -> Result<(), String> {
    let mut migrated = value.clone();
    migrate_retired_programmer_attributes_in_place(&mut migrated)?;
    *value = migrated;
    Ok(())
}

fn migrate_retired_programmer_attributes_in_place(
    value: &mut serde_json::Value,
) -> Result<(), String> {
    let Some(programmer) = value.as_object_mut() else {
        return Ok(());
    };
    for field in [
        "values",
        "dynamic_values",
        "preload_pending",
        "preload_active",
        "preload_dynamic_pending",
        "preload_dynamic_active",
    ] {
        if let Some(values) = programmer
            .get_mut(field)
            .and_then(serde_json::Value::as_array_mut)
        {
            migrate_programmer_value_records(values, field)?;
        }
    }
    for field in [
        "group_values",
        "preload_group_pending",
        "preload_group_active",
    ] {
        if let Some(groups) = programmer
            .get_mut(field)
            .and_then(serde_json::Value::as_object_mut)
        {
            for (group_id, attributes) in groups {
                let Some(attributes) = attributes.as_object_mut() else {
                    continue;
                };
                migrate_programmer_attribute_map(attributes, &format!("{field}/{group_id}"))?;
            }
        }
    }
    for history in ["undo", "redo"] {
        if let Some(snapshots) = programmer
            .get_mut(history)
            .and_then(serde_json::Value::as_array_mut)
        {
            for snapshot in snapshots {
                migrate_retired_programmer_attributes_in_place(snapshot)?;
            }
        }
    }
    migrate_embedded_programmer_attributes(value, "programmer")?;
    Ok(())
}

fn migrate_programmer_value_records(
    values: &mut [serde_json::Value],
    path: &str,
) -> Result<(), String> {
    let mut addresses = std::collections::HashMap::new();
    for (index, value) in values.iter().enumerate() {
        let Some(body) = value.as_object() else {
            continue;
        };
        let Some(fixture_id) = body.get("fixture_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(attribute) = body.get("attribute").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let canonical = canonical_migration(attribute).map_or(attribute, |migration| migration.0);
        let key = (fixture_id.to_owned(), canonical.to_owned());
        if let Some(previous) = addresses.insert(key, attribute.to_owned())
            && previous != attribute
        {
            return Err(format!(
                "attribute migration conflict at {path}/{index}: fixture {fixture_id} stores both legacy {attribute} and canonical {canonical} values"
            ));
        }
    }
    for value in values {
        let Some(attribute) = value
            .get("attribute")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some((canonical, transform)) = canonical_migration(&attribute) else {
            continue;
        };
        if let Some(stored) = value.get_mut("value") {
            if path.contains("dynamic") {
                migrate_programmer_dynamic_value(stored, transform, path)?;
            } else {
                migrate_programmer_attribute_value(stored, transform, path)?;
            }
        } else if transform == light_core::CanonicalAttributeTransform::InvertNormalized {
            return Err(format!(
                "attribute migration failed at {path}: value is missing"
            ));
        }
        value["attribute"] = serde_json::Value::String(canonical.into());
    }
    Ok(())
}

fn migrate_programmer_attribute_map(
    attributes: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), String> {
    let migrations = attributes
        .keys()
        .filter_map(|source| {
            canonical_migration(source)
                .map(|(target, transform)| (source.clone(), target, transform))
        })
        .collect::<Vec<_>>();
    for (source, target, _) in &migrations {
        if source != target && attributes.contains_key(*target) {
            return Err(format!(
                "attribute migration conflict at {path}: stored Group values contain both legacy {source} and canonical {target}"
            ));
        }
    }
    for (source, target, transform) in migrations {
        let mut stored = attributes
            .remove(&source)
            .expect("collected Programmer migration source remains present");
        if stored.get("kind").is_some() {
            migrate_programmer_attribute_value(&mut stored, transform, path)?;
        } else if let Some(value) = stored.get_mut("value") {
            migrate_programmer_attribute_value(value, transform, path)?;
        } else if transform == light_core::CanonicalAttributeTransform::InvertNormalized {
            return Err(format!(
                "attribute migration failed at {path}/{source}: Group value payload is missing"
            ));
        }
        attributes.insert(target.into(), stored);
    }
    Ok(())
}

fn migrate_programmer_dynamic_value(
    value: &mut serde_json::Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), String> {
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("static") => {
            let stored = value.get_mut("value").ok_or_else(|| {
                format!("attribute migration failed at {path}: static value is missing")
            })?;
            migrate_programmer_attribute_value(stored, transform, path)
        }
        Some("fix_at")
            if transform == light_core::CanonicalAttributeTransform::InvertNormalized =>
        {
            let stored = value
                .get("value")
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| {
                    format!("attribute migration failed at {path}: Fix At value must be a number")
                })?;
            value["value"] = serde_json::to_value(light_core::transform_canonical_normalized(
                stored as f32,
                transform,
            ))
            .map_err(|error| error.to_string())?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn migrate_programmer_attribute_value(
    value: &mut serde_json::Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), String> {
    if transform == light_core::CanonicalAttributeTransform::Identity {
        return Ok(());
    }
    let stored = value.clone();
    let mut typed = serde_json::from_value::<light_core::AttributeValue>(value.clone())
        .map_err(|error| format!("attribute migration failed at {path}: {error}"))?;
    let before = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
    light_core::transform_canonical_value(&mut typed, transform)
        .map_err(|error| format!("attribute migration failed at {path}: {error}"))?;
    let after = serde_json::to_value(typed).map_err(|error| error.to_string())?;
    *value = stored;
    light_application::lossless_json::apply_delta(value, &before, &after);
    Ok(())
}

fn migrate_embedded_programmer_attributes(
    value: &mut serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let looks_like_dynamic = value.get("target_binding").is_some() && value.get("lanes").is_some();
    if looks_like_dynamic
        && let Ok(mut definition) =
            serde_json::from_value::<light_dynamics::DynamicDefinition>(value.clone())
        && light_dynamics::validate_definition(&definition).is_ok()
    {
        let before = serde_json::to_value(&definition).map_err(|error| error.to_string())?;
        light_dynamics::migrate_canonical_attributes(&mut definition)?;
        let after = serde_json::to_value(definition).map_err(|error| error.to_string())?;
        light_application::lossless_json::apply_delta(value, &before, &after);
    }
    if let Some(body) = value.as_object_mut() {
        if body
            .get("attribute")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_legacy_strobe_attribute)
        {
            body.insert(
                "attribute".into(),
                serde_json::Value::String("shutter".into()),
            );
        }
        for (field, value) in body {
            migrate_embedded_programmer_attributes(value, &format!("{path}/{field}"))?;
        }
    } else if let Some(values) = value.as_array_mut() {
        for (index, value) in values.iter_mut().enumerate() {
            migrate_embedded_programmer_attributes(value, &format!("{path}/{index}"))?;
        }
    }
    Ok(())
}

fn is_legacy_strobe_attribute(attribute: &str) -> bool {
    matches!(
        light_core::canonical_attribute_migration_id(attribute),
        Some(("shutter", light_core::CanonicalAttributeTransform::Identity))
    )
}

fn canonical_migration(
    attribute: &str,
) -> Option<(&'static str, light_core::CanonicalAttributeTransform)> {
    light_core::canonical_attribute_migration_id(attribute)
}

/// Programmers persisted before the DEGRP rework may carry the removed `frozen_group` selection
/// expression (including inside undo/redo snapshots). Dereference it to the concrete fixtures the
/// selection already resolved to, matching current DEGRP semantics.
pub(super) fn migrate_frozen_group_selection(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let frozen = object
        .get("selection_expression")
        .and_then(|expression| expression.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("frozen_group");
    if frozen {
        let items = object
            .get("selected")
            .and_then(serde_json::Value::as_array)
            .map(|selected| {
                selected
                    .iter()
                    .map(|fixture_id| {
                        serde_json::json!({"type": "fixture", "fixture_id": fixture_id})
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        object.insert(
            "selection_expression".into(),
            serde_json::json!({"type": "sources", "items": items}),
        );
    }
    for history in ["undo", "redo"] {
        if let Some(snapshots) = object
            .get_mut(history)
            .and_then(serde_json::Value::as_array_mut)
        {
            for snapshot in snapshots {
                migrate_frozen_group_selection(snapshot);
            }
        }
    }
}
