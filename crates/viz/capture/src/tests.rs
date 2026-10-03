use super::*;
use viz_scene::{
    BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance, RenderQuality, SceneryKind, SceneryObject,
};

#[test]
fn capture_quality_can_be_selected_explicitly() {
    let options = parse(
        [
            "--show",
            "demo.show",
            "--output",
            "frames",
            "--quality",
            "ultra",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .expect("capture arguments should parse")
    .expect("capture should run");
    assert_eq!(options.quality, RenderQuality::Ultra);
}

#[test]
fn capture_rejects_an_unknown_quality() {
    let error = parse(
        [
            "--show",
            "demo.show",
            "--output",
            "frames",
            "--quality",
            "cinema",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .err()
    .expect("unknown quality should fail");
    assert_eq!(error, "cinema is not a render quality");
}

fn two_light_surface_scene() -> (Scene, SceneValues, ViewConfiguration) {
    use viz_scene::{glam::Vec3, uuid::Uuid};

    let fixture = |name: &str, position: Vec3| FixtureInstance {
        drawn_as_scenery: false,
        invisible: false,
        instance_id: Uuid::new_v4(),
        fixture_id: Uuid::new_v4(),
        name: name.to_owned(),
        number: None,
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
        address: None,
        model: None,
        fallback: None,
    };
    let emitter = |fixture_index| EmitterInstance {
        fixture_index,
        head_index: 0,
        label: "Main".to_owned(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 12.0,
        field_angle_degrees: 28.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
        effect: None,
    };

    // Light zero is deliberately far outside the shot. Light one illuminates the receiving
    // deck in front of the camera. A storage-array stride error makes culling light one from
    // fields in the middle of light zero, which removes this pool while leaving a plausible
    // fixture aperture and beam volume behind.
    let mut scene = Scene {
        fixtures: vec![
            fixture("Offscreen", Vec3::new(-20.0, 4.0, 0.0)),
            fixture("Surface", Vec3::new(6.0, 4.0, 0.0)),
        ],
        emitters: vec![emitter(0), emitter(1)],
        scenery: vec![SceneryObject {
            id: Uuid::new_v4(),
            name: "Receiving deck".to_owned(),
            position: Vec3::new(6.0, -0.05, 0.0),
            size: Vec3::new(4.0, 0.1, 4.0),
            colour: [0.35, 0.35, 0.35],
            roughness: 0.8,
            kind: SceneryKind::Riser,
            ..SceneryObject::default()
        }],
        ..Scene::default()
    };
    scene.recompute_bounds();

    let mut values = SceneValues::default();
    values.resize(2);
    values.atmosphere.density = 0.0;
    for value in &mut values.emitters {
        value.intensity = 1.0;
        value.held_intensity = 1.0;
    }

    let mut view = ViewConfiguration::default();
    view.camera.position = Vec3::new(6.0, 4.5, 7.0);
    view.camera.target = Vec3::new(6.0, 0.0, 0.0);
    view.ambient = 0.0;
    view.show_labels = false;
    view.floor_grid = false;
    (scene, values, view)
}

/// The proven-lit deck of [`two_light_surface_scene`], with something hung between the working
/// light and the deck it lights.
///
/// Building on a configuration already known to put light on that deck is the point: an
/// occlusion test that starts from a shot the light never reached proves nothing.
fn occluder_scene(kind: SceneryKind) -> (Scene, SceneValues, ViewConfiguration) {
    use viz_scene::{glam::Vec3, uuid::Uuid};

    let (mut scene, mut values, view) = two_light_surface_scene();
    // Light zero is deliberately off in the wings; light one is the one over the deck.
    values.emitters[0].intensity = 0.0;
    values.emitters[0].held_intensity = 0.0;
    scene.scenery.push(SceneryObject {
        id: Uuid::new_v4(),
        name: "Occluder".to_owned(),
        position: Vec3::new(6.0, 2.0, 0.0),
        size: Vec3::new(4.0, 0.4, 4.0),
        colour: [0.02, 0.02, 0.02],
        roughness: 0.95,
        kind,
        ..SceneryObject::default()
    });
    scene.recompute_bounds();
    (scene, values, view)
}

fn picture_brightness(image: &viz_render::CapturedImage) -> f64 {
    let total: u64 = image
        .rgba
        .chunks_exact(4)
        .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
        .sum();
    total as f64 / (image.rgba.len() / 4) as f64
}

/// Opaque scenery stops light at every quality tier.
///
/// A drape that lets light through is not a cheaper picture, it is a wrong one. The control in
/// each pass is the same rig with nothing hung in it: without that, an occlusion test that
/// started from a shot the light never reached would pass on nothing at all.
#[test]
fn opaque_scenery_stops_light_at_every_quality_tier() {
    let overlay = viz_render::Overlay::default();
    let mut renderer = viz_render::Renderer::headless(320, 180)
        .expect("a headless renderer; this machine has no GPU or software adapter");
    let evidence = std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from);
    let (open, open_values, _) = two_light_surface_scene();
    let open_lit = darkened(&open_values, 0);
    let open_dark = all_out(&open_lit);

    for kind in [SceneryKind::Prop, SceneryKind::Curtain] {
        let (scene, values, mut view) = occluder_scene(kind);
        let dark = all_out(&values);
        for quality in RenderQuality::ALL {
            view.quality = quality;
            let reaches = picture_brightness(
                &renderer
                    .capture(&open, &open_lit, &view, &overlay, 1.0)
                    .expect("the deck with nothing hung over it"),
            ) - picture_brightness(
                &renderer
                    .capture(&open, &open_dark, &view, &overlay, 0.0)
                    .expect("the same deck with the light out"),
            );
            let blocked_image = renderer
                .capture(&scene, &values, &view, &overlay, 1.0)
                .expect("the deck with the occluder hung over it");
            if let Some(directory) = evidence.as_deref() {
                std::fs::create_dir_all(directory).expect("capture evidence directory");
                write_png(
                    &directory.join(format!(
                        "tl-401-{}-{}.png",
                        format!("{kind:?}").to_lowercase(),
                        quality.label()
                    )),
                    blocked_image.width,
                    blocked_image.height,
                    &blocked_image.rgba,
                )
                .expect("occlusion evidence");
            }
            let blocked = picture_brightness(&blocked_image)
                - picture_brightness(
                    &renderer
                        .capture(&scene, &dark, &view, &overlay, 0.0)
                        .expect("the occluded deck with the light out"),
                );
            assert!(
                reaches > 2.0,
                "{} never lit the deck with nothing in the way, so this proves nothing: {reaches:.3}",
                quality.label()
            );
            // What is left is the occluder's own lit face, not light on the deck behind it.
            assert!(
                blocked < reaches * 0.25,
                "{kind:?} at {} let light past: open +{reaches:.3}, occluded +{blocked:.3}",
                quality.label()
            );
        }
    }
}

fn darkened(values: &SceneValues, emitter: usize) -> SceneValues {
    let mut copy = values.clone();
    copy.emitters[emitter].intensity = 0.0;
    copy.emitters[emitter].held_intensity = 0.0;
    copy
}

fn all_out(values: &SceneValues) -> SceneValues {
    let mut copy = values.clone();
    for value in &mut copy.emitters {
        value.intensity = 0.0;
        value.held_intensity = 0.0;
    }
    copy
}

/// A shaft stops where it meets something opaque, at every tier.
///
/// With haze the beam volume is the picture, and it is drawn by a pass that writes no depth of
/// its own. A drape hung across the shaft must take the part below it away, not just shade the
/// floor underneath.
#[test]
fn a_shaft_stops_where_it_meets_a_drape_at_every_quality_tier() {
    let overlay = viz_render::Overlay::default();
    let mut renderer = viz_render::Renderer::headless(320, 180)
        .expect("a headless renderer; this machine has no GPU or software adapter");
    let evidence = std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from);
    let (open, open_values, _) = two_light_surface_scene();
    let mut open_lit = darkened(&open_values, 0);
    open_lit.atmosphere.density = 0.6;
    let open_dark = all_out(&open_lit);

    let (scene, mut values, mut view) = occluder_scene(SceneryKind::Curtain);
    values.atmosphere.density = 0.6;
    let dark = all_out(&values);

    for quality in RenderQuality::ALL {
        view.quality = quality;
        let shaft = picture_brightness(
            &renderer
                .capture(&open, &open_lit, &view, &overlay, 1.0)
                .expect("the hazy shaft with nothing across it"),
        ) - picture_brightness(
            &renderer
                .capture(&open, &open_dark, &view, &overlay, 0.0)
                .expect("the same haze with the light out"),
        );
        let cut_image = renderer
            .capture(&scene, &values, &view, &overlay, 1.0)
            .expect("the hazy shaft with a drape across it");
        if let Some(directory) = evidence.as_deref() {
            std::fs::create_dir_all(directory).expect("capture evidence directory");
            write_png(
                &directory.join(format!("tl-401-shaft-{}.png", quality.label())),
                cut_image.width,
                cut_image.height,
                &cut_image.rgba,
            )
            .expect("shaft evidence");
        }
        let cut = picture_brightness(&cut_image)
            - picture_brightness(
                &renderer
                    .capture(&scene, &dark, &view, &overlay, 0.0)
                    .expect("the drape and haze with the light out"),
            );
        assert!(
            shaft > 2.0,
            "{} drew no shaft with nothing across it, so this proves nothing: {shaft:.3}",
            quality.label()
        );
        // The length above the drape is still legitimately lit; the length below it is not.
        assert!(
            cut < shaft * 0.75,
            "{} kept the shaft below the drape: open +{shaft:.3}, cut +{cut:.3}",
            quality.label()
        );
    }
}

/// The generated demo show, built exactly as a capture builds it.
fn demo_scene_and_values(name: &str) -> (Scene, SceneValues) {
    let directory = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
        PathBuf::from,
    );
    // One workspace per test: these run in parallel and each builds its own library.
    let workspace = directory.join("viz-capture-golden").join(name);
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).expect("workspace");
    let library =
        light_fixture::FixtureLibrary::open(workspace.join("fixtures.sqlite")).expect("library");
    library
        .load_fixture_package_directory(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library"),
        )
        .expect("the shipped packages load");
    let show = workspace.join("demo-show.show");
    viz_demo::generate(library, &show).expect("the demo generates");
    let (scene, bindings, preview) = scene_from(&show).expect("the scene builds");
    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    let mut decoder = viz_project::Decoder::new(bindings);
    decoder.apply(&scene, &preview, &mut values, 0.0);
    (scene, values)
}

fn demo_scene(name: &str) -> Scene {
    demo_scene_and_values(name).0
}

fn body_of(scene: &Scene, name: &str) -> BodyKind {
    scene
        .fixtures
        .iter()
        .find(|fixture| fixture.name.starts_with(name))
        .unwrap_or_else(|| panic!("the demo rig has no fixture named {name}"))
        .body
        .kind
}

/// The surface pool from a light after index zero must survive quality changes, small camera
/// moves and repeated redraws. This is the actual operator path: the culling compute shader
/// chooses the lights evaluated by the surface shader, while the beam pass is independent and
/// can otherwise make the broken frame look superficially alive.
#[test]
fn a_second_light_keeps_its_surface_pool_across_views_and_quality_tiers() {
    let (scene, values, mut view) = two_light_surface_scene();
    let overlay = viz_render::Overlay::default();
    let mut renderer = viz_render::Renderer::headless(320, 180)
        .expect("a headless renderer; this machine has no GPU or software adapter");

    let mut reference = values.clone();
    reference.emitters[1].intensity = 0.0;
    reference.emitters[1].held_intensity = 0.0;
    let dark_image = renderer
        .capture(&scene, &reference, &view, &overlay, 0.0)
        .expect("the receiving surface without its light");
    let dark = picture_brightness(&dark_image);
    let evidence = std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from);
    if let Some(directory) = evidence.as_deref() {
        std::fs::create_dir_all(directory).expect("capture evidence directory");
        write_png(
            &directory.join("tl-202-surface-dark.png"),
            dark_image.width,
            dark_image.height,
            &dark_image.rgba,
        )
        .expect("dark surface evidence");
    }

    let base_position = view.camera.position;
    for quality in RenderQuality::ALL {
        view.quality = quality;
        for camera_offset in [-0.3_f32, 0.3] {
            view.camera.position.x = base_position.x + camera_offset;
            let first = renderer
                .capture(&scene, &values, &view, &overlay, 1.0)
                .expect("the active pool renders");
            let repeated = renderer
                .capture(&scene, &values, &view, &overlay, 1.0)
                .expect("the same active pool renders again");
            let first_brightness = picture_brightness(&first);
            let repeated_brightness = picture_brightness(&repeated);
            if quality == RenderQuality::Ultra
                && camera_offset > 0.0
                && let Some(directory) = evidence.as_deref()
            {
                write_png(
                    &directory.join("tl-202-surface-lit.png"),
                    repeated.width,
                    repeated.height,
                    &repeated.rgba,
                )
                .expect("lit surface evidence");
            }
            assert!(
                first_brightness > dark + 4.0,
                "{} at camera offset {camera_offset} lost the surface pool: dark {dark:.2}, lit {first_brightness:.2}",
                quality.label()
            );
            assert!(
                (first_brightness - repeated_brightness).abs() < 0.25,
                "{} changed on an identical redraw: {first_brightness:.2} to {repeated_brightness:.2}",
                quality.label()
            );
        }
    }
}

