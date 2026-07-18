use super::*;

type SchemaUpdate = (String, String, serde_json::Value, u64);

fn fixture_schema_updates(store: &ShowStore) -> Result<Vec<SchemaUpdate>, String> {
    let fixture_objects = store
        .objects("patched_fixture")
        .map_err(|error| error.to_string())?;
    let all_fixture_numbers_missing = !fixture_objects.is_empty()
        && fixture_objects.iter().all(|object| {
            object
                .body
                .get("fixture_number")
                .and_then(serde_json::Value::as_u64)
                .is_none()
                && object
                    .body
                    .get("virtual_fixture_number")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
        });
    let mut inferred_fixture_numbers = HashMap::new();
    if all_fixture_numbers_missing {
        let mut candidates = fixture_objects
            .iter()
            .map(|object| {
                (
                    object.id.clone(),
                    object
                        .body
                        .get("universe")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(u64::MAX),
                    object
                        .body
                        .get("address")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(u64::MAX),
                    object
                        .body
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
            .collect::<Vec<_>>();
        candidates
            .sort_by(|left, right| (left.1, left.2, &left.0).cmp(&(right.1, right.2, &right.0)));
        let mut used = std::collections::BTreeSet::new();
        for (id, _, _, name) in &candidates {
            if let Some(number) = default_show::default_fixture_number(name)
                && used.insert(number)
            {
                inferred_fixture_numbers.insert(id.clone(), number);
            }
        }
        let mut next = 1_u32;
        for (id, _, _, _) in &candidates {
            if inferred_fixture_numbers.contains_key(id) {
                continue;
            }
            while used.contains(&next) {
                next += 1;
            }
            inferred_fixture_numbers.insert(id.clone(), next);
            used.insert(next);
            next += 1;
        }
    }

    let mut used_virtual_fixture_numbers = fixture_objects
        .iter()
        .filter_map(|object| {
            object
                .body
                .get("virtual_fixture_number")
                .and_then(serde_json::Value::as_u64)
                .and_then(|number| u32::try_from(number).ok())
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut next_virtual_fixture_number = 1_u32;
    let mut updates = Vec::<(String, String, serde_json::Value, u64)>::new();
    for object in fixture_objects {
        let original = object.body;
        let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(original.clone())
            .map_err(|error| format!("invalid patched fixture: {error}"))?;
        if all_fixture_numbers_missing {
            fixture.fixture_number = inferred_fixture_numbers.get(&object.id).copied();
        }
        if !fixture.definition.is_dmx_patchable() {
            fixture.fixture_number = None;
            if fixture.virtual_fixture_number.is_none() {
                while used_virtual_fixture_numbers.contains(&next_virtual_fixture_number) {
                    next_virtual_fixture_number += 1;
                }
                fixture.virtual_fixture_number = Some(next_virtual_fixture_number);
                used_virtual_fixture_numbers.insert(next_virtual_fixture_number);
                next_virtual_fixture_number += 1;
            }
        }
        light_fixture::migrate_patched_fixture_to_v2(&mut fixture)
            .map_err(|error| format!("fixture schema-v1-to-v2 migration failed: {error}"))?;
        let normalized = serde_json::to_value(fixture).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    Ok(updates)
}

fn other_schema_updates(store: &ShowStore) -> Result<Vec<SchemaUpdate>, String> {
    let mut updates = Vec::new();
    for object in store.objects("group").map_err(|error| error.to_string())? {
        let original = object.body;
        let mut group =
            serde_json::from_value::<light_programmer::GroupDefinition>(original.clone())
                .map_err(|error| format!("invalid group: {error}"))?;
        group.id.clone_from(&object.id);
        let normalized = serde_json::to_value(group).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store.objects("preset").map_err(|error| error.to_string())? {
        let original = object.body;
        let mut preset = serde_json::from_value::<light_programmer::Preset>(original.clone())
            .map_err(|error| format!("invalid preset: {error}"))?;
        preset.reconcile_address(&object.id)?;
        let normalized = serialize_preset_preserving_extensions(&original, &preset)
            .map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
    {
        let original = object.body;
        let mut cue_list = serde_json::from_value::<light_playback::CueList>(original.clone())
            .map_err(|error| format!("invalid cue list: {error}"))?;
        cue_list.migrate_legacy_chaser_xfade(&default_speed_groups());
        let normalized = serde_json::to_value(cue_list).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store
        .objects("playback")
        .map_err(|error| error.to_string())?
    {
        let original = object.body;
        let playback =
            serde_json::from_value::<light_playback::PlaybackDefinition>(original.clone())
                .map_err(|error| format!("invalid playback: {error}"))?;
        let normalized = serde_json::to_value(playback).map_err(|error| error.to_string())?;
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    for object in store.objects("route").map_err(|error| error.to_string())? {
        let original = object.body;
        let destination_was_missing = original.get("destination").is_none();
        let delivery_mode_was_missing = original.get("delivery_mode").is_none();
        let mut route = serde_json::from_value::<light_output::OutputRoute>(original.clone())
            .map_err(|error| format!("invalid output route: {error}"))?;
        if destination_was_missing {
            route.destination = None;
        }
        if delivery_mode_was_missing {
            route.delivery_mode = Some(route.resolved_delivery_mode());
        }
        route
            .validate()
            .map_err(|error| format!("invalid output route: {error}"))?;
        let mut normalized = serde_json::to_value(&route).map_err(|error| error.to_string())?;
        // Preserve the supported historical default as explicit current-schema data. `None`
        // selects the protocol's standard destination, but an omitted legacy field must migrate
        // once to `null` rather than remaining indistinguishable from an unnormalised object.
        if destination_was_missing && let Some(body) = normalized.as_object_mut() {
            body.insert("destination".into(), serde_json::Value::Null);
        }
        if normalized != original {
            updates.push((object.kind, object.id, normalized, object.revision));
        }
    }
    Ok(updates)
}

fn apply_schema_updates(
    entry: &ShowEntry,
    store: &ShowStore,
    updates: Vec<SchemaUpdate>,
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    let migration_probe = std::path::Path::new(&entry.path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!(".migration-probe-{}.show", Uuid::new_v4()));
    store
        .backup_to(&migration_probe)
        .map_err(|error| error.to_string())?;
    let probe_result = (|| {
        let probe_store = ShowStore::open(&migration_probe).map_err(|error| error.to_string())?;
        for (kind, id, body, revision) in &updates {
            probe_store
                .put_object(kind, id, body, *revision)
                .map_err(|error| error.to_string())?;
        }
        drop(probe_store);
        let probe = ShowEntry {
            path: migration_probe.display().to_string(),
            ..entry.clone()
        };
        load_engine_snapshot_with_override(&probe, None)?
            .validate()
            .map_err(|error| error.to_string())
    })();
    let _ = std::fs::remove_file(&migration_probe);
    let _ = std::fs::remove_file(format!("{}-wal", migration_probe.display()));
    let _ = std::fs::remove_file(format!("{}-shm", migration_probe.display()));
    probe_result?;
    for (kind, id, body, revision) in updates {
        store
            .put_object(&kind, &id, &body, revision)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn reconcile_show_schema_defaults(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    let mut updates = fixture_schema_updates(&store)?;
    updates.extend(other_schema_updates(&store)?);
    apply_schema_updates(entry, &store, updates)
}
