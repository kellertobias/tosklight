//! Behaviour of the per-frame instance build.
use super::*;

fn head() -> EmitterInstance {
    EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Head".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 8.0,
        field_angle_degrees: 40.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: viz_scene::EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    }
}

/// A lamp four metres above the deck lights a pool, not a dot.
///
/// The rim of a pool is further from the lamp than the point directly below it. Handing the
/// shaders the axial distance as the light's range culls everything except the exact centre,
/// which an operator sees as a lamp that lights a single point.
#[test]
fn a_lamp_pointing_straight_down_reaches_the_whole_pool() {
    let origin = Vec3::new(0.0, 4.0, 0.0);
    let half_angle = 25.0_f32.to_radians();
    let reach = beam_reach(origin, Vec3::NEG_Y, half_angle);

    // Where the edge of the field lands on the deck.
    let rim = Vec3::new(4.0 * half_angle.tan(), 0.0, 0.0);
    assert!(
        reach >= rim.distance(origin),
        "the range {reach} stops short of the rim of its own pool at {}",
        rim.distance(origin)
    );
    // And it does not reach absurdly further than it needs to.
    assert!(reach < rim.distance(origin) * 1.2);
}

#[test]
fn a_beam_aimed_along_the_floor_still_has_a_bounded_throw() {
    let reach = beam_reach(Vec3::new(0.0, 2.0, 0.0), Vec3::X, 20.0_f32.to_radians());
    assert!(reach > 1.0 && reach <= BEAM_THROW_METRES * 1.6);
}

/// A moving head has to point where the desk says. The base stays bolted to the truss, the
/// yoke turns with pan, and the head turns about its own trunnions with tilt.
#[test]
fn tilt_turns_the_head_about_its_trunnions_and_leaves_the_base_alone() {
    use viz_scene::{FixtureModel, ModelPart, ModelPartKind};

    let part = |name: &str, kind: ModelPartKind, height: f32| ModelPart {
        name: name.into(),
        kind,
        positions: vec![[0.0, height, 0.0], [0.1, height, 0.0], [0.0, height, 0.1]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        colour: [0.1, 0.1, 0.1],
        roughness: 0.5,
        metallic: 0.2,
    };
    let model = FixtureModel {
        parts: vec![
            part("base", ModelPartKind::Base, 0.0),
            part("yoke", ModelPartKind::Yoke, 0.2),
            part("head", ModelPartKind::Head, 0.4),
        ],
        extent: Vec3::splat(0.25),
        head_pivot: Vec3::new(0.0, 0.4, 0.0),
        emitter_anchor: None,
        emitter_size: None,
        emitter_axis: None,
        has_head: true,
        warnings: Vec::new(),
    };

    let mut scene = Scene::default();
    let mut fixture = fixture();
    fixture.model = Some(0);
    fixture.position = Vec3::new(1.0, 5.0, 0.0);
    scene.fixtures.push(fixture);
    scene.models.push(model.clone());
    // The head follows the emitter's resolved angles, so the fixture needs one.
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Head".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 8.0,
        field_angle_degrees: 30.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: viz_scene::EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });

    let transforms = |pan: f32, tilt: f32| {
        let mut frame = FrameInstances::default();
        push_model(
            &mut frame,
            &scene,
            &scene.fixtures[0],
            0,
            0,
            &model,
            &[(pan, tilt)],
        );
        let take = |part_index: u32| {
            frame
                .meshes
                .iter()
                .find(|(kind, _)| *kind == MeshKind::ModelPart(0, part_index))
                .map(|(_, instances)| Mat4::from_cols_array_2d(&instances[0].model))
                .expect("the part was drawn")
        };
        (take(0), take(1), take(2))
    };

    let (base_still, yoke_still, head_still) = transforms(0.0, 0.0);
    let (base_moved, yoke_moved, head_moved) = transforms(40.0, 55.0);

    assert_eq!(base_still, base_moved, "the base is bolted to the truss");
    assert_ne!(yoke_still, yoke_moved, "the yoke turns with pan");
    assert_ne!(head_still, head_moved, "the head turns with pan and tilt");

    // Tilting about the trunnions keeps the head where it is; tilting about the hanging point
    // would throw it metres away.
    let travel = (head_moved.transform_point3(model.head_pivot)
        - head_still.transform_point3(model.head_pivot))
    .length();
    assert!(
        travel < 0.35,
        "the head swung {travel} m instead of turning on the spot"
    );
}

/// Zooming in puts the same light through a smaller cone, so the beam gets brighter. This is
/// the difference an operator feels between a zoom and an iris.
#[test]
fn zooming_in_narrows_the_beam_and_brightens_it() {
    let emitter = head();
    let narrow = EmitterValues {
        zoom: 0.0,
        ..EmitterValues::default()
    };
    let wide = EmitterValues {
        zoom: 1.0,
        ..EmitterValues::default()
    };

    let narrow = resolve_optics(&emitter, &narrow);
    let wide = resolve_optics(&emitter, &wide);
    assert!(narrow.half_angle < wide.half_angle, "zoom narrows the cone");
    assert!(
        narrow.gain > wide.gain * 2.0,
        "a narrow beam is much brighter: {} against {}",
        narrow.gain,
        wide.gain
    );
}

