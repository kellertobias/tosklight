//! How a layer update body becomes the live controls one reducer command carries.
//!
//! The route in [`super::outputs`] owns lookup, ownership, and the answer; this module owns what
//! the body's fields mean for a layer: which ranges they must lie in, and how a partial change
//! merges with the layer's current values.

use media_domain::{
    EffectBankState, EffectSlot, LayerControls, LayerState, MediaAddress, ScalingMode, Tint,
};

use crate::error::ApiError;
use crate::routes::outputs::{validate_range, validate_unit};
use crate::wire::UpdateLayer;

/// Rejects any stated control outside the range the layer accepts.
pub(super) fn validate_layer_ranges(body: &UpdateLayer) -> Result<(), ApiError> {
    for (name, value) in [
        ("dimmer", body.dimmer),
        ("volume", body.volume),
        ("tintRed", body.tint_red),
        ("tintGreen", body.tint_green),
        ("tintBlue", body.tint_blue),
        ("grayscale", body.grayscale),
        ("maskOpacity", body.mask_opacity),
        ("effectMix", body.effect_mix),
        ("tvCurvature", body.tv_curvature),
        ("effectDistortion", body.effect_distortion),
        ("imageGrain", body.image_grain),
        ("compressionDamage", body.compression_damage),
        ("blockSize", body.block_size),
        ("tileDisplacement", body.tile_displacement),
        ("chromaDamage", body.chroma_damage),
        ("effectGlitching", body.effect_glitching),
        ("blurAmount", body.blur_amount),
        ("feedbackAmount", body.feedback_amount),
        ("feedbackMotion", body.feedback_motion),
        ("blur", body.blur),
        ("beatMoveAmount", body.beat_move_amount),
        ("beatScanFalloff", body.beat_scan_falloff),
        ("beatScaleAmount", body.beat_scale_amount),
    ] {
        validate_unit(name, value)?;
    }
    for (name, value, minimum, maximum) in [
        ("scaleX", body.scale_x, 0.0, 10.0),
        ("scaleY", body.scale_y, 0.0, 10.0),
        ("positionX", body.position_x, -2.0, 2.0),
        ("positionY", body.position_y, -2.0, 2.0),
        ("rotation", body.rotation, -360.0, 360.0),
        ("maskScaleX", body.mask_scale_x, 0.0, 2.0),
        ("maskScaleY", body.mask_scale_y, 0.0, 2.0),
        ("maskPositionX", body.mask_position_x, -2.0, 2.0),
        ("maskPositionY", body.mask_position_y, -2.0, 2.0),
        ("beatMoveDecay", body.beat_move_decay, 0.05, 5.0),
        ("kaleidoscopeAngle", body.kaleidoscope_angle, -180.0, 180.0),
        ("rasterizeDotSize", body.rasterize_dot_size, 2.0, 32.0),
        ("beatScanWidth", body.beat_scan_width, 0.01, 0.25),
        ("beatScanDuration", body.beat_scan_duration, 0.2, 3.0),
        ("beatTurnRotation", body.beat_turn_rotation, -30.0, 30.0),
        ("beatScaleDecay", body.beat_scale_decay, 0.05, 5.0),
        ("beatGridDensity", body.beat_grid_density, 6.0, 64.0),
        ("beatGridHeight", body.beat_grid_height, 0.0, 1.0),
        ("beatGridDuration", body.beat_grid_duration, 0.2, 4.0),
        ("beatGridHue", body.beat_grid_hue, 0.0, 360.0),
        ("beatGridBrightness", body.beat_grid_brightness, 0.1, 2.0),
        ("modelPan", body.model_pan, -360.0, 360.0),
        ("modelTilt", body.model_tilt, -360.0, 360.0),
    ] {
        validate_range(name, value, minimum, maximum)?;
    }
    Ok(())
}