#[test]
fn a_named_fixture_capture_frames_the_fixture_body() {
    let scene = demo_scene("fixture-framing");
    let bounds = fixture_bounds(&scene, "Sunstrip").expect("the demo carries Sunstrips");
    assert!(
        bounds.extent().x > 0.5,
        "the one-metre extrusion is in frame"
    );
    assert!(
        fixture_bounds(&scene, "Not in this show").is_err(),
        "a typo must fail rather than silently capture the whole rig"
    );
}

#[test]
fn the_shipped_sunstrip_has_ten_cells_inside_its_extrusion() {
    let scene = demo_scene("sunstrip-face-bounds");
    let (fixture_index, fixture) = scene
        .fixtures
        .iter()
        .enumerate()
        .find(|(_, fixture)| fixture.name == "Sunstrip 1")
        .expect("the representative strip");
    let emitters: Vec<_> = scene
        .emitters
        .iter()
        .filter(|emitter| emitter.fixture_index == fixture_index as u32)
        .collect();
    assert_eq!(
        emitters.len(),
        10,
        "the shared master is not an eleventh cell"
    );

    let left = emitters
        .iter()
        .map(|emitter| emitter.local_origin.x - emitter.optics.source.width * 0.5)
        .fold(f32::INFINITY, f32::min);
    let right = emitters
        .iter()
        .map(|emitter| emitter.local_origin.x + emitter.optics.source.width * 0.5)
        .fold(f32::NEG_INFINITY, f32::max);
    let body_half = fixture.body.size.x * 0.5;
    assert!(left >= -body_half - 1e-6, "left cell overhangs: {left}");
    assert!(right <= body_half + 1e-6, "right cell overhangs: {right}");
}

