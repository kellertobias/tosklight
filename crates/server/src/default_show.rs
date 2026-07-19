use light_fixture::{PatchedFixture, PortablePatchedFixtureRecord};
use light_show::{PortableShowDocument, PortableShowTransaction, StoreError};
use serde_json::Value;
use std::path::Path;

#[path = "default_show/definition.rs"]
mod definition;
#[path = "default_show/seed.rs"]
mod seed;

const DEFAULT_SHOW_NAME: &str = "Default Stage Show";

pub fn name() -> &'static str {
    DEFAULT_SHOW_NAME
}

fn trailing_number(name: &str) -> Option<u32> {
    name.rsplit_once(' ')?.1.parse().ok()
}

pub(crate) fn default_fixture_number(name: &str) -> Option<u32> {
    match name {
        "Middle ACL Set" => Some(28),
        "Outside ACL Set" => Some(29),
        "Stage Hazer" => Some(99),
        "Overhead RGB Multi-patch" => Some(999),
        _ if name.starts_with("Front Fresnel ") => trailing_number(name),
        _ if name.starts_with("Back Profile ") => trailing_number(name).map(|value| 100 + value),
        _ if name.starts_with("Back LED Wash ") => trailing_number(name).map(|value| 200 + value),
        _ if name.starts_with("Back Trackspot ") => trailing_number(name).map(|value| 300 + value),
        _ if name.starts_with("Floor RGBW PAR ") => trailing_number(name).map(|value| 400 + value),
        _ if name.starts_with("Back RGB Sunstrip ") => {
            trailing_number(name).map(|value| 500 + value)
        }
        _ if name.starts_with("Front RGB Strobe ") => {
            trailing_number(name).map(|value| 600 + value)
        }
        _ if name.starts_with("Dimmer ") => trailing_number(name),
        _ if name.starts_with("RGB LED ") => trailing_number(name).map(|value| 20 + value),
        _ => None,
    }
}

fn default_patch(name: &str) -> Option<(u16, u16)> {
    match name {
        "Middle ACL Set" => Some((1, 11)),
        "Outside ACL Set" => Some((1, 12)),
        "Stage Hazer" => Some((1, 13)),
        "Overhead RGB Multi-patch" => Some((4, 1)),
        _ if name.starts_with("Front Fresnel ") => {
            trailing_number(name).map(|number| (1, number as u16))
        }
        _ if name.starts_with("Back Profile ") => {
            trailing_number(name).map(|number| (2, 1 + (number as u16 - 1) * 6))
        }
        _ if name.starts_with("Back LED Wash ") => {
            trailing_number(name).map(|number| (2, 49 + (number as u16 - 1) * 6))
        }
        _ if name.starts_with("Back Trackspot ") => {
            trailing_number(name).map(|number| (2, 79 + (number as u16 - 1) * 3))
        }
        _ if name.starts_with("Floor RGBW PAR ") => {
            trailing_number(name).map(|number| (3, 1 + (number as u16 - 1) * 5))
        }
        _ if name.starts_with("Back RGB Sunstrip ") => {
            trailing_number(name).map(|number| (3, 61 + (number as u16 - 1) * 30))
        }
        _ if name.starts_with("Front RGB Strobe ") => {
            trailing_number(name).map(|number| (3, 241 + (number as u16 - 1) * 4))
        }
        _ => None,
    }
}

