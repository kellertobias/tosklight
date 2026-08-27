//! Showing a preload on top of what is actually lit.
//!
//! A preload is what the rig is *about* to do: values an operator has set but not yet released.
//! The desk resolves it for its own Stage as colour and intensity — enough to answer "which
//! fixtures are coming up, and in what" — but a renderer drawing real beams needs where a moving
//! head will point and what it will point through, and none of that is in a colour.
//!
//! So the preload is not a second picture. It is an overlay: the live rig is drawn from the desk's
//! output universes exactly as always, and the preload's own static values are laid over the
//! fixtures that have them. A fixture with nothing preloaded keeps showing what it is doing now,
//! and one that is preloaded shows what it is about to do, in every attribute the preload names.
//!
//! Dynamics are deliberately not applied. A dynamic is a running function rather than a value, and
//! reproducing one here would mean a second implementation of something the desk already owns —
//! and a second implementation of a moving target is one that will eventually disagree. A preloaded
//! dynamic therefore shows its fixture's live state, which is the honest answer.

use viz_scene::{EmitterValues, Scene, SceneValues};

/// One attribute of one fixture, as the desk's preload projection reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct PreloadValue {
    pub fixture_id: uuid::Uuid,
    /// The desk's own attribute name, `intensity` or `position.pan` or `beam.gobo`.
    pub attribute: String,
    /// Normalized `0..=1`, which is the form every renderer parameter is in.
    pub value: f32,
}

/// Lay the preload over the live values, in place.
///
/// Returns how many emitters were changed, so a caller can tell a preload that did nothing from one
/// that was never applied.
pub fn apply(scene: &Scene, preload: &[PreloadValue], values: &mut SceneValues) -> usize {
    if preload.is_empty() {
        return 0;
    }
    let mut changed = 0;
    for (index, emitter) in scene.emitters.iter().enumerate() {
        let Some(fixture) = scene.fixtures.get(emitter.fixture_index as usize) else {
            continue;
        };
        let Some(slot) = values.emitters.get_mut(index) else {
            continue;
        };
        let mut touched = false;
        for entry in preload
            .iter()
            .filter(|entry| entry.fixture_id == fixture.fixture_id)
        {
            touched |= set(slot, &entry.attribute, entry.value);
        }
        if touched {
            changed += 1;
        }
    }
    changed
}

