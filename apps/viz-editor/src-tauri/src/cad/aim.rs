//! Where a lamp's light leaves it and which way it goes, for the plan's direction indicator.
//!
//! The indicator starts at the emitter of the body the fixture is drawn with — its own model, or
//! the shipped default model — scaled to the fixture, turned by the bracket angle about the
//! model's hinge and then by the placement's rotation. Its direction is the emitter's axis turned
//! the same way, which is how the Visualizer aims the lamp: the bracket about the fixture's own
//! transverse axis (+X, `Quat::from_rotation_x`, positive nose-down) after the mounting rotation.
//! A moving head is aimed at its home pan and tilt.
//!
//! Model axes are the Visualizer's renderer axes (x across, y up, z toward the audience); plan axes
//! are the desk's (x across, y upstage, z up), so a model point `(x, y, z)` is `(x, −z, y)` on the
//! plan — the inverse of the Visualizer's `to_world`. Every CAD view draws model geometry the same
//! way, so the indicator leaves the lens the plan draws and points where the 3D view aims it.

use super::CadEntity;
use super::profile_drawing::model_drawing_views;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_application::PatchSnapshot;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use viz_scene::glam::{Quat, Vec3};

/// What the plan's direction indicator needs beyond the entity's position.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadAim {
    /// Degrees the placement's mounting bracket is set to, positive nose-down.
    pub bracket_angle: f32,
    /// Where the light leaves the lamp, in millimetres from the entity's position in plan axes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emitter_offset_millimetres: Option<[f32; 3]>,
}

/// The emitter of the body a profile is drawn with, in the model's own frame at fixture scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterFrame {
    /// Where light leaves, in millimetres.
    pub anchor: Vec3,
    /// Which way it leaves at rest.
    pub axis: Vec3,
    /// What the bracket turns the body about, in millimetres.
    pub hinge: Vec3,
}

impl EmitterFrame {
    /// The emitter offset and direction in plan axes for a placement's rotation and bracket.
    pub fn aimed(&self, rotation_degrees: [f32; 3], bracket_degrees: f32) -> ([f32; 3], [f32; 3]) {
        let bracket = Quat::from_rotation_x(bracket_degrees.to_radians());
        let mount = mounting_rotation(rotation_degrees);
        let local = self.hinge + bracket * (self.anchor - self.hinge);
        let offset = mount * local;
        let direction = (mount * bracket * self.axis).normalize_or(Vec3::NEG_Y);
        (to_plan(offset), to_plan(direction))
    }
}

/// The placement's rotation in model axes: desk `(rx, ry, rz)` as `Rx(rx) · Ry(rz) · Rz(ry)`,
/// exactly as `rotateModelPoint` turns the plan's model geometry.
fn mounting_rotation(rotation: [f32; 3]) -> Quat {
    Quat::from_rotation_x(rotation[0].to_radians())
        * Quat::from_rotation_y(rotation[2].to_radians())
        * Quat::from_rotation_z(rotation[1].to_radians())
}

/// Model (renderer) axes to plan (desk) axes: the inverse of the Visualizer's `(x, z, −y)`.
fn to_plan(point: Vec3) -> [f32; 3] {
    [point.x, -point.z, point.y]
}

/// Where a lamp without a known lens points: the Visualizer's default emitter aims down its local −Y.
const UNKNOWN_EMITTER: EmitterFrame = EmitterFrame {
    anchor: Vec3::ZERO,
    axis: Vec3::NEG_Y,
    hinge: Vec3::ZERO,
};

/// Give every lamp its bracket angle, its emitter offset and the direction it really points.
/// Venue objects and generated scenery keep the direction they were given.
pub fn aim_lamps(mut entities: Vec<CadEntity>, snapshot: &PatchSnapshot) -> Vec<CadEntity> {
    let profiles = snapshot
        .profile_revisions
        .iter()
        .map(|profile| ((profile.profile_id.0, profile.profile_revision), profile))
        .collect::<HashMap<_, _>>();
    let fixtures = snapshot
        .fixtures
        .iter()
        .map(|fixture| (fixture.patch.fixture_id.0, fixture))
        .collect::<HashMap<_, _>>();
    for entity in &mut entities {
        let Some(fixture) = fixtures.get(&entity.logical_fixture_id) else {
            continue;
        };
        entity.aim.bracket_angle = super::scenery::bracket_angle(&fixture.patch, entity.id);
        if entity.kind == "venue" || entity.scenery.is_some() {
            continue;
        }
        let frame = profiles
            .get(&(
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ))
            .and_then(|profile| {
                emitter_frame_cached(
                    &entity.drawing_id,
                    &profile.profile_snapshot,
                    fixture.profile.mode_id,
                )
            });
        let (offset, direction) = frame
            .unwrap_or(UNKNOWN_EMITTER)
            .aimed(entity.rotation_degrees, entity.aim.bracket_angle);
        entity.aim.emitter_offset_millimetres = frame.is_some().then_some(offset);
        entity.output_direction = direction;
    }
    entities
}

