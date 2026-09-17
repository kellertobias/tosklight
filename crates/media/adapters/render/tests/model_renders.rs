//! Reference renders for layers mapped onto 3D models.
//!
//! A flat quad in the XY plane, facing the camera, is the one model whose mapped image is known
//! exactly: at pan and tilt 0 it must look like the flat layer scaled to the quad's projected
//! size. Everything here compares a mapped render against the equivalent flat render rather than
//! against stored images, so the assertions hold on any adapter.

use std::sync::Arc;

use media_domain::geometry::Size;
use media_domain::{
    BlendMode, BuiltinModel, LayerState, MasterState, MediaAddress, ModelGeometry, ModelMapping,
    ModelVertex, OutputId, PresentationMode, ScalingMode, SourceStatus, Timestamp,
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
fn model_zero_renders_byte_for_byte_like_today() {
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
}

#[test]
fn a_missing_model_is_mapped_onto_the_default_plane() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    output.set_models(&models());
    let mut reference = renderer(&gpu);
    reference.set_models(&ModelGeometries::from([(
        1,
        BuiltinModel::Plane.geometry(),
    )]));
    let missing = |pan: f32, tilt: f32| {
        layer(LayerState {
            model: ModelMapping {
                model: 9,
                pan,
                tilt,
            },
            ..Default::default()
        })
    };
    let on_plane = render(
        &mut reference,
        &[draw(&mapped(LayerState::default(), 30.0, 20.0), &source)],
    );
    let fallback = render(&mut output, &[draw(&missing(30.0, 20.0), &source)]);
    let share = agreement(&on_plane, &fallback, 2);
    assert!(
        share > 0.99,
        "a missing model draws exactly like the Plane, never black: {share}"
    );
    let on_quad = render(
        &mut output,
        &[draw(&mapped(LayerState::default(), 30.0, 20.0), &source)],
    );
    assert!(
        agreement(&on_quad, &fallback, 12) < 0.97,
        "it is not the installed model"
    );
}

#[test]
fn every_built_in_model_renders_its_own_shape() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    let library: ModelGeometries = BuiltinModel::ALL
        .into_iter()
        .map(|model| (model.default_slot(), model.geometry()))
        .collect();
    assert!(output.set_models(&library).is_empty());

    let at_slot = |slot: u8| {
        layer(LayerState {
            model: ModelMapping {
                model: slot,
                pan: 30.0,
                tilt: 20.0,
            },
            ..Default::default()
        })
    };
    let images: Vec<Vec<u8>> = BuiltinModel::ALL
        .iter()
        .map(|model| {
            render(
                &mut output,
                &[draw(&at_slot(model.default_slot()), &source)],
            )
        })
        .collect();
    for (model, image) in BuiltinModel::ALL.iter().zip(&images) {
        let share = lit_share(image);
        assert!(share > 0.1 && share < 0.9, "{model:?} lights {share}");
    }
    for (a, first) in images.iter().enumerate() {
        for (b, second) in images.iter().enumerate().skip(a + 1) {
            assert!(
                agreement(first, second, 12) < 0.97,
                "{:?} and {:?} render alike",
                BuiltinModel::ALL[a],
                BuiltinModel::ALL[b]
            );
        }
    }

    // The built-in Plane is output-shaped, not the normalized reference quad.
    let plane = &images[0];
    let mut quad_output = renderer(&gpu);
    quad_output.set_models(&models());
    let quad = render(
        &mut quad_output,
        &[draw(&mapped(LayerState::default(), 30.0, 20.0), &source)],
    );
    assert!(lit_share(plane) > lit_share(&quad) * 1.5);
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
fn clearing_the_models_maps_layers_onto_the_plane() {
    let gpu = gpu();
    let source = quadrants(&gpu);
    let mut output = renderer(&gpu);
    let sphere = ModelGeometries::from([(1, BuiltinModel::Sphere.geometry())]);
    output.set_models(&sphere);
    let state = mapped(LayerState::default(), 0.0, 0.0);
    let on_sphere = render(&mut output, &[draw(&state, &source)]);
    output.set_models(&ModelGeometries::new());
    let on_plane = render(&mut output, &[draw(&state, &source)]);
    let mut reference = renderer(&gpu);
    reference.set_models(&ModelGeometries::from([(
        7,
        BuiltinModel::Plane.geometry(),
    )]));
    let plane = render(
        &mut reference,
        &[draw(
            &LayerState {
                model: ModelMapping {
                    model: 7,
                    ..state.model
                },
                ..state.clone()
            },
            &source,
        )],
    );
    assert!(agreement(&on_plane, &plane, 2) > 0.99);
    assert!(agreement(&on_sphere, &plane, 12) < 0.97);
}

/// Quadrants like [`quadrants`], at any size.
fn quadrants_of(gpu: &Gpu, size: Size) -> SourceTexture {
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

fn flat_turned(state: LayerState, pan: f32, tilt: f32) -> LayerState {
    LayerState {
        model: ModelMapping {
            model: 0,
            pan,
            tilt,
        },
        ..state
    }
}

#[test]
fn a_flat_layer_turned_by_pan_or_tilt_keeps_its_rectangle_and_turns() {
    let gpu = gpu();
    // A wide output and a tall source, so aspect and the Fit scaling mode both matter.
    let output_size = Size::new(96, 54);
    let source = quadrants_of(&gpu, Size::new(40, 60));
    let mut output = OutputRenderer::off_screen(
        &gpu,
        OutputId::new(),
        output_size,
        PresentationMode::Unlocked,
    )
    .unwrap();
    // Fill overflows the output at the other modes' scale, which would hide the foreshortening.
    for (mode, scale) in [
        (ScalingMode::Fit, 1.0),
        (ScalingMode::Fill, 0.5),
        (ScalingMode::Stretch, 1.0),
    ] {
        let flat = LayerState {
            address: MediaAddress::new(1, 1),
            source_status: SourceStatus::Ready,
            scaling_mode: mode,
            scale_x: 0.6 * scale,
            scale_y: 0.7 * scale,
            position_x: 0.1,
            rotation: 10.0,
            ..Default::default()
        };
        let reference = render(&mut output, &[draw(&flat, &source)]);
        // A barely turned Flat layer takes the 3D path and still lands on the flat rectangle.
        let nudged = render(
            &mut output,
            &[draw(&flat_turned(flat.clone(), 0.01, -0.01), &source)],
        );
        let share = agreement(&reference, &nudged, 12);
        assert!(share > 0.97, "{mode:?}: only {share} of the pixels agree");

        let panned = render(
            &mut output,
            &[draw(&flat_turned(flat.clone(), 60.0, 0.0), &source)],
        );
        let tilted = render(
            &mut output,
            &[draw(&flat_turned(flat.clone(), 0.0, 60.0), &source)],
        );
        for (name, turned) in [("pan", &panned), ("tilt", &tilted)] {
            assert!(
                agreement(&reference, turned, 12) < 0.95,
                "{mode:?}: {name} 60 changes the image"
            );
            assert!(
                lit_share(turned) < lit_share(&reference) * 0.8,
                "{mode:?}: {name} 60 foreshortens the layer"
            );
            assert!(lit_share(turned) > 0.02, "{mode:?}: {name} 60 still draws");
        }
    }
}