/// Put one named attribute onto one emitter. False for a name this renderer has no parameter for,
/// which is not a fault: a preload can name anything a fixture has, and what it draws with is a
/// smaller set than what a desk can control.
///
/// The names are the desk's own, from the attribute reference: `pan`, `tilt`, `zoom`, `iris`,
/// `softness`, `gobo.1`. They are not spelled the way this renderer's own parameters are, and the
/// difference is the whole reason a preload used to arrive and do nothing — every name but
/// `intensity` and the three colour mixers fell through to `false`.
fn set(emitter: &mut EmitterValues, attribute: &str, value: f32) -> bool {
    let value = value.clamp(0.0, 1.0);
    match attribute {
        // A preloaded level is what the fixture is about to be at, so the level an observer is
        // still seeing comes with it. Left behind, the persistence-of-vision level holds the
        // fixture at what it was last actually lit at and the preload never shows.
        "intensity" => {
            emitter.intensity = value;
            emitter.held_intensity = value;
        }
        "color.red" => emitter.colour[0] = value,
        "color.green" => emitter.colour[1] = value,
        "color.blue" => emitter.colour[2] = value,
        // Additive emitters the renderer has no separate channel for still carry light, and a
        // preload that names one is saying the fixture is about to be brighter in that direction.
        "color.white" => {
            emitter.colour = [
                emitter.colour[0].max(value),
                emitter.colour[1].max(value),
                emitter.colour[2].max(value),
            ];
        }
        "color.amber" | "color.warm_white" => {
            emitter.colour[0] = emitter.colour[0].max(value);
            emitter.colour[1] = emitter.colour[1].max(value * 0.75);
        }
        // Subtractive filtration is the complement of the mixer it sits in front of.
        "color.cyan" => emitter.colour[0] = 1.0 - value,
        "color.magenta" => emitter.colour[1] = 1.0 - value,
        "color.yellow" => emitter.colour[2] = 1.0 - value,
        "pan" => emitter.pan = value,
        "tilt" => emitter.tilt = value,
        "zoom" => emitter.zoom = value,
        "iris" => emitter.iris = value,
        // The desk calls the fixture's primary diffusion Softness, whichever mechanism the profile
        // chose for it; this renderer draws all of them as frost.
        "softness" | "frost" | "frost.1" | "beam.edge" => emitter.frost = value,
        "focus" => emitter.focus = value,
        "gobo.1" => emitter.gobo = value,
        "gobo.1.rotation" => emitter.gobo_rotation = value.mul_add(2.0, -1.0),
        "prism" | "prism.1" => emitter.prism = value,
        "prism.1.rotation" => emitter.prism_rotation = value.mul_add(2.0, -1.0),
        "shutter" => emitter.shutter = value,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two fixtures, one emitter each. Only the identity and the link between them matter here:
    /// the overlay is about which values land on which emitter, not about geometry.
    fn scene_with_two_fixtures() -> (Scene, uuid::Uuid, uuid::Uuid) {
        use viz_scene::glam::Vec3;
        use viz_scene::{
            BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
            FixtureInstance,
        };
        let mut scene = Scene::default();
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        for fixture_id in [first, second] {
            scene.fixtures.push(FixtureInstance {
                instance_id: uuid::Uuid::new_v4(),
                fixture_id,
                name: String::new(),
                number: None,
                position: Vec3::ZERO,
                rotation_degrees: Vec3::ZERO,
                position_master: None,
                bracket_degrees: 0.0,
                shaper_degrees: None,
                installed_colour: [1.0; 3],
                installed_shaper_angles_degrees: [0.0; 4],
                body: FixtureBody {
                    size: Vec3::splat(0.3),
                    kind: BodyKind::MovingHead,
                },
                patched: true,
                address: None,
                model: None,
                fallback: None,
            });
            scene.emitters.push(EmitterInstance {
                fixture_index: (scene.fixtures.len() - 1) as u32,
                head_index: 0,
                label: String::new(),
                local_origin: Vec3::ZERO,
                tilt_pivot: Vec3::ZERO,
                local_orientation_degrees: Vec3::ZERO,
                pan: None,
                tilt: None,
                beam_angle_degrees: 12.0,
                field_angle_degrees: 36.0,
                optics: EmitterOptics::default(),
                kind: EmitterKind::Beam,
                cells: EmitterLayoutCells::single(),
                laser: None,
                effect: None,
                shaper_roles: [false; 4],
                live_shaper_angle_roles: [false; 4],
                live_shaper_rotation_role: false,
            });
        }
        (scene, first, second)
    }

    fn values_for(scene: &Scene) -> SceneValues {
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values
    }

    /// The point of an overlay: a fixture nobody preloaded goes on showing what it is doing.
    #[test]
    fn only_the_preloaded_fixtures_change() {
        let (scene, first, _second) = scene_with_two_fixtures();
        let mut values = values_for(&scene);
        values.emitters[0].intensity = 0.25;
        values.emitters[1].intensity = 0.75;

        let changed = apply(
            &scene,
            &[PreloadValue {
                fixture_id: first,
                attribute: "intensity".to_owned(),
                value: 1.0,
            }],
            &mut values,
        );

        assert_eq!(changed, 1);
        assert!((values.emitters[0].intensity - 1.0).abs() < 0.001);
        assert!(
            (values.emitters[1].intensity - 0.75).abs() < 0.001,
            "a fixture with nothing preloaded is left alone"
        );
    }

    /// The reason this exists rather than the desk's colour-and-intensity projection: a moving head
    /// has to point where it is about to point.
    #[test]
    fn every_attribute_the_renderer_draws_with_is_carried() {
        let (scene, first, _) = scene_with_two_fixtures();
        let mut values = values_for(&scene);
        let preload = |attribute: &str, value: f32| PreloadValue {
            fixture_id: first,
            attribute: attribute.to_owned(),
            value,
        };
        apply(
            &scene,
            &[
                preload("pan", 0.8),
                preload("tilt", 0.2),
                preload("zoom", 0.6),
                preload("gobo.1", 0.4),
                preload("iris", 0.3),
                preload("softness", 0.7),
                preload("color.red", 1.0),
            ],
            &mut values,
        );
        let lit = &values.emitters[0];
        assert!((lit.pan - 0.8).abs() < 0.001, "a preloaded head points");
        assert!((lit.tilt - 0.2).abs() < 0.001);
        assert!((lit.zoom - 0.6).abs() < 0.001);
        assert!((lit.gobo - 0.4).abs() < 0.001, "and through what it will");
        assert!((lit.iris - 0.3).abs() < 0.001);
        assert!((lit.frost - 0.7).abs() < 0.001);
        assert!((lit.colour[0] - 1.0).abs() < 0.001);
    }

    /// The names have to be the desk's own, from the attribute reference, and nothing else.
    ///
    /// This is what the overlay got wrong: it matched names of its own invention, so a preload
    /// arrived intact, matched nothing but `intensity`, and drew a rig that had not moved. A test
    /// spelling the attributes the same wrong way agreed with it.
    #[test]
    fn the_desk_attribute_names_are_the_ones_that_land() {
        let (scene, first, _) = scene_with_two_fixtures();
        for attribute in [
            "intensity",
            "pan",
            "tilt",
            "zoom",
            "iris",
            "focus",
            "softness",
            "gobo.1",
            "gobo.1.rotation",
            "shutter",
            "color.red",
            "color.green",
            "color.blue",
            "color.white",
            "color.cyan",
        ] {
            let mut values = values_for(&scene);
            let changed = apply(
                &scene,
                &[PreloadValue {
                    fixture_id: first,
                    attribute: attribute.to_owned(),
                    value: 0.5,
                }],
                &mut values,
            );
            assert_eq!(changed, 1, "`{attribute}` is one the renderer draws with");
        }
    }

    /// A preloaded level has to survive persistence of vision, which otherwise holds the fixture
    /// at whatever it was last actually lit at and hides the preload entirely.
    #[test]
    fn a_preloaded_level_comes_up_through_the_held_level() {
        let (scene, first, _) = scene_with_two_fixtures();
        let mut values = values_for(&scene);
        values.emitters[0].intensity = 0.0;
        values.emitters[0].held_intensity = 0.9;
        apply(
            &scene,
            &[PreloadValue {
                fixture_id: first,
                attribute: "intensity".to_owned(),
                value: 0.2,
            }],
            &mut values,
        );
        assert!((values.emitters[0].held_intensity - 0.2).abs() < 0.001);
    }

    /// A desk can control more than a renderer draws with. Naming one of those is not a fault, and
    /// must leave everything else applied.
    #[test]
    fn an_attribute_the_renderer_cannot_draw_is_skipped_rather_than_fatal() {
        let (scene, first, _) = scene_with_two_fixtures();
        let mut values = values_for(&scene);
        let changed = apply(
            &scene,
            &[
                PreloadValue {
                    fixture_id: first,
                    attribute: "control.lamp_on".to_owned(),
                    value: 1.0,
                },
                PreloadValue {
                    fixture_id: first,
                    attribute: "intensity".to_owned(),
                    value: 0.5,
                },
            ],
            &mut values,
        );
        assert_eq!(changed, 1);
        assert!((values.emitters[0].intensity - 0.5).abs() < 0.001);
    }

    /// Nothing preloaded is not the same as a preload of nothing.
    #[test]
    fn an_empty_preload_changes_nothing() {
        let (scene, _, _) = scene_with_two_fixtures();
        let mut values = values_for(&scene);
        values.emitters[0].intensity = 0.4;
        assert_eq!(apply(&scene, &[], &mut values), 0);
        assert!((values.emitters[0].intensity - 0.4).abs() < 0.001);
    }
}
