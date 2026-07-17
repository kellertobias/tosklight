use light_core::{AttributeKey, FixtureId};
use light_fixture::{
    ByteOrder, ChannelComponent, FixtureDefinition, FixtureLocation, FixturePhysicalProperties,
    FixtureVector, LogicalHead, MultiPatchInstance, Parameter, ParameterMetadata, PatchedFixture,
    PatchedHead, SignalLossPolicy,
};
use light_show::{ShowStore, StoreError};
use serde_json::json;
use std::{collections::BTreeMap, path::Path};

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

fn parameter(attribute: &str, offset: u16, default: f32) -> Parameter {
    Parameter {
        attribute: AttributeKey(attribute.into()),
        components: vec![ChannelComponent {
            offset,
            byte_order: ByteOrder::MsbFirst,
        }],
        default,
        virtual_dimmer: false,
        metadata: ParameterMetadata::default(),
        capabilities: Vec::new(),
    }
}

fn definition(name: &str, device_type: &str, attributes: &[&str]) -> FixtureDefinition {
    FixtureDefinition {
        schema_version: 1,
        id: FixtureId::new(),
        revision: 1,
        manufacturer: "ToskLight Built-in".into(),
        device_type: device_type.into(),
        name: name.into(),
        model: name.into(),
        mode: attributes
            .iter()
            .map(|value| match *value {
                "intensity" => "D",
                "pan" => "P",
                "tilt" => "T",
                "color.red" => "R",
                "color.green" => "G",
                "color.blue" => "B",
                "color.white" => "W",
                _ => "?",
            })
            .collect(),
        footprint: attributes.len() as u16,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters: attributes
                .iter()
                .enumerate()
                .map(|(offset, attribute)| {
                    parameter(
                        attribute,
                        offset as u16,
                        if matches!(*attribute, "pan" | "tilt") {
                            0.5
                        } else {
                            0.0
                        },
                    )
                })
                .collect(),
        }],
        color_calibration: None,
        physical: FixturePhysicalProperties::default(),
        model_asset: None,
        icon_asset: None,
        hazardous: false,
        direct_control_protocols: Vec::new(),
        signal_loss_policy: SignalLossPolicy::HoldLast,
        safe_values: BTreeMap::new(),
        profile_id: None,
        mode_id: None,
        profile_snapshot: None,
    }
}

fn sunstrip_definition() -> FixtureDefinition {
    let mut fixture = definition(
        "RGB LED Sunstrip 10",
        "strip light",
        &["color.red", "color.green", "color.blue"],
    );
    fixture.mode = "10 × RGB".into();
    fixture.footprint = 30;
    fixture.heads = (0..10)
        .map(|index| LogicalHead {
            index,
            name: format!("Cell {}", index + 1),
            shared: false,
            parameters: std::iter::once(Parameter {
                attribute: AttributeKey::intensity(),
                components: Vec::new(),
                default: 0.0,
                virtual_dimmer: true,
                metadata: ParameterMetadata::default(),
                capabilities: Vec::new(),
            })
            .chain(
                ["color.red", "color.green", "color.blue"]
                    .iter()
                    .enumerate()
                    .map(|(component, attribute)| {
                        let mut parameter = parameter(attribute, index * 3 + component as u16, 0.0);
                        parameter.virtual_dimmer = true;
                        parameter
                    }),
            )
            .collect(),
        })
        .collect();
    fixture
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
            fixture.definition = sunstrip_definition();
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

fn patched(
    name: String,
    fixture_number: u32,
    definition: &FixtureDefinition,
    x: f32,
    y: f32,
    z: f32,
    rotation_y: f32,
) -> PatchedFixture {
    let (universe, address) = default_patch(&name).expect("built-in fixture has a default patch");
    PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(fixture_number),
        name,
        definition: definition.clone(),
        universe: Some(universe),
        address: Some(address),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: FixtureLocation {
            x: (x * 1000.0) as i32,
            y: (y * 1000.0) as i32,
            z: (z * 1000.0) as i32,
        },
        rotation: FixtureVector {
            x: 0.0,
            y: rotation_y,
            z: 0.0,
        },
        logical_heads: definition
            .heads
            .iter()
            .filter(|head| !head.shared)
            .map(|head| PatchedHead {
                head_index: head.index,
                fixture_id: FixtureId::new(),
            })
            .collect(),
        multipatch: Vec::new(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    }
}

fn position(x: f32, y: f32, z: f32, rotation_y: f32) -> serde_json::Value {
    json!({"x":x,"y":y,"z":z,"rotationX":0,"rotationY":rotation_y,"rotationZ":0})
}

fn multipatch(name: String, x: f32, y: f32, z: f32, rotation_y: f32) -> MultiPatchInstance {
    MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        name,
        universe: None,
        address: None,
        split_patches: Vec::new(),
        location: FixtureLocation {
            x: (x * 1000.0) as i32,
            y: (y * 1000.0) as i32,
            z: (z * 1000.0) as i32,
        },
        rotation: FixtureVector {
            x: 0.0,
            y: rotation_y,
            z: 0.0,
        },
    }
}

