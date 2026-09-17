//! Reference renders for the built-in Plane: it always has the aspect ratio of its output.
//!
//! Unturned at scale 1, a layer on the Plane — selected in a slot or as the missing-model
//! fallback — must look exactly like the same layer drawn Flat, on any output shape, and it must
//! follow a resolution change without a restart.

use media_domain::geometry::Size;
use media_domain::{
    BuiltinModel, LayerState, MasterState, MediaAddress, ModelMapping, OutputId, PresentationMode,
    ScalingMode, SourceStatus, Timestamp,
};
use media_render::{Gpu, LayerDraw, ModelGeometries, OutputRenderer, SourceTexture};

/// A 16:9 and a 4:3 output, small enough for a software adapter.
const WIDE: Size = Size::new(96, 54);
const CLASSIC: Size = Size::new(64, 48);
/// A slot with no model: the layer falls back to the Plane.
const MISSING: u8 = 9;
const PLANE_SLOT: u8 = 1;

fn gpu() -> Gpu {
    Gpu::off_screen().expect("Plane renders need a GPU or software adapter")
}

fn renderer(gpu: &Gpu, size: Size) -> OutputRenderer {
    let mut output =
        OutputRenderer::off_screen(gpu, OutputId::new(), size, PresentationMode::Unlocked)
            .expect("an off-screen output");
    let library = ModelGeometries::from([(PLANE_SLOT, BuiltinModel::Plane.geometry())]);
    assert!(output.set_models(&library).is_empty());
    output
}

/// Four coloured quadrants, so a squeezed, flipped or shifted mapping cannot pass.
fn quadrants(gpu: &Gpu, size: Size) -> SourceTexture {
    let mut pixels = Vec::new();
    for y in 0..size.height {
        for x in 0..size.width {
            let colour = match (x < size.width / 2, y < size.height / 2) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => [255, 255, 255, 255],
            };
            pixels.extend_from_slice(&colour);
        }
    }
    SourceTexture::from_rgba8(gpu, size, &pixels).unwrap()
}

fn flat(scaling_mode: ScalingMode) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode,
        ..Default::default()
    }
}

fn on_model(state: &LayerState, model: u8, pan: f32, tilt: f32) -> LayerState {
    LayerState {
        model: ModelMapping { model, pan, tilt },
        ..state.clone()
    }
}

fn render(output: &mut OutputRenderer, state: &LayerState, source: &SourceTexture) -> Vec<u8> {
    output.present(
        &[LayerDraw {
            state,
            source,
            mask: None,
        }],
        &MasterState::default(),
        None,
        Timestamp::ZERO,
        None,
    );
    output.read_image()
}

fn agreement(a: &[u8], b: &[u8], tolerance: u8) -> f32 {
    assert_eq!(a.len(), b.len());
    let close = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(*b).all(|(a, b)| a.abs_diff(*b) <= tolerance))
        .count();
    close as f32 / (a.len() / 4) as f32
}

fn lit_share(image: &[u8]) -> f32 {
    let lit = image
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[..3].iter().any(|channel| *channel > 16))
        .count();
    lit as f32 / (image.len() / 4) as f32
}

/// Asserts the unturned Plane — in its slot and as the fallback — draws like `state` Flat.
fn assert_plane_is_flat(
    output: &mut OutputRenderer,
    state: &LayerState,
    source: &SourceTexture,
    what: &str,
) {
    let reference = render(output, state, source);
    for slot in [PLANE_SLOT, MISSING] {
        let plane = render(output, &on_model(state, slot, 0.0, 0.0), source);
        let share = agreement(&reference, &plane, 12);
        assert!(
            share > 0.97,
            "{what}, slot {slot}: only {share} of the pixels match Flat"
        );
    }
}

