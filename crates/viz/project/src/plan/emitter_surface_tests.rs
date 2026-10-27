//! Package-level source placement checks against actual emitting GLB glass, across modes/scales.
use super::*;

fn packaged(filename: &str, mode: usize, scale: f32) -> PatchedFixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library")
        .join(filename);
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode_id = profile.modes[mode].id;
    PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: profile.name.clone(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            instance_id: Uuid::new_v4(),
            name: "Alignment check".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(1.0, 4.0, 2.0),
            rotation_degrees: Vec3::new(10.0, 30.0, 5.0),
            invert_pan: false,
            invert_tilt: false,
            color_calibration: None,
            position_calibration: None,
            bracket_angle: 45.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
            model_scale: scale,
            scenery_options: Default::default(),
            scenery_size_metres: None,
        }],
    }
}

fn assert_sources_on_glass(plan: &ScenePlan, label: &str) {
    for emitter in &plan.scene.emitters {
        if !matches!(emitter.kind, EmitterKind::Beam | EmitterKind::Emissive) {
            continue;
        }
        let fixture = &plan.scene.fixtures[emitter.fixture_index as usize];
        let model = &plan.scene.models[fixture.model.unwrap() as usize];
        let surfaces = model.emitting_surfaces();
        if surfaces.is_empty() {
            continue;
        }
        let scale = model.scale_to(fixture.body.size);
        let rotation = viz_scene::euler_degrees(emitter.local_orientation_degrees);
        let offsets = if emitter.cells.aperture_offsets.is_empty() {
            &emitter.cells.offsets
        } else {
            &emitter.cells.aperture_offsets
        };
        for offset in offsets {
            let position = emitter.local_origin + rotation * *offset;
            let axis = model.emitter_axis.unwrap().abs();
            let across = Vec3::ONE - axis;
            let on_glass = surfaces.iter().any(|surface| {
                let delta = position - surface.anchor * scale;
                let dimensions: Vec<f32> = (0..3)
                    .filter(|i| across[*i] > 0.5)
                    .map(|i| delta[i].abs())
                    .collect();
                (delta * axis).length() < 0.0002
                    && dimensions[0] <= surface.size.x * scale * 0.5 + 0.0002
                    && dimensions[1] <= surface.size.y * scale * 0.5 + 0.0002
            });
            assert!(
                on_glass,
                "{label}: {} source {position:?} misses actual emitting glass",
                emitter.label
            );
        }
    }
}

#[test]
fn robe_zones_are_concentric_glass_with_bounded_logical_beams() {
    for (file, counts) in [
        ("robe--robin-300-ledwash.toskfixture", [1, 6, 12]),
        ("robe--robin-600x-ledwash.toskfixture", [7, 12, 18]),
    ] {
        for scale in [0.5, 1.0, 2.0] {
            let fixture = packaged(file, 0, scale);
            let heads: Vec<_> = fixture.profile.modes[0]
                .heads
                .iter()
                .filter(|head| !head.master_shared)
                .map(|head| head.id)
                .collect();
            let plan = compile(&[fixture]);
            assert_eq!(plan.scene.emitter_ids, heads);
            assert_eq!(plan.scene.emitters.len(), 3);
            assert_eq!(
                plan.scene
                    .emitters
                    .iter()
                    .map(|e| e.cells.aperture_offsets.len())
                    .collect::<Vec<_>>(),
                counts
            );
            assert!(plan.scene.emitters.iter().all(|e| e.cells.len() == 1));
            assert_sources_on_glass(&plan, file);
            let whole = compile(&[packaged(file, 1, scale)]);
            assert_eq!(whole.scene.emitters.len(), 1);
            assert_eq!(
                whole.scene.emitters[0].cells.aperture_offsets.len(),
                counts.iter().sum::<usize>()
            );
            let zone_output: f32 = plan.scene.emitters.iter().map(|e| e.optics.output).sum();
            assert!((zone_output - whole.scene.emitters[0].optics.output).abs() < 0.0001);
        }
    }
}