/// The iris masks the beam. The pool gets smaller; what is left is as bright as it was.
#[test]
fn closing_the_iris_narrows_the_beam_without_changing_its_brightness() {
    let emitter = head();
    let open = EmitterValues::default();
    let closed = EmitterValues {
        iris: 0.7,
        ..EmitterValues::default()
    };

    let open = resolve_optics(&emitter, &open);
    let closed = resolve_optics(&emitter, &closed);
    assert!(
        closed.half_angle < open.half_angle * 0.5,
        "the iris masks the beam"
    );
    assert!(
        (closed.gain - open.gain).abs() < 1e-4,
        "an iris must not change the brightness: {} against {}",
        closed.gain,
        open.gain
    );
}

#[test]
fn frost_and_a_lens_off_the_gate_both_soften_the_edge() {
    let emitter = head();
    let sharp = resolve_optics(&emitter, &EmitterValues::default());

    let frosted = EmitterValues {
        frost: 0.8,
        ..EmitterValues::default()
    };
    assert!(resolve_optics(&emitter, &frosted).feather > sharp.feather);

    let defocused = EmitterValues {
        focus: 1.0,
        ..EmitterValues::default()
    };
    assert!(resolve_optics(&emitter, &defocused).feather > sharp.feather);
}

#[test]
fn the_open_gobo_slot_is_no_gobo_at_all() {
    let mut value = EmitterValues::default();
    assert_eq!(value.gobo_slot(GOBO_SLOTS), 0);
    value.gobo = 0.99;
    assert_eq!(value.gobo_slot(GOBO_SLOTS), GOBO_SLOTS - 1);
    assert_eq!(value.prism_facets(), 0, "no prism until the wheel moves");
    value.prism = 0.5;
    assert!(value.prism_facets() >= 3);
}

use viz_scene::{EmitterLayoutCells, EmitterOptics, FixtureBody, LightSource, MotionAxis};

fn fixture() -> FixtureInstance {
    FixtureInstance {
        instance_id: viz_scene::uuid::Uuid::nil(),
        fixture_id: viz_scene::uuid::Uuid::nil(),
        name: "Test".into(),
        number: None,
        position: Vec3::new(0.0, 6.0, 0.0),
        rotation_degrees: Vec3::ZERO,
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody::default(),
        patched: true,
        address: None,
        model: None,
        fallback: None,
    }
}

#[test]
fn plot_prefers_packaged_artwork_before_renderer_fallbacks() {
    let fixture = fixture();
    let fixture_id = fixture.fixture_id;
    let mut scene = Scene {
        fixtures: vec![fixture],
        plan_artwork: vec![viz_scene::PlanArtwork {
            view: viz_scene::ProjectionView::Top,
            vertices: vec![[-0.1, 0.0, -0.1], [0.1, 0.0, -0.1], [0.0, 0.0, 0.1]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        }],
        fixture_plan: vec![viz_scene::FixturePlanBinding {
            fixture_id,
            artwork: [Some(0), None, None, None, None],
            fallback: viz_scene::PlanFallback::GenericType,
        }],
        ..Scene::default()
    };
    let style = FrameStyle {
        plot: true,
        projection_view: viz_scene::ProjectionView::Top,
        ..FrameStyle::default()
    };
    let packaged = build(&scene, &SceneValues::default(), &style);
    assert!(
        packaged
            .meshes
            .iter()
            .any(|(kind, instances)| { *kind == MeshKind::PlanArtwork(0) && instances.len() == 1 })
    );
    assert!(
        !packaged
            .meshes
            .iter()
            .any(|(kind, _)| *kind == MeshKind::Cube)
    );

    scene.fixture_plan[0].artwork = [None; 5];
    let generic = build(&scene, &SceneValues::default(), &style);
    assert!(
        generic
            .meshes
            .iter()
            .any(|(kind, _)| *kind == MeshKind::Cube)
    );
    let generic_lines = generic.lines.len();

    scene.fixture_plan[0].fallback = viz_scene::PlanFallback::UnknownBox;
    let unknown = build(&scene, &SceneValues::default(), &style);
    assert!(
        unknown
            .meshes
            .iter()
            .any(|(kind, _)| *kind == MeshKind::Cube)
    );
    assert_ne!(unknown.lines.len(), generic_lines);
}

#[test]
fn shipped_profile_svg_reaches_the_literal_plan_artwork_mesh() {
    let package = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../assets/fixture-library/generic--dimmer-profile.toskfixture"
    ))
    .expect("shipped fixture package");
    let profile = light_fixture::read_fixture_package(&package).expect("valid shipped package");
    let mode_id = profile.modes[0].id;
    let fixture_id = uuid::Uuid::new_v4();
    let plan = viz_project::compile(&[viz_project::PatchedFixture {
        fixture_id,
        name: "Shipped Dimmer Profile".into(),
        number: Some(101),
        profile: std::sync::Arc::new(profile),
        mode_id,
        instances: vec![viz_project::PhysicalInstance {
            instance_id: uuid::Uuid::new_v4(),
            name: "Shipped Dimmer Profile".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(0.0, 6.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: light_fixture::InstalledFixtureAppearance::default(),
        }],
    }]);
    let binding = plan
        .scene
        .fixture_plan
        .iter()
        .find(|binding| binding.fixture_id == fixture_id)
        .expect("shipped fixture has a plan binding");
    assert!(binding.artwork.iter().all(Option::is_some));
    assert_eq!(plan.scene.plan_artwork.len(), 5);

    let instances = build(
        &plan.scene,
        &SceneValues::default(),
        &FrameStyle {
            plot: true,
            projection_view: viz_scene::ProjectionView::Top,
            ..FrameStyle::default()
        },
    );
    assert!(
        instances
            .meshes
            .iter()
            .any(|(kind, drawn)| { matches!(kind, MeshKind::PlanArtwork(_)) && drawn.len() == 1 })
    );
}

