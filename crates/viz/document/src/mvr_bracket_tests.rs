//! MVR export of a bracket-angled lamp: its light leaves where it does in ToskLight (TL-477).
//!
//! The local reference is the Visualizer's own pose: the patch compiled by `viz_project::compile`,
//! the body placed by `FixtureInstance::placed_by` about the manifest hinge, and the emitter
//! resolved the way `viz_render::instances::emitter_pose` resolves it at home pan and tilt. The
//! exported side places the same body-local emitter with nothing but the MVR matrix, as another
//! application would.

use super::{patch_one, temp_path};
use crate::PlanningDocument;
use light_core::Revision;
use light_fixture::{
    FixtureLocation, FixtureProfile, FixtureVector, PatchedFixtureProfileReference,
};
use light_show::{FixtureProfileRevision, ShowStore};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use viz_scene::glam::Vec3;

/// A shipped lamp that turns in its hanging frame, and one whose shipped body has no hinge.
fn lamp(hinged: bool) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Acme".into();
    if hinged {
        profile.name = "Fresnel".into();
        profile.fixture_type = "fresnel".into();
        profile.body_model = Some("fresnel-barn-doors".into());
    } else {
        profile.name = "Wash Head".into();
        profile.fixture_type = "moving head".into();
        profile.body_model = Some("moving-head-wash".into());
    }
    profile.short_name = profile.name.clone();
    profile
}

struct Doc {
    document: PlanningDocument,
    reference: PatchedFixtureProfileReference,
    path: PathBuf,
}

impl Drop for Doc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn document(name: &str, profile: &FixtureProfile) -> Doc {
    let path = temp_path(name);
    let document = PlanningDocument::create(&path, "Bracket show").expect("create show");
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap()).unwrap();
    ShowStore::open(&path)
        .unwrap()
        .insert_fixture_profile_revision(&stored)
        .expect("retain profile revision");
    Doc {
        document,
        reference: PatchedFixtureProfileReference {
            profile_id: profile.id,
            profile_revision: Revision::from(profile.revision),
            mode_id: profile.modes[0].id,
        },
        path,
    }
}

#[derive(Clone, Copy, Debug)]
struct Hang {
    location: FixtureLocation,
    rotation: FixtureVector,
    bracket: f32,
    scale: Option<f32>,
}

fn patch(doc: &Doc, hang: Hang) {
    let mut command = patch_one(doc.document.show_id(), doc.reference);
    let patch = &mut command.fixtures[0].patch;
    patch.location = hang.location;
    patch.rotation = hang.rotation;
    patch.bracket_angle = hang.bracket;
    patch.model_scale = hang.scale;
    doc.document.patch_fixtures(command).expect("patch");
}

/// Renderer axes to desk axes, and back: `(x, y, z)` is `(x, −z, y)` on the desk.
fn to_desk(point: Vec3) -> Vec3 {
    Vec3::new(point.x, -point.z, point.y)
}

/// The Visualizer's compiled lamp, placed as the desk's viz bridge places it.
fn compiled(profile: &FixtureProfile, hang: Hang) -> viz_project::ScenePlan {
    let fixture = viz_project::PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: profile.name.clone(),
        number: Some(1),
        profile: Arc::new(profile.clone()),
        mode_id: profile.modes[0].id,
        instances: vec![viz_project::PhysicalInstance {
            model_scale: light_fixture::resolved_model_scale(hang.scale),
            scenery_options: Default::default(),
            scenery_size_metres: None,
            instance_id: Uuid::new_v4(),
            name: profile.name.clone(),
            split_patches: vec![(1, Some((1, 1)))],
            // Desk millimetres `(x, y, z)` are renderer metres `(x, z, −y)`; the rotation is
            // `(x, z, y)` — the same mapping `viz_desk::transform` applies.
            position: Vec3::new(
                hang.location.x as f32,
                hang.location.z as f32,
                -(hang.location.y as f32),
            ) / 1000.0,
            rotation_degrees: Vec3::new(hang.rotation.x, hang.rotation.z, hang.rotation.y),
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: hang.bracket,
            shaper_angle: None,
            installed_appearance: Default::default(),
        }],
    };
    viz_project::compile(&[fixture])
}