/// Merges the body's stated changes with the layer's current values into one controls change.
pub(super) fn layer_controls(
    body: &UpdateLayer,
    current: &LayerState,
    scaling_mode: Option<ScalingMode>,
    effects: Option<[EffectSlot; 4]>,
) -> Result<LayerControls, ApiError> {
    let tint = (body.tint_red.is_some() || body.tint_green.is_some() || body.tint_blue.is_some())
        .then(|| {
            Tint::new(
                body.tint_red.unwrap_or(current.tint.red),
                body.tint_green.unwrap_or(current.tint.green),
                body.tint_blue.unwrap_or(current.tint.blue),
            )
        });
    let mask_address = (body.mask_folder.is_some() || body.mask_file.is_some()).then(|| {
        MediaAddress::new(
            body.mask_folder.unwrap_or(current.mask.address.folder),
            body.mask_file.unwrap_or(current.mask.address.file),
        )
    });
    let effect_banks = effect_banks(body, current)?;
    let blend = body.blend_dmx.map(media_domain::LayerBlend::from_dmx);
    let visualizer_controls = visualizer_controls(body, current)?;
    let model = (body.model.is_some() || body.model_pan.is_some() || body.model_tilt.is_some())
        .then(|| media_domain::ModelMapping {
            model: body.model.unwrap_or(current.model.model),
            pan: body.model_pan.unwrap_or(current.model.pan),
            tilt: body.model_tilt.unwrap_or(current.model.tilt),
        });
    Ok(LayerControls {
        address: body
            .changes_address()
            .then(|| body.address(current.address)),
        play_mode: body.play_mode_dmx.map(media_domain::PlayMode::from_dmx),
        scale_x: body.scale_x,
        scale_y: body.scale_y,
        scaling_mode,
        position_x: body.position_x,
        position_y: body.position_y,
        rotation: body.rotation,
        dimmer: body.dimmer,
        volume: body.volume,
        tint,
        grayscale: body.grayscale,
        mask_address,
        mask_scale_x: body.mask_scale_x,
        mask_scale_y: body.mask_scale_y,
        mask_position_x: body.mask_position_x,
        mask_position_y: body.mask_position_y,
        mask_invert: body.mask_invert,
        mask_opacity: body.mask_opacity,
        speed_multiplier: body
            .speed_multiplier_dmx
            .map(media_domain::SpeedMultiplier::from_dmx),
        playback_bpm: body.playback_bpm.map(|value| (value != 0).then_some(value)),
        blur: body.blur,
        effects,
        effect_banks,
        blend: blend.map(|blend| blend.mode),
        strobe_hz: blend.map(|blend| blend.strobe_hz),
        in_point: body.in_point,
        out_point: body.out_point,
        visualizer_controls,
        model,
    })
}

/// The DMX effect-bank edit: one bank's select, strength, or one parameter.
fn effect_banks(
    body: &UpdateLayer,
    current: &LayerState,
) -> Result<Option<[EffectBankState; 2]>, ApiError> {
    if body.effect_select.is_none()
        && body.effect_strength.is_none()
        && body.effect_parameter_value.is_none()
    {
        return Ok(None);
    }
    let bank = usize::from(body.effect_bank.ok_or_else(|| {
        ApiError::bad_request(
            "missing-effect-bank",
            "effectBank 0 or 1 is required when changing Effect Select, Effect Strength, \
             or an effect parameter",
        )
    })?);
    if bank >= current.effect_banks.len() {
        return Err(ApiError::bad_request(
            "effect-bank-out-of-range",
            "effectBank must be 0 or 1",
        ));
    }
    validate_unit("effectStrength", body.effect_strength)?;
    let mut banks = current.effect_banks;
    if let Some(select) = body.effect_select {
        banks[bank].select = select;
    }
    if let Some(strength) = body.effect_strength {
        banks[bank].strength = strength;
    }
    if let Some(value) = body.effect_parameter_value {
        let index = body
            .effect_parameter_index
            .map(usize::from)
            .filter(|index| *index < media_domain::personality::EFFECT_BANK_PARAMETERS)
            .ok_or_else(|| {
                ApiError::bad_request(
                    "effect-parameter-index-out-of-range",
                    "effectParameterIndex 0 through 3 is required with effectParameterValue",
                )
            })?;
        banks[bank].parameters[index] = value;
    }
    Ok(Some(banks))
}

/// One visualizer parameter change, merged with the layer's other visualizer controls.
fn visualizer_controls(
    body: &UpdateLayer,
    current: &LayerState,
) -> Result<Option<[u8; media_domain::personality::VISUALIZER_PARAMETERS]>, ApiError> {
    let Some(value) = body.visualizer_parameter_value else {
        return Ok(None);
    };
    let index = body
        .visualizer_parameter_index
        .map(usize::from)
        .filter(|index| *index < media_domain::personality::VISUALIZER_PARAMETERS)
        .ok_or_else(|| {
            ApiError::bad_request(
                "visualizer-parameter-index-out-of-range",
                "visualizerParameterIndex 0 through 3 is required with \
                 visualizerParameterValue",
            )
        })?;
    let mut controls = current.visualizer_controls;
    controls[index] = value;
    Ok(Some(controls))
}
