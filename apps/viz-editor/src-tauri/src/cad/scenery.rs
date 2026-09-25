//! What the plan needs to know about a generated Venue object: the size it was placed at and how
//! it is built, so a truss is drawn from its own chords and bracing rather than guessed from its
//! name.

use light_fixture::{ChainMode, FixtureVector, PatchedFixturePatch, SceneryOptions};

use super::CadEntity;
use serde::Serialize;
use uuid::Uuid;

/// How a generated Venue object is built, as the plan draws it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadScenery {
    /// The generated kind: `truss`, `curtain`, `chain`, `riser` and so on.
    pub kind: String,
    /// Chords in a truss section: 1 a pipe, 2 a ladder, 3 a triangle, 4 a box.
    pub chords: u8,
    /// A truss's bracing: `standard` or `deco`.
    pub pattern: String,
    /// What a stage element stands on: `scissor` or `fixed`. Only a riser has one.
    pub feet: String,
    /// Which sides of a flight of stairs carry a handrail, as seen climbing it: `none`, `left`,
    /// `right` or `both`. The placement's choice, else what the profile was made with.
    pub handrails: light_fixture::StairHandrails,
    /// How a chain is rigged: plain, a hoist at the top or a hoist at the bottom. Only a chain has
    /// one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainMode>,
    /// What a rigged chain's end away from its hoist is fixed with. Only a chain has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<ChainAnchor>,
}

/// Where a fixture is held, as the plan reasons about it: the clip it hangs by.
///
/// Every measurement is in the entity's own millimetres from the middle of its box — `x` across,
/// `y` deep, `z` up — so the plan can rotate and place it like any other part of the entity. The
/// profile declares the clip against the body it was measured on; this carries it over to the box
/// the entity is actually drawn at, so a fixture placed at another scale keeps its clamp.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadMounting {
    /// What the fixture hangs by: `clamp`, `yoke` or `none`.
    pub hardware: String,
    /// The middle of the clip.
    pub centre: [f32; 3],
    /// Half the clip's reach across, deep and up.
    pub half_extent: [f32; 3],
    /// Where a pipe's axis lies once the fixture hangs from the clip.
    pub pipe: [f32; 3],
}

/// How much of a fixture's height a clamp takes when its profile declares none.
///
/// A profile written before clips could be declared still has to hang, and this is the guess the
/// plan made for every fixture before it: a slice off the top of the body, which is where the
/// hardware sits on nearly everything that flies. A declared clip replaces it.
const GUESSED_SHARE: f32 = 0.25;
const GUESSED_FLOOR: f32 = 40.0;
const GUESSED_CAP: f32 = 300.0;

/// The clip one placement hangs by, at the size that placement is drawn.
pub fn cad_mounting(profile: Option<&serde_json::Value>, size: [f32; 3]) -> Option<CadMounting> {
    let [width, depth, height] = size;
    if !(width > 0.0 && depth > 0.0 && height > 0.0) {
        return None;
    }
    let Some(declared) = profile.and_then(|profile| profile.get("mounting")) else {
        let slice = (height * GUESSED_SHARE).clamp(GUESSED_FLOOR, GUESSED_CAP);
        return Some(CadMounting {
            hardware: "clamp".to_owned(),
            centre: [0.0, 0.0, (height - slice) / 2.0],
            half_extent: [width / 2.0, depth / 2.0, slice / 2.0],
            pipe: [0.0, 0.0, height / 2.0],
        });
    };
    let hardware = declared
        .get("hardware")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("clamp")
        .to_owned();
    // The clip was authored against one body; the entity may be drawn at another, so each axis is
    // carried over in the proportion it was measured in. A body of zero leaves it as it stands.
    let body = vector(declared.get("body_millimetres"));
    let ratio = [
        if body[0] > 0.0 { width / body[0] } else { 1.0 },
        if body[1] > 0.0 { depth / body[1] } else { 1.0 },
        if body[2] > 0.0 { height / body[2] } else { 1.0 },
    ];
    let carried = |key: &str| {
        let value = vector(declared.get(key));
        [
            value[0] * ratio[0],
            value[1] * ratio[1],
            value[2] * ratio[2],
        ]
    };
    Some(CadMounting {
        hardware,
        centre: carried("centre_millimetres"),
        half_extent: carried("half_extent_millimetres"),
        pipe: carried("pipe_millimetres"),
    })
}

/// A declared `{ x, y, z }` in millimetres, as the plan's across, deep and up.
fn vector(value: Option<&serde_json::Value>) -> [f32; 3] {
    let read = |key: &str| {
        value
            .and_then(|value| value.get(key))
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32
    };
    [read("x"), read("y"), read("z")]
}