/// [`emitter_frame`], read once per profile revision and mode however many placements ask.
fn emitter_frame_cached(
    key: &str,
    snapshot: &serde_json::Value,
    mode_id: uuid::Uuid,
) -> Option<EmitterFrame> {
    static FRAMES: OnceLock<Mutex<HashMap<String, Option<EmitterFrame>>>> = OnceLock::new();
    let frames = FRAMES.get_or_init(Default::default);
    if let Some(frame) = frames.lock().ok()?.get(key) {
        return *frame;
    }
    let frame = serde_json::from_value::<light_fixture::FixtureProfile>(snapshot.clone())
        .ok()
        .and_then(|profile| emitter_frame(&profile, Some(mode_id)));
    frames.lock().ok()?.insert(key.to_owned(), frame);
    frame
}

/// The emitter of the body a profile is drawn with: its own model at its body size, or the
/// shipped default model at the scale the CAD draws it, turned about the hinge its drawings record.
pub fn emitter_frame(
    profile: &light_fixture::FixtureProfile,
    mode_id: Option<uuid::Uuid>,
) -> Option<EmitterFrame> {
    if let Some(chosen) = viz_project::profile_default_model(profile, mode_id) {
        let model = viz_scene::read_glb(chosen.model.bytes).ok()?;
        return frame(&model, chosen.scale, drawing_hinge(chosen.model.name));
    }
    if profile.scenery.is_some() || profile.crowd.is_some() {
        return None;
    }
    let encoded = profile.model_asset.as_deref()?.split_once(',')?.1;
    let model = viz_scene::read_glb(&STANDARD.decode(encoded).ok()?).ok()?;
    let body = viz_project::body_size(
        viz_project::classify(&profile.fixture_type),
        model.has_head,
        profile.physical.width_millimetres,
        profile.physical.height_millimetres,
        profile.physical.depth_millimetres,
    );
    frame(&model, model.scale_to(body), None)
}

fn frame(model: &viz_scene::FixtureModel, scale: f32, hinge: Option<Vec3>) -> Option<EmitterFrame> {
    let anchor = model.emitter_anchor?;
    let millimetres = scale * 1000.0;
    Some(EmitterFrame {
        anchor: anchor * millimetres,
        axis: model
            .emitter_axis
            .unwrap_or(Vec3::NEG_Y)
            .normalize_or(Vec3::NEG_Y),
        hinge: hinge.map_or(Vec3::ZERO, |hinge| hinge * scale),
    })
}