/// The golden scene the plan asks for: every class the renderer has to draw, resolving to the
/// body it is meant to resolve to rather than merely to something visible.
///
/// A fixture package that loses its type, or a fallback that starts answering `Generic`, turns
/// a Sunstrip into a box or a scanner into a lantern — visible in a capture, and invisible in
/// a test that only counted fixtures.
#[test]
fn every_demo_fixture_class_resolves_to_its_intended_body() {
    let scene = demo_scene("bodies");
    for (fixture, expected) in [
        // Moving fixtures are bodies with a yoke, whatever else they are.
        ("Wash", BodyKind::MovingHead),
        ("Beam", BodyKind::MovingHead),
        // Bars of cells.
        ("Sunstrip", BodyKind::Bar),
        ("Fourblinder", BodyKind::Bar),
        // Static lanterns on a clamp, profiles among them.
        ("Profile", BodyKind::Lantern),
        ("ACL", BodyKind::Lantern),
        ("LED", BodyKind::Lantern),
        ("Front Truss", BodyKind::Lantern),
        // Machines that make no light of their own, and the laser, which is its own thing.
        ("Hazer", BodyKind::Machine),
        ("Laser", BodyKind::Machine),
    ] {
        assert_eq!(
            body_of(&scene, fixture),
            expected,
            "{fixture} resolved to the wrong body"
        );
    }
}