/// What the end of a chain away from its hoist is fixed with, decided by what hangs there.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainAnchor {
    /// A steelflex wrapped round a chord of a three- or four-point truss.
    Steelflex,
    /// A flange clamped round a pipe or a ladder truss's tube.
    Flange,
    /// A shackle made fast straight to the steel: nothing to wrap or clamp within reach.
    Shackle,
}

/// How far from a chain's free end a truss or pipe still counts as what that end hangs from.
const ANCHOR_REACH_MILLIMETRES: f32 = 500.0;

/// Give every rigged chain the fixing its free end needs: a steelflex on a three- or four-point
/// truss within reach, a flange on a pipe or ladder truss, and a plain shackle otherwise. A truss's
/// run is read from its size and its yaw, which is how trusses are flown.
pub fn connect_chains(mut entities: Vec<CadEntity>) -> Vec<CadEntity> {
    let rigging: Vec<Rigging> = entities.iter().filter_map(Rigging::of).collect();
    for entity in &mut entities {
        let end = match entity.scenery.as_ref().and_then(|scenery| scenery.chain) {
            Some(ChainMode::MotorTop) => -1.0,
            Some(ChainMode::MotorBottom) => 1.0,
            Some(ChainMode::Plain) | None => continue,
        };
        let [x, y, z] = entity.position_millimetres.map(|value| value as f32);
        let point = [x, y, z + end * entity.size_millimetres[2] / 2.0];
        let nearest = rigging
            .iter()
            .filter_map(|rigging| {
                let distance = rigging.distance(point);
                (distance <= ANCHOR_REACH_MILLIMETRES).then_some((distance, rigging.chords))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0));
        if let Some(scenery) = entity.scenery.as_mut() {
            scenery.anchor = Some(match nearest {
                Some((_, chords)) if chords >= 3 => ChainAnchor::Steelflex,
                Some(_) => ChainAnchor::Flange,
                None => ChainAnchor::Shackle,
            });
        }
    }
    entities
}

/// A truss or pipe as a chain end reaches for it: the line it runs along and how thick it is.
struct Rigging {
    centre: [f32; 3],
    along: [f32; 3],
    length: f32,
    section: f32,
    chords: u8,
}

impl Rigging {
    fn of(entity: &CadEntity) -> Option<Self> {
        let scenery = entity
            .scenery
            .as_ref()
            .filter(|scenery| scenery.kind == "truss")?;
        let [width, depth, height] = entity.size_millimetres;
        let yaw = entity.rotation_degrees[2].to_radians();
        let (along, length, section) = if width >= depth && width >= height {
            ([yaw.cos(), yaw.sin(), 0.0], width, depth.max(height))
        } else if depth >= height {
            ([-yaw.sin(), yaw.cos(), 0.0], depth, width.max(height))
        } else {
            ([0.0, 0.0, 1.0], height, width.max(depth))
        };
        Some(Self {
            centre: entity.position_millimetres.map(|value| value as f32),
            along,
            length,
            section,
            chords: scenery.chords,
        })
    }

    /// How far `point` is from the outside of this run, in millimetres.
    fn distance(&self, point: [f32; 3]) -> f32 {
        let offset: [f32; 3] = std::array::from_fn(|axis| point[axis] - self.centre[axis]);
        let t = (0..3)
            .map(|axis| offset[axis] * self.along[axis])
            .sum::<f32>()
            .clamp(-self.length / 2.0, self.length / 2.0);
        let gap = (0..3)
            .map(|axis| (offset[axis] - self.along[axis] * t).powi(2))
            .sum::<f32>()
            .sqrt();
        gap - self.section / 2.0
    }
}

/// The generated object one placement shows, with how the operator rigged it, or `None` for a
/// fixture or a modelled object.
pub fn cad_scenery(
    profile: Option<&serde_json::Value>,
    options: &SceneryOptions,
) -> Option<CadScenery> {
    let mut scenery = profile_scenery(profile?)?;
    if scenery.kind == "chain" {
        scenery.chain = Some(options.chain_mode());
    }
    scenery.handrails = options.stair_handrails(scenery.handrails.left());
    Some(scenery)
}

/// The generated object a profile describes, before any placement's choices.
fn profile_scenery(profile: &serde_json::Value) -> Option<CadScenery> {
    let scenery = profile.get("scenery")?;
    let text = |key: &str, fallback: &str| {
        scenery
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    };
    Some(CadScenery {
        kind: text("kind", "prop"),
        chords: scenery
            .get("chords")
            .and_then(serde_json::Value::as_u64)
            .map_or(0, |chords| chords.min(4) as u8),
        pattern: text("pattern", "standard"),
        feet: text("feet", "scissor"),
        handrails: if scenery
            .get("handrails")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            light_fixture::StairHandrails::Both
        } else {
            light_fixture::StairHandrails::None
        },
        chain: None,
        anchor: None,
    })
}

