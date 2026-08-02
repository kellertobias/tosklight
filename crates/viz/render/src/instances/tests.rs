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
        (aperture_colour - expected * 9.02).length() < 1e-5,
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