pub fn initialise(path: impl AsRef<Path>) -> Result<light_core::ShowId, StoreError> {
    if path.as_ref().exists() {
        return ShowStore::open(path)?.id();
    }
    let (store, show_id) = ShowStore::create(path, DEFAULT_SHOW_NAME)?;
    let fresnel = definition("PC Fresnel", "fresnel", &["intensity"]);
    let profile = definition(
        "Profile Moving Light",
        "moving profile",
        &[
            "intensity",
            "pan",
            "tilt",
            "color.red",
            "color.green",
            "color.blue",
        ],
    );
    let wash = definition(
        "A7 LED Wash",
        "moving wash",
        &[
            "intensity",
            "pan",
            "tilt",
            "color.red",
            "color.green",
            "color.blue",
        ],
    );
    let sunstrip = sunstrip_definition();
    let strobe = definition(
        "Square RGB LED Strobe",
        "strobe",
        &["intensity", "color.red", "color.green", "color.blue"],
    );
    let par = definition(
        "RGBW LED PAR",
        "par",
        &[
            "intensity",
            "color.red",
            "color.green",
            "color.blue",
            "color.white",
        ],
    );
    let acl = definition("ACL Long-nose PAR Set", "par", &["intensity"]);
    let rgb_multipatch = definition(
        "RGB Multi-patch Strobe",
        "strobe",
        &["intensity", "color.red", "color.green", "color.blue"],
    );
    let scanner = definition(
        "Trackspot Mirror Scanner",
        "scanner",
        &["intensity", "pan", "tilt"],
    );
    let hazer = definition("Hazer", "hazer", &["fog", "fan"]);
    let mut fixtures = Vec::new();
    for (index, x) in [-5.0, -4.0, -3.0, 3.0, 4.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Front Fresnel {}", index + 1),
            1 + index as u32,
            &fresnel,
            x,
            1.0,
            4.65,
            0.0,
        ));
    }
    for (index, x) in [-5.25, -3.75, -2.25, -0.75, 0.75, 2.25, 3.75, 5.25]
        .into_iter()
        .enumerate()
    {
        fixtures.push(patched(
            format!("Back Profile {}", index + 1),
            101 + index as u32,
            &profile,
            x,
            7.0,
            4.65,
            0.0,
        ));
    }
    for (index, x) in [-4.5, -2.25, 0.0, 2.25, 4.5].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back LED Wash {}", index + 1),
            201 + index as u32,
            &wash,
            x,
            7.0,
            4.65,
            0.0,
        ));
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back RGB Sunstrip {}", index + 1),
            501 + index as u32,
            &sunstrip,
            x,
            7.75,
            2.1,
            0.0,
        ));
    }
    for (index, x) in [-2.1, -0.7, 0.7, 2.1].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Front RGB Strobe {}", index + 1),
            601 + index as u32,
            &strobe,
            x,
            0.9,
            4.7,
            0.0,
        ));
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Floor RGBW PAR {}", index + 1),
            401 + index as u32,
            &par,
            x,
            2.5,
            0.3,
            -90.0,
        ));
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Floor RGBW PAR {}", index + 7),
            407 + index as u32,
            &par,
            x,
            5.0,
            0.3,
            -90.0,
        ));
    }
    let mut middle_acl = patched("Middle ACL Set".into(), 28, &acl, -1.4, 6.6, 3.8, -32.0);
    middle_acl.multipatch = (1..8)
        .map(|index| {
            let x = -1.4 + index as f32 * 0.4;
            multipatch(
                format!("Middle ACL {}", index + 1),
                x,
                6.6,
                3.8,
                -32.0 + index as f32 * (64.0 / 7.0),
            )
        })
        .collect();
    fixtures.push(middle_acl);
    let outside_positions = [
        (-5.2, -34.0),
        (-4.75, -22.0),
        (-4.3, -10.0),
        (-3.85, 2.0),
        (3.85, -2.0),
        (4.3, 10.0),
        (4.75, 22.0),
        (5.2, 34.0),
    ];
    let mut outside_acl = patched(
        "Outside ACL Set".into(),
        29,
        &acl,
        outside_positions[0].0,
        6.65,
        3.8,
        outside_positions[0].1,
    );
    outside_acl.multipatch = outside_positions
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(index, (x, rotation))| {
            multipatch(format!("Outside ACL {}", index + 1), x, 6.65, 3.8, rotation)
        })
        .collect();
    fixtures.push(outside_acl);
    let rgb_positions = [
        (-2.25, 3.7),
        (-0.75, 3.7),
        (0.75, 3.7),
        (2.25, 3.7),
        (-2.25, 4.35),
        (-0.75, 4.35),
        (0.75, 4.35),
        (2.25, 4.35),
    ];
    let mut rgb_grid = patched(
        "Overhead RGB Multi-patch".into(),
        999,
        &rgb_multipatch,
        rgb_positions[0].0,
        rgb_positions[0].1,
        5.2,
        0.0,
    );
    rgb_grid.multipatch = rgb_positions
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(index, (x, y))| multipatch(format!("Overhead RGB {}", index + 1), x, y, 5.2, 0.0))
        .collect();
    fixtures.push(rgb_grid);
    for (index, x) in [-4.5, -1.5, 1.5, 4.5].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back Trackspot {}", index + 1),
            301 + index as u32,
            &scanner,
            x,
            6.15,
            3.25,
            0.0,
        ));
    }
    fixtures.push(patched(
        "Stage Hazer".into(),
        99,
        &hazer,
        5.5,
        7.7,
        0.25,
        0.0,
    ));
    let positions3d = fixtures
        .iter()
        .map(|fixture| {
            let location = fixture.location;
            (
                fixture.fixture_id.0.to_string(),
                position(
                    location.x as f32 / 1000.0,
                    location.y as f32 / 1000.0,
                    location.z as f32 / 1000.0,
                    fixture.rotation.y,
                ),
            )
        })
        .chain(fixtures.iter().flat_map(|fixture| {
            fixture.multipatch.iter().map(|instance| {
                let location = instance.location;
                (
                    instance.id.to_string(),
                    position(
                        location.x as f32 / 1000.0,
                        location.y as f32 / 1000.0,
                        location.z as f32 / 1000.0,
                        instance.rotation.y,
                    ),
                )
            })
        }))
        .collect::<serde_json::Map<_, _>>();
    for fixture in &fixtures {
        store.put_object(
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            &serde_json::to_value(fixture)?,
            0,
        )?;
    }
    let mut assets = Vec::new();
    for (side, y) in [("front", 1.0), ("back", 7.0)] {
        for segment in 0..4 {
            assets.push(json!({
                "id":format!("{side}-truss-{segment}"), "name":format!("{} truss segment {}", if side == "front" { "Front" } else { "Back" }, segment + 1),
                "format":"builtin", "builtinId":"truss-3m", "position":position(-4.5 + segment as f32 * 3.0, y, 5.0, 0.0), "scale":1
            }));
        }
    }
    store.put_object(
        "stage_layout",
        "main",
        &json!({"version":2,"positions":{},"positions3d":positions3d,"assets":assets}),
        0,
    )?;
    Ok(show_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeds_the_complete_non_overlapping_default_rig() {
        let path = std::env::temp_dir().join(format!(
            "tosklight-default-show-{}.show",
            uuid::Uuid::new_v4()
        ));
        initialise(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        let fixtures = store
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .map(|object| serde_json::from_value::<PatchedFixture>(object.body).unwrap())
            .collect::<Vec<_>>();
        light_fixture::validate_patch(&fixtures).unwrap();
        assert_eq!(fixtures.len(), 49);
        let mut fresnels = fixtures
            .iter()
            .filter(|fixture| fixture.name.starts_with("Front Fresnel"))
            .collect::<Vec<_>>();
        fresnels.sort_by(|left, right| left.name.cmp(&right.name));
        assert_eq!(
            fresnels
                .iter()
                .map(|fixture| fixture.fixture_number.unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
        let numbers = fixtures
            .iter()
            .map(|fixture| fixture.fixture_number.unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(numbers.len(), fixtures.len());
        for (prefix, expected) in [
            ("Back Profile", 101..=108),
            ("Back LED Wash", 201..=205),
            ("Back Trackspot", 301..=304),
            ("Floor RGBW PAR", 401..=412),
            ("Back RGB Sunstrip", 501..=506),
        ] {
            let actual = fixtures
                .iter()
                .filter(|fixture| fixture.name.starts_with(prefix))
                .map(|fixture| fixture.fixture_number.unwrap())
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected.collect());
        }
        for (name, number) in [
            ("Stage Hazer", 99),
            ("Middle ACL Set", 28),
            ("Outside ACL Set", 29),
            ("Overhead RGB Multi-patch", 999),
        ] {
            assert_eq!(
                fixtures
                    .iter()
                    .find(|fixture| fixture.name == name)
                    .unwrap()
                    .fixture_number,
                Some(number)
            );
        }
        let hazer = fixtures
            .iter()
            .find(|fixture| fixture.name == "Stage Hazer")
            .unwrap();
        assert_eq!((hazer.universe, hazer.address), (Some(1), Some(13)));
        assert_eq!(hazer.definition.footprint, 2);
        assert_eq!(
            hazer.definition.heads[0]
                .parameters
                .iter()
                .map(|parameter| parameter.attribute.0.as_str())
                .collect::<Vec<_>>(),
            vec!["fog", "fan"]
        );
        let sunstrip = fixtures
            .iter()
            .find(|fixture| fixture.fixture_number == Some(501))
            .unwrap();
        assert_eq!(sunstrip.logical_heads.len(), 10);
        assert!(sunstrip.definition.heads.iter().all(|head| {
            head.parameters.iter().any(|parameter| {
                parameter.attribute.is_intensity()
                    && parameter.virtual_dimmer
                    && parameter.components.is_empty()
            }) && head
                .parameters
                .iter()
                .filter(|parameter| parameter.attribute.0.starts_with("color."))
                .all(|parameter| parameter.virtual_dimmer)
        }));
        assert_eq!(
            crate::resolve_fixture_reference(&fixtures, "501.2").unwrap(),
            sunstrip
                .logical_heads
                .iter()
                .find(|head| head.head_index == 1)
                .unwrap()
                .fixture_id
        );
        assert_eq!(
            crate::parse_fixture_selection(&fixtures, &["501".into(), ".".into(), "2".into()])
                .unwrap(),
            vec![
                sunstrip
                    .logical_heads
                    .iter()
                    .find(|head| head.head_index == 1)
                    .unwrap()
                    .fixture_id
            ]
        );
        let sunstrip_502 = fixtures
            .iter()
            .find(|fixture| fixture.fixture_number == Some(502))
            .unwrap();
        let children_501 = sunstrip
            .logical_heads
            .iter()
            .map(|head| head.fixture_id)
            .collect::<Vec<_>>();
        let children_502 = sunstrip_502
            .logical_heads
            .iter()
            .map(|head| head.fixture_id)
            .collect::<Vec<_>>();
        assert_eq!(
            crate::parse_fixture_selection(&fixtures, &["501".into()]).unwrap(),
            std::iter::once(sunstrip.fixture_id)
                .chain(children_501.iter().copied())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            crate::parse_fixture_selection(
                &fixtures,
                &["501".into(), "THRU".into(), "502".into()],
            )
            .unwrap(),
            children_501
                .iter()
                .chain(&children_502)
                .copied()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            crate::parse_fixture_selection(
                &fixtures,
                &[
                    "501".into(),
                    ".".into(),
                    "0".into(),
                    "THRU".into(),
                    "502".into(),
                    ".".into(),
                    "0".into(),
                ],
            )
            .unwrap(),
            vec![sunstrip.fixture_id, sunstrip_502.fixture_id]
        );
        assert_eq!(
            crate::parse_fixture_selection(
                &fixtures,
                &[
                    "501".into(),
                    ".".into(),
                    "2".into(),
                    "THRU".into(),
                    "501".into(),
                    ".".into(),
                    "4".into(),
                ],
            )
            .unwrap(),
            children_501[1..4].to_vec()
        );
        assert_eq!(
            crate::parse_fixture_selection(
                &fixtures,
                &[
                    "501".into(),
                    "+".into(),
                    "501".into(),
                    ".".into(),
                    "1".into()
                ],
            )
            .unwrap(),
            std::iter::once(sunstrip.fixture_id)
                .chain(children_501.iter().copied())
                .collect::<Vec<_>>()
        );
        assert!(
            crate::parse_fixture_selection(
                &fixtures,
                &[
                    "501".into(),
                    ".".into(),
                    "1".into(),
                    "THRU".into(),
                    "502".into(),
                    ".".into(),
                    "1".into(),
                ],
            )
            .is_err()
        );
        assert!(crate::parse_fixture_selection(&fixtures, &["501".into(), "+".into()],).is_err());
        assert!(crate::resolve_fixture_reference(&fixtures, "501.11").is_err());
        for (name, expected_universe, expected_address) in [
            ("Front Fresnel 1", 1, 1),
            ("Front Fresnel 6", 1, 6),
            ("Middle ACL Set", 1, 11),
            ("Outside ACL Set", 1, 12),
            ("Back Profile 1", 2, 1),
            ("Back LED Wash 1", 2, 49),
            ("Back Trackspot 1", 2, 79),
            ("Floor RGBW PAR 1", 3, 1),
            ("Back RGB Sunstrip 1", 3, 61),
            ("Front RGB Strobe 1", 3, 241),
            ("Overhead RGB Multi-patch", 4, 1),
        ] {
            let fixture = fixtures
                .iter()
                .find(|fixture| fixture.name == name)
                .unwrap();
            assert_eq!(
                (fixture.universe, fixture.address),
                (Some(expected_universe), Some(expected_address)),
                "unexpected patch for {name}"
            );
        }
        assert_eq!(
            fixtures
                .iter()
                .filter_map(|fixture| fixture.universe)
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([1, 2, 3, 4])
        );
        let mut occupied = std::collections::BTreeSet::new();
        for fixture in &fixtures {
            for channel in
                fixture.address.unwrap()..fixture.address.unwrap() + fixture.definition.footprint
            {
                assert!(
                    occupied.insert((fixture.universe.unwrap(), channel)),
                    "overlap at {}.{channel}",
                    fixture.universe.unwrap()
                );
            }
        }
        let layout = store.objects("stage_layout").unwrap().pop().unwrap().body;
        assert_eq!(layout["positions3d"].as_object().unwrap().len(), 70);
        assert_eq!(layout["assets"].as_array().unwrap().len(), 8);
        let multipatched = fixtures
            .iter()
            .filter(|fixture| !fixture.multipatch.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(multipatched.len(), 3);
        assert!(
            multipatched
                .iter()
                .all(|fixture| fixture.multipatch.len() == 7)
        );
        assert_eq!(
            fixtures
                .iter()
                .filter(|fixture| fixture.definition.device_type == "scanner")
                .count(),
            4
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn upgrades_the_legacy_single_universe_default_patch() {
        let path = std::env::temp_dir().join(format!(
            "tosklight-default-show-upgrade-{}.show",
            uuid::Uuid::new_v4()
        ));
        initialise(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        for object in store.objects("patched_fixture").unwrap() {
            let mut fixture: PatchedFixture = serde_json::from_value(object.body).unwrap();
            fixture.universe = Some(1);
            store
                .put_object(
                    "patched_fixture",
                    &object.id,
                    &serde_json::to_value(fixture).unwrap(),
                    object.revision,
                )
                .unwrap();
        }
        drop(store);

        upgrade(&path).unwrap();

        let fixtures = ShowStore::open(&path)
            .unwrap()
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .map(|object| serde_json::from_value::<PatchedFixture>(object.body).unwrap())
            .collect::<Vec<_>>();
        for (name, expected) in [
            ("Middle ACL Set", (Some(1), Some(11))),
            ("Back Profile 1", (Some(2), Some(1))),
            ("Floor RGBW PAR 1", (Some(3), Some(1))),
            ("Overhead RGB Multi-patch", (Some(4), Some(1))),
        ] {
            let fixture = fixtures
                .iter()
                .find(|fixture| fixture.name == name)
                .unwrap();
            assert_eq!((fixture.universe, fixture.address), expected);
        }
        std::fs::remove_file(path).unwrap();
    }
}