/// The size one placed object is drawn at, as width, depth and height in millimetres: the size
/// the operator placed it at, or its profile's own, times the model scale it was placed at.
pub fn entity_size(
    profile: Option<&serde_json::Value>,
    fixture: &PatchedFixturePatch,
    instance: Uuid,
) -> [f32; 3] {
    let scale = light_fixture::resolved_model_scale(fixture.model_scale);
    unscaled_entity_size(profile, fixture, instance).map(|value| value * scale)
}

fn unscaled_entity_size(
    profile: Option<&serde_json::Value>,
    fixture: &PatchedFixturePatch,
    instance: Uuid,
) -> [f32; 3] {
    let placed = fixture
        .multipatch
        .iter()
        .find(|candidate| candidate.id == instance)
        .map_or(fixture.scenery_size_metres, |candidate| {
            candidate.scenery_size_metres
        });
    match placed {
        Some(size) if [size.x, size.y, size.z].iter().all(|value| *value > 0.0) => {
            placed_millimetres(size)
        }
        _ => profile.map_or([500.0; 3], dimensions),
    }
}

/// The degrees one placement's mounting bracket is set to: a multi-patch instance's own, or the
/// fixture's for the root placement.
pub fn bracket_angle(fixture: &PatchedFixturePatch, instance: Uuid) -> f32 {
    fixture
        .multipatch
        .iter()
        .find(|candidate| candidate.id == instance)
        .map_or(fixture.bracket_angle, |candidate| candidate.bracket_angle)
}

/// A profile's "Manufacturer Name" as the plan labels it.
pub fn profile_label(profile: &serde_json::Value) -> String {
    let text = |key: &str| profile.get(key).and_then(serde_json::Value::as_str);
    let label = format!(
        "{} {}",
        text("manufacturer").unwrap_or_default(),
        text("name").unwrap_or("Unknown fixture")
    );
    label.trim().to_owned()
}

/// A placed size is stored in millimetres as x across, y up and z deep, like every other
/// measurement the patch carries.
fn placed_millimetres(size: FixtureVector) -> [f32; 3] {
    [size.x, size.z, size.y].map(|value| value.max(20.0))
}

fn dimensions(profile: &serde_json::Value) -> [f32; 3] {
    let physical = profile.get("physical").unwrap_or(profile);
    [
        number(physical, "width_millimetres", 500.0),
        number(physical, "depth_millimetres", 500.0),
        number(physical, "height_millimetres", 500.0),
    ]
}