/// Where the lamp's light leaves and which way it goes, in desk millimetres, as ToskLight draws it.
fn local_light(profile: &FixtureProfile, hang: Hang) -> (Vec3, Vec3) {
    let plan = compiled(profile, hang);
    let fixture = &plan.scene.fixtures[0];
    let emitter = &plan.scene.emitters[0];
    let (position, body) = fixture.placed_by(&[]);
    let origin = position + body * emitter.local_origin;
    let direction =
        body * viz_scene::euler_degrees(emitter.local_orientation_degrees) * Vec3::NEG_Y;
    (to_desk(origin) * 1000.0, to_desk(direction).normalize())
}

/// The same emitter placed by nothing but an MVR matrix, as another application places the
/// fixture's geometry: `o + R · p` for the body-local point `p`.
fn exported_light(profile: &FixtureProfile, hang: Hang, matrix: [f64; 12]) -> (Vec3, Vec3) {
    let plan = compiled(profile, hang);
    let emitter = &plan.scene.emitters[0];
    let local = to_desk(emitter.local_origin) * 1000.0;
    let aim = to_desk(viz_scene::euler_degrees(emitter.local_orientation_degrees) * Vec3::NEG_Y);
    let m = matrix.map(|value| value as f32);
    let (u, v, w) = (
        Vec3::new(m[0], m[1], m[2]),
        Vec3::new(m[3], m[4], m[5]),
        Vec3::new(m[6], m[7], m[8]),
    );
    let o = Vec3::new(m[9], m[10], m[11]);
    let turn = |p: Vec3| u * p.x + v * p.y + w * p.z;
    (o + turn(local), turn(aim).normalize())
}

fn assert_same_light(label: &str, local: (Vec3, Vec3), other: (Vec3, Vec3)) {
    let distance = (local.0 - other.0).length();
    assert!(
        distance <= 1.0,
        "{label}: light leaves {distance} mm away\n local {local:?}\n other {other:?}"
    );
    let angle = local.1.dot(other.1).clamp(-1.0, 1.0).acos().to_degrees();
    assert!(
        angle <= 0.1,
        "{label}: beam differs by {angle}°\n local {local:?}\n other {other:?}"
    );
}

fn hang(x: f32, yaw: f32, bracket: f32, scale: Option<f32>) -> Hang {
    Hang {
        location: FixtureLocation {
            x: 1250,
            y: -3400,
            z: 6100,
        },
        rotation: FixtureVector { x, y: 0.0, z: yaw },
        bracket,
        scale,
    }
}

/// Yaw, bracket and scale combinations, including a lamp hung off true.
const HANGS: [(f32, f32, f32, Option<f32>); 5] = [
    (0.0, 0.0, 45.0, None),
    (0.0, 90.0, 45.0, None),
    (0.0, -135.0, -30.0, None),
    (12.0, 30.0, 70.0, Some(1.5)),
    (-8.0, 200.0, 15.0, Some(0.75)),
];

fn exported_matrix(doc: &Doc) -> ([f64; 12], light_mvr::MvrDocument) {
    let export = doc.document.export_mvr().expect("export");
    let archive = PlanningDocument::read_mvr(&export.data).expect("read back what was written");
    (archive.fixtures[0].matrix, archive)
}

#[test]
fn the_shared_hinge_is_the_one_the_visualizer_turns_the_body_about() {
    for scale in [None, Some(1.5)] {
        let profile = lamp(true);
        let hang = hang(0.0, 0.0, 45.0, scale);
        let plan = compiled(&profile, hang);
        let local = plan.scene.fixtures[0]
            .bracket_hinge
            .expect("the Fresnel has a hinge");
        let shared = viz_project::fixture_bracket_hinge_millimetres(
            &profile,
            Some(profile.modes[0].id),
            light_fixture::resolved_model_scale(scale),
        )
        .expect("the shared hinge");
        assert!(
            (Vec3::from_array(shared) - to_desk(local) * 1000.0).length() < 1e-2,
            "{shared:?} {local:?}"
        );
    }
    let head = lamp(false);
    assert_eq!(
        compiled(&head, hang(0.0, 0.0, 45.0, None)).scene.fixtures[0].bracket_hinge,
        None
    );
    assert_eq!(
        viz_project::fixture_bracket_hinge_millimetres(&head, None, 1.0),
        None
    );
}