/// The hinge a shipped model's side drawing records, in model millimetres: the page's x is the
/// model's z and the page's y runs down the model's y. The model is symmetric across x.
fn drawing_hinge(model: &str) -> Option<Vec3> {
    let side = model_drawing_views(model)
        .into_iter()
        .find(|view| view.view == "side")?;
    let root = side.svg.split_once('>')?.0;
    let (_, rest) = root.split_once("data-hinge=\"")?;
    let (value, _) = rest.split_once('"')?;
    let mut numbers = value.split_whitespace().map(str::parse::<f32>);
    let (Some(Ok(z)), Some(Ok(down))) = (numbers.next(), numbers.next()) else {
        return None;
    };
    Some(Vec3::new(0.0, -down, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PAR-like lamp: hinge 200 mm below the clamp, lens 300 mm below it, hanging face-down.
    fn par() -> EmitterFrame {
        EmitterFrame {
            anchor: Vec3::new(0.0, -300.0, 0.0),
            axis: Vec3::NEG_Y,
            hinge: Vec3::new(0.0, -200.0, 0.0),
        }
    }

    fn close(left: [f32; 3], right: [f32; 3]) -> bool {
        left.iter().zip(right).all(|(a, b)| (a - b).abs() < 1e-3)
    }

    #[test]
    fn with_no_bracket_the_indicator_leaves_the_lens_straight_down() {
        let (offset, direction) = par().aimed([0.0; 3], 0.0);
        // Plan axes: z is up, so the lens is 300 mm below the position and the beam points down.
        assert!(close(offset, [0.0, 0.0, -300.0]), "{offset:?}");
        assert!(close(direction, [0.0, 0.0, -1.0]), "{direction:?}");
    }

    #[test]
    fn a_bracket_angle_turns_the_beam_and_swings_the_lens_about_the_hinge() {
        let (offset, direction) = par().aimed([0.0; 3], 45.0);
        let (sin, cos) = (45f32.to_radians().sin(), 45f32.to_radians().cos());
        // The lens swings on its 100 mm arm about the hinge; the beam turns the same 45°. A positive
        // bracket turns a face-down lamp's beam toward the Visualizer's −Z, which is upstage (+y).
        assert!(
            close(offset, [0.0, 100.0 * sin, -200.0 - 100.0 * cos]),
            "{offset:?}"
        );
        assert!(close(direction, [0.0, sin, -cos]), "{direction:?}");
        // The same turn the Visualizer gives a bracket: Rx(bracket) after the mount, applied to −Y.
        let visualizer = Quat::from_rotation_x(45f32.to_radians()) * Vec3::NEG_Y;
        assert!(close(direction, to_plan(visualizer)));
    }

    /// The convention the plan shares with the Visualizer: its `to_world` puts a desk point
    /// `(x, y, z)` at renderer `(x, z, −y)`, so renderer +Z — where a lamp turned to face the
    /// audience points — is desk −y, downstage, on the plan.
    #[test]
    fn a_lamp_facing_downstage_has_its_lens_and_indicator_downstage_as_in_the_visualizer() {
        let to_world = |[x, y, z]: [f32; 3]| Vec3::new(x, z, -y);
        let (offset, direction) = par().aimed([0.0; 3], -90.0);
        // Visualizer: the bracket turns the face-down beam to +Z, toward the audience camera.
        let visualizer = Quat::from_rotation_x((-90f32).to_radians()) * Vec3::NEG_Y;
        assert!((visualizer - Vec3::Z).length() < 1e-5, "{visualizer:?}");
        assert!((to_world(direction) - visualizer).length() < 1e-4);
        assert!(
            close(direction, [0.0, -1.0, 0.0]),
            "points downstage: {direction:?}"
        );
        assert!(
            offset[1] < -99.0,
            "the lens swings downstage of the hinge: {offset:?}"
        );
        // The same lamp facing downstage by its mounting rotation instead agrees.
        let (_, rotated) = par().aimed([-90.0, 0.0, 0.0], 0.0);
        assert!(close(rotated, direction), "{rotated:?}");
    }

    #[test]
    fn the_bracket_turns_in_the_lamps_own_frame_after_its_rotation() {
        // Yawed a quarter turn, the bracket still tilts the lamp about its own transverse axis.
        let (_, direction) = par().aimed([0.0, 0.0, 90.0], -90.0);
        assert!(
            direction[2].abs() < 1e-3,
            "no longer looking down: {direction:?}"
        );
        assert!(direction[0].abs() > 0.99, "{direction:?}");
    }

    #[test]
    fn a_shipped_blinder_is_aimed_from_its_lens_and_its_drawing_hinge() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/generic--blinder-4.toskfixture");
        let profile = light_fixture::read_fixture_package(&std::fs::read(package).unwrap())
            .expect("fixture package reads");
        let frame = emitter_frame(&profile, None).expect("a shipped body with a lens");
        assert!(
            frame.anchor.y < -100.0,
            "the lens hangs below the clamp: {frame:?}"
        );
        assert!(
            frame.hinge.y < 0.0,
            "the hinge comes from the drawing: {frame:?}"
        );
        let (level, _) = frame.aimed([0.0; 3], 0.0);
        let (bracketed, _) = frame.aimed([0.0; 3], 45.0);
        assert!(!close(level, bracketed), "the lens moves with the bracket");
    }
}