fn emitter() -> EmitterInstance {
    EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Main".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: Some(MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        }),
        tilt: Some(MotionAxis {
            axis: Vec3::X,
            min_degrees: -135.0,
            max_degrees: 135.0,
        }),
        beam_angle_degrees: 12.0,
        field_angle_degrees: 30.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    }
}

#[test]
fn a_centred_moving_head_aims_straight_down() {
    let pose = emitter_pose(&fixture(), &emitter(), 0.0, 0.0, 0.5);
    assert!((pose.direction - Vec3::NEG_Y).length() < 1e-5);
}

#[test]
fn tilting_forward_swings_the_beam_towards_the_audience() {
    let pose = emitter_pose(&fixture(), &emitter(), 0.0, -90.0, 0.5);
    assert!(
        pose.direction.z > 0.9,
        "unexpected aim {:?}",
        pose.direction
    );
}

#[test]
fn a_dark_beam_emitter_still_produces_a_visible_aperture() {
    let mut scene = Scene::default();
    scene.fixtures.push(fixture());
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    let frame = build(&scene, &values, &FrameStyle::default());
    let apertures = frame
        .meshes
        .iter()
        .find(|(kind, _)| *kind == MeshKind::Lens)
        .map(|(_, instances)| instances.len())
        .unwrap_or_default();
    assert_eq!(apertures, 1);
    assert!(frame.lights.is_empty(), "a dark fixture emits no light");
}

/// A lamp's light leaves through a face, and the face has to stand across the aim: a round source
/// drawn as a ball, or as a disc lying along the beam, is the thing that makes a rig read as a
/// string of glowing marbles instead of as lit lenses.
#[test]
fn a_round_source_is_drawn_as_a_thin_lens_standing_across_the_aim() {
    let mut scene = Scene::default();
    scene.fixtures.push(fixture());
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    let frame = build(&scene, &values, &FrameStyle::default());
    let (_, lenses) = frame
        .meshes
        .iter()
        .find(|(kind, _)| *kind == MeshKind::Lens)
        .expect("a round source draws a lens");
    let model = glam::Mat4::from_cols_array_2d(&lenses[0].model);
    let (scale, rotation, _) = model.to_scale_rotation_translation();
    let source = scene.emitters[0].optics.source;
    assert!(
        (scale.x - source.width).abs() < 1e-4,
        "lens width {scale:?}"
    );
    assert!(
        scale.z < scale.x * 0.5,
        "a lens is glass in a housing, not a ball: {scale:?}"
    );
    let pose = emitter_pose(&scene.fixtures[0], &scene.emitters[0], 0.0, 0.0, 0.5);
    let facing = rotation * Vec3::Z;
    assert!(
        facing.dot(pose.direction) > 0.999,
        "the lens faces {facing:?} instead of the aim {:?}",
        pose.direction
    );
}

#[test]
fn a_lit_beam_emitter_produces_one_light_and_one_cone() {
    let mut scene = Scene::default();
    scene.fixtures.push(fixture());
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    let frame = build(
        &scene,
        &values,
        &FrameStyle {
            draw_aim_lines: true,
            // The ground grid is lines too, and this is counting the emitter's own.
            floor_grid: false,
            ..FrameStyle::default()
        },
    );
    assert_eq!(frame.lights.len(), 1);
    assert_eq!(frame.beams.len(), 1);
    assert_eq!(frame.lines.len(), 2);
}

#[test]
fn installed_colour_tints_aperture_beam_light_and_semantic_export_once() {
    let mut scene = Scene::default();
    let mut fixture = fixture();
    fixture.installed_colour = [0.5, 0.25, 1.0];
    scene.fixtures.push(fixture);
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].colour = [0.8, 0.4, 0.2];

    let frame = build(&scene, &values, &FrameStyle::default());
    let expected = Vec3::new(0.4, 0.1, 0.2);
    let beam = Vec3::from_slice(&frame.beams[0].colour[..3]);
    assert!((beam - expected).length() < 1e-6, "beam colour {beam:?}");
    let light = Vec3::from_slice(&frame.lights[0].colour_intensity[..3]);
    let gain = resolve_optics(&scene.emitters[0], &values.emitters[0]).gain;
    assert!(
        (light - expected * gain).length() < 1e-6,
        "light colour {light:?}"
    );
    let semantic = semantic_lights(&scene, &values);
    assert!((semantic[0].colour - expected).length() < 1e-6);
    let aperture = frame
        .meshes
        .iter()
        .find(|(kind, _)| *kind == MeshKind::Lens)
        .and_then(|(_, entries)| entries.first())
        .expect("lit aperture");
    let aperture_colour = Vec3::from_slice(&aperture.emissive[..3]);
    assert!(
        (aperture_colour - expected * (0.02 + super::APERTURE_RADIANCE)).length() < 1e-5,
        "aperture colour {aperture_colour:?}"
    );
}

