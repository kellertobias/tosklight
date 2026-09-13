use media_domain::geometry::{Size, layer_transform};
use media_domain::{
    BlurParameters, BlurType, EffectSlot, LayerState, MasterState, OutputId, ScalingMode,
    Timestamp, Tint,
};

use super::{LayerUniform, MasterUniform};

#[test]
fn the_layer_uniform_matches_the_geometry_the_domain_computed() {
    let layer = LayerState {
        scale_x: 2.0,
        rotation: 90.0,
        scaling_mode: ScalingMode::Original,
        dimmer: 0.5,
        tint: Tint::new(1.0, 0.0, 0.0),
        grayscale: 0.25,
        mask: media_domain::MaskState {
            position_x: 0.5,
            position_y: -0.75,
            ..Default::default()
        },
        ..Default::default()
    };
    let source = Size::new(100, 50);
    let output = Size::new(1920, 1080);
    let uniform = LayerUniform::new(
        &layer,
        source,
        output,
        None,
        OutputId::default(),
        Timestamp::ZERO,
    );
    let transform = layer_transform(&layer, source, output);

    assert_eq!(uniform.center, [transform.center.x, transform.center.y]);
    assert_eq!(uniform.size, [transform.size.0, transform.size.1]);
    assert_eq!(uniform.output, [1920.0, 1080.0]);
    assert!(
        (uniform.rotation[0] - 0.0).abs() < 1e-6,
        "cosine of a quarter turn"
    );
    assert!(
        (uniform.rotation[1] - 1.0).abs() < 1e-6,
        "sine of a quarter turn"
    );
    assert_eq!(
        uniform.tint,
        [1.0, 0.0, 0.0, 0.5],
        "dimmer rides in the tint's alpha"
    );
    assert_eq!(uniform.controls[0], 0.25);
    assert_eq!(&uniform.mask_source[1..3], &[0.5, -0.75]);
}

#[test]
fn the_uniforms_are_the_size_the_shaders_declare() {
    assert_eq!(std::mem::size_of::<LayerUniform>(), 1280);
    // Two more vec4s than before display regions: the slice and its quarter-turn.
    assert_eq!(std::mem::size_of::<MasterUniform>(), 144);
}

#[test]
fn blur_type_and_amount_reach_the_shader_in_their_effect_slot() {
    let mut blur = EffectSlot::blur();
    blur.mix = 0.8;
    blur.parameters = BlurParameters {
        amount: 0.65,
        blur_type: BlurType::Axial,
    }
    .as_array()
    .to_vec();
    let mut layer = LayerState::default();
    layer.effects[1] = blur;
    let uniform = LayerUniform::new(
        &layer,
        Size::new(100, 50),
        Size::new(1920, 1080),
        None,
        OutputId::default(),
        Timestamp::ZERO,
    );
    assert_eq!(uniform.effect_types[1], 3);
    assert_eq!(uniform.effect_mixes[1], 0.8);
    assert_eq!(&uniform.effect_parameters[1][..2], &[0.65, 4.0]);
}

#[test]
fn layer_shader_with_all_blur_modes_is_valid_wgsl() {
    let module = naga::front::wgsl::parse_str(include_str!("../shaders/layer.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn the_master_uniform_carries_the_flip_as_a_per_axis_sign() {
    let master = MasterState {
        flip_mirror: media_domain::FlipMirror::Horizontal,
        dimmer: 0.75,
        ..Default::default()
    };
    let uniform = MasterUniform::new(&master, None, false, None);
    assert_eq!(&uniform.flip_mask[..2], &[-1.0, 1.0]);
    assert_eq!(uniform.tint[3], 0.75);
}

#[test]
fn the_master_uniform_carries_geometry_and_all_shaper_edges() {
    let master = MasterState {
        position_x: 0.25,
        position_y: -0.5,
        scale_x: 1.5,
        scale_y: 0.75,
        scaling_mode: media_domain::ScalingMode::Stretch,
        rotation: 90.0,
        mask_position_x: -0.5,
        mask_position_y: 0.75,
        shaper: media_domain::MasterShaper {
            left: 0.1,
            right: 0.2,
            top: 0.3,
            bottom: 0.4,
            left_rotation: 10.0,
            right_rotation: -10.0,
            top_rotation: 20.0,
            bottom_rotation: -20.0,
            rotation: 30.0,
        },
        ..Default::default()
    };
    let uniform = MasterUniform::new(&master, None, false, None);
    assert_eq!(uniform.transform, [0.25, -0.5, 1.5, 0.75]);
    assert_eq!(&uniform.mask_transform[..2], &[-0.5, 0.75]);
    assert_eq!(uniform.shaper_edges, [0.1, 0.2, 0.3, 0.4]);
    assert!((uniform.rotation[0]).abs() < 1e-6);
    assert!((uniform.rotation[1] - 1.0).abs() < 1e-6);
    assert!((uniform.rotation[2] - 30_f32.to_radians().cos()).abs() < 1e-6);
    assert!((uniform.shaper_edge_tangents[0] - 10_f32.to_radians().tan()).abs() < 1e-6);
}

#[test]
fn a_layer_preview_requests_alpha_preservation_from_the_master_shader() {
    let uniform = MasterUniform::new(&MasterState::default(), None, true, None);
    assert_eq!(uniform.flip_mask[3], -1.0);
    let program = MasterUniform::new(&MasterState::default(), None, false, None);
    assert_eq!(program.flip_mask[3], 0.0);
}