#[test]
fn a_full_frame_clip_on_the_plane_looks_like_flat_on_wide_and_classic_outputs() {
    let gpu = gpu();
    for size in [WIDE, CLASSIC] {
        let mut output = renderer(&gpu, size);
        let clip = quadrants(&gpu, size);
        let what = format!("{}x{}", size.width, size.height);
        assert_plane_is_flat(&mut output, &flat(ScalingMode::Fit), &clip, &what);
        // Full frame, so the whole output is lit, not a square in its middle.
        let plane = render(
            &mut output,
            &on_model(&flat(ScalingMode::Fit), MISSING, 0.0, 0.0),
            &clip,
        );
        assert!(lit_share(&plane) > 0.97, "{what}: {}", lit_share(&plane));
    }
}

#[test]
fn every_scaling_mode_on_the_plane_covers_the_flat_area() {
    let gpu = gpu();
    // A clip whose shape matches neither output.
    let clip = quadrants(&gpu, Size::new(40, 60));
    for size in [WIDE, CLASSIC] {
        let mut output = renderer(&gpu, size);
        for mode in [
            ScalingMode::Fit,
            ScalingMode::Fill,
            ScalingMode::Stretch,
            ScalingMode::Original,
        ] {
            let what = format!("{}x{} {mode:?}", size.width, size.height);
            assert_plane_is_flat(&mut output, &flat(mode), &clip, &what);
        }
    }
}

#[test]
fn scale_and_position_stay_relative_to_the_output_shaped_plane() {
    let gpu = gpu();
    for size in [WIDE, CLASSIC] {
        let mut output = renderer(&gpu, size);
        let clip = quadrants(&gpu, size);
        let state = LayerState {
            scale_x: 0.5,
            scale_y: 0.8,
            position_x: 0.2,
            position_y: -0.1,
            ..flat(ScalingMode::Fit)
        };
        let what = format!("{}x{} scaled", size.width, size.height);
        assert_plane_is_flat(&mut output, &state, &clip, &what);
        let plane = render(&mut output, &on_model(&state, PLANE_SLOT, 0.0, 0.0), &clip);
        let expected = 0.5 * 0.8;
        let share = lit_share(&plane);
        assert!(
            (share - expected).abs() < 0.05,
            "{what}: Scale X/Y cover {share}, expected {expected}"
        );
    }
}

#[test]
fn a_turned_plane_and_the_fallback_turn_alike() {
    let gpu = gpu();
    let mut output = renderer(&gpu, WIDE);
    let clip = quadrants(&gpu, WIDE);
    let state = LayerState {
        scale_x: 0.6,
        scale_y: 0.6,
        ..flat(ScalingMode::Fit)
    };
    let facing = render(&mut output, &on_model(&state, PLANE_SLOT, 0.0, 0.0), &clip);
    let panned = render(&mut output, &on_model(&state, PLANE_SLOT, 50.0, 0.0), &clip);
    let fallback = render(&mut output, &on_model(&state, MISSING, 50.0, 0.0), &clip);
    assert!(agreement(&panned, &fallback, 2) > 0.99);
    assert!(lit_share(&panned) < lit_share(&facing) * 0.9);
    assert!(lit_share(&panned) > 0.05);
}

#[test]
fn the_plane_follows_a_live_resolution_change() {
    let gpu = gpu();
    let mut output = renderer(&gpu, WIDE);
    let state = on_model(&flat(ScalingMode::Fit), MISSING, 0.0, 0.0);
    let wide_clip = quadrants(&gpu, WIDE);
    assert!(lit_share(&render(&mut output, &state, &wide_clip)) > 0.97);

    output.recreate(CLASSIC);
    let classic_clip = quadrants(&gpu, CLASSIC);
    let plane = render(&mut output, &state, &classic_clip);
    assert_eq!(plane.len(), (CLASSIC.width * CLASSIC.height * 4) as usize);
    assert!(
        lit_share(&plane) > 0.97,
        "the Plane is 4:3 after the change: {}",
        lit_share(&plane)
    );
    assert_plane_is_flat(
        &mut output,
        &flat(ScalingMode::Fit),
        &classic_clip,
        "resized",
    );
}