#[test]
fn typed_shaper_roles_use_static_angles_until_live_values_take_ownership() {
    let mut scene = Scene::default();
    let mut fixture = fixture();
    fixture.bracket_degrees = -35.0;
    fixture.shaper_degrees = Some(30.0);
    fixture.installed_shaper_angles_degrees = [10.0, 20.0, 30.0, 40.0];
    scene.fixtures.push(fixture);
    let mut head = emitter();
    head.shaper_roles = [true, true, false, true];
    head.live_shaper_angle_roles = [false, true, false, false];
    scene.emitters.push(head);
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].shaper_blades = [0.5; 4];
    values.emitters[0].shaper_blade_angles_degrees[1] = -75.0;

    let static_module = build(&scene, &values, &FrameStyle::default()).lights[0];
    let expected_direction = scene.fixtures[0].orientation() * Vec3::NEG_Y;
    let actual_direction = Vec3::from_slice(&static_module.direction_cos_outer[..3]);
    assert!(
        (actual_direction - expected_direction).length() < 1e-6,
        "the GPU light direction must include the installed bracket angle"
    );
    let expected = [10.0_f32, -75.0, 0.0, 40.0].map(f32::to_radians);
    for (actual, expected) in static_module.shaper_angles.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 1e-6);
    }
    let tangent = Vec3::from_slice(&static_module.tangent_frost[..3]);
    let expected_tangent = rotate_about(
        scene.fixtures[0].orientation() * Vec3::X,
        expected_direction,
        30_f32.to_radians(),
    );
    assert!(
        (tangent - expected_tangent).length() < 1e-5,
        "the installed module rotation must be relative to the bracketed fixture"
    );

    scene.emitters[0].live_shaper_rotation_role = true;
    values.emitters[0].shaper_rotation_degrees = -45.0;
    let live_module = build(&scene, &values, &FrameStyle::default()).lights[0];
    let tangent = Vec3::from_slice(&live_module.tangent_frost[..3]);
    let expected_tangent = rotate_about(
        scene.fixtures[0].orientation() * Vec3::X,
        expected_direction,
        -45_f32.to_radians(),
    );
    assert!(
        (tangent - expected_tangent).length() < 1e-5,
        "live module rotation must be relative to the bracketed fixture"
    );
}

/// The lens a shaft leaves through, read off the proxy the renderer actually draws: the cone
/// is scaled from a virtual apex, so the radius at the lens is the taper times the offset.
fn lens_radius(beam: &BeamInstance) -> f32 {
    let model = Mat4::from_cols_array_2d(&beam.model);
    let (scale, _, _) = model.to_scale_rotation_translation();
    let taper = scale.x / scale.z.max(1e-6);
    taper * beam.params[2]
}

fn beam_of(scene: &Scene) -> BeamInstance {
    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    for emitter in &mut values.emitters {
        emitter.intensity = 1.0;
    }
    let frame = build(scene, &values, &FrameStyle::default());
    frame.beams.first().copied().expect("one beam")
}

/// A shaft leaves a lens, not a point. A half-metre-wide lamp whose beam springs from nothing
/// is what an operator standing under the rig sees as wrong straight away.
#[test]
fn a_shaft_starts_at_the_width_of_the_lens_it_leaves() {
    let mut scene = Scene::default();
    scene.fixtures.push(fixture());
    let mut head = emitter();
    head.optics.source = LightSource::round(0.24);
    scene.emitters.push(head);
    let beam = beam_of(&scene);
    let radius = lens_radius(&beam);
    assert!(
        (radius - 0.12).abs() < 0.01,
        "a 240 mm lens should start its shaft 240 mm across, got {} mm",
        radius * 2000.0
    );
    // And the proxy is pulled back behind the lamp by exactly that much cone.
    let translation = Mat4::from_cols_array_2d(&beam.model).w_axis.truncate();
    let origin = scene.fixtures[0].position;
    assert!(
        ((translation - origin).length() - beam.params[2]).abs() < 1e-3,
        "the cone has to start at its own virtual apex"
    );
}

/// The lens is the fixture's, not the renderer's: a point-source spot and a wide flood are
/// different fixtures and their shafts have to start differently.
#[test]
fn the_shaft_follows_the_light_source_it_was_given() {
    let shaft_of = |source: LightSource| {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let mut head = emitter();
        head.optics.source = source;
        scene.emitters.push(head);
        lens_radius(&beam_of(&scene))
    };
    let wide = shaft_of(LightSource::round(0.3));
    let narrow = shaft_of(LightSource::round(0.06));
    assert!(
        wide > narrow * 3.0,
        "the shaft has to follow the source: {wide} against {narrow}"
    );
    // An oval or rectangular lens has no single radius, so the shaft takes its mean.
    let oval = shaft_of(LightSource {
        form: SourceForm::Oval,
        width: 0.3,
        height: 0.1,
    });
    assert!(
        (oval - 0.1).abs() < 0.01,
        "an oval lens should give the mean of its axes, got {oval}"
    );
}

/// The whole point of the four numbers: two lamps at the same angle and the same level still
/// have to look different. A profile cuts its rim; a flood has no rim to cut.
#[test]
fn a_profile_draws_a_harder_rim_than_a_flood() {
    let rim_of = |sharpness: f32| {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let mut head = emitter();
        head.optics.sharpness = sharpness;
        scene.emitters.push(head);
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        let frame = build(&scene, &values, &FrameStyle::default());
        let light = frame.lights.first().copied().expect("one light");
        // `x` is the cosine of the inner angle and `w` of the outer: the gap between them is
        // the width of the rim the shaders draw.
        (
            light.params[0],
            light.direction_cos_outer[3],
            light.params[1],
        )
    };
    let (profile_inner, outer, profile_feather) = rim_of(0.9);
    let (flood_inner, flood_outer, flood_feather) = rim_of(0.05);
    assert!(
        (outer - flood_outer).abs() < 1e-6,
        "the field itself is unchanged"
    );
    // The rim is drawn between the inner and outer angles. Narrower band, harder edge.
    assert!(
        profile_inner - outer < (flood_inner - outer) * 0.5,
        "a profile has to cut within a fraction of its field: {profile_inner} against \
             {flood_inner}, outer {outer}"
    );
    assert!(
        profile_feather < flood_feather * 0.5,
        "a flood blends across much more of its field: {profile_feather} against {flood_feather}"
    );
}

