//! Typed edits for the legacy native-effect configuration surface.

use media_domain::{
    ANALOG_TV_EFFECT, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT, BEAT_MOVE_EFFECT,
    BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT, BeatFormFlashParameters,
    BeatGridWaveOrigin, BeatGridWaveParameters, BeatMoveDirection, BeatMoveParameters,
    BeatScaleTurnParameters, BeatScanEdge, BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT,
    DRAWN_IMAGE_EFFECT, DrawnImageParameters, EffectSlot, FEEDBACK_EFFECT, FeedbackMotion,
    FeedbackParameters, KALEIDOSCOPE_EFFECT, KaleidoscopeParameters, LayerState,
    OPACITY_CYCLE_EFFECT, OpacityCycleInterval, RASTERIZE_EFFECT, RasterizeMode,
    RasterizeParameters,
};

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::wire::UpdateLayer;

pub(super) fn updated_effects(
    state: &ApiState,
    body: &UpdateLayer,
    current: &LayerState,
    layer: usize,
) -> Result<Option<[EffectSlot; 4]>, ApiError> {
    if !body.changes_effect() {
        return Ok(None);
    }
    let (mut effects, slot) = selected_effects(body, current, layer)?;
    let effect = &mut effects[slot];
    apply_common_edits(state, body, current, slot, effect)?;
    apply_spatial_edits(body, effect)?;
    apply_beat_edits(body, effect)?;
    apply_tv_edits(body, effect)?;
    effect.normalize();
    Ok(Some(effects))
}

fn selected_effects(
    body: &UpdateLayer,
    current: &LayerState,
    layer: usize,
) -> Result<([EffectSlot; 4], usize), ApiError> {
    let slot = usize::from(body.effect_slot.ok_or_else(|| {
        ApiError::bad_request("effect-slot-required", "choose effectSlot 0, 1, 2, or 3")
    })?);
    if slot >= current.effects.len() {
        return Err(ApiError::bad_request(
            "effect-slot-out-of-range",
            "effectSlot must be 0, 1, 2, or 3",
        ));
    }
    let mut effects = current.effects.clone();
    if let Some(effect_type) = body.effect_type.as_deref() {
        effects[slot] = match effect_type {
            ANALOG_TV_EFFECT => EffectSlot::analog_tv(),
            DIGITAL_TV_EFFECT => EffectSlot::digital_tv(),
            BLUR_EFFECT => EffectSlot::blur(),
            FEEDBACK_EFFECT => EffectSlot::feedback(),
            OPACITY_CYCLE_EFFECT => EffectSlot::opacity_cycle(),
            BEAT_MOVE_EFFECT => EffectSlot::beat_move(),
            KALEIDOSCOPE_EFFECT => EffectSlot::kaleidoscope(),
            RASTERIZE_EFFECT => EffectSlot::rasterize(),
            BEAT_SCAN_EFFECT => EffectSlot::beat_scan(),
            BEAT_SCALE_TURN_EFFECT => EffectSlot::beat_scale_turn(),
            BEAT_GRID_WAVE_EFFECT => EffectSlot::beat_grid_wave(),
            BEAT_FORM_FLASH_EFFECT => EffectSlot::beat_form_flash(),
            DRAWN_IMAGE_EFFECT => EffectSlot::drawn_image(),
            "none" => EffectSlot::default(),
            _ => {
                return Err(ApiError::bad_request(
                    "effect-unsupported",
                    format!("this Media Server cannot render effect {effect_type:?}"),
                ));
            }
        };
        effects[slot].seed = ((layer as u32) << 8) | slot as u32;
    }
    Ok((effects, slot))
}

