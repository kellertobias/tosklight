use super::DEFAULT_SHOW_NAME;
use super::definition::{definition, multipatch, patched, sunstrip_definition};
use light_fixture::{FixtureDefinition, FixtureLocation, PatchedFixture};
use light_show::{ShowStore, StoreError};
use serde_json::json;
use std::path::Path;

struct DefaultDefinitions {
    fresnel: FixtureDefinition,
    profile: FixtureDefinition,
    wash: FixtureDefinition,
    sunstrip: FixtureDefinition,
    strobe: FixtureDefinition,
    par: FixtureDefinition,
    acl: FixtureDefinition,
    rgb_multipatch: FixtureDefinition,
    scanner: FixtureDefinition,
    hazer: FixtureDefinition,
}

fn default_definitions() -> DefaultDefinitions {
    let moving = [
        "intensity",
        "pan",
        "tilt",
        "color.red",
        "color.green",
        "color.blue",
    ];
    DefaultDefinitions {
        fresnel: definition("PC Fresnel", "fresnel", &["intensity"]),
        profile: definition("Profile Moving Light", "moving profile", &moving),
        wash: definition("A7 LED Wash", "moving wash", &moving),
        sunstrip: sunstrip_definition(),
        strobe: definition(
            "Square RGB LED Strobe",
            "strobe",
            &["intensity", "color.red", "color.green", "color.blue"],
        ),
        par: definition(
            "RGBW LED PAR",
            "par",
            &[
                "intensity",
                "color.red",
                "color.green",
                "color.blue",
                "color.white",
            ],
        ),
        acl: definition("ACL Long-nose PAR Set", "par", &["intensity"]),
        rgb_multipatch: definition(
            "RGB Multi-patch Strobe",
            "strobe",
            &["intensity", "color.red", "color.green", "color.blue"],
        ),
        scanner: definition(
            "Trackspot Mirror Scanner",
            "scanner",
            &["intensity", "pan", "tilt"],
        ),
        hazer: definition("Hazer", "hazer", &["fog", "fan"]),
    }
}

struct FixtureLine<'a> {
    name: &'a str,
    first_name: u32,
    first_number: u32,
    definition: &'a FixtureDefinition,
    x: &'a [f32],
    y: f32,
    z: f32,
    rotation_y: f32,
}

fn location(x: f32, y: f32, z: f32) -> FixtureLocation {
    FixtureLocation {
        x: (x * 1000.0) as i32,
        y: (y * 1000.0) as i32,
        z: (z * 1000.0) as i32,
    }
}

fn fixture(
    name: String,
    number: u32,
    definition: &FixtureDefinition,
    position: (f32, f32, f32),
    rotation_y: f32,
) -> PatchedFixture {
    patched(
        name,
        number,
        definition,
        location(position.0, position.1, position.2),
        rotation_y,
    )
}

fn instance(
    name: String,
    position: (f32, f32, f32),
    rotation_y: f32,
) -> light_fixture::MultiPatchInstance {
    multipatch(
        name,
        location(position.0, position.1, position.2),
        rotation_y,
    )
}

fn fixture_line(line: FixtureLine<'_>) -> Vec<PatchedFixture> {
    line.x
        .iter()
        .enumerate()
        .map(|(index, x)| {
            fixture(
                format!("{} {}", line.name, line.first_name + index as u32),
                line.first_number + index as u32,
                line.definition,
                (*x, line.y, line.z),
                line.rotation_y,
            )
        })
        .collect()
}

fn ordinary_fixtures(definitions: &DefaultDefinitions) -> Vec<PatchedFixture> {
    let mut fixtures = Vec::new();
    for line in fixture_lines(definitions) {
        fixtures.extend(fixture_line(line));
    }
    fixtures
}

fn fixture_lines(definitions: &DefaultDefinitions) -> [FixtureLine<'_>; 7] {
    [
        FixtureLine {
            name: "Front Fresnel",
            first_name: 1,
            first_number: 1,
            definition: &definitions.fresnel,
            x: &[-5.0, -4.0, -3.0, 3.0, 4.0, 5.0],
            y: 1.0,
            z: 4.65,
            rotation_y: 0.0,
        },
        FixtureLine {
            name: "Back Profile",
            first_name: 1,
            first_number: 101,
            definition: &definitions.profile,
            x: &[-5.25, -3.75, -2.25, -0.75, 0.75, 2.25, 3.75, 5.25],
            y: 7.0,
            z: 4.65,
            rotation_y: 0.0,
        },
        FixtureLine {
            name: "Back LED Wash",
            first_name: 1,
            first_number: 201,
            definition: &definitions.wash,
            x: &[-4.5, -2.25, 0.0, 2.25, 4.5],
            y: 7.0,
            z: 4.65,
            rotation_y: 0.0,
        },
        FixtureLine {
            name: "Back RGB Sunstrip",
            first_name: 1,
            first_number: 501,
            definition: &definitions.sunstrip,
            x: &[-5.0, -3.0, -1.0, 1.0, 3.0, 5.0],
            y: 7.75,
            z: 2.1,
            rotation_y: 0.0,
        },
        FixtureLine {
            name: "Front RGB Strobe",
            first_name: 1,
            first_number: 601,
            definition: &definitions.strobe,
            x: &[-2.1, -0.7, 0.7, 2.1],
            y: 0.9,
            z: 4.7,
            rotation_y: 0.0,
        },
        FixtureLine {
            name: "Floor RGBW PAR",
            first_name: 1,
            first_number: 401,
            definition: &definitions.par,
            x: &[-5.0, -3.0, -1.0, 1.0, 3.0, 5.0],
            y: 2.5,
            z: 0.3,
            rotation_y: -90.0,
        },
        FixtureLine {
            name: "Floor RGBW PAR",
            first_name: 7,
            first_number: 407,
            definition: &definitions.par,
            x: &[-5.0, -3.0, -1.0, 1.0, 3.0, 5.0],
            y: 5.0,
            z: 0.3,
            rotation_y: -90.0,
        },
    ]
}

