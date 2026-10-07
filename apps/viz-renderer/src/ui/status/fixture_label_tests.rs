use super::*;
use glam::Vec3;
use viz_scene::{
    Camera, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance, Scene, SceneValues, SceneryKind, SceneryObject, ViewConfiguration, ViewMode,
};

fn fixture(number: u32, position: Vec3) -> FixtureInstance {
    FixtureInstance {
        drawn_as_scenery: false,
        invisible: false,
        instance_id: viz_scene::uuid::Uuid::new_v4(),
        fixture_id: viz_scene::uuid::Uuid::new_v4(),
        name: format!("Fixture {number}"),
        number: Some(number),
        position,
        rotation_degrees: Vec3::ZERO,
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody::default(),
        patched: true,
        address: Some((1, number as u16)),
        model: None,
        fallback: None,
    }
}

fn labels(scene: &Scene, show_labels: bool) -> Overlay {
    let mut view = ViewConfiguration::default();
    view.show_labels = show_labels;
    let camera =
        ResolvedCamera::resolve(&Camera::default(), view.mode, 1600.0 / 900.0, scene.bounds);
    let mut overlay = Overlay::default();
    build_fixture_labels(
        &mut overlay,
        scene,
        &SceneValues::default(),
        &camera,
        &view,
        1600.0,
        900.0,
    );
    overlay
}