/// The determinism the demo video depends on.
///
/// CI composites a video from separate capture runs, so the same commit and the same arguments
/// have to produce the same picture. Everything that could vary is stated rather than measured
/// — resolution, camera, time step, haze — and auto-exposure adapts over time, which is why a
/// capture settles first.
///
/// It is not bit-exact, and measuring showed why: two runs differ on a few dozen bytes of a
/// 230,400-byte frame, each by a single value. Settling longer shrinks it (38 bytes at 8
/// frames, 13 at 60) without reaching zero, so it is not exposure converging — it is the GPU
/// resolving multisampled coverage in whatever order it likes. So the guarantee is stated as
/// what the video actually needs: nothing moves perceptibly between runs.
#[test]
fn two_capture_runs_of_the_same_frame_are_identical() {
    let scene = demo_scene("determinism");
    let mut view = ViewConfiguration::default();
    view.mode = ViewMode::Full3d;
    view.camera = viz_scene::Camera::framed(ViewMode::Full3d, scene.bounds);
    let overlay = viz_render::Overlay::default();
    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    values.atmosphere.density = viz_scene::DEFAULT_DENSITY;

    let capture_once = || {
        let mut renderer = viz_render::Renderer::headless(320, 180)
            .expect("a headless renderer; this machine has no GPU or software adapter");
        for frame in 0..60 {
            renderer
                .capture(&scene, &values, &view, &overlay, frame as f32 / 30.0)
                .expect("settling frame");
        }
        renderer
            .capture(&scene, &values, &view, &overlay, 60.0 / 30.0)
            .expect("recorded frame")
            .rgba
    };

    let first = capture_once();
    let second = capture_once();
    assert_eq!(first.len(), second.len(), "the two runs differ in size");
    let deltas: Vec<u8> = first
        .iter()
        .zip(&second)
        .map(|(left, right)| left.abs_diff(*right))
        .filter(|delta| *delta > 0)
        .collect();
    let worst = deltas.iter().copied().max().unwrap_or(0);
    // Not bit-exact, and it cannot be: a GPU is free to vary the order it resolves
    // multisampled coverage in, so a handful of pixels land a single value apart between runs.
    // What matters for a composited video is that nothing moves perceptibly, so the bound is
    // stated in those terms instead of pretending to an exactness the hardware does not offer.
    assert!(
        worst <= 1,
        "{} bytes differ between two runs of the same frame, the worst by {worst}; that is \
         more than rounding and a composited demo video would flicker",
        deltas.len()
    );
    assert!(
        deltas.len() * 1_000 < first.len(),
        "{} of {} bytes differ between two runs — under a tenth of a percent is rounding, \
         this is something moving",
        deltas.len(),
        first.len()
    );
}