fn apply_common_edits(
    state: &ApiState,
    body: &UpdateLayer,
    current: &LayerState,
    slot: usize,
    effect: &mut EffectSlot,
) -> Result<(), ApiError> {
    if let Some(parameters) = body.visualizer_parameters {
        if slot != 0 {
            return Err(ApiError::bad_request(
                "visualizer-controls-slot",
                "a visualizer is controlled through effectSlot 0",
            ));
        }
        if state
            .configuration
            .load()
            .visualizers
            .resolve(current.address)
            .is_none()
        {
            return Err(ApiError::bad_request(
                "visualizer-controls-source",
                "select a generated visualizer on this layer first",
            ));
        }
        effect.visualizer_parameters = Some(parameters.into_parameters());
    }
    if let Some(enabled) = body.effect_enabled {
        effect.enabled = enabled;
    }
    if let Some(mix) = body.effect_mix {
        effect.mix = mix;
    }
    if let Some(amount) = body.blur_amount {
        require_effect(effect, BLUR_EFFECT, "blur-parameters-effect", "Blur")?;
        effect.parameters = BlurParameters {
            amount,
            ..BlurParameters::default()
        }
        .as_array()
        .to_vec();
    }
    if body.feedback_amount.is_some()
        || body.feedback_motion.is_some()
        || body.feedback_direction.is_some()
    {
        require_effect(
            effect,
            FEEDBACK_EFFECT,
            "feedback-parameters-effect",
            "Feedback",
        )?;
        let mut parameters = FeedbackParameters::from_normalized(&effect.parameters);
        parameters.amount = body.feedback_amount.unwrap_or(parameters.amount);
        parameters.motion = body.feedback_motion.unwrap_or(parameters.motion);
        if let Some(direction) = body.feedback_direction.as_deref() {
            parameters.direction = FeedbackMotion::parse(direction).ok_or_else(|| {
                ApiError::bad_request(
                    "feedback-direction-invalid",
                    "feedbackDirection is not a supported motion direction",
                )
            })?;
        }
        effect.parameters = parameters.as_array().to_vec();
    }
    apply_cycle_interval(body, effect)
}

fn apply_cycle_interval(body: &UpdateLayer, effect: &mut EffectSlot) -> Result<(), ApiError> {
    let Some(interval) = body.cycle_interval.as_deref() else {
        return Ok(());
    };
    require_effect(
        effect,
        OPACITY_CYCLE_EFFECT,
        "cycle-interval-effect",
        "layer opacity cycle",
    )?;
    let interval = match interval {
        "every-beat" => OpacityCycleInterval::EveryBeat,
        "every-half-beat" => OpacityCycleInterval::EveryHalfBeat,
        "every-second" => OpacityCycleInterval::EverySecond,
        _ => {
            return Err(ApiError::bad_request(
                "cycle-interval-invalid",
                "cycleInterval must be every-beat, every-half-beat, or every-second",
            ));
        }
    };
    effect.parameters = vec![interval.parameter()];
    Ok(())
}