fn push_lamp(scene: &mut Scene, fixture: FixtureInstance) {
    let fixture_index = scene.fixtures.len() as u32;
    scene.fixtures.push(fixture);
    scene.emitters.push(EmitterInstance {
        fixture_index,
        head_index: 0,
        label: "Main".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 10.0,
        field_angle_degrees: 20.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
}

#[test]
fn passive_color_marks_include_dark_uv_heads_without_tinting_visible_color() {
    let mut scene = Scene::default();
    push_lamp(&mut scene, fixture(7, Vec3::ZERO));
    let mut second_head = scene.emitters[0].clone();
    second_head.head_index = 1;
    scene.emitters.push(second_head);
    let mut values = SceneValues::default();
    values.resize(2);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].held_intensity = 1.0;
    values.emitters[0].colour = [1.0, 0.0, 0.0];
    values.emitters[1].uv_drive = 1.0 / 255.0;
    values.emitters[1].colour = [0.0; 3];
    values.emitters[1].physical_color = Some(viz_scene::PhysicalColorState {
        visible_complete: false,
        uv_drive: 1.0 / 255.0,
        ..Default::default()
    });
    let light = fixture_lighting(&scene, &values)[0].unwrap();
    assert_eq!(light.colour, [1.0, 0.0, 0.0]);
    assert_eq!(light.label("7".to_owned()), "7 UV ⚠");

    // A measured-zero visible spill still has UV activity, but needs no uncertainty mark.
    values.emitters[1]
        .physical_color
        .as_mut()
        .unwrap()
        .visible_complete = true;
    values.emitters[1].physical_color.as_mut().unwrap().quality = 3;
    assert_eq!(
        fixture_lighting(&scene, &values)[0]
            .unwrap()
            .label("7".into()),
        "7 UV"
    );
    values.emitters[1].uv_drive = 0.0;
    assert_eq!(
        fixture_lighting(&scene, &values)[0]
            .unwrap()
            .label("7".into()),
        "7"
    );
}

/// Optional native GPU evidence: real packaged RGBWAUV optics through raw DMX decoding,
/// physical evaluation, the normal Stage labels and the normal render core. Values are
/// estimated fixture data, not claims of measured physical color matching.
#[test]
#[ignore = "requires a native GPU and LIGHT_VISUAL_DIR for capture artifacts"]
fn physical_color_reference_capture() {
    let output = std::path::PathBuf::from(
        std::env::var_os("LIGHT_VISUAL_DIR").expect("canonical capture output"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode_id = profile
        .modes
        .iter()
        .find(|m| m.channels.len() == 7)
        .unwrap()
        .id;
    let fixture_id = viz_scene::uuid::Uuid::new_v4();
    let plan = viz_project::compile(&[viz_project::PatchedFixture {
        fixture_id,
        number: Some(7),
        name: "RGBWAUV reference".into(),
        profile: std::sync::Arc::new(profile),
        mode_id,
        instances: vec![viz_project::PhysicalInstance {
            instance_id: fixture_id,
            name: "RGBWAUV reference".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::new(0.0, 3.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
            scenery_size_metres: None,
            scenery_options: Default::default(),
            model_scale: 1.0,
            color_calibration: None,
            position_calibration: None,
        }],
    }]);
    let mut decoder = viz_project::Decoder::new(plan.bindings);
    let mut values = SceneValues::default();
    let mut view = ViewConfiguration::default();
    view.show_labels = true;
    view.camera.position = Vec3::new(2.0, 2.3, 4.0);
    view.camera.target = Vec3::new(0.0, 2.0, 0.0);
    view.camera.fov_degrees = 35.0;
    let mut renderer = viz_render::Renderer::headless(960, 720).expect("native GPU");
    for (name, raw, uv, complete) in [
        ("magenta", [255, 0, 255, 0, 0, 0], false, true),
        ("warm-white", [255, 190, 110, 80, 0, 0], false, true),
        ("uv-only", [0, 0, 0, 0, 0, 255], true, false),
        ("magenta-uv", [255, 0, 255, 0, 0, 255], true, false),
    ] {
        let mut slots = [0; viz_dmx::DMX_SLOTS];
        slots[..6].copy_from_slice(&raw);
        decoder.apply(
            &plan.scene,
            &[viz_dmx::UniverseFrame {
                logical_universe: 1,
                slots,
                received_micros: 0,
                stale: false,
            }],
            &mut values,
            0.0,
        );
        values.atmosphere.density = 0.45;
        let light = fixture_lighting(&plan.scene, &values)[0].unwrap();
        assert_eq!(light.uv_active, uv);
        assert_eq!(
            values.emitters[0].physical_color.unwrap().visible_complete,
            complete
        );
        if name == "uv-only" {
            assert_eq!(values.emitters[0].colour, [0.0; 3]);
            assert!(
                viz_render::semantic_lights(&plan.scene, &values).is_empty(),
                "UV alone emits no visible light or black occluding cone"
            );
        }
        for (width, height) in [(960, 720), (560, 560)] {
            renderer.resize(width, height);
            let camera = ResolvedCamera::resolve(
                &view.camera,
                view.mode,
                width as f32 / height as f32,
                plan.scene.bounds,
            );
            let mut overlay = Overlay::default();
            build_fixture_labels(
                &mut overlay,
                &plan.scene,
                &values,
                &camera,
                &view,
                width as f32,
                height as f32,
            );
            assert!(!overlay.quads.is_empty(), "fixture label in frame");
            let image = renderer
                .capture(&plan.scene, &values, &view, &overlay, 0.0)
                .unwrap();
            std::fs::write(
                output.join(format!("tl546-{name}-{width}.png")),
                crate::png::encode_rgba(image.width, image.height, &image.rgba),
            )
            .unwrap();
        }
    }
}

#[test]
fn full_3d_labels_are_screen_space_and_obey_the_authoritative_switch() {
    let mut near = Scene::default();
    push_lamp(&mut near, fixture(7, Vec3::new(0.0, 3.0, 0.0)));
    near.recompute_bounds();
    let visible = labels(&near, true);
    assert!(!visible.quads.is_empty(), "Full 3D receives a fixture tag");
    assert!(
        labels(&near, false).quads.is_empty(),
        "show_labels is authoritative"
    );

    let mut far = Scene::default();
    push_lamp(&mut far, fixture(7, Vec3::new(0.0, 3.0, -6.0)));
    far.recompute_bounds();
    let far = labels(&far, true);
    assert_eq!(
        visible.quads[0].rect[3], far.quads[0].rect[3],
        "tag height is constant in physical pixels instead of shrinking with distance"
    );
}

#[test]
fn dense_overlap_keeps_only_the_nearest_label_deterministically() {
    let camera = Camera::default();
    let near_position = camera.target;
    let far_position = camera.position + (camera.target - camera.position) * 1.5;
    let near = fixture(1, near_position);
    let mut single = Scene::default();
    push_lamp(&mut single, near.clone());
    single.recompute_bounds();
    let expected = labels(&single, true);

    let mut dense = Scene::default();
    for number in 100..180 {
        push_lamp(&mut dense, fixture(number, far_position));
    }
    // Deliberately append the near fixture after every far one: depth, not source order, wins.
    push_lamp(&mut dense, near);
    dense.recompute_bounds();
    let actual = labels(&dense, true);

    assert_eq!(
        actual.quads.len(),
        expected.quads.len(),
        "colliding far labels are dropped instead of blanketing the rig"
    );
    assert_eq!(actual.quads[0].rect, expected.quads[0].rect);
}

#[test]
fn labels_skip_non_lamps_and_lamps_hidden_by_scenery() {
    let position = Camera::default().target;
    let mut machine_only = Scene::default();
    machine_only.fixtures.push(fixture(1, position));
    machine_only.recompute_bounds();
    assert!(
        labels(&machine_only, true).quads.is_empty(),
        "a Venue object or non-light-producing machine never receives a label"
    );

    let mut visible = Scene::default();
    push_lamp(&mut visible, fixture(2, position));
    visible.recompute_bounds();
    assert!(!labels(&visible, true).quads.is_empty());

    let camera = Camera::default();
    let mut hidden = visible;
    hidden.scenery.push(SceneryObject {
        id: viz_scene::uuid::Uuid::new_v4(),
        name: "Front curtain".into(),
        position: camera.position.lerp(position, 0.5),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::splat(3.0),
        colour: [0.1; 3],
        roughness: 1.0,
        kind: SceneryKind::Curtain,
        chords: 0,
        detail: Default::default(),
        position_master: None,
    });
    hidden.recompute_bounds();
    assert!(
        labels(&hidden, true).quads.is_empty(),
        "the curtain suppresses the fixture's floating label"
    );
}

#[test]
fn plan_colour_dot_and_plain_text_contract_are_unchanged() {
    let mut scene = Scene::default();
    scene.fixtures.push(fixture(7, Vec3::new(0.0, 3.0, 0.0)));
    scene.emitters.push(EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Main".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 10.0,
        field_angle_degrees: 20.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].held_intensity = 1.0;
    let render = |show_labels| {
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::TopDown;
        view.show_labels = show_labels;
        let camera = ResolvedCamera::resolve(
            &Camera::framed(view.mode, scene.bounds),
            view.mode,
            1600.0 / 900.0,
            scene.bounds,
        );
        let mut overlay = Overlay::default();
        build_fixture_labels(&mut overlay, &scene, &values, &camera, &view, 1600.0, 900.0);
        overlay
    };

    let dots_only = render(false);
    let labelled = render(true);
    assert_eq!(
        dots_only.quads.len(),
        7,
        "the established seven-span colour dot remains"
    );
    assert_eq!(
        labelled.quads.len(),
        12,
        "dot plus plain number/address glyphs, no panel"
    );
    for (labelled_dot, original_dot) in labelled.quads[..7].iter().zip(&dots_only.quads) {
        assert_eq!(labelled_dot.rect, original_dot.rect);
        assert_eq!(labelled_dot.colour, original_dot.colour);
    }
}
