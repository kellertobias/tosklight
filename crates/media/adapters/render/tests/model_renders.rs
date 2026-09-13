//! Reference renders for layers mapped onto 3D models.
//!
//! A flat quad in the XY plane, facing the camera, is the one model whose mapped image is known
//! exactly: at pan and tilt 0 it must look like the flat layer scaled to the quad's projected
//! size. Everything here compares a mapped render against the equivalent flat render rather than
//! against stored images, so the assertions hold on any adapter.

use std::sync::Arc;

use media_domain::geometry::Size;
use media_domain::{
    BlendMode, LayerState, MasterState, MediaAddress, ModelGeometry, ModelMapping, ModelVertex,
    OutputId, PresentationMode, ScalingMode, SourceStatus, Timestamp,
};
use media_render::{Gpu, LayerDraw, ModelGeometries, OutputRenderer, SourceTexture};

const OUTPUT: Size = Size::new(64, 64);

fn gpu() -> Gpu {
    Gpu::off_screen().expect("model renders need a GPU or software adapter")
}

fn renderer(gpu: &Gpu) -> OutputRenderer {
    OutputRenderer::off_screen(gpu, OutputId::new(), OUTPUT, PresentationMode::Unlocked)
        .expect("a 64x64 off-screen output")
}

/// Four coloured quadrants — red top left, green top right, blue bottom left, white bottom right —
/// so a flipped or rotated UV mapping cannot pass.
fn quadrants(gpu: &Gpu) -> SourceTexture {
    let mut pixels = Vec::new();
    for y in 0..OUTPUT.height {
        for x in 0..OUTPUT.width {
            let colour = match (x < OUTPUT.width / 2, y < OUTPUT.height / 2) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => [255, 255, 255, 255],
            };
            pixels.extend_from_slice(&colour);
        }
    }
    SourceTexture::from_rgba8(gpu, OUTPUT, &pixels).unwrap()
}

/// A square quad facing +Z, UV (0,0) at its top left, normalized like every imported model: its
/// corners sit on the unit sphere, so its half-extent is 1/√2.
fn quad() -> ModelGeometry {
    let vertex = |x: f32, y: f32, u: f32, v: f32| ModelVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [u, v],
    };
    ModelGeometry {
        vertices: vec![
            vertex(-1.0, -1.0, 0.0, 1.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(1.0, 1.0, 1.0, 0.0),
            vertex(-1.0, 1.0, 0.0, 0.0),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
    .normalized()
    .unwrap()
}

fn models() -> ModelGeometries {
    ModelGeometries::from([(1, Arc::new(quad()))])
}

fn layer(state: LayerState) -> LayerState {
    LayerState {
        address: MediaAddress::new(1, 1),
        source_status: SourceStatus::Ready,
        scaling_mode: ScalingMode::Stretch,
        ..state
    }
}

fn mapped(state: LayerState, pan: f32, tilt: f32) -> LayerState {
    layer(LayerState {
        model: ModelMapping {
            model: 1,
            pan,
            tilt,
        },
        ..state
    })
}

/// The flat layer the quad model projects to at pan and tilt 0.
fn flat_equivalent(state: LayerState) -> LayerState {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    layer(LayerState {
        scale_x: state.scale_x * half,
        scale_y: state.scale_y * half,
        ..state
    })
}

fn render(renderer: &mut OutputRenderer, layers: &[LayerDraw<'_>]) -> Vec<u8> {
    renderer.present(layers, &MasterState::default(), None, Timestamp::ZERO, None);
    renderer.read_image()
}

fn draw<'a>(state: &'a LayerState, source: &'a SourceTexture) -> LayerDraw<'a> {
    LayerDraw {
        state,
        source,
        mask: None,
    }
}

/// The share of pixels whose channels all lie within `tolerance` of the other image.
fn agreement(a: &[u8], b: &[u8], tolerance: u8) -> f32 {
    let close = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .all(|(a, b)| a.abs_diff(*b) <= tolerance)
        })
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

#[test]
fn model_zero_and_a_missing_model_render_byte_for_byte_like_today() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut plain = renderer(&gpu);
    let mut with_models = renderer(&gpu);
    assert!(with_models.set_models(&models()).is_empty());

    let state = layer(LayerState {
        scale_x: 0.8,
        rotation: 12.0,
        position_x: 0.1,
        dimmer: 0.7,
        ..Default::default()
    });
    let today = render(&mut plain, &[draw(&state, &source)]);
    assert_eq!(render(&mut with_models, &[draw(&state, &source)]), today);

    let missing = LayerState {
        model: ModelMapping {
            model: 9,
            pan: 45.0,
            tilt: 10.0,
        },
        ..state.clone()
    };
    assert_eq!(
        render(&mut with_models, &[draw(&missing, &source)]),
        today,
        "a model that is not installed draws the layer flat, never black"
    );
}

#[test]
fn a_camera_facing_quad_matches_the_flat_layer() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    output.set_models(&models());

    let flat = render(
        &mut output,
        &[draw(
            &flat_equivalent(layer(LayerState::default())),
            &source,
        )],
    );
    let model = render(
        &mut output,
        &[draw(&mapped(LayerState::default(), 0.0, 0.0), &source)],
    );
    let share = agreement(&flat, &model, 12);
    assert!(share > 0.97, "only {share} of the pixels agree");
    // The top-left quadrant of the mapped quad is red: the UVs were not flipped.
    let at =
        |image: &[u8], x: usize, y: usize| image[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].to_vec();
    assert_eq!(at(&model, 20, 20), vec![255, 0, 0, 255]);
    assert_eq!(at(&model, 43, 43), vec![255, 255, 255, 255]);
}

