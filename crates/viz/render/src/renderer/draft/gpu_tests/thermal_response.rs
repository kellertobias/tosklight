//! Real surface pixels prove runtime thermal state reaches both native filled backends.
use super::*;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn evidence_directory() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LIGHT_THERMAL_GPU_EVIDENCE_DIR")?);
    assert!(
        root.is_absolute(),
        "explicit evidence root must be absolute/canonical"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = root.join(format!("thermal-response-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&directory).expect("create caller-owned thermal evidence directory");
    Some(directory)
}

fn retain(
    directory: Option<&std::path::Path>,
    quality: RenderQuality,
    phase: &str,
    image: &CapturedImage,
) {
    let Some(directory) = directory else {
        return;
    };
    let path = directory.join(format!("{quality:?}-{phase}.ppm"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    write!(file, "P6\n{} {}\n255\n", image.width, image.height).unwrap();
    let rgb: Vec<u8> = image
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect();
    file.write_all(&rgb).unwrap();
    println!("thermal pixel evidence: {}", path.display());
}

fn advance(values: &mut SceneValues, scene: &Scene, elapsed: f64, target: f32) {
    let colour = values.emitters[0].colour;
    assert_eq!(values.emitters[0].intensity, target);
    values.advance_lamp_response(scene, elapsed);
    assert_eq!(
        values.emitters[0].intensity, target,
        "thermal advance changed authoritative DMX target"
    );
    assert_eq!(
        values.emitters[0].colour, colour,
        "thermal advance changed authoritative RGB"
    );
    assert_eq!(
        values.emitters[0].held_intensity, 0.0,
        "persistence must never contribute to this proof"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU thermal-response pixel acceptance"]
fn tungsten_warming_cooling_and_instant_reset_reach_draft_and_ultra_pixels() {
    let mut renderer = Renderer::headless(513, 513).expect("requested native GPU required");
    let evidence = evidence_directory();
    for quality in [RenderQuality::Draft, RenderQuality::Ultra] {
        let (mut scene, mut values, mut view) = rig();
        view.quality = quality;
        scene.fixtures[0].lamp_response = LampResponseProfile::TungstenNominal;
        scene.emitters[0].beam_angle_degrees = 48.0;
        scene.emitters[0].field_angle_degrees = 48.0;
        scene.emitters[0].optics.sharpness = 0.42;
        scene.emitters[0].optics.uniformity = 0.55;
        values.emitters[0].intensity = 0.0;
        // Initialize known off equilibrium; no persistence API is called anywhere in this test.
        advance(&mut values, &scene, 0.0, 0.0);
        let off = capture(&mut renderer, &scene, &values, &view);
        assert_eq!(receiver(&off, &scene, &view, Vec3::ZERO), [0.0; 3]);

        values.emitters[0].intensity = 0.2;
        advance(&mut values, &scene, 0.025, 0.2);
        let warming = capture(&mut renderer, &scene, &values, &view);
        let warm = receiver(&warming, &scene, &view, Vec3::ZERO);
        advance(&mut values, &scene, 2.0, 0.2);
        let steady_image = capture(&mut renderer, &scene, &values, &view);
        let steady = receiver(&steady_image, &scene, &view, Vec3::ZERO);
        for channel in 0..3 {
            assert!(
                warm[channel] > 5.0 && steady[channel] > warm[channel] + 2.0,
                "positive partial warm-up must precede steady pool: {quality:?}, {warm:?} -> {steady:?}"
            );
        }

        values.emitters[0].intensity = 0.0;
        advance(&mut values, &scene, 0.025, 0.0);
        let cooling = capture(&mut renderer, &scene, &values, &view);
        let tail = receiver(&cooling, &scene, &view, Vec3::ZERO);
        assert!(
            tail.iter().all(|level| *level > 5.0),
            "raw DMX zero must retain a visible physical cooling tail: {quality:?}, {tail:?}"
        );

        // Change only the response profile while a visible tail exists, keeping source identity,
        // position, orientation, floor, camera and receiver fixed. Instant must discard that tail.
        let mut instant_scene = scene.clone();
        let mut instant_values = values.clone();
        instant_scene.fixtures[0].lamp_response = LampResponseProfile::Instant;
        assert_eq!(
            instant_scene.fixtures[0].instance_id,
            scene.fixtures[0].instance_id
        );
        assert_eq!(
            instant_scene.emitters[0].head_index,
            scene.emitters[0].head_index
        );
        assert_eq!(instant_scene.emitter_ids, scene.emitter_ids);
        advance(&mut instant_values, &instant_scene, 0.0, 0.0);
        assert!(instant_values.emitters[0].lamp_response.is_none());
        let instant = capture(&mut renderer, &instant_scene, &instant_values, &view);
        assert_eq!(
            receiver(&instant, &instant_scene, &view, Vec3::ZERO),
            [0.0; 3]
        );
        // A positive Instant dimmer with current black RGB must not resurrect previous white RGB.
        instant_values.emitters[0].intensity = 0.2;
        instant_values.emitters[0].colour = [0.0; 3];
        advance(&mut instant_values, &instant_scene, 0.0, 0.2);
        let black_rgb = capture(&mut renderer, &instant_scene, &instant_values, &view);
        assert_eq!(
            receiver(&black_rgb, &instant_scene, &view, Vec3::ZERO),
            [0.0; 3]
        );

        advance(&mut values, &scene, 2.0, 0.0);
        assert_eq!(values.emitters[0].visible_intensity(), 0.0);
        let settled = capture(&mut renderer, &scene, &values, &view);
        assert_eq!(receiver(&settled, &scene, &view, Vec3::ZERO), [0.0; 3]);
        println!("{quality:?} thermal centre warm/steady/tail: {warm:?}, {steady:?}, {tail:?}");
        for (phase, image) in [
            ("off", &off),
            ("warming", &warming),
            ("steady", &steady_image),
            ("cooling", &cooling),
            ("instant-off", &instant),
            ("instant-black-rgb", &black_rgb),
            ("settled", &settled),
        ] {
            retain(evidence.as_deref(), quality, phase, image);
        }
    }
}