fn middle_acl(definition: &FixtureDefinition) -> PatchedFixture {
    let mut fixture = fixture(
        "Middle ACL Set".into(),
        28,
        definition,
        (-1.4, 6.6, 3.8),
        -32.0,
    );
    fixture.multipatch = (1..8)
        .map(|index| {
            instance(
                format!("Middle ACL {}", index + 1),
                (-1.4 + index as f32 * 0.4, 6.6, 3.8),
                -32.0 + index as f32 * (64.0 / 7.0),
            )
        })
        .collect();
    fixture
}

fn outside_acl(definition: &FixtureDefinition) -> PatchedFixture {
    let positions = [
        (-5.2, -34.0),
        (-4.75, -22.0),
        (-4.3, -10.0),
        (-3.85, 2.0),
        (3.85, -2.0),
        (4.3, 10.0),
        (4.75, 22.0),
        (5.2, 34.0),
    ];
    let mut fixture = fixture(
        "Outside ACL Set".into(),
        29,
        definition,
        (positions[0].0, 6.65, 3.8),
        positions[0].1,
    );
    fixture.multipatch = positions
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(index, (x, rotation))| {
            instance(
                format!("Outside ACL {}", index + 1),
                (x, 6.65, 3.8),
                rotation,
            )
        })
        .collect();
    fixture
}

fn rgb_grid(definition: &FixtureDefinition) -> PatchedFixture {
    let positions = [
        (-2.25, 3.7),
        (-0.75, 3.7),
        (0.75, 3.7),
        (2.25, 3.7),
        (-2.25, 4.35),
        (-0.75, 4.35),
        (0.75, 4.35),
        (2.25, 4.35),
    ];
    let mut fixture = fixture(
        "Overhead RGB Multi-patch".into(),
        999,
        definition,
        (positions[0].0, positions[0].1, 5.2),
        0.0,
    );
    fixture.multipatch = positions
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(index, (x, y))| instance(format!("Overhead RGB {}", index + 1), (x, y, 5.2), 0.0))
        .collect();
    fixture
}

fn default_fixtures() -> Vec<PatchedFixture> {
    let definitions = default_definitions();
    let mut fixtures = ordinary_fixtures(&definitions);
    fixtures.extend([
        middle_acl(&definitions.acl),
        outside_acl(&definitions.acl),
        rgb_grid(&definitions.rgb_multipatch),
    ]);
    fixtures.extend(fixture_line(FixtureLine {
        name: "Back Trackspot",
        first_name: 1,
        first_number: 301,
        definition: &definitions.scanner,
        x: &[-4.5, -1.5, 1.5, 4.5],
        y: 6.15,
        z: 3.25,
        rotation_y: 0.0,
    }));
    fixtures.push(fixture(
        "Stage Hazer".into(),
        99,
        &definitions.hazer,
        (5.5, 7.7, 0.25),
        0.0,
    ));
    fixtures
}

fn position(location: FixtureLocation, rotation_y: f32) -> serde_json::Value {
    json!({
        "x": location.x as f32 / 1000.0,
        "y": location.y as f32 / 1000.0,
        "z": location.z as f32 / 1000.0,
        "rotationX": 0,
        "rotationY": rotation_y,
        "rotationZ": 0,
    })
}

fn stage_positions(fixtures: &[PatchedFixture]) -> serde_json::Map<String, serde_json::Value> {
    let fixtures_positions = fixtures.iter().map(|fixture| {
        (
            fixture.fixture_id.0.to_string(),
            position(fixture.location, fixture.rotation.y),
        )
    });
    let instance_positions = fixtures.iter().flat_map(|fixture| {
        fixture.multipatch.iter().map(|instance| {
            (
                instance.id.to_string(),
                position(instance.location, instance.rotation.y),
            )
        })
    });
    fixtures_positions.chain(instance_positions).collect()
}

fn persist_default_fixtures(
    store: &ShowStore,
    fixtures: &[PatchedFixture],
) -> Result<(), StoreError> {
    for fixture in fixtures {
        store.put_object(
            "patched_fixture",
            &fixture.fixture_id.0.to_string(),
            &serde_json::to_value(fixture)?,
            0,
        )?;
    }
    let positions3d = stage_positions(fixtures);
    store.put_object(
        "stage_layout",
        "main",
        &json!({"version":2,"positions":{},"positions3d":positions3d}),
        0,
    )?;
    Ok(())
}

pub(super) fn initialise(path: impl AsRef<Path>) -> Result<light_core::ShowId, StoreError> {
    if path.as_ref().exists() {
        return ShowStore::open(path)?.id();
    }
    let (store, show_id) = ShowStore::create(path, DEFAULT_SHOW_NAME)?;
    persist_default_fixtures(&store, &default_fixtures())?;
    Ok(show_id)
}