/// Sharpness and uniformity are separate: an even wash has no rim and is still flat across the
/// middle, and that has to survive the trip to the GPU.
#[test]
fn uniformity_reaches_the_shaders_independently_of_the_rim() {
    let light_with = |sharpness: f32, uniformity: f32| {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let mut head = emitter();
        head.optics.sharpness = sharpness;
        head.optics.uniformity = uniformity;
        scene.emitters.push(head);
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        build(&scene, &values, &FrameStyle::default())
            .lights
            .first()
            .copied()
            .expect("one light")
    };
    let even = light_with(0.1, 0.95);
    let hot = light_with(0.1, 0.1);
    assert!(
        (even.params[1] - hot.params[1]).abs() < 1e-6,
        "the rim is the same"
    );
    assert!(
        even.params[2] > hot.params[2] + 0.5,
        "uniformity has to survive as its own number: {} against {}",
        even.params[2],
        hot.params[2]
    );
}

/// A 400 W engine is brighter than a 100 W one at the same level. Output is the fixture's, not
/// the operator's.
#[test]
fn a_stronger_engine_is_brighter_at_the_same_level() {
    let radiance_of = |output: f32| {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let mut head = emitter();
        head.optics.output = output;
        scene.emitters.push(head);
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 0.5;
        values.emitters[0].colour = [1.0, 1.0, 1.0];
        let frame = build(&scene, &values, &FrameStyle::default());
        frame.lights.first().expect("one light").colour_intensity[0]
    };
    assert!(radiance_of(2.0) > radiance_of(0.5) * 3.0);
}

/// The same light through a wider gate is spread thinner. A flood and a beam of the same
/// engine do not put the same intensity on a wall, and an operator picking between them is
/// picking exactly that.
#[test]
fn a_wider_field_spreads_the_same_light_thinner() {
    let radiance_of = |narrow_degrees: f32, wide_degrees: f32| {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let mut head = emitter();
        head.beam_angle_degrees = narrow_degrees;
        head.field_angle_degrees = wide_degrees;
        scene.emitters.push(head);
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        values.emitters[0].colour = [1.0, 1.0, 1.0];
        values.emitters[0].zoom = 0.5;
        build(&scene, &values, &FrameStyle::default())
            .lights
            .first()
            .expect("one light")
            .colour_intensity[0]
    };
    let beam = radiance_of(3.0, 8.0);
    let profile = radiance_of(10.0, 32.0);
    let flood = radiance_of(45.0, 90.0);
    assert!(
        beam > profile && profile > flood,
        "intensity has to fall as the field opens: {beam}, {profile}, {flood}"
    );
}

/// Cells sit close together on a bar. Sizing each one from the bar's whole body would merge
/// them into a single smear the moment they light.
#[test]
fn the_cells_of_a_bar_keep_their_own_lenses() {
    let mut scene = Scene::default();
    let mut fixture = fixture();
    fixture.body = FixtureBody {
        size: Vec3::new(1.2, 0.3, 0.3),
        kind: BodyKind::Bar,
    };
    scene.fixtures.push(fixture);
    let mut bar = emitter();
    let spacing = 0.1;
    bar.cells = viz_scene::EmitterLayoutCells {
        offsets: (0..8)
            .map(|index| Vec3::new(index as f32 * spacing - 0.35, 0.0, 0.0))
            .collect(),
    };
    scene.emitters.push(bar);
    let beam = beam_of(&scene);
    assert!(
        lens_radius(&beam) <= spacing * 0.5,
        "a cell's lens has to fit between its neighbours, got {}",
        lens_radius(&beam)
    );
}

/// The lines view is a model-and-lines diagram, not a dim picture.
///
/// An operator on it is asking two questions — where are the lamps, and where are they pointed —
/// and every other thing in the frame is between them and the answer. Fixture models stay, the
/// rigging and the soft goods go, and the aim of every directional head is drawn whether or not
/// anything is lit.
mod lines_view {
    use super::*;
    use viz_scene::{FixtureModel, ModelPart, ModelPartKind, SceneryKind, SceneryObject, ViewMode};

    fn lines_style() -> FrameStyle {
        FrameStyle {
            draw_beams: false,
            draw_aim_lines: true,
            aim_guides: true,
            fixture_models: true,
            emitter_apertures: false,
            scenery_surfaces: false,
            floor_grid: false,
            scenery: |kind| ViewMode::Lines3d.draws_scenery(kind),
            ..FrameStyle::default()
        }
    }

