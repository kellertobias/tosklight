//! Where a shipped lamp's body turns in its hanging frame.
//!
//! The model manifest (`assets/models/manifest.json`) records it as the `hanging-frame` swivel; a
//! GLB does not carry it. This is the one reading of that record: the line drawings the CAD turns
//! by the bracket angle, the CAD's aim indicator and the Visualizer's body all take their hinge
//! from here, so the plan and the 3D view turn the body about the same point.

use glam::Vec3;
use std::collections::HashMap;
use std::sync::OnceLock;

const MANIFEST: &str = include_str!("../../../../assets/models/manifest.json");

/// The hinge one manifest entry records for its hanging frame, in model metres.
pub fn manifest_bracket_hinge(entry: &serde_json::Value) -> Option<Vec3> {
    let swivel = entry["swivels"]
        .as_array()?
        .iter()
        .find(|swivel| swivel["node"] == "hanging-frame")?;
    let point = swivel["point_metres"].as_array()?;
    Some(Vec3::new(
        point.first()?.as_f64()? as f32,
        point.get(1)?.as_f64()? as f32,
        point.get(2)?.as_f64()? as f32,
    ))
}

/// The hinge of the shipped model named `model`, in model metres, or `None` when it has no
/// hanging frame to turn in. A `-no-clamp` variant turns about its clamped sibling's hinge when it
/// records none of its own.
pub fn bracket_hinge(model: &str) -> Option<Vec3> {
    static HINGES: OnceLock<HashMap<String, Vec3>> = OnceLock::new();
    let hinges = HINGES.get_or_init(|| {
        serde_json::from_str::<serde_json::Value>(MANIFEST)
            .ok()
            .and_then(|manifest| {
                Some(
                    manifest["models"]
                        .as_array()?
                        .iter()
                        .filter_map(|entry| {
                            Some((
                                entry["model"].as_str()?.to_owned(),
                                manifest_bracket_hinge(entry)?,
                            ))
                        })
                        .collect(),
                )
            })
            .unwrap_or_default()
    });
    hinges.get(model).copied().or_else(|| {
        model
            .strip_suffix("-no-clamp")
            .and_then(|base| hinges.get(base).copied())
    })
}

/// Read a shipped model with the hinge its manifest records.
pub fn read_shipped_model(
    model: &crate::DefaultModel,
) -> Result<viz_scene::FixtureModel, viz_scene::ModelError> {
    let mut read = viz_scene::read_glb(model.bytes)?;
    read.bracket_hinge = bracket_hinge(model.name);
    Ok(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fresnel_turns_about_its_bracket_bolts_with_or_without_a_clamp() {
        let hinge = bracket_hinge("fresnel-barn-doors").expect("the Fresnel has a hanging frame");
        assert!(
            (hinge - Vec3::new(0.0, -0.353, 0.0)).length() < 1e-4,
            "{hinge:?}"
        );
        assert_eq!(bracket_hinge("fresnel-barn-doors-no-clamp"), Some(hinge));
        let read = read_shipped_model(&crate::default_model::FRESNEL).expect("the model reads");
        assert_eq!(
            read.bracket_hinge,
            bracket_hinge(crate::default_model::FRESNEL.name)
        );
        assert!(read.bracket_hinge.is_some());
    }

    #[test]
    fn a_model_without_a_hanging_frame_has_no_hinge() {
        assert_eq!(bracket_hinge("no-such-model"), None);
        assert_eq!(bracket_hinge("moving-head-wash"), None);
    }
}