fn apply_spatial_edits(body: &UpdateLayer, effect: &mut EffectSlot) -> Result<(), ApiError> {
    if body.beat_move_amount.is_some()
        || body.beat_move_direction.is_some()
        || body.beat_move_decay.is_some()
    {
        require_effect(
            effect,
            BEAT_MOVE_EFFECT,
            "beat-move-parameters-effect",
            "Beat Move",
        )?;
        let mut parameters = BeatMoveParameters::from_parameters(&effect.parameters);
        parameters.amount = body.beat_move_amount.unwrap_or(parameters.amount);
        parameters.decay_seconds = body.beat_move_decay.unwrap_or(parameters.decay_seconds);
        if let Some(direction) = body.beat_move_direction.as_deref() {
            parameters.direction = BeatMoveDirection::parse(direction).ok_or_else(|| {
                ApiError::bad_request(
                    "beat-move-direction-invalid",
                    "beatMoveDirection must be up, down, left, or right",
                )
            })?;
        }
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.kaleidoscope_repetitions.is_some() || body.kaleidoscope_angle.is_some() {
        require_effect(
            effect,
            KALEIDOSCOPE_EFFECT,
            "kaleidoscope-parameters-effect",
            "Kaleidoscope",
        )?;
        let mut parameters = KaleidoscopeParameters::from_parameters(&effect.parameters);
        if let Some(repetitions) = body.kaleidoscope_repetitions {
            if repetitions > 12 {
                return Err(ApiError::bad_request(
                    "kaleidoscope-repetitions-range",
                    "kaleidoscopeRepetitions must be between 0 (Off) and 12",
                ));
            }
            parameters.repetitions = repetitions;
        }
        parameters.angle_degrees = body.kaleidoscope_angle.unwrap_or(parameters.angle_degrees);
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.rasterize_mode.is_some() || body.rasterize_dot_size.is_some() {
        require_effect(
            effect,
            RASTERIZE_EFFECT,
            "rasterize-parameters-effect",
            "Rasterized Print",
        )?;
        let mut parameters = RasterizeParameters::from_parameters(&effect.parameters);
        if let Some(mode) = body.rasterize_mode.as_deref() {
            parameters.mode = RasterizeMode::parse(mode).ok_or_else(|| {
                ApiError::bad_request(
                    "rasterize-mode-invalid",
                    "rasterizeMode must be black-and-white or cmyk",
                )
            })?;
        }
        parameters.dot_size = body.rasterize_dot_size.unwrap_or(parameters.dot_size);
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.beat_scan_width.is_some()
        || body.beat_scan_edge.is_some()
        || body.beat_scan_falloff.is_some()
        || body.beat_scan_duration.is_some()
    {
        if effect.effect_type.as_deref() != Some(BEAT_SCAN_EFFECT) {
            return Err(ApiError::bad_request(
                "beat-scan-parameters-effect",
                "choose the Beat Scan effect before changing its controls",
            ));
        }
        let mut parameters = BeatScanParameters::from_parameters(&effect.parameters);
        parameters.width = body.beat_scan_width.unwrap_or(parameters.width);
        parameters.falloff = body.beat_scan_falloff.unwrap_or(parameters.falloff);
        parameters.duration_seconds = body
            .beat_scan_duration
            .unwrap_or(parameters.duration_seconds);
        if let Some(edge) = body.beat_scan_edge.as_deref() {
            parameters.edge = BeatScanEdge::parse(edge).ok_or_else(|| {
                ApiError::bad_request(
                    "beat-scan-edge-invalid",
                    "beatScanEdge must be sharp or soft",
                )
            })?;
        }
        effect.parameters = parameters.as_array().to_vec();
    }
    Ok(())
}

fn apply_beat_edits(body: &UpdateLayer, effect: &mut EffectSlot) -> Result<(), ApiError> {
    if body.beat_scale_amount.is_some()
        || body.beat_turn_enabled.is_some()
        || body.beat_turn_rotation.is_some()
        || body.beat_scale_decay.is_some()
    {
        if effect.effect_type.as_deref() != Some(BEAT_SCALE_TURN_EFFECT) {
            return Err(ApiError::bad_request(
                "beat-scale-turn-parameters-effect",
                "choose the Beat Scale and Turn effect before changing its controls",
            ));
        }
        let mut parameters = BeatScaleTurnParameters::from_parameters(&effect.parameters);
        parameters.scale_amount = body.beat_scale_amount.unwrap_or(parameters.scale_amount);
        parameters.turn_enabled = body.beat_turn_enabled.unwrap_or(parameters.turn_enabled);
        parameters.rotation_degrees = body
            .beat_turn_rotation
            .unwrap_or(parameters.rotation_degrees);
        parameters.decay_seconds = body.beat_scale_decay.unwrap_or(parameters.decay_seconds);
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.beat_grid_density.is_some()
        || body.beat_grid_height.is_some()
        || body.beat_grid_duration.is_some()
        || body.beat_grid_origin.is_some()
        || body.beat_grid_hue.is_some()
        || body.beat_grid_brightness.is_some()
    {
        if effect.effect_type.as_deref() != Some(BEAT_GRID_WAVE_EFFECT) {
            return Err(ApiError::bad_request(
                "beat-grid-wave-parameters-effect",
                "choose the Beat Grid Wave effect before changing its controls",
            ));
        }
        let mut parameters = BeatGridWaveParameters::from_parameters(&effect.parameters);
        parameters.density = body.beat_grid_density.unwrap_or(parameters.density);
        parameters.height = body.beat_grid_height.unwrap_or(parameters.height);
        parameters.duration_seconds = body
            .beat_grid_duration
            .unwrap_or(parameters.duration_seconds);
        parameters.hue_degrees = body.beat_grid_hue.unwrap_or(parameters.hue_degrees);
        parameters.brightness = body.beat_grid_brightness.unwrap_or(parameters.brightness);
        if let Some(origin) = body.beat_grid_origin.as_deref() {
            parameters.origin = BeatGridWaveOrigin::parse(origin).ok_or_else(|| {
                ApiError::bad_request(
                    "beat-grid-origin-invalid",
                    "beatGridOrigin must be centre, top, right, bottom, or left",
                )
            })?;
        }
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.beat_form_enlargement.is_some()
        || body.beat_form_lifetime.is_some()
        || body.beat_form_density.is_some()
        || body.beat_form_variation.is_some()
    {
        if effect.effect_type.as_deref() != Some(BEAT_FORM_FLASH_EFFECT) {
            return Err(ApiError::bad_request(
                "beat-form-flash-parameters-effect",
                "choose the Beat Form Flash effect before changing its controls",
            ));
        }
        let mut parameters = BeatFormFlashParameters::from_parameters(&effect.parameters);
        parameters.enlargement = body.beat_form_enlargement.unwrap_or(parameters.enlargement);
        parameters.lifetime_seconds = body
            .beat_form_lifetime
            .unwrap_or(parameters.lifetime_seconds);
        parameters.density = body.beat_form_density.unwrap_or(parameters.density);
        parameters.variation = body.beat_form_variation.unwrap_or(parameters.variation);
        effect.parameters = parameters.as_array().to_vec();
    }
    if body.drawn_strength.is_some() || body.drawn_line_detail.is_some() {
        if effect.effect_type.as_deref() != Some(DRAWN_IMAGE_EFFECT) {
            return Err(ApiError::bad_request(
                "drawn-image-parameters-effect",
                "choose the Drawn Image effect before changing its controls",
            ));
        }
        let mut parameters = DrawnImageParameters::from_parameters(&effect.parameters);
        parameters.strength = body.drawn_strength.unwrap_or(parameters.strength);
        parameters.line_detail = body.drawn_line_detail.unwrap_or(parameters.line_detail);
        effect.parameters = parameters.as_array().to_vec();
    }
    Ok(())
}

fn apply_tv_edits(body: &UpdateLayer, effect: &mut EffectSlot) -> Result<(), ApiError> {
    let changes_parameters = body.tv_curvature.is_some()
        || body.effect_distortion.is_some()
        || body.image_grain.is_some()
        || body.compression_damage.is_some()
        || body.block_size.is_some()
        || body.tile_displacement.is_some()
        || body.chroma_damage.is_some()
        || body.effect_glitching.is_some();
    if changes_parameters {
        match effect.effect_type.as_deref() {
            Some(ANALOG_TV_EFFECT)
                if body.compression_damage.is_none()
                    && body.block_size.is_none()
                    && body.tile_displacement.is_none()
                    && body.chroma_damage.is_none() =>
            {
                let mut parameters =
                    media_domain::AnalogTvParameters::from_normalized(&effect.parameters);
                parameters.curvature = body.tv_curvature.unwrap_or(parameters.curvature);
                parameters.distortion = body.effect_distortion.unwrap_or(parameters.distortion);
                parameters.image_grain = body.image_grain.unwrap_or(parameters.image_grain);
                parameters.glitching = body.effect_glitching.unwrap_or(parameters.glitching);
                effect.parameters = parameters.as_array().to_vec();
            }
            Some(DIGITAL_TV_EFFECT)
                if body.tv_curvature.is_none()
                    && body.effect_distortion.is_none()
                    && body.image_grain.is_none() =>
            {
                let mut parameters =
                    media_domain::DigitalTvParameters::from_normalized(&effect.parameters);
                parameters.compression_damage = body
                    .compression_damage
                    .unwrap_or(parameters.compression_damage);
                parameters.block_size = body.block_size.unwrap_or(parameters.block_size);
                parameters.tile_displacement = body
                    .tile_displacement
                    .unwrap_or(parameters.tile_displacement);
                parameters.chroma_damage = body.chroma_damage.unwrap_or(parameters.chroma_damage);
                parameters.glitching = body.effect_glitching.unwrap_or(parameters.glitching);
                effect.parameters = parameters.as_array().to_vec();
            }
            _ => {
                return Err(ApiError::bad_request(
                    "effect-parameters-invalid",
                    "the typed parameters must match the selected effect in this slot",
                ));
            }
        }
    }
    Ok(())
}

fn require_effect(
    effect: &EffectSlot,
    expected: &str,
    code: &'static str,
    label: &str,
) -> Result<(), ApiError> {
    if effect.effect_type.as_deref() == Some(expected) {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            code,
            format!("choose the {label} effect before changing its controls"),
        ))
    }
}