/// Stages built-in-show compatibility fixes without writing or decoding an already lean record as
/// a runtime fixture. The general show compiler subsequently performs portable schema migration.
pub(crate) fn stage_upgrade(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
) -> Result<(), StoreError> {
    let records = document
        .objects_of_kind("patched_fixture")
        .map(|object| {
            PortablePatchedFixtureRecord::decode(object.body().clone())
                .map(|record| (object, record))
                .map_err(portable_patch_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let migrate_single_universe_patch = records.len() == 49
        && records
            .iter()
            .all(|(_, record)| record.patch().is_ok_and(|patch| patch.universe == Some(1)));
    let mut changed = false;
    for (object, record) in records {
        let migrated = migrate_default_record(record, migrate_single_universe_patch)?;
        if migrated.body() != object.body() {
            changed = true;
            transaction.put("patched_fixture", object.key().id(), migrated.into_body());
        }
    }
    if changed {
        transaction.mark_patch_changed();
    }
    Ok(())
}

fn migrate_default_record(
    mut record: PortablePatchedFixtureRecord,
    migrate_single_universe_patch: bool,
) -> Result<PortablePatchedFixtureRecord, StoreError> {
    if !record.is_legacy_inline() {
        let mut patch = record.patch().map_err(portable_patch_error)?;
        apply_default_patch_fields(&mut patch, migrate_single_universe_patch);
        record.update_patch(&patch).map_err(portable_patch_error)?;
        return Ok(record);
    }
    let mut fixture = serde_json::from_value::<PatchedFixture>(record.body().clone())?;
    let before = serde_json::to_value(&fixture)?;
    apply_default_fixture_fixes(&mut fixture, migrate_single_universe_patch);
    let after = serde_json::to_value(fixture)?;
    let mut body = record.into_body();
    merge_typed_delta(&mut body, &before, &after);
    PortablePatchedFixtureRecord::decode(body).map_err(portable_patch_error)
}

fn apply_default_fixture_fixes(fixture: &mut PatchedFixture, migrate_patch: bool) {
    if fixture.fixture_number.is_none() {
        fixture.fixture_number = default_fixture_number(&fixture.name);
    }
    if migrate_patch && let Some((universe, address)) = default_patch(&fixture.name) {
        fixture.universe = Some(universe);
        fixture.address = Some(address);
        update_primary_split(&mut fixture.split_patches, universe, address);
    }
    if requires_sunstrip_upgrade(fixture) {
        fixture.definition = definition::sunstrip_definition();
    }
}

fn apply_default_patch_fields(
    patch: &mut light_fixture::PatchedFixturePatch,
    migrate_single_universe_patch: bool,
) {
    if patch.fixture_number.is_none() {
        patch.fixture_number = default_fixture_number(&patch.name);
    }
    if migrate_single_universe_patch && let Some((universe, address)) = default_patch(&patch.name) {
        patch.universe = Some(universe);
        patch.address = Some(address);
        update_primary_split(&mut patch.split_patches, universe, address);
    }
}

fn update_primary_split(splits: &mut [light_fixture::SplitPatch], universe: u16, address: u16) {
    if let Some(primary) = splits.iter_mut().find(|split| split.split == 1) {
        primary.universe = Some(universe);
        primary.address = Some(address);
    }
}

fn requires_sunstrip_upgrade(fixture: &PatchedFixture) -> bool {
    fixture.name.starts_with("Back RGB Sunstrip ")
        && !fixture.definition.heads.iter().all(|head| {
            head.parameters
                .iter()
                .any(|parameter| parameter.attribute.is_intensity() && parameter.virtual_dimmer)
        })
}

fn portable_patch_error(error: light_fixture::PortablePatchError) -> StoreError {
    StoreError::Invalid(error.to_string())
}

fn merge_typed_delta(stored: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    match (stored, before, after) {
        (Value::Object(stored), Value::Object(before), Value::Object(after)) => {
            for (key, before_value) in before {
                let Some(after_value) = after.get(key) else {
                    stored.remove(key);
                    continue;
                };
                if before_value != after_value {
                    match stored.get_mut(key) {
                        Some(current) => merge_typed_delta(current, before_value, after_value),
                        None => {
                            stored.insert(key.clone(), after_value.clone());
                        }
                    }
                }
            }
            for (key, value) in after {
                if !before.contains_key(key) {
                    stored.insert(key.clone(), value.clone());
                }
            }
        }
        (Value::Array(stored), Value::Array(before), Value::Array(after))
            if stored.len() == before.len() && before.len() == after.len() =>
        {
            for ((stored, before), after) in stored.iter_mut().zip(before).zip(after) {
                merge_typed_delta(stored, before, after);
            }
        }
        (stored, _, after) => *stored = after.clone(),
    }
}

pub fn initialise(path: impl AsRef<Path>) -> Result<light_core::ShowId, StoreError> {
    seed::initialise(path)
}

#[cfg(test)]
#[path = "default_show/tests.rs"]
mod tests;
