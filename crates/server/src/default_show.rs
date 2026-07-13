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
                        if *attribute == "pan" {
                            0.5
                    } else if *attribute == "tilt" {
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
            parameters: ["color.red", "color.green", "color.blue"]
                .iter()
                .enumerate()
                .map(|(component, attribute)| {
                    parameter(attribute, index * 3 + component as u16, 0.0)
                })
                .collect(),
        })
        .collect();
    fixture
}

fn patched(
    name: String,
    fixture_number: u32,
    definition: &FixtureDefinition,
    address: u16,
    x: f32,
    y: f32,
    z: f32,
    rotation_y: f32,
) -> PatchedFixture {
    PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(fixture_number),
        name,
        definition: definition.clone(),
        universe: Some(1),
        address: Some(address),
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
    let mut address = 1_u16;
    for (index, x) in [-5.0, -4.0, -3.0, 3.0, 4.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Front Fresnel {}", index + 1),
            1 + index as u32,
            &fresnel,
            address,
            x,
            1.0,
            4.65,
            0.0,
        ));
        address += fresnel.footprint;
    }
    for (index, x) in [-5.25, -3.75, -2.25, -0.75, 0.75, 2.25, 3.75, 5.25]
        .into_iter()
        .enumerate()
    {
        fixtures.push(patched(
            format!("Back Profile {}", index + 1),
            101 + index as u32,
            &profile,
            address,
            x,
            7.0,
            4.65,
            0.0,
        ));
        address += profile.footprint;
    }
    for (index, x) in [-4.5, -2.25, 0.0, 2.25, 4.5].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back LED Wash {}", index + 1),
            201 + index as u32,
            &wash,
            address,
            x,
            7.0,
            4.65,
            0.0,
        ));
        address += wash.footprint;
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back RGB Sunstrip {}", index + 1),
            501 + index as u32,
            &sunstrip,
            address,
            x,
            7.75,
            2.1,
            0.0,
        ));
        address += sunstrip.footprint;
    }
    for (index, x) in [-2.1, -0.7, 0.7, 2.1].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Front RGB Strobe {}", index + 1),
            601 + index as u32,
            &strobe,
            address,
            x,
            0.9,
            4.7,
            0.0,
        ));
        address += strobe.footprint;
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Floor RGBW PAR {}", index + 1),
            401 + index as u32,
            &par,
            address,
            x,
            2.5,
            0.3,
            -90.0,
        ));
        address += par.footprint;
    }
    for (index, x) in [-5.0, -3.0, -1.0, 1.0, 3.0, 5.0].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Floor RGBW PAR {}", index + 7),
            407 + index as u32,
            &par,
            address,
            x,
            5.0,
            0.3,
            -90.0,
        ));
        address += par.footprint;
    }
    let mut middle_acl = patched(
        "Middle ACL Set".into(),
        28,
        &acl,
        address,
        -1.4,
        6.6,
        3.8,
        -32.0,
    );
    address += acl.footprint;
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
        address,
        outside_positions[0].0,
        6.65,
        3.8,
        outside_positions[0].1,
    );
    address += acl.footprint;
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
        address,
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
    address += rgb_multipatch.footprint;
    for (index, x) in [-4.5, -1.5, 1.5, 4.5].into_iter().enumerate() {
        fixtures.push(patched(
            format!("Back Trackspot {}", index + 1),
            301 + index as u32,
            &scanner,
            address,
            x,
            6.15,
            3.25,
            0.0,
        ));
        address += scanner.footprint;
    }
    fixtures.push(patched(
        "Stage Hazer".into(),
        99,
        &hazer,
        address,
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
        assert_eq!(hazer.address, Some(359));
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
        assert_eq!(
            crate::resolve_fixture_reference(&fixtures, "501.2").unwrap(),
            sunstrip
                .logical_heads
                .iter()
                .find(|head| head.head_index == 1)
                .unwrap()
                .fixture_id
        );
        assert!(crate::resolve_fixture_reference(&fixtures, "501.11").is_err());
        let mut occupied = std::collections::BTreeSet::new();
        for fixture in &fixtures {
            for channel in
                fixture.address.unwrap()..fixture.address.unwrap() + fixture.definition.footprint
            {
                assert!(occupied.insert(channel), "overlap at {channel}");
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
}