#[test]
fn embedded_robe_zone_profiles_also_inherit_model_rings() {
    let mut fixture = packaged("robe--robin-300-ledwash.toskfixture", 0, 1.0);
    Arc::get_mut(&mut fixture.profile).unwrap().body_model = None;
    let plan = compile(&[fixture]);
    assert_eq!(
        plan.scene
            .emitters
            .iter()
            .map(|e| e.cells.aperture_offsets.len())
            .collect::<Vec<_>>(),
        [7, 12, 18]
    );
    assert_sources_on_glass(&plan, "legacy embedded zone profile");
}

#[test]
fn corrected_generic_packages_leave_their_actual_glass_at_each_scale() {
    for file in [
        "generic--dimmer-fresnel.toskfixture",
        "generic--acl.toskfixture",
        "generic--blinder-2.toskfixture",
        "generic--blinder-4.toskfixture",
        "generic--blinder-8.toskfixture",
    ] {
        let modes = packaged(file, 0, 1.0).profile.modes.len();
        for mode in 0..modes {
            for scale in [0.5, 1.0, 2.0] {
                assert_sources_on_glass(&compile(&[packaged(file, mode, scale)]), file);
            }
        }
    }
}

#[test]
fn every_shipped_noncontract_profile_mode_places_sources_on_emitting_glass() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library");
    let mut profiles = 0;
    let mut modes = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_none_or(|extension| extension != "toskfixture")
        {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let fixture = packaged(name, 0, 1.0);
        if fixture.profile.patch_policy == PatchPolicy::VisualOnly {
            continue;
        }
        profiles += 1;
        for mode in 0..fixture.profile.modes.len() {
            // A Media Server's layer heads are virtual colour surfaces, not physical lamps.
            let personality = &fixture.profile.modes[mode];
            if personality.heads.iter().all(|head| {
                light_fixture::media_color::has_media_color_identity(personality, head.id)
            }) {
                continue;
            }
            if fixture
                .profile
                .mode_geometry(&fixture.profile.modes[mode])
                .physical_contract
                .is_some()
            {
                continue;
            }
            modes += 1;
            assert_sources_on_glass(&compile(&[packaged(name, mode, 1.0)]), name);
        }
    }
    assert!(profiles >= 59, "audited only {profiles} control profiles");
    assert!(modes >= 100, "audited only {modes} personalities");
    eprintln!(
        "Audited {profiles} shipped control profiles and {modes} noncontract personalities against real GLB glass"
    );
}

#[test]
fn every_shipped_lamp_model_exposes_real_glass_cells() {
    let mut count = 0;
    for shipped in crate::all_default_models() {
        let model = crate::read_shipped_model(shipped).unwrap();
        // Atmosphere leaves a metal nozzle, not emitting glass.
        if shipped.name == "hazer" {
            continue;
        }
        let surfaces = model.emitting_surfaces();
        assert!(
            !surfaces.is_empty(),
            "{} has no emitting glass",
            shipped.name
        );
        for surface in surfaces {
            assert!(surface.anchor.is_finite() && surface.size.is_finite());
            assert!(surface.size.min_element() > 0.001);
        }
        count += 1;
    }
    eprintln!("Audited real glass connectivity for {count} renderer-owned lamp/projector models");
    assert!(count >= 44);
}

#[test]
fn generic_acl_keeps_narrow_cone_soft_field_and_actual_glass_alignment() {
    let plan = compile(&[packaged("generic--acl.toskfixture", 0, 1.0)]);
    assert_eq!(plan.scene.emitters.len(), 1);
    let emitter = &plan.scene.emitters[0];
    assert_eq!(
        (emitter.beam_angle_degrees, emitter.field_angle_degrees),
        (8.0, 10.0)
    );
    assert!((emitter.optics.sharpness - 0.4).abs() < 0.0001);
    assert!((emitter.optics.uniformity - 0.3).abs() < 0.0001);
    assert_sources_on_glass(&plan, "soft ACL preserves its real GLB lens");
}