    pub(super) fn package_model() -> FixtureModel {
        FixtureModel {
            parts: vec![ModelPart {
                name: "authored-body".into(),
                kind: ModelPartKind::Base,
                positions: vec![[-0.2, -0.3, 0.0], [0.2, -0.3, 0.0], [0.0, 0.3, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
                colour: [0.08, 0.09, 0.1],
                roughness: 0.5,
                metallic: 0.3,
            }],
            extent: Vec3::new(0.2, 0.3, 0.01),
            head_pivot: Vec3::ZERO,
            emitter_anchor: None,
            emitter_size: None,
            emitter_axis: None,
            has_head: false,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn a_package_model_replaces_the_old_body_box() {
        let mut scene = Scene::default();
        let mut instance = fixture();
        instance.model = Some(0);
        scene.fixtures.push(instance);
        scene.models.push(package_model());

        let frame = build(&scene, &SceneValues::default(), &lines_style());
        assert!(
            frame.meshes.iter().any(|(kind, entries)| {
                *kind == MeshKind::ModelPart(0, 0) && entries.len() == 1
            })
        );
        assert!(
            frame.lines.is_empty(),
            "the authored fixture is no longer replaced by a box outline"
        );
    }

    fn scenery(kind: SceneryKind) -> SceneryObject {
        SceneryObject {
            id: viz_scene::uuid::Uuid::nil(),
            name: "Object".into(),
            position: Vec3::new(0.0, 3.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            size: Vec3::splat(2.0),
            colour: [0.5; 3],
            roughness: 0.6,
            kind,
            chords: 4,
        }
    }

    /// A dark lamp still has an aim, and where a dark lamp is pointed is the whole reason to open
    /// this view while focusing a rig.
    #[test]
    fn an_unlit_head_still_shows_where_it_is_pointed() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.emitters.push(emitter());
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 0.0;

        let frame = build(&scene, &values, &lines_style());
        assert!(
            !frame.lines.is_empty(),
            "an unlit head draws its guideline anyway"
        );
        assert!(frame.beams.is_empty(), "and no beam volume");
    }

    /// The guideline is dashed, so it never reads as a beam that is switched on.
    #[test]
    fn the_guideline_is_drawn_as_dashes_rather_than_one_line() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.emitters.push(emitter());
        let mut values = SceneValues::default();
        values.resize(1);

        let frame = build(&scene, &values, &lines_style());
        assert!(
            frame.lines.len() > 4,
            "a dashed guide is many short segments, got {} vertices",
            frame.lines.len()
        );
    }

    /// Lit, the head draws its own line and the dashes go.
    ///
    /// Two lines down one aim read as two aims, and a rig of them is a picture nobody can count.
    /// The dashes are what a dark lamp has instead of a beam, not as well as one.
    #[test]
    fn a_lit_head_replaces_its_guideline_with_its_own_line() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.emitters.push(emitter());
        let mut values = SceneValues::default();
        values.resize(1);

        let dark = build(&scene, &values, &lines_style()).lines.len();
        values.emitters[0].intensity = 1.0;
        values.emitters[0].held_intensity = 1.0;
        let lit = build(&scene, &values, &lines_style()).lines.len();
        assert!(
            lit < dark,
            "the dashes give way to one line: {dark} vertices dark, {lit} lit"
        );
    }

    /// Half and full have to look different, and one percent has to look like almost nothing.
    ///
    /// Nothing in this view is tonemapped, so a line drawn at its literal level reads far brighter
    /// than the level is — half and full were nearly the same line.
    #[test]
    fn the_lit_line_spreads_the_level_across_the_visible_range() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.emitters.push(emitter());

        let brightness_at = |level: f32| {
            let mut values = SceneValues::default();
            values.resize(1);
            values.emitters[0].intensity = level;
            values.emitters[0].held_intensity = level;
            let frame = build(&scene, &values, &lines_style());
            // The lit line is drawn in the lamp's own colour, which is white here. This measures
            // that line and ignores the shaded fixture body.
            frame
                .lines
                .iter()
                .filter(|vertex| vertex.colour[0] > 0.5)
                .map(|vertex| vertex.colour[3])
                .fold(0.0_f32, f32::max)
        };

        let (one, half, full) = (brightness_at(0.01), brightness_at(0.5), brightness_at(1.0));
        assert!(one < 0.06, "one percent is barely there, got {one}");
        assert!(
            full > half * 2.5,
            "half to full is a real step: {half} to {full}"
        );
    }

    /// A truss drawn as a box is a wall across the picture hiding the lamps hanging off it.
    #[test]
    fn the_rigging_and_the_soft_goods_are_not_drawn() {
        let mut scene = Scene::default();
        scene.scenery.push(scenery(SceneryKind::Truss));
        scene.scenery.push(scenery(SceneryKind::Curtain));
        let values = SceneValues::default();

        let frame = build(&scene, &values, &lines_style());
        assert!(
            frame.meshes.iter().all(|(_, entries)| entries.is_empty()),
            "no truss and no drape in a lines view"
        );
    }

    /// What the rig is arranged around stays: staging is what a fixture is aimed at. It is drawn
    /// as an outline, because nothing in this view is lit and a solid box would be a black shape
    /// in a black room.
    #[test]
    fn the_staging_is_drawn_as_an_outline_rather_than_a_solid() {
        let mut scene = Scene::default();
        scene.scenery.push(scenery(SceneryKind::Riser));
        let values = SceneValues::default();

        let frame = build(&scene, &values, &lines_style());
        assert!(
            frame.meshes.iter().all(|(_, entries)| entries.is_empty()),
            "no solid geometry in an outline view"
        );
        assert_eq!(frame.lines.len(), 24, "twelve edges, two vertices each");
    }

    /// An invalid package model arrives with no resolved model index. It still gets a deliberate
    /// body rather than the broken-looking outline that prompted TL-171.
    #[test]
    fn an_unresolved_model_uses_a_readable_procedural_body() {
        let mut scene = Scene::default();
        let mut instance = fixture();
        instance.body.size = Vec3::new(0.4, 0.9, 0.3);
        instance.position = Vec3::ZERO;
        scene.fixtures.push(instance);

        let frame = build(&scene, &SceneValues::default(), &lines_style());
        assert!(
            frame
                .meshes
                .iter()
                .any(|(kind, entries)| !matches!(kind, MeshKind::ModelPart(_, _))
                    && !entries.is_empty()),
            "the renderer supplies a procedural fixture body"
        );
        assert!(frame.lines.is_empty(), "the fallback is not the old Z-box");
    }
}

/// The ground reference is lines on the floor, not a floor.
///
/// A filled plane is a surface: it takes light, it hides what is under it, and it turns the bottom
/// of the picture into a large flat area competing with the rig. Lines give the scale and the
/// centre without being lit at all.
mod floor_grid {
    use super::*;

