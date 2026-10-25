use super::*;
use std::{
    fs,
    io::BufReader,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn independent_manual_softness_keeps_all_four_exterior_half_planes_dark() {
    let mut gate = open_gate();
    gate.params[1] = 0.58;
    gate.frost = 0.8;
    gate.shaper_params[0] = 0.04;
    for (blade, outside, inside) in [
        (0, Vec2::new(0.301, 0.0), Vec2::new(0.20, 0.0)),
        (1, Vec2::new(-0.301, 0.0), Vec2::new(-0.20, 0.0)),
        (2, Vec2::new(0.0, 0.301), Vec2::new(0.0, 0.20)),
        (3, Vec2::new(0.0, -0.301), Vec2::new(0.0, -0.20)),
    ] {
        gate.shapers = [0.0; 4];
        // Outer edge 0.30 = 1 - insertion + manual softness; fade stays inside it.
        gate.shapers[blade] = 0.74;
        assert_eq!(gate.transmission(outside, &[]), 0.0, "blade {blade}");
        assert!(gate.transmission(inside, &[]) > 0.0, "blade {blade}");
    }
}

#[test]
fn zero_default_preserves_dynamic_feather_and_manual_rotation_full_stroke_is_explicit() {
    let mut gate = open_gate();
    assert_eq!(gate.shaper_params, [0.0; 4]);
    gate.params[1] = 0.58;
    gate.shapers[0] = 1.0;
    let point = Vec2::new(0.3, 0.0);
    let legacy = gate.transmission(point, &[]);
    assert!(
        legacy > 0.0,
        "legacy dynamic feather keeps its broad penumbra"
    );
    gate.shaper_params[0] = 0.04;
    assert_eq!(gate.transmission(point, &[]), 0.0);
    gate.shapers[0] = 1.4;
    assert!(gate.invalid_reason(&[]).is_none());
    assert!(gate.transmission(Vec2::new(-0.6, 0.0), &[]) > 0.0);
    assert_eq!(gate.transmission(Vec2::new(-0.2, 0.0), &[]), 0.0);
    gate.angles[0] = std::f32::consts::FRAC_PI_2;
    assert!(gate.transmission(Vec2::new(0.0, 0.6), &[]) > 0.0);
    assert_eq!(gate.transmission(Vec2::new(0.0, 0.2), &[]), 0.0);
    gate.shaper_params[0] = 0.0;
    assert!(
        gate.invalid_reason(&[]).is_some(),
        "dynamic stroke must not silently change"
    );
}

#[test]
fn manual_softness_has_its_own_mask_identity_and_all_float_slots_are_validated() {
    let mut gate = open_gate();
    let legacy = gate.key();
    gate.shaper_params[0] = 0.04;
    assert_ne!(gate.key(), legacy);
    for invalid in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
        gate.shaper_params[0] = invalid;
        assert!(gate.invalid_reason(&[]).is_some());
    }
    gate.shaper_params = [0.04, 0.0, 0.0, 0.0];
    gate.shapers[0] = 2.01;
    assert!(gate.invalid_reason(&[]).is_some());
    gate.shapers[0] = 0.0;
    gate.prisms[7][3] = f32::NAN;
    assert!(
        gate.invalid_reason(&[]).is_some(),
        "last float cannot escape a hard-coded prefix length"
    );
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
            });
        let directory = root.join(format!(
            "manual-optics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn actual_ultimate_png_mask_bakes_the_independent_cut_without_radial_softness_spill() {
    let mut gate = open_gate();
    gate.params[1] = 0.58;
    gate.frost = 0.8;
    gate.shapers = [0.74; 4];
    gate.shaper_params = [0.04, 0.0, 0.0, 0.0];
    let manifest = OpticsManifest {
        lights: vec![],
        time_seconds: 1.0,
        angular_mask_edge: EDGE,
        angular_mask_peak: gate::PEAK,
        unsupported: vec![],
        masks: vec![gate.clone()],
        artwork: vec![],
    };
    let directory = Scratch::new();
    manifest.write_assets(&directory.0).unwrap();
    let decoder = png::Decoder::new(BufReader::new(
        fs::File::open(directory.0.join("optics-mask-0000.png")).unwrap(),
    ));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        (info.width, info.height, info.bit_depth, info.color_type),
        (
            EDGE,
            EDGE,
            png::BitDepth::Sixteen,
            png::ColorType::Grayscale
        )
    );
    let mut lit_inside = false;
    for y in 0..EDGE {
        for x in 0..EDGE {
            let point = Vec2::new(
                (x as f32 + 0.5) / EDGE as f32 * 2.0 - 1.0,
                (y as f32 + 0.5) / EDGE as f32 * 2.0 - 1.0,
            );
            let offset = ((y * EDGE + x) * 2) as usize;
            let actual = u16::from_be_bytes([pixels[offset], pixels[offset + 1]]);
            let expected = (gate.transmission(point, &[]) / gate::PEAK * 65535.0).round() as u16;
            assert_eq!(actual, expected);
            if point.abs().max_element() >= 0.301 {
                assert_eq!(actual, 0);
            }
            if point.abs().max_element() < 0.2 && actual > 0 {
                lit_inside = true;
            }
        }
    }
    assert!(lit_inside);
}

#[test]
fn each_fully_open_manual_blade_does_not_clip_its_rim_when_the_opposite_blade_is_active() {
    let mut gate = open_gate();
    gate.params[1] = 0.58;
    gate.shaper_params[0] = 0.04;
    for (opposite, point) in [
        (1, Vec2::new(0.98, 0.0)),
        (0, Vec2::new(-0.98, 0.0)),
        (3, Vec2::new(0.0, 0.98)),
        (2, Vec2::new(0.0, -0.98)),
    ] {
        gate.shapers = [0.0; 4];
        let uncut = gate.transmission(point, &[]);
        assert!(uncut > 0.0);
        gate.shapers[opposite] = 0.2;
        assert_eq!(
            gate.transmission(point, &[]),
            uncut,
            "open blade at {point:?}"
        );
    }
}

#[test]
fn each_full_stroke_manual_blade_is_opaque_including_the_opposite_rim_penumbra() {
    let mut gate = open_gate();
    gate.params[1] = 0.58;
    gate.shaper_params[0] = 0.04;
    for blade in 0..4 {
        gate.shapers = [0.0; 4];
        gate.shapers[blade] = 2.0;
        assert!(gate.invalid_reason(&[]).is_none());
        for point in [
            Vec2::ZERO,
            Vec2::new(-0.98, 0.0),
            Vec2::new(0.98, 0.0),
            Vec2::new(0.0, -0.98),
            Vec2::new(0.0, 0.98),
        ] {
            assert_eq!(
                gate.transmission(point, &[]),
                0.0,
                "blade {blade}, {point:?}"
            );
        }
    }
}

#[test]
fn dynamic_zero_blade_keeps_its_established_edge_interpolation() {
    let mut gate = open_gate();
    gate.params[1] = 0.58;
    let point = Vec2::new(0.98, 0.0);
    let uncut = gate.transmission(point, &[]);
    gate.shapers[1] = 0.2;
    // The active opposite blade makes legacy evaluation include its zero-position neighbour.
    let t = ((point.x - (1.0 + 0.58)) / (-2.0 * 0.58)).clamp(0.0, 1.0);
    let expected = uncut * (t * t * (3.0 - 2.0 * t));
    assert!((gate.transmission(point, &[]) - expected).abs() < uncut * 1e-5);
    assert!(expected < uncut);
}