#[test]
fn a_quarter_pan_turns_a_flat_quad_edge_on() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    output.set_models(&models());

    let facing = render(
        &mut output,
        &[draw(&mapped(LayerState::default(), 0.0, 0.0), &source)],
    );
    let edge_on = render(
        &mut output,
        &[draw(&mapped(LayerState::default(), 90.0, 0.0), &source)],
    );
    assert!(lit_share(&facing) > 0.45, "{}", lit_share(&facing));
    assert!(
        lit_share(&edge_on) < 0.03,
        "an edge-on quad is mostly empty: {}",
        lit_share(&edge_on)
    );
    let tilted = render(
        &mut output,
        &[draw(&mapped(LayerState::default(), 0.0, 60.0), &source)],
    );
    let share = lit_share(&tilted);
    assert!(
        share > 0.15 && share < 0.4,
        "tilt foreshortens the quad vertically: {share}"
    );
}

#[test]
fn roll_is_the_flat_rotation() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    output.set_models(&models());

    let rotated = LayerState {
        rotation: 30.0,
        ..Default::default()
    };
    let flat = render(
        &mut output,
        &[draw(&flat_equivalent(layer(rotated.clone())), &source)],
    );
    let model = render(&mut output, &[draw(&mapped(rotated, 0.0, 0.0), &source)]);
    let share = agreement(&flat, &model, 12);
    assert!(share > 0.95, "only {share} of the pixels agree");
}

#[test]
fn position_dimmer_and_blend_apply_to_the_mapped_result_like_a_flat_layer() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let background = SourceTexture::solid(&gpu, Size::new(1, 1), [40, 80, 120, 255]).unwrap();
    let mut output = renderer(&gpu);
    output.set_models(&models());

    let base = layer(LayerState::default());
    let look = LayerState {
        position_x: -0.3,
        position_y: 0.25,
        dimmer: 0.5,
        blend: BlendMode::Add,
        ..Default::default()
    };
    let flat_top = flat_equivalent(layer(look.clone()));
    let flat = render(
        &mut output,
        &[draw(&base, &background), draw(&flat_top, &source)],
    );
    let mapped_top = mapped(look, 0.0, 0.0);
    let model = render(
        &mut output,
        &[draw(&base, &background), draw(&mapped_top, &source)],
    );
    let share = agreement(&flat, &model, 12);
    assert!(share > 0.95, "only {share} of the pixels agree");
}

#[test]
fn clearing_the_models_returns_layers_to_flat() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    output.set_models(&models());
    let state = mapped(LayerState::default(), 90.0, 0.0);
    assert!(lit_share(&render(&mut output, &[draw(&state, &source)])) < 0.03);
    output.set_models(&ModelGeometries::new());
    assert!(
        lit_share(&render(&mut output, &[draw(&state, &source)])) > 0.9,
        "the unmapped layer fills the output again"
    );
}