    #[test]
    fn the_grid_is_drawn_as_lines_and_adds_no_geometry() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.recompute_bounds();
        let values = SceneValues::default();

        let without = build(
            &scene,
            &values,
            &FrameStyle {
                floor_grid: false,
                ..FrameStyle::default()
            },
        );
        let with = build(&scene, &values, &FrameStyle::default());
        assert!(
            with.lines.len() > without.lines.len(),
            "the grid arrives as lines"
        );
        let meshes = |frame: &FrameInstances| -> usize {
            frame.meshes.iter().map(|(_, entries)| entries.len()).sum()
        };
        assert_eq!(
            meshes(&with),
            meshes(&without),
            "and brings no surface with it"
        );
    }

    /// A grid whose lines all look the same gives no answer to "where is centre".
    #[test]
    fn the_centre_lines_are_drawn_stronger_than_the_rest() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        scene.recompute_bounds();
        let frame = build(&scene, &SceneValues::default(), &FrameStyle::default());
        let brightest = frame
            .lines
            .iter()
            .map(|vertex| vertex.colour[0])
            .fold(0.0_f32, f32::max);
        let dimmest = frame
            .lines
            .iter()
            .map(|vertex| vertex.colour[0])
            .fold(f32::MAX, f32::min);
        assert!(
            brightest > dimmest * 1.5,
            "the centre lines stand out: {dimmest} to {brightest}"
        );
    }
}

/// Selection is the question an operator asks a Stage most often — which of these am I about to
/// change — so it is the one thing allowed to stand out, and it has to be told rather than guessed.
mod selection {
    use super::*;
    use viz_scene::{FixtureModel, ModelPart, ModelPartKind};

    fn outline_style() -> FrameStyle {
        FrameStyle {
            draw_beams: false,
            fixture_models: false,
            floor_grid: false,
            ..FrameStyle::default()
        }
    }

