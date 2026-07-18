use light_fixture::PatchedFixture;
use light_show::{ShowStore, StoreError};
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

/// Upgrades an existing built-in show without replacing user programming.
pub fn upgrade(path: impl AsRef<Path>) -> Result<(), StoreError> {
    let store = ShowStore::open(path)?;
    let fixtures = store
        .objects("patched_fixture")?
        .into_iter()
        .map(|object| {
            let fixture = serde_json::from_value::<PatchedFixture>(object.body.clone())?;
            Ok((object, fixture))
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    let migrate_single_universe_patch = fixtures.len() == 49
        && fixtures
            .iter()
            .all(|(_, fixture)| fixture.universe == Some(1));
    for (object, mut fixture) in fixtures {
        let mut changed = false;
        if fixture.fixture_number.is_none() {
            fixture.fixture_number = default_fixture_number(&fixture.name);
            changed = fixture.fixture_number.is_some();
        }
        if fixture.name.starts_with("Back RGB Sunstrip ")
            && !fixture.definition.heads.iter().all(|head| {
                head.parameters
                    .iter()
                    .any(|parameter| parameter.attribute.is_intensity() && parameter.virtual_dimmer)
            })
        {
            fixture.definition = definition::sunstrip_definition();
            changed = true;
        }
        if migrate_single_universe_patch
            && let Some((universe, address)) = default_patch(&fixture.name)
        {
            fixture.universe = Some(universe);
            fixture.address = Some(address);
            changed = true;
        }
        if changed {
            store.put_object(
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                &serde_json::to_value(fixture)?,
                object.revision,
            )?;
        }
    }
    Ok(())
}

pub fn initialise(path: impl AsRef<Path>) -> Result<light_core::ShowId, StoreError> {
    seed::initialise(path)
}

#[cfg(test)]
#[path = "default_show/tests.rs"]
mod tests;