/// The whole dimmer has to be worth moving.
///
/// The Stage used to adapt its exposure to how much light the rig was producing, the way an
/// eye does. On a rig that is the wrong instinct: taking the fixtures down opened the exposure
/// by nearly as much as they had closed, so the first tenth of a fade was the entire visible
/// change and the top nine tenths drew the same picture. What an operator needs from a Stage
/// is the opposite — dim has to look dim, and the difference between half and full has to be
/// as plain as the difference between out and a tenth.
#[test]
fn the_picture_keeps_getting_brighter_all_the_way_up_the_dimmer() {
    let scene = demo_scene("exposure");
    let mut view = ViewConfiguration::default();
    view.mode = ViewMode::Full3d;
    view.camera = viz_scene::Camera::framed(ViewMode::Full3d, scene.bounds);
    let overlay = viz_render::Overlay::default();
    let mut renderer = viz_render::Renderer::headless(320, 180)
        .expect("a headless renderer; this machine has no GPU or software adapter");

    let mut brightness_at = |level: f32| {
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values.atmosphere.density = viz_scene::DEFAULT_DENSITY;
        for emitter in &mut values.emitters {
            emitter.intensity = level;
            emitter.held_intensity = level;
        }
        let frame = renderer
            .capture(&scene, &values, &view, &overlay, 1.0)
            .expect("a captured frame");
        let total: u64 = frame
            .rgba
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
            .sum();
        total as f64 / (frame.rgba.len() / 4) as f64
    };

    let levels = [0.0_f32, 0.1, 0.25, 0.5, 1.0];
    let measured: Vec<f64> = levels.iter().copied().map(&mut brightness_at).collect();
    for pair in measured.windows(2) {
        assert!(
            pair[1] > pair[0],
            "every step up the dimmer is brighter than the one below it: {measured:?}"
        );
    }

    // The half-to-full step is the one that used to vanish. It has to be a real share of the
    // range rather than the rounding it became.
    let range = measured[4] - measured[0];
    let top_step = measured[4] - measured[3];
    assert!(
        top_step > range * 0.12,
        "half to full is {top_step:.2} of a {range:.2} range, which is a fader with nothing \
         in its top half: {measured:?}"
    );

    // And the bottom of the fader must not already be most of the picture.
    let bottom_step = measured[1] - measured[0];
    assert!(
        bottom_step < range * 0.5,
        "out to a tenth is {bottom_step:.2} of a {range:.2} range, so the fade happens \
         entirely in the bottom of the fader: {measured:?}"
    );
}

