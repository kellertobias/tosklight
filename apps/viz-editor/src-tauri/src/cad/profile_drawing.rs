//! What the CAD draws a patched profile with: live model meshes and projection SVGs for a profile
//! that carries a model, and the shipped default model's 2D drawings for one that does not.

use super::CadState;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_application::PatchSnapshot;
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

include!(concat!(env!("OUT_DIR"), "/model_drawings.rs"));

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadProjection {
    pub view: String,
    pub svg: String,
    pub view_box_millimetres: [f32; 4],
    pub origin_millimetres: [f32; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadDrawing {
    pub id: String,
    pub projections: Vec<CadProjection>,
    pub live_meshes: Vec<CadLiveMesh>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_drawing: Option<CadModelDrawing>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadLiveMesh {
    pub pose: String,
    pub triangles: Vec<CadLiveTriangle>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadLiveTriangle {
    pub points_millimetres: [[f32; 3]; 3],
    pub colour: [f32; 3],
}

/// The editable line drawings of the shipped model a profile without its own model is drawn as.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadModelDrawing {
    /// The shipped model's id, as in `assets/models`.
    pub model: String,
    /// From the drawing's millimetres to the fixture's physical size.
    pub scale: f32,
    pub views: Vec<CadModelDrawingView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadModelDrawingView {
    /// `top`, `front` or `side`.
    pub view: &'static str,
    pub svg: &'static str,
    /// The same view without the clamp or other mounting hardware, when the model has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_clamp_svg: Option<&'static str>,
}

/// The embedded drawing of one model id in one view.
fn embedded(id: &str, view: &str) -> Option<&'static str> {
    MODEL_DRAWINGS
        .iter()
        .find(|(model, candidate, _)| *model == id && *candidate == view)
        .map(|(_, _, svg)| *svg)
}

/// The views drawn for a shipped model, each with its no-clamp variant when one ships.
pub fn model_drawing_views(model: &str) -> Vec<CadModelDrawingView> {
    let no_clamp = format!("{model}-no-clamp");
    ["top", "front", "side"]
        .into_iter()
        .filter_map(|view| {
            Some(CadModelDrawingView {
                view,
                svg: embedded(model, view)?,
                no_clamp_svg: embedded(&no_clamp, view),
            })
        })
        .collect()
}

pub(super) fn drawing_id(
    profile: &light_application::PatchProfileRevisionProjection,
    mode_id: Uuid,
) -> String {
    format!(
        "{}:{}:{}:{}",
        profile.profile_id.0, profile.profile_revision, profile.content_digest, mode_id
    )
}

pub(super) fn drawings(snapshot: &PatchSnapshot, cad: &CadState) -> Vec<CadDrawing> {
    let mut cache = cad.drawings.lock();
    let profiles = snapshot
        .profile_revisions
        .iter()
        .map(|profile| ((profile.profile_id.0, profile.profile_revision), profile))
        .collect::<HashMap<_, _>>();
    snapshot
        .fixtures
        .iter()
        .filter_map(|fixture| {
            let stored = profiles.get(&(
                fixture.profile.profile_id.0,
                fixture.profile.profile_revision,
            ))?;
            let id = drawing_id(stored, fixture.profile.mode_id);
            cache
                .entry(id.clone())
                .or_insert_with(|| {
                    drawing(&id, &stored.profile_snapshot, Some(fixture.profile.mode_id))
                })
                .clone()
        })
        .collect()
}

pub(super) fn drawing(
    id: &str,
    snapshot: &serde_json::Value,
    mode_id: Option<Uuid>,
) -> Option<CadDrawing> {
    let profile = serde_json::from_value::<light_fixture::FixtureProfile>(snapshot.clone()).ok()?;
    if let Some(chosen) = viz_project::profile_default_model(&profile, mode_id) {
        let views = model_drawing_views(chosen.model.name);
        return (!views.is_empty()).then(|| CadDrawing {
            id: id.to_owned(),
            projections: Vec::new(),
            live_meshes: Vec::new(),
            model_drawing: Some(CadModelDrawing {
                model: chosen.model.name.to_owned(),
                scale: chosen.scale,
                views,
            }),
        });
    }
    let live_meshes = live_meshes(&profile, mode_id);
    let generated;
    let projections = if let Some(projections) = profile.projection_assets.as_ref() {
        projections
    } else {
        generated = viz_project::generate_profile_projections(&profile).ok()?;
        &generated
    };
    let projections = projections
        .views
        .iter()
        .filter_map(|projection| {
            let encoded = projection
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")?;
            let svg = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
            Some(CadProjection {
                view: projection.view.wire().to_owned(),
                svg,
                view_box_millimetres: projection.view_box_millimetres,
                origin_millimetres: projection.origin_millimetres,
            })
        })
        .collect::<Vec<_>>();
    (!projections.is_empty()).then(|| CadDrawing {
        id: id.to_owned(),
        projections,
        live_meshes,
        model_drawing: None,
    })
}

fn live_meshes(profile: &light_fixture::FixtureProfile, mode_id: Option<Uuid>) -> Vec<CadLiveMesh> {
    viz_project::generate_live_projection_meshes_for_mode(profile, mode_id)
        .unwrap_or_default()
        .into_iter()
        .map(|mesh| CadLiveMesh {
            pose: match mesh.pose {
                viz_project::LiveProjectionPose::Top => "top",
                viz_project::LiveProjectionPose::Elevation => "elevation",
            }
            .to_owned(),
            triangles: mesh
                .triangles
                .into_iter()
                .map(|triangle| CadLiveTriangle {
                    points_millimetres: triangle.points_millimetres,
                    colour: triangle.colour,
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{drawing, model_drawing_views};

    fn modelless_profile(fixture_type: &str) -> light_fixture::FixtureProfile {
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
    fn a_profile_without_a_model_is_drawn_from_its_shipped_model_with_a_no_clamp_variant() {
        let profile = modelless_profile("blinder");
        let mode_id = profile.modes[0].id;
        let drawing = drawing(
            "blinder:1",
            &serde_json::to_value(&profile).unwrap(),
            Some(mode_id),
        )
        .expect("a model-less lamp still gets a CAD drawing");
        let model = drawing.model_drawing.expect("drawn from its shipped model");
        assert!(drawing.projections.is_empty() && drawing.live_meshes.is_empty());
        assert_eq!(
            model.model,
            viz_project::choose_default_model(None, "blinder", Default::default()).name
        );
        assert_eq!(
            model.views.iter().map(|view| view.view).collect::<Vec<_>>(),
            ["top", "front", "side"]
        );
        for view in &model.views {
            assert!(view.svg.contains("id=\"silhouette\""), "{}", view.view);
            let no_clamp = view
                .no_clamp_svg
                .expect("the flown blinder has a no-clamp drawing");
            assert!(no_clamp.contains("-no-clamp"), "{}", view.view);
            assert_ne!(no_clamp, view.svg);
        }
    }

    #[test]
    fn moving_heads_and_pars_ship_drawings_in_every_view() {
        for model in ["moving-head-profile", "par-64-short-nose-black"] {
            let views = model_drawing_views(model);
            assert_eq!(views.len(), 3, "{model}");
            assert!(
                views.iter().all(|view| view.no_clamp_svg.is_some()),
                "{model}"
            );
        }
    }

    /// A lamp without its own model can be given any shipped body, so every one of them needs its
    /// three drawings embedded — and a hinged lamp's side view has to carry the hinge, the hardware
    /// and the lean the CAD turns it by.
    #[test]
    fn every_shipped_default_model_is_embedded_in_every_view() {
        for model in viz_project::all_default_models() {
            let views = model_drawing_views(model.name);
            assert_eq!(
                views.iter().map(|view| view.view).collect::<Vec<_>>(),
                ["top", "front", "side"],
                "{} has no drawing in some view",
                model.name
            );
            if viz_project::faces_forward(model.name) {
                let side = views[2].svg;
                for needle in [
                    "data-hinge=\"",
                    "data-bracket=\"-70\"",
                    "id=\"silhouette-hardware\"",
                    "<g id=\"hardware\">",
                ] {
                    assert!(side.contains(needle), "{}: missing {needle}", model.name);
                }
                let no_clamp = views[2].no_clamp_svg.expect("a no-clamp variant");
                assert!(no_clamp.contains("data-bracket=\"-70\""), "{}", model.name);
            }
        }
    }

    #[test]
    fn generated_scenery_keeps_its_own_drawing() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/venue--stage-railing-2-m.toskfixture");
        let profile = light_fixture::read_fixture_package(&std::fs::read(package).unwrap())
            .expect("fixture package reads");
        let drawing = drawing("railing:1", &serde_json::to_value(&profile).unwrap(), None);
        assert!(drawing.is_none_or(|drawing| drawing.model_drawing.is_none()));
    }
}
