//! Bounded immutable input reader; only generic validation failures escape this module.
use super::*;
use light_fixture::FixtureProfile;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io::Read;
const MAX_INPUT_BYTES: u64 = 32 * 1024 * 1024;
const INVALID: &str = "invalid immutable legacy pair input";
pub(super) struct Input {
    pub(super) value: Value,
    pub(super) digest: String,
    pub(super) fixtures: Vec<PatchedFixture>,
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| INVALID)?;
    if !metadata.is_file() || metadata.len() > MAX_INPUT_BYTES {
        return Err(INVALID.into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| INVALID)?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| INVALID)?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(INVALID.into());
    }
    Ok(bytes)
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| INVALID.into())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key].as_str().ok_or_else(|| INVALID.into())
}
fn id(value: &Value, key: &str) -> Result<Uuid, String> {
    text(value, key)?.parse().map_err(|_| INVALID.into())
}
fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|_| INVALID.into())
}
fn require(value: bool) -> Result<(), String> {
    if value { Ok(()) } else { Err(INVALID.into()) }
}
fn binding_bytes(binding: &Value, prefix: &str) -> Result<Vec<u8>, String> {
    let relative = text(binding, "file")?;
    require(
        relative.starts_with(prefix)
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
    )?;
    let path = repository().join(relative);
    let bytes = read(&path)?;
    require(sha(&bytes) == text(binding, "sha256")?)?;
    Ok(bytes)
}
impl Input {
    pub(super) fn load(path: &Path) -> Result<Self, String> {
        require(path.is_absolute())?;
        let bytes = read(path)?;
        let digest = sha(&bytes);
        let expected = std::env::var("LIGHT_MIXED576_INPUT_SHA256")
            .map_err(|_| "frozen pair input SHA256 required")?;
        require(
            expected.len() == 64
                && expected.bytes().all(|b| b.is_ascii_hexdigit())
                && digest == expected,
        )?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| INVALID)?;
        require(
            value["kind"] == "private.production-js-mixed576-live-input/v1"
                && array(&value["patch"], "fixtures")?.len() == 576
                && value["scene"]["patch"]["occupiedSlots"] == 8064
                && value["scene"]["patch"]["universeCount"] == 16
                && value["plan"]["dynamicTargetCount"] == 5408
                && array(&value["plan"], "definitions")?.len() == 20
                && array(&value["plan"], "activations")?.len() == 20,
        )?;
        let mut profiles = HashMap::new();
        let bindings = array(&value, "package_bindings")?;
        require(!bindings.is_empty() && bindings.len() <= 32)?;
        for binding in bindings {
            let mut profile = light_fixture::read_fixture_package(&binding_bytes(
                binding,
                "assets/fixture-library/",
            )?)
            .map_err(|_| INVALID)?;
            require(profile.id.0 == id(binding, "profile_id")?)?;
            apply_runtime_profile_compatibility(&mut profile);
            profile.validate().map_err(|_| INVALID)?;
            require(profiles.insert(profile.id.0, profile).is_none())?;
        }
        let sources = array(&value, "source_bindings")?;
        require(!sources.is_empty() && sources.len() <= 16)?;
        for binding in sources {
            binding_bytes(binding, "tools/")?;
        }
        let fixtures = fixtures(&value, &profiles)?;
        descriptors(&value, &fixtures, &profiles)?;
        Ok(Self {
            value,
            digest,
            fixtures,
        })
    }
}
fn fixtures(
    value: &Value,
    profiles: &HashMap<Uuid, FixtureProfile>,
) -> Result<Vec<PatchedFixture>, String> {
    let mut counts = BTreeMap::new();
    let mut occupied = BTreeSet::new();
    let mut universes = BTreeSet::new();
    let mut result = Vec::new();
    let mut identities = BTreeSet::new();
    for row in array(&value["patch"], "fixtures")? {
        let profile = profiles.get(&id(row, "profile_id")?).ok_or(INVALID)?;
        let mode_id = id(row, "mode_id")?;
        let mode = profile
            .modes
            .iter()
            .find(|m| m.id == mode_id)
            .ok_or(INVALID)?;
        *counts.entry(profile.name.as_str()).or_insert(0usize) += 1;
        let fixture_id = FixtureId(id(row, "fixture_id")?);
        require(identities.insert(fixture_id.0))?;
        let number = decode::<u32>(&row["fixture_number"])?;
        let mut fixture = super::super::sustained_show::patched_definition(
            fixture_id,
            Some(number),
            text(row, "name")?.to_owned(),
            profile.resolved_definition(mode_id).map_err(|_| INVALID)?,
        );
        fixture.split_patches = decode(&row["split_patches"])?;
        fixture.layer_id = text(row, "layer_id")?.to_owned();
        fixture.location = decode(&row["location"])?;
        fixture.rotation = decode(&row["rotation"])?;
        fixture.logical_heads = decode(&row["logical_heads"])?;
        fixture.move_in_black_enabled = decode(&row["move_in_black_enabled"])?;
        fixture.move_in_black_delay_millis = decode(&row["move_in_black_delay_millis"])?;
        require(!fixture.split_patches.is_empty())?;
        for split in &fixture.split_patches {
            let universe = split.universe.ok_or(INVALID)?;
            let address = split.address.ok_or(INVALID)?;
            let footprint = mode
                .splits
                .iter()
                .find(|m| m.number == split.split)
                .ok_or(INVALID)?
                .footprint;
            universes.insert(universe);
            for offset in 0..footprint {
                let slot = address.checked_add(offset).ok_or(INVALID)?;
                require((1..=512).contains(&slot) && occupied.insert((universe, slot)))?;
            }
        }
        result.push(fixture);
    }
    require(
        counts.get("Robin LEDBeam 150") == Some(&224)
            && counts.get("Sunstrip LED RGB 42206") == Some(&96)
            && counts.get("Dimmer") == Some(&256)
            && counts.len() == 3
            && occupied.len() == SLOTS
            && universes.len() == 16,
    )?;
    Ok(result)
}
fn descriptors(
    value: &Value,
    fixtures: &[PatchedFixture],
    profiles: &HashMap<Uuid, FixtureProfile>,
) -> Result<(), String> {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut targets = BTreeSet::new();
    for descriptor in array(&value["plan"], "targetDescriptors")? {
        let fixture = fixtures
            .iter()
            .find(|f| Some(f.fixture_id.0) == id(descriptor, "fixtureId").ok())
            .ok_or(INVALID)?;
        let profile = profiles
            .get(&fixture.definition.profile_id.ok_or(INVALID)?.0)
            .ok_or(INVALID)?;
        let mode = profile
            .modes
            .iter()
            .find(|m| Some(m.id) == fixture.definition.mode_id)
            .ok_or(INVALID)?;
        let head_id = id(descriptor, "headId")?;
        let head = mode.heads.iter().find(|h| h.id == head_id).ok_or(INVALID)?;
        let channels = mode
            .channels
            .iter()
            .filter(|c| c.head_id == head_id)
            .collect::<Vec<_>>();
        let mut actual = BTreeSet::new();
        for channel in &channels {
            let mapped = match channel.attribute.0.as_ref() {
                "intensity" => Some("intensity"),
                "color.red" | "color.cyan" => Some("color.red"),
                "color.green" | "color.magenta" => Some("color.green"),
                "color.blue" | "color.yellow" => Some("color.blue"),
                "color.amber" => Some("color.amber"),
                "color.white" => Some("color.white_blend"),
                "color.uv" => Some("color.uv"),
                "pan" => Some("position.pan"),
                "tilt" => Some("position.tilt"),
                _ => None,
            };
            if let Some(mapped) = mapped {
                actual.insert(mapped.to_owned());
            }
        }
        if actual.contains("position.pan") || actual.contains("position.tilt") {
            actual.insert("position.pan".into());
            actual.insert("position.tilt".into());
        }
        if channels.iter().any(|c| c.reacts_to_virtual_intensity) {
            actual.insert("intensity".into());
        }
        let expected = array(descriptor, "attributes")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or(INVALID))
            .collect::<Result<BTreeSet<_>, _>>()?;
        require(actual == expected)?;
        let target = if head.master_shared {
            fixture.fixture_id
        } else {
            fixture
                .logical_heads
                .iter()
                .find(|h| h.profile_head_id == Some(head_id))
                .ok_or(INVALID)?
                .fixture_id
        };
        require(target.0 == id(descriptor, "target")? && targets.insert(target.0))?;
        for attribute in actual {
            *counts.entry(attribute).or_default() += 1;
        }
    }
    require(counts.values().sum::<usize>() == 5408 && counts.get("intensity") == Some(&1184))?;
    let coverage = value["plan"]["laneCoverage"].as_object().ok_or(INVALID)?;
    require(coverage.len() == counts.len())?;
    for (key, expected) in coverage {
        require(counts.get(key).copied() == expected.as_u64().map(|n| n as usize))?;
    }
    Ok(())
}
