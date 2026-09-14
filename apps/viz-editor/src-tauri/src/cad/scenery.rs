//! What the plan needs to know about a generated Venue object: the size it was placed at and how
//! it is built, so a truss is drawn from its own chords and bracing rather than guessed from its
//! name.

use light_fixture::{ChainMode, FixtureVector, PatchedFixturePatch, SceneryOptions};
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
    /// How a chain is rigged: plain, a hoist at the top or a hoist at the bottom. Only a chain has
    /// one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<ChainMode>,
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
        chain: None,
    })
}

/// The size one placed object is drawn at, as width, depth and height in millimetres: the size
/// the operator placed it at, or its profile's own.
pub fn entity_size(
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
                chain: None,
            })
        );
        let standard = json!({ "scenery": { "kind": "truss", "chords": 4 } });
        assert_eq!(profile_scenery(&standard).unwrap().pattern, "standard");
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
}
