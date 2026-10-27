//! Explicit pixel proof on the rig's real floor: no receiver or source relocation per case.
use super::*;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn evidence_directory() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LIGHT_OPTICS_GPU_EVIDENCE_DIR")?);
    assert!(
        root.is_absolute(),
        "explicit evidence root must be absolute/canonical"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = root.join(format!("manual-shapers-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&directory).expect("create caller-owned evidence directory");
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
    println!("manual barn-door pixel evidence: {}", path.display());
}

#[test]
#[ignore = "requires explicitly serialized native GPU manual barn-door pixel acceptance"]
fn soft_fresnel_manual_open_partial_and_full_stroke_pool_in_draft_and_ultra() {
    let mut renderer = Renderer::headless(513, 513).expect("requested native GPU required");
    let (mut scene, mut values, mut view) = rig();
    scene.emitters[0].beam_angle_degrees = 48.0;
    scene.emitters[0].field_angle_degrees = 48.0;
    scene.emitters[0].optics.sharpness = 0.42;
    scene.emitters[0].optics.uniformity = 0.55;
    values.emitters[0].intensity = 0.2;
    let evidence = evidence_directory();
    // Floor top is y=0 and the existing fixture is at y=4. Its unchanged orientation points
    // down, with local +u along world +X. Every receiver stays on that physical surface.
    let centre = Vec3::ZERO;
    let interior = Vec3::new(-0.8, 0.0, 0.0);
    let exterior = Vec3::new(0.8, 0.0, 0.0);
    for quality in [RenderQuality::Draft, RenderQuality::Ultra] {
        view.quality = quality;
        scene.fixtures[0].installed_shaper_blades = [0.0; 4];
        let open = capture(&mut renderer, &scene, &values, &view);
        let level = |image: &CapturedImage, point| receiver(image, &scene, &view, point)[0];
        let (open_inside, open_outside) = (level(&open, interior), level(&open, exterior));
        let falloff = [
            level(&open, centre),
            level(&open, exterior),
            level(&open, Vec3::new(1.55, 0.0, 0.0)),
        ];
        println!("{quality:?} open centre/middle/edge: {falloff:?}");
        assert!(
            open_inside > 20.0 && open_outside > 20.0,
            "open wash positive controls: {open_inside}, {open_outside}"
        );
        assert!(
            falloff[0] > falloff[1] + 8.0 && falloff[1] > falloff[2] + 8.0,
            "soft centre-to-field fade must precede the cut: {quality:?} {falloff:?}"
        );
        retain(evidence.as_deref(), quality, "open-soft-wash", &open);

        // Manual fraction .5 maps to native insertion1: +u blade crosses the centre.
        // Keep revision, camera, source, floor and DMX unchanged; only installed hardware moves.
        scene.fixtures[0].installed_shaper_blades[0] = 0.5;
        let partial = capture(&mut renderer, &scene, &values, &view);
        let inside = receiver(&partial, &scene, &view, interior)[0];
        let outside = receiver(&partial, &scene, &view, exterior)[0];
        println!("{quality:?} partial inside/outside: {inside}, {outside}");
        assert!(
            inside > open_inside * 0.65,
            "cut lost its interior receiver: {quality:?} {inside} / {open_inside}"
        );
        assert!(
            outside < open_outside * 0.15 + 2.0,
            "cut retained exterior spill: {quality:?} {outside} / {open_outside}"
        );
        retain(evidence.as_deref(), quality, "partial-cut", &partial);

        scene.fixtures[0].installed_shaper_blades[0] = 1.0;
        let closed = capture(&mut renderer, &scene, &values, &view);
        for x in [-1.55, -0.8, 0.0, 0.8, 1.55] {
            for z in [-0.8, 0.0, 0.8] {
                let dark = receiver(&closed, &scene, &view, Vec3::new(x, 0.0, z))[0];
                assert!(
                    dark < 6.0,
                    "full manual stroke retained pool at {x}, {z}: {quality:?} {dark}"
                );
            }
        }
        retain(evidence.as_deref(), quality, "full-stroke-closed", &closed);
    }
}