#[test]
fn an_exported_bracketed_lamp_shines_from_where_it_does_locally() {
    for hinged in [true, false] {
        let profile = lamp(hinged);
        for (index, (x, yaw, bracket, scale)) in HANGS.into_iter().enumerate() {
            let hang = hang(x, yaw, bracket, scale);
            let doc = document(&format!("mvr-bracket-{hinged}-{index}"), &profile);
            patch(&doc, hang);
            let (matrix, _) = exported_matrix(&doc);
            assert_same_light(
                &format!("hinged {hinged}, {hang:?}"),
                local_light(&profile, hang),
                exported_light(&profile, hang, matrix),
            );
        }
    }
}

/// The old export turned the Fresnel about its clamp, which put its lens centimetres away.
#[test]
fn turning_the_whole_lamp_about_its_origin_would_miss_the_lens() {
    let profile = lamp(true);
    let hang = hang(0.0, 90.0, 45.0, None);
    let whole =
        light_application::mvr_transform::mvr_matrix(hang.location, hang.rotation, hang.bracket);
    let (local, old) = (
        local_light(&profile, hang),
        exported_light(&profile, hang, whole),
    );
    assert!((local.0 - old.0).length() > 50.0, "{local:?} {old:?}");
}

#[test]
fn a_level_bracket_exports_exactly_as_before() {
    for hinged in [true, false] {
        let profile = lamp(hinged);
        let hang = hang(12.0, 30.0, 0.0, Some(1.5));
        let doc = document(&format!("mvr-level-{hinged}"), &profile);
        patch(&doc, hang);
        let (matrix, archive) = exported_matrix(&doc);
        let before =
            light_application::mvr_transform::mvr_matrix(hang.location, hang.rotation, 0.0);
        assert_eq!(matrix.map(f64::to_bits), before.map(f64::to_bits));
        let metadata = String::from_utf8(
            archive.files[light_application::mvr_export::TOSKLIGHT_MVR_FIXTURE_METADATA_PATH]
                .clone(),
        )
        .unwrap();
        assert!(!metadata.contains("bracket_hinge"), "{metadata}");
    }
}

#[test]
fn a_bracketed_lamp_exported_and_imported_again_shines_the_same_way() {
    for hinged in [true, false] {
        let profile = lamp(hinged);
        for (index, (x, yaw, bracket, scale)) in HANGS.into_iter().enumerate() {
            let hang = hang(x, yaw, bracket, scale);
            let source = document(&format!("mvr-trip-source-{hinged}-{index}"), &profile);
            patch(&source, hang);
            let (_, archive) = exported_matrix(&source);

            let target = document(&format!("mvr-trip-target-{hinged}-{index}"), &profile);
            target
                .document
                .import_mvr(archive, Default::default())
                .expect("import");
            let snapshot = target.document.patch_snapshot().expect("snapshot");
            let imported = &snapshot.fixtures[0].patch;
            assert_eq!(imported.bracket_angle, bracket, "the bracket is kept");
            for (axis, back, was) in [
                ("x", imported.location.x, hang.location.x),
                ("y", imported.location.y, hang.location.y),
                ("z", imported.location.z, hang.location.z),
            ] {
                assert!((back - was).abs() <= 1, "{axis}: {back} is not {was}");
            }
            let back = Hang {
                location: imported.location,
                rotation: imported.rotation,
                bracket: imported.bracket_angle,
                scale: imported.model_scale,
            };
            assert_same_light(
                &format!("round trip, hinged {hinged}, {hang:?}"),
                local_light(&profile, hang),
                local_light(&profile, back),
            );
        }
    }
}
