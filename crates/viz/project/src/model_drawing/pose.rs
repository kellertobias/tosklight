//! The pose each shipped model is drawn in, view by view, where a plan reads it better turned in
//! its hanging frame than hanging straight down.
//!
//! - PARs and Fresnels point forward in the top view, so they show their length.
//! - Blinders, flat LED PARs, strobes and the flood face the audience (+Z) in the top and front
//!   views and lean [`DEFAULT_LEAN_DEGREES`] from vertical in the side view, the way they are rigged.
//! - LED wash moving heads look at the viewer in the front view instead of pointing down.

use glam::{Quat, Vec3};

/// How far a face-forward lamp's side drawing leans from vertical toward the viewer — its beam that
/// far below level. It is only the pose the file is drawn in: the drawing records it as the bracket
/// angle it equals (−70°), and the CAD turns the body to the fixture's own bracket angle.
pub const DEFAULT_LEAN_DEGREES: f32 = 20.0;

/// How one drawing poses a model, all in the model's own metres and axes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawingPose {
    /// Where the body turns in its hanging frame. A drawing with a hinge keeps the hanging
    /// hardware apart from the body, so a consumer can turn the body alone.
    pub hinge: Option<Vec3>,
    /// The direction the body's light leaves in once turned about the hinge.
    pub body_target: Option<Vec3>,
    /// The direction a moving head points, instead of the view's usual pose.
    pub head_target: Option<Vec3>,
}

/// LED wash moving heads, drawn looking at the viewer from the front.
const FACING_WASHES: [&str; 3] = [
    "moving-head-led-wash-300",
    "moving-head-led-wash-400",
    "moving-head-led-wash-500",
];

/// The model id without its `-no-clamp` suffix: both variants are drawn in the same pose.
fn base_id(model: &str) -> &str {
    model.strip_suffix("-no-clamp").unwrap_or(model)
}

/// Whether a model is one of the lamps rigged facing the audience: blinders (every cell count and
/// layout), the flat LED PAR, the strobes and the flood.
pub fn faces_forward(model: &str) -> bool {
    let base = base_id(model);
    base.starts_with("blinder")
        || matches!(
            base,
            "flat-led-par" | "led-strobe" | "strobe-xenon" | "flood-asymmetric"
        )
}

/// The direction a face-forward body's light leaves in when it leans `lean_degrees` from
/// vertical: level toward +Z at 0°, turned nose-down about +X as the angle grows — the same axis
/// and sense the Visualizer turns a fixture by its bracket angle.
pub fn lean_target(lean_degrees: f32) -> Vec3 {
    Quat::from_rotation_x(lean_degrees.to_radians()) * Vec3::Z
}

/// The pose `model` is drawn in for the drawing named `view` (`top`, `front` or `side`), given
/// the hinge its manifest records for the hanging frame.
pub fn drawing_pose(model: &str, view: &str, hinge: Option<Vec3>) -> DrawingPose {
    let base = base_id(model);
    let mut pose = DrawingPose {
        hinge,
        ..DrawingPose::default()
    };
    if view == "front" && FACING_WASHES.contains(&base) {
        pose.head_target = Some(Vec3::Z);
    }
    if hinge.is_none() {
        return pose;
    }
    if faces_forward(base) {
        if view == "side" {
            pose.body_target = Some(lean_target(DEFAULT_LEAN_DEGREES));
        } else {
            pose.body_target = Some(Vec3::Z);
        }
    } else if view == "top" && (base.contains("par") || base.starts_with("fresnel")) {
        pose.body_target = Some(Vec3::Z);
    }
    pose
}

#[cfg(test)]
mod tests {
    use super::*;

    const HINGE: Option<Vec3> = Some(Vec3::new(0.0, -0.3, 0.0));

    #[test]
    fn the_face_forward_family_faces_the_viewer_and_leans_from_the_side() {
        for model in [
            "blinder-2-cell",
            "blinder-4-cell-no-clamp",
            "blinder-8-cell",
            "blinder-8-cell-horizontal",
            "blinder-8-cell-horizontal-no-clamp",
            "flat-led-par",
            "flat-led-par-no-clamp",
            "led-strobe",
            "led-strobe-no-clamp",
            "strobe-xenon",
            "strobe-xenon-no-clamp",
            "flood-asymmetric",
            "flood-asymmetric-no-clamp",
        ] {
            assert!(faces_forward(model), "{model}");
            for view in ["top", "front"] {
                let pose = drawing_pose(model, view, HINGE);
                assert_eq!(pose.body_target, Some(Vec3::Z), "{model} {view}");
                assert_eq!(pose.hinge, HINGE);
            }
            let side = drawing_pose(model, "side", HINGE);
            let target = side.body_target.expect("the side view leans");
            // Leaning 20° from vertical: mostly toward the audience, a little down.
            assert!(
                (target.z - 20f32.to_radians().cos()).abs() < 1e-5,
                "{target:?}"
            );
            assert!(
                (target.y + 20f32.to_radians().sin()).abs() < 1e-5,
                "{target:?}"
            );
        }
        for model in [
            "par-64-short-nose-black",
            "led-par-x-in-1",
            "moving-head-wash",
        ] {
            assert!(!faces_forward(model), "{model}");
        }
    }

    #[test]
    fn pars_and_fresnels_point_forward_only_from_above() {
        for model in ["par-64-short-nose-black-no-clamp", "fresnel-barn-doors-2kw"] {
            assert_eq!(drawing_pose(model, "top", HINGE).body_target, Some(Vec3::Z));
            for view in ["front", "side"] {
                assert_eq!(drawing_pose(model, view, HINGE).body_target, None);
            }
        }
        // A model without a hinge cannot be turned in its frame.
        assert_eq!(
            drawing_pose("blinder-4-cell", "side", None),
            DrawingPose::default()
        );
    }

    #[test]
    fn led_wash_heads_look_at_the_viewer_from_the_front_only() {
        for model in [
            "moving-head-led-wash-300",
            "moving-head-led-wash-400-no-clamp",
            "moving-head-led-wash-500",
        ] {
            assert_eq!(
                drawing_pose(model, "front", None).head_target,
                Some(Vec3::Z)
            );
            assert_eq!(drawing_pose(model, "top", None).head_target, None);
            assert_eq!(drawing_pose(model, "side", None).head_target, None);
        }
        assert_eq!(
            drawing_pose("moving-head-wash", "front", None).head_target,
            None
        );
    }

    #[test]
    fn a_positive_lean_turns_the_beam_down_about_the_transverse_axis() {
        let level = lean_target(0.0);
        assert!((level - Vec3::Z).length() < 1e-6);
        let steep = lean_target(70.0);
        assert!(steep.y < -0.9 && steep.z > 0.3, "{steep:?}");
    }
}