fn number(value: &serde_json::Value, key: &str, fallback: f32) -> f32 {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .map_or(fallback, |value| value as f32)
        .max(20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_truss_profile_tells_the_plan_its_chords_and_bracing() {
        let deco = json!({ "scenery": { "kind": "truss", "chords": 3, "pattern": "deco" } });
        assert_eq!(
            profile_scenery(&deco),
            Some(CadScenery {
                kind: "truss".into(),
                chords: 3,
                pattern: "deco".into(),
                feet: "scissor".into(),
                handrails: light_fixture::StairHandrails::None,
                chain: None,
                anchor: None,
            })
        );
        let standard = json!({ "scenery": { "kind": "truss", "chords": 4 } });
        assert_eq!(profile_scenery(&standard).unwrap().pattern, "standard");
        // A deck says what it stands on; a riser written before the choice existed is a lift.
        let deck = json!({ "scenery": { "kind": "riser", "feet": "fixed" } });
        assert_eq!(profile_scenery(&deck).unwrap().feet, "fixed");
        let lift = json!({ "scenery": { "kind": "riser" } });
        assert_eq!(profile_scenery(&lift).unwrap().feet, "scissor");
        // A flight of stairs is its own kind, and says whether it carries a rail up each side.
        let railed = json!({ "scenery": { "kind": "stairs", "handrails": true } });
        let flight = profile_scenery(&railed).unwrap();
        assert_eq!(flight.kind, "stairs");
        assert_eq!(flight.handrails, light_fixture::StairHandrails::Both);
        let plain = json!({ "scenery": { "kind": "stairs" } });
        assert_eq!(
            profile_scenery(&plain).unwrap().handrails,
            light_fixture::StairHandrails::None
        );
        // The placement chooses the sides, over either profile.
        let mut options = light_fixture::SceneryOptions::default();
        options.handrails = Some(light_fixture::StairHandrails::Left);
        assert_eq!(
            cad_scenery(Some(&plain), &options).unwrap().handrails,
            light_fixture::StairHandrails::Left
        );
        options.handrails = Some(light_fixture::StairHandrails::None);
        assert_eq!(
            cad_scenery(Some(&railed), &options).unwrap().handrails,
            light_fixture::StairHandrails::None
        );
        assert_eq!(profile_scenery(&json!({ "name": "Spot" })), None);
    }

    #[test]
    fn a_chain_tells_the_plan_how_it_is_rigged_and_nothing_else_does() {
        let chain = json!({ "scenery": { "kind": "chain" } });
        let mut options = SceneryOptions::default();
        let rigged = |options: &SceneryOptions| cad_scenery(Some(&chain), options).unwrap();
        assert_eq!(rigged(&options).chain, Some(ChainMode::MotorTop));
        for mode in [ChainMode::Plain, ChainMode::MotorBottom] {
            options.set_chain_mode(mode);
            assert_eq!(rigged(&options).chain, Some(mode));
        }
        let wire = serde_json::to_value(rigged(&options)).unwrap();
        assert_eq!(wire["chain"], "motor_bottom");

        let truss = json!({ "scenery": { "kind": "truss", "chords": 4 } });
        let truss = cad_scenery(Some(&truss), &options).unwrap();
        assert_eq!(truss.chain, None);
        assert!(serde_json::to_value(truss).unwrap().get("chain").is_none());
        assert_eq!(cad_scenery(None, &options), None);
    }

    #[test]
    fn a_placed_size_wins_over_the_profile_and_turns_into_plan_axes() {
        let profile = json!({ "physical": {
            "width_millimetres": 4000.0, "depth_millimetres": 340.0, "height_millimetres": 340.0
        } });
        assert_eq!(
            placed_millimetres(FixtureVector {
                x: 6000.0,
                y: 290.0,
                z: 340.0
            }),
            [6000.0, 340.0, 290.0]
        );
        assert_eq!(dimensions(&profile), [4000.0, 340.0, 340.0]);
    }

    #[test]
    fn a_model_scale_multiplies_the_placed_or_profile_size_in_the_plan() {
        let hall = json!({ "physical": {
            "width_millimetres": 20000.0, "depth_millimetres": 10000.0, "height_millimetres": 4000.0
        } });
        let id = Uuid::new_v4();
        let patch = |value: serde_json::Value| -> PatchedFixturePatch {
            serde_json::from_value(value).expect("patch")
        };
        let built = patch(json!({ "fixture_id": id }));
        assert_eq!(
            entity_size(Some(&hall), &built, id),
            [20000.0, 10000.0, 4000.0]
        );
        let half = patch(json!({ "fixture_id": id, "model_scale": 0.5 }));
        assert_eq!(
            entity_size(Some(&hall), &half, id),
            [10000.0, 5000.0, 2000.0]
        );
        let placed = patch(json!({
            "fixture_id": id,
            "model_scale": 2.0,
            "scenery_size_metres": { "x": 6000.0, "y": 290.0, "z": 340.0 }
        }));
        assert_eq!(entity_size(None, &placed, id), [12000.0, 680.0, 580.0]);
        // A scale no patch could have stored draws the object at the size it was built.
        let broken = patch(json!({ "fixture_id": id, "model_scale": 0.0 }));
        assert_eq!(
            entity_size(Some(&hall), &broken, id),
            [20000.0, 10000.0, 4000.0]
        );
    }

    fn placed(
        kind: &str,
        chords: u8,
        chain: Option<ChainMode>,
        z: i32,
        size: [f32; 3],
    ) -> CadEntity {
        CadEntity {
            id: Uuid::new_v4(),
            logical_fixture_id: Uuid::new_v4(),
            name: kind.into(),
            fixture_number: None,
            fixture_display_id: "0.1".into(),
            dmx_address: "Visual only".into(),
            fixture_profile: String::new(),
            mode: String::new(),
            note: String::new(),
            kind: "venue".into(),
            fixture_type: "rigging".into(),
            drawing_id: String::new(),
            layer_id: String::new(),
            selectable: true,
            position_millimetres: [0, 0, z],
            rotation_degrees: [0.0; 3],
            size_millimetres: size,
            output_direction: [0.0; 3],
            aim: Default::default(),
            scenery: Some(CadScenery {
                kind: kind.into(),
                chords,
                pattern: "standard".into(),
                feet: "scissor".into(),
                handrails: light_fixture::StairHandrails::None,
                chain,
                anchor: None,
            }),
            mounting: None,
            imported_model: false,
            position_reference: None,
        }
    }

    #[test]
    fn a_chain_is_fixed_by_what_its_free_end_hangs_from() {
        let chain = |mode| placed("chain", 0, Some(mode), 3000, [100.0, 100.0, 2000.0]);
        let anchor_of = |entities: Vec<CadEntity>| {
            connect_chains(entities)
                .into_iter()
                .find(|entity| entity.name == "chain")
                .and_then(|entity| entity.scenery)
                .and_then(|scenery| scenery.anchor)
        };
        // The hoist is on top, so the free end is 2 m up, 55 mm above a box truss's top.
        let truss = placed("truss", 4, None, 1800, [4000.0, 290.0, 290.0]);
        assert_eq!(
            anchor_of(vec![chain(ChainMode::MotorTop), truss.clone()]),
            Some(ChainAnchor::Steelflex)
        );
        let pipe = placed("truss", 1, None, 1900, [3000.0, 50.0, 50.0]);
        assert_eq!(
            anchor_of(vec![chain(ChainMode::MotorTop), pipe]),
            Some(ChainAnchor::Flange)
        );
        let ladder = placed("truss", 2, None, 1800, [3000.0, 290.0, 290.0]);
        assert_eq!(
            anchor_of(vec![chain(ChainMode::MotorTop), ladder]),
            Some(ChainAnchor::Flange)
        );
        let far = placed("truss", 4, None, 0, [4000.0, 290.0, 290.0]);
        assert_eq!(
            anchor_of(vec![chain(ChainMode::MotorTop), far]),
            Some(ChainAnchor::Shackle)
        );
        // With the hoist at the bottom, the free end is the top one.
        let above = placed("truss", 3, None, 4200, [4000.0, 290.0, 290.0]);
        assert_eq!(
            anchor_of(vec![chain(ChainMode::MotorBottom), above, truss]),
            Some(ChainAnchor::Steelflex)
        );
        assert_eq!(anchor_of(vec![chain(ChainMode::Plain)]), None);
        assert_eq!(
            serde_json::to_value(ChainAnchor::Steelflex).unwrap(),
            "steelflex"
        );
    }

    /// A profile that says nothing about how it hangs still hangs: the plan guesses a clamp off
    /// the top of the body, which is what every fixture got before clips were declared.
    #[test]
    fn a_fixture_with_no_declared_clip_keeps_the_clamp_the_plan_always_guessed() {
        let clip = cad_mounting(None, [1000.0, 1000.0, 1000.0]).expect("a box has a clamp");
        assert_eq!(clip.hardware, "clamp");
        assert_eq!(clip.centre, [0.0, 0.0, 375.0]);
        assert_eq!(clip.half_extent, [500.0, 500.0, 125.0]);
        assert_eq!(clip.pipe, [0.0, 0.0, 500.0]);
    }

    /// A declared clip is carried over to the body the fixture is actually drawn at, so a lamp
    /// placed at twice its size keeps its clamp on its own top rather than half way down.
    #[test]
    fn a_declared_clip_is_carried_over_to_the_size_the_fixture_is_drawn_at() {
        let profile = serde_json::json!({
            "mounting": {
                "hardware": "clamp",
                "centre_millimetres": { "x": 0.0, "y": 0.0, "z": 120.0 },
                "half_extent_millimetres": { "x": 150.0, "y": 200.0, "z": 30.0 },
                "pipe_millimetres": { "x": 0.0, "y": 0.0, "z": 150.0 },
                "body_millimetres": { "x": 300.0, "y": 400.0, "z": 300.0 },
            }
        });
        let same = cad_mounting(Some(&profile), [300.0, 400.0, 300.0]).unwrap();
        assert_eq!(same.pipe, [0.0, 0.0, 150.0]);
        assert_eq!(same.half_extent, [150.0, 200.0, 30.0]);
        let doubled = cad_mounting(Some(&profile), [600.0, 800.0, 600.0]).unwrap();
        assert_eq!(doubled.pipe, [0.0, 0.0, 300.0]);
        assert_eq!(doubled.half_extent, [300.0, 400.0, 60.0]);
    }

    /// A fixture that hangs from nothing says so, and the plan reads it back as such.
    #[test]
    fn a_fixture_that_hangs_from_nothing_carries_that_answer_through() {
        let profile = serde_json::json!({ "mounting": { "hardware": "none" } });
        let clip = cad_mounting(Some(&profile), [500.0, 500.0, 500.0]).unwrap();
        assert_eq!(clip.hardware, "none");
    }
}
