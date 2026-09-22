//! The shipped body a CAD drawing stands in for, when a profile carries no model of its own.
//!
//! The Visualizer draws such a fixture with a shipped default model (see `default_model`), fitted
//! to the fixture's body size. A plan or an elevation of the same rig has to show the same body at
//! the same size, so this answers both questions the way the scene compiler does: which model, and
//! the uniform scale from that model's own millimetres to the fixture's.

use crate::default_model::{self, DefaultModel, FixtureTraits};
use glam::Vec3;
use light_fixture::{FixtureMode, FixtureProfile, GeometryMotionKind};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

/// A shipped model chosen for a profile, and the scale it is drawn at.
#[derive(Clone, Copy)]
pub struct ProfileDefaultModel {
    pub model: &'static DefaultModel,
    /// Multiplies the model's own coordinates to the fixture's physical size.
    pub scale: f32,
    /// The body the model is fitted to, in metres: the profile's own size, or what its family of
    /// bodies falls back to when the profile declares none.
    pub body_size_metres: Vec3,
}

/// The shipped model the Visualizer draws this profile with, or `None` when the profile brings its
/// own model or is generated scenery (a truss, curtain, chain, riser or crowd), which is built
/// rather than drawn from a body.
///
/// `mode_id` is the patched mode; a mode the profile no longer has reads as its first mode.
pub fn profile_default_model(
    profile: &FixtureProfile,
    mode_id: Option<Uuid>,
) -> Option<ProfileDefaultModel> {
    if profile.model_asset.is_some() || profile.scenery.is_some() || profile.crowd.is_some() {
        return None;
    }
    let mode = mode_id
        .and_then(|id| profile.modes.iter().find(|mode| mode.id == id))
        .or_else(|| profile.modes.first());
    let traits = mode.map(traits).unwrap_or_default();
    let model = default_model::choose(profile.body_model.as_deref(), &profile.fixture_type, traits);
    let moving = mode.is_some_and(|mode| has_motion(profile, mode));
    let body_size = crate::fallback::body_size(
        crate::fallback::classify(&profile.fixture_type),
        moving,
        profile.physical.width_millimetres,
        profile.physical.height_millimetres,
        profile.physical.depth_millimetres,
    );
    let scale = model_extent(model).map_or(1.0, |extent| {
        viz_scene::FixtureModel {
            extent,
            ..Default::default()
        }
        .scale_to(body_size)
    });
    Some(ProfileDefaultModel {
        model,
        scale,
        body_size_metres: body_size,
    })
}

/// What a mode has channels for, read the same way the scene compiler reads it.
fn traits(mode: &FixtureMode) -> FixtureTraits {
    let mut traits = FixtureTraits::default();
    for channel in &mode.channels {
        traits.observe(&channel.attribute.0, channel.attribute.is_intensity());
    }
    traits
}

/// Whether the mode's geometry pans or tilts, which is what sizes a body as a moving head.
fn has_motion(profile: &FixtureProfile, mode: &FixtureMode) -> bool {
    profile.mode_geometry(mode).nodes.iter().any(|node| {
        node.motion.as_ref().is_some_and(|motion| {
            motion.kind == GeometryMotionKind::Rotation
                && matches!(
                    motion.attribute.as_ref().map(|attribute| &*attribute.0),
                    Some("pan" | "tilt")
                )
        })
    })
}

/// The model's half-extent, read once per shipped model however many profiles ask.
fn model_extent(model: &'static DefaultModel) -> Option<Vec3> {
    static EXTENTS: OnceLock<Mutex<HashMap<&'static str, Option<Vec3>>>> = OnceLock::new();
    let extents = EXTENTS.get_or_init(Default::default);
    if let Some(extent) = extents.lock().ok()?.get(model.name) {
        return *extent;
    }
    let extent = viz_scene::read_glb(model.bytes)
        .ok()
        .map(|parsed| parsed.extent);
    extents.lock().ok()?.insert(model.name, extent);
    extent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(fixture_type: &str) -> FixtureProfile {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/venue--stage-railing-2-m.toskfixture");
        let mut profile = light_fixture::read_fixture_package(&std::fs::read(package).unwrap())
            .expect("fixture package reads");
        profile.model_asset = None;
        profile.projection_assets = None;
        profile.scenery = None;
        profile.crowd = None;
        profile.body_model = None;
        profile.fixture_type = fixture_type.into();
        profile
    }

    #[test]
    fn a_profile_without_a_model_gets_the_body_the_visualizer_draws() {
        let mut blinder = profile("blinder");
        blinder.physical.width_millimetres = Some(480.0);
        blinder.physical.height_millimetres = Some(480.0);
        blinder.physical.depth_millimetres = Some(200.0);
        let chosen = profile_default_model(&blinder, None).expect("a shipped body");
        assert_eq!(
            chosen.model.name,
            default_model::choose(None, "blinder", FixtureTraits::default()).name
        );
        assert!(
            chosen.scale > 0.05 && chosen.scale < 20.0,
            "{}",
            chosen.scale
        );
    }

    #[test]
    fn a_named_body_model_wins_and_scenery_or_a_model_asset_gets_none() {
        let mut named = profile("fixture");
        named.body_model = Some("par-64-short-nose-black".into());
        assert_eq!(
            profile_default_model(&named, None).unwrap().model.name,
            "par-64-short-nose-black"
        );
        let mut modelled = profile("fixture");
        modelled.model_asset = Some("data:model/gltf-binary;base64,".into());
        assert!(profile_default_model(&modelled, None).is_none());
    }
}
