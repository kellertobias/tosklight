//! Controlled shaft-resolution comparison; no alternate shader, scene, cadence or light budget.
use super::*;
use std::{fs, path::PathBuf};

pub(super) fn evidence() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LIGHT_DRAFT_QUARTER_EVIDENCE_DIR")?);
    assert!(root.is_absolute(), "evidence destination must be absolute");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    assert!(
        root.components()
            .any(|part| part.as_os_str() == ".artifacts")
    );
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = root.join(format!("quarter-shafts-{}-{stamp}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    Some(directory)
}
fn retain(directory: Option<&std::path::Path>, name: &str, image: &CapturedImage) {
    let Some(directory) = directory else {
        return;
    };
    let path = directory.join(format!("{name}.png"));
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut encoder = png::Encoder::new(output, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&image.rgba)
        .unwrap();
    println!("Draft quarter comparison image: {}", path.display());
}
fn compare(
    name: &str,
    old: &CapturedImage,
    new: &CapturedImage,
    clear: &CapturedImage,
    narrow: bool,
) {
    assert_eq!((new.width, new.height), (old.width, old.height));
    let mut old_energy = 0_u64;
    let mut new_energy = 0_u64;
    let mut difference = 0_u64;
    let mut visible_old = 0;
    let mut visible_new = 0;
    let mut errors = Vec::new();
    for ((old, new), clear) in old
        .rgba
        .chunks_exact(4)
        .zip(new.rgba.chunks_exact(4))
        .zip(clear.rgba.chunks_exact(4))
    {
        // Ignore floor/surface lighting: unchanged clear-air pixels separate the actual shaft.
        let a = i32::from(old[0]) - i32::from(clear[0]);
        let b = i32::from(new[0]) - i32::from(clear[0]);
        visible_old += usize::from(a > 8);
        visible_new += usize::from(b > 8);
        if a <= 4 && b <= 4 {
            continue;
        }
        old_energy += a.max(0) as u64;
        new_energy += b.max(0) as u64;
        difference += (a - b).unsigned_abs() as u64;
        errors.push((a - b).unsigned_abs());
    }
    assert!(
        visible_old > 30 && visible_new > 30,
        "{name}: empty/lost shaft {visible_old}/{visible_new}"
    );
    errors.sort_unstable();
    let relative = difference as f64 / old_energy.max(1) as f64;
    let p99 = errors[errors.len() * 99 / 100];
    println!(
        "quarter shaft comparison {name}: oldVisible={visible_old} newVisible={visible_new} oldEnergy={old_energy} newEnergy={new_energy} relativeL1={relative:.6} p99Delta={p99}"
    );
    // A 2-degree shaft approaches one reduced pixel: retain a separate conservative bound,
    // rather than averaging its loss away across the background or the wide-wash cases.
    let tolerance = if narrow { 0.50 } else { 0.20 };
    assert!(
        relative < tolerance,
        "{name}: shaft shape/energy difference {relative} >= {tolerance}"
    );
    assert!(
        new_energy as f64 > old_energy as f64 * 0.75,
        "{name}: shaft energy lost"
    );
    assert!(
        (new_energy as f64) < old_energy as f64 * 1.25,
        "{name}: shaft energy gained"
    );
}

#[test]
#[ignore = "requires explicitly serialized native GPU quarter-shaft image comparison"]
fn quarter_shafts_preserve_representative_profiles_motion_and_odd_resize_against_35_percent() {
    let mut renderer = Renderer::headless(961, 541).expect("requested native GPU required");
    let directory = evidence();
    for (name, angle, sharpness, uniformity, shaped) in [
        ("fresnel", 48.0, 0.42, 0.55, false),
        ("par", 28.0, 0.40, 0.45, false),
        ("acl", 10.0, 0.40, 0.60, false),
        ("narrow", 2.0, 0.92, 0.95, false),
        ("manual-cut", 48.0, 0.42, 0.55, true),
    ] {
        let (mut scene, mut values, mut view) = rig();
        scene.emitters[0].beam_angle_degrees = angle;
        scene.emitters[0].field_angle_degrees = angle;
        scene.emitters[0].optics.sharpness = sharpness;
        scene.emitters[0].optics.uniformity = uniformity;
        if shaped {
            scene.fixtures[0].installed_shaper_blades[0] = 0.5;
        }
        values.emitters[0].intensity = 0.4;
        values.atmosphere.density = 0.35;
        view.camera.position = Vec3::new(4.0, 2.5, 6.0);
        view.camera.target = Vec3::new(0.0, 2.0, 0.0);
        if name == "narrow" {
            // The distant wash framing produced no visible 2-degree baseline at either scale.
            // Keep the physical lamp/angle/receiver, but inspect the actual shaft close up with
            // full output and denser haze. Both resolutions receive these identical inputs.
            view.camera.position = Vec3::new(0.35, 2.3, 0.6);
            values.emitters[0].intensity = 1.0;
            values.atmosphere.density = 0.8;
        }
        let density = values.atmosphere.density;
        for (width, height) in [(961, 541), (513, 289)] {
            renderer.resize(width, height);
            for phase in ["still", "subpixel"] {
                if phase == "subpixel" {
                    view.camera.position.x += if name == "narrow" { 0.0002 } else { 0.002 };
                }
                // Initialize/rebind physical targets before replacing only the beam attachment.
                let _ = capture(&mut renderer, &scene, &values, &view);
                let old_size = |n: u32| (u64::from(n) * 7).div_ceil(20) as u32;
                renderer.set_draft_beam_size_for_test((old_size(width), old_size(height)));
                let old = capture(&mut renderer, &scene, &values, &view);
                renderer.set_draft_beam_size_for_test(super::super::resources::beam_size((
                    width, height,
                )));
                let new = capture(&mut renderer, &scene, &values, &view);
                let repeat = capture(&mut renderer, &scene, &values, &view);
                assert_eq!(new.rgba, repeat.rgba, "stationary quarter frame changed");
                values.atmosphere.density = 0.0;
                let clear = capture(&mut renderer, &scene, &values, &view);
                values.atmosphere.density = density;
                let case = format!("{name}-{width}x{height}-{phase}");
                retain(directory.as_deref(), &format!("{case}-35"), &old);
                retain(directory.as_deref(), &format!("{case}-25"), &new);
                if name == "narrow" {
                    retain(directory.as_deref(), &format!("{case}-clear"), &clear);
                }
                compare(&case, &old, &new, &clear, name == "narrow");
            }
        }
    }
}

/// Diagnostic only: it deliberately reports a weak/empty baseline, not quarter acceptance.
#[test]
#[ignore = "requires explicitly serialized native GPU narrow-reference diagnosis"]
fn narrow_reference_probe_fresh_backends_report_signal_and_optics() {
    let directory = evidence();
    for (label, quality, beam_size) in [
        ("fresh-Draft35", RenderQuality::Draft, Some((337, 190))),
        ("fresh-Draft25", RenderQuality::Draft, Some((241, 136))),
        ("fresh-Ultra", RenderQuality::Ultra, None),
    ] {
        let mut renderer = Renderer::headless(961, 541).expect("requested native GPU required");
        let (mut scene, mut values, mut view) = rig();
        scene.revision = 17;
        scene.emitters[0].beam_angle_degrees = 2.0;
        scene.emitters[0].field_angle_degrees = 2.0;
        scene.emitters[0].optics.sharpness = 0.92;
        scene.emitters[0].optics.uniformity = 0.95;
        values.emitters[0].intensity = 1.0;
        values.atmosphere.density = 0.8;
        view.quality = quality;
        view.fog_variation = FogVariation {
            lamp_cloudiness: 0.0,
            lamp_turbulence: 0.0,
            laser_cloudiness: 0.0,
            laser_turbulence: 0.0,
        };
        view.camera.position = Vec3::new(0.35, 2.3, 0.6);
        view.camera.target = Vec3::new(0.0, 2.0, 0.0);
        let _ = capture(&mut renderer, &scene, &values, &view);
        if let Some(size) = beam_size {
            renderer.set_draft_beam_size_for_test(size);
        }
        let image = capture(&mut renderer, &scene, &values, &view);
        let light = renderer.shaded_frame().lights[0];
        let proxy = renderer.shaded_frame().beams[0];
        println!(
            "narrow probe {label}: positionRange={:?} directionCos={:?} params={:?} shadow={:?} proxy={:?} intensity={:?}",
            light.position_range,
            light.direction_cos_outer,
            light.params,
            light.shadow,
            proxy.params,
            light.colour_intensity
        );
        values.atmosphere.density = 0.0;
        let clear = capture(&mut renderer, &scene, &values, &view);
        retain(directory.as_deref(), label, &image);
        retain(directory.as_deref(), &format!("{label}-clear"), &clear);
        let mut positive = Vec::new();
        for (a, b) in image.rgba.chunks_exact(4).zip(clear.rgba.chunks_exact(4)) {
            let difference = i16::from(a[0]) - i16::from(b[0]);
            if difference > 0 {
                positive.push(difference);
            }
        }
        positive.sort_unstable();
        println!(
            "narrow probe {label}: shaftMax={} nonzero={} over4={} over8={} positiveMedian={} positiveP99={}",
            positive.last().copied().unwrap_or(0),
            positive.len(),
            positive.iter().filter(|n| **n > 4).count(),
            positive.iter().filter(|n| **n > 8).count(),
            positive.get(positive.len() / 2).copied().unwrap_or(0),
            positive
                .get(positive.len() * 99 / 100)
                .copied()
                .unwrap_or(0)
        );
    }
}