/// A model from a package, one of the audited built-in set, or a Venue object's declared
/// shape built at the size it was placed. Whichever supplied it, a fixture with no geometry
/// draws as nothing, which in a still capture looks exactly like one that is simply unlit.
#[test]
fn every_demo_fixture_resolves_to_geometry() {
    let scene = demo_scene("models");
    let shapeless: Vec<&str> = scene
        .fixtures
        .iter()
        .filter(|fixture| fixture.model.is_none() && !fixture.drawn_as_scenery)
        .map(|fixture| fixture.name.as_str())
        .collect();
    assert!(
        shapeless.is_empty(),
        "these fixtures resolved to no geometry and would draw as nothing: {shapeless:?}"
    );
}

/// A scanner classifies as a lantern, and lightless Venue scenery as `Generic`; both are right.
/// No light may fall through to `Generic`, which means the projection could not tell what it was.
#[test]
fn no_demo_fixture_falls_through_to_a_shapeless_body() {
    let scene = demo_scene("shapeless");
    let lit: Vec<u32> = scene.emitters.iter().map(|e| e.fixture_index).collect();
    let shapeless: Vec<&str> = scene
        .fixtures
        .iter()
        .zip(0u32..)
        .filter(|(fixture, i)| lit.contains(i) && fixture.body.kind == BodyKind::Generic)
        .map(|(fixture, _)| fixture.name.as_str())
        .collect();
    assert!(
        shapeless.is_empty(),
        "these fixtures resolved to no recognisable body: {shapeless:?}"
    );
}

/// A capture of the demo has to have something to light. An empty or unpatched rig renders a
/// dark stage, which is indistinguishable from a broken renderer in a still frame.
#[test]
fn the_demo_scene_carries_emitters_for_every_fixture() {
    let scene = demo_scene("emitters");
    assert!(!scene.fixtures.is_empty(), "the demo rig is empty");
    assert!(
        scene.emitters.len() >= scene.fixtures.len(),
        "{} fixtures produced only {} emitters",
        scene.fixtures.len(),
        scene.emitters.len()
    );
}