    fn fixture_model() -> FixtureModel {
        FixtureModel {
            parts: vec![ModelPart {
                name: "body".into(),
                kind: ModelPartKind::Base,
                positions: vec![[-0.2, -0.3, 0.0], [0.2, -0.3, 0.0], [0.0, 0.3, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
                colour: [0.08, 0.09, 0.1],
                roughness: 0.5,
                metallic: 0.3,
            }],
            extent: Vec3::new(0.4, 0.6, 0.2),
            head_pivot: Vec3::ZERO,
            emitter_anchor: None,
            emitter_size: None,
            emitter_axis: None,
            has_head: false,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn a_selected_fixture_is_drawn_in_the_selection_ink_and_the_rest_are_not() {
        let mut scene = Scene::default();
        let mut chosen = fixture();
        chosen.fixture_id = viz_scene::uuid::Uuid::from_u128(7);
        chosen.position = Vec3::ZERO;
        let mut other = fixture();
        other.fixture_id = viz_scene::uuid::Uuid::from_u128(8);
        other.position = Vec3::new(5.0, 0.0, 0.0);
        scene.fixtures.push(chosen);
        scene.fixtures.push(other);

        let mut values = SceneValues::default();
        values.resize(0);
        values
            .selected_fixtures
            .insert(viz_scene::uuid::Uuid::from_u128(7));

        let style = outline_style();
        let frame = build(&scene, &values, &style);
        let inks: Vec<Vec3> = frame
            .lines
            .iter()
            .map(|vertex| Vec3::from_slice(&vertex.colour[..3]))
            .collect();
        assert!(
            inks.iter()
                .any(|ink| (*ink - style.selected_ink).length() < 1e-5),
            "the selected fixture is drawn in the selection ink"
        );
        assert!(
            inks.iter()
                .any(|ink| (*ink - style.symbol_ink).length() < 1e-5),
            "and the unselected one is not"
        );
    }

    /// Nothing selected means nothing stands out, rather than everything doing.
    #[test]
    fn with_nothing_selected_no_fixture_takes_the_selection_ink() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture());
        let values = SceneValues::default();

        let style = outline_style();
        let frame = build(&scene, &values, &style);
        assert!(
            frame.lines.iter().all(|vertex| {
                (Vec3::from_slice(&vertex.colour[..3]) - style.selected_ink).length() > 1e-4
            }),
            "no selection, no selection ink"
        );
    }

    #[test]
    fn selected_full_output_model_gets_an_additive_cage_and_unselected_models_do_not_change() {
        let mut scene = Scene::default();
        scene.models.push(fixture_model());
        let mut chosen = fixture();
        chosen.fixture_id = viz_scene::uuid::Uuid::from_u128(7);
        chosen.position = Vec3::new(0.0, 6.0, 0.0);
        chosen.model = Some(0);
        let mut other = fixture();
        other.fixture_id = viz_scene::uuid::Uuid::from_u128(8);
        other.position = Vec3::new(5.0, 6.0, 0.0);
        other.model = Some(0);
        scene.fixtures.extend([chosen, other]);
        scene.emitters.push(emitter());

        let style = FrameStyle {
            floor_grid: false,
            ..FrameStyle::default()
        };
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        values.emitters[0].held_intensity = 1.0;
        let unselected = build(&scene, &values, &style);

        values
            .selected_fixtures
            .insert(viz_scene::uuid::Uuid::from_u128(9));
        let unknown_selection = build(&scene, &values, &style);
        assert!(
            unknown_selection.lines.is_empty(),
            "the renderer does not infer a selection that the desk did not provide"
        );

        values.selected_fixtures.clear();
        values
            .selected_fixtures
            .insert(viz_scene::uuid::Uuid::from_u128(7));
        let selected = build(&scene, &values, &style);

        assert_eq!(selected.lines.len(), 24, "one twelve-edge selection cage");
        assert!(selected.beams.len() == unselected.beams.len() && !selected.beams.is_empty());
        assert!(selected.lines.iter().all(|vertex| {
            (Vec3::from_slice(&vertex.colour[..3]) - style.selected_ink).length() < 1e-5
                && (vertex.colour[3] - 1.0).abs() < 1e-6
        }));
        assert!(
            selected
                .lines
                .iter()
                .all(|vertex| vertex.position[0].abs() < 1.0),
            "only the chosen model at x=0 receives a cage; the other is at x=5"
        );
        assert_eq!(selected.meshes.len(), unselected.meshes.len());
        for ((selected_kind, selected_instances), (plain_kind, plain_instances)) in
            selected.meshes.iter().zip(&unselected.meshes)
        {
            assert_eq!(selected_kind, plain_kind);
            assert_eq!(
                bytemuck::cast_slice::<MeshInstance, u8>(selected_instances),
                bytemuck::cast_slice::<MeshInstance, u8>(plain_instances),
                "selection must not recolour or replace either fixture model"
            );
        }

        values.selected_fixtures.clear();
        assert!(
            build(&scene, &values, &style).lines.is_empty(),
            "deselecting removes the additive mark"
        );
    }

    #[test]
    fn proxy_instances_sharing_one_selected_fixture_id_each_get_a_cage() {
        let selected_id = viz_scene::uuid::Uuid::from_u128(7);
        let mut first = fixture();
        first.fixture_id = selected_id;
        first.position = Vec3::new(-3.0, 6.0, 0.0);
        let mut second = fixture();
        second.fixture_id = selected_id;
        second.position = Vec3::new(3.0, 6.0, 0.0);
        let mut unrelated = fixture();
        unrelated.fixture_id = viz_scene::uuid::Uuid::from_u128(8);
        unrelated.position = Vec3::new(8.0, 6.0, 0.0);
        let mut scene = Scene::default();
        scene.fixtures.extend([first, second, unrelated]);

        let mut values = SceneValues::default();
        values.selected_fixtures.insert(selected_id);
        let frame = build(
            &scene,
            &values,
            &FrameStyle {
                draw_beams: false,
                floor_grid: false,
                ..FrameStyle::default()
            },
        );

        assert_eq!(
            frame.lines.len(),
            48,
            "two cages, one per selected instance"
        );
        assert!(
            frame.lines.iter().all(|vertex| vertex.position[0] < 4.0),
            "the unrelated proxy at x=8 remains plain"
        );
        assert!(
            frame.lines.iter().any(|vertex| vertex.position[0] < -2.0)
                && frame.lines.iter().any(|vertex| vertex.position[0] > 2.0),
            "both physical instances of the selected logical fixture are marked"
        );
    }
}

/// A model-and-lines view keeps its fixture body but no simulated-light surfaces.
///
/// The lit faces used to be pushed into every view that drew a fixture model. Splitting those two
/// decisions keeps the real model without making an unlit lines diagram look switched on.
#[test]
fn a_lines_view_draws_the_model_without_an_emitter_aperture() {
    let mut scene = Scene::default();
    let mut instance = fixture();
    instance.model = Some(0);
    scene.fixtures.push(instance);
    scene.models.push(lines_view::package_model());
    scene.emitters.push(emitter());
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].held_intensity = 1.0;

    let frame = build(
        &scene,
        &values,
        &FrameStyle {
            draw_beams: false,
            draw_aim_lines: true,
            aim_guides: true,
            fixture_models: true,
            emitter_apertures: false,
            scenery_surfaces: false,
            floor_grid: false,
            ..FrameStyle::default()
        },
    );
    assert!(
        frame
            .meshes
            .iter()
            .any(|(kind, entries)| { *kind == MeshKind::ModelPart(0, 0) && entries.len() == 1 })
    );
    assert!(
        frame
            .meshes
            .iter()
            .all(|(kind, entries)| *kind != MeshKind::Lens || entries.is_empty()),
        "the emitting face belongs only to a simulated-light view"
    );
    assert!(!frame.lines.is_empty(), "and the aim line is still drawn");
}
