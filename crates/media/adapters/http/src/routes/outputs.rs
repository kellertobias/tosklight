//! Outputs, their layers, and the live control of both.
//!
//! Selecting media and setting a dimmer are live control, not edits: they carry no request
//! identity, because a caller that sent a selection twice meant it twice. What protects them is
//! ownership — the web interface reads but does not write until it explicitly takes over the
//! selected output.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_application::configuration::{load, save};
use media_domain::{
    ANALOG_TV_EFFECT, Applied, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT, BEAT_MOVE_EFFECT,
    BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT, BeatFormFlashParameters,
    BeatGridWaveOrigin, BeatGridWaveParameters, BeatMoveDirection, BeatMoveParameters,
    BeatScaleTurnParameters, BeatScanEdge, BeatScanParameters, BlurParameters, Command,
    CommandKind, CommandSource, DIGITAL_TV_EFFECT, DRAWN_IMAGE_EFFECT, DrawnImageParameters,
    EffectSlot, FEEDBACK_EFFECT, FeedbackMotion, FeedbackParameters, FlipMirror,
    KALEIDOSCOPE_EFFECT, KaleidoscopeParameters, LayerControls, MasterControls, MasterShaper,
    MediaAddress, MediaState, OPACITY_CYCLE_EFFECT, OpacityCycleInterval, OutputId,
    RASTERIZE_EFFECT, RasterizeMode, RasterizeParameters, ScalingMode, Timestamp, Tint, apply,
};

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{
    DmxMapView, OutputConfigurationView, OutputView, UpdateLayer, UpdateMaster,
    UpdateOutputConfiguration,
};

pub(super) async fn outputs(State(state): State<ApiState>) -> impl IntoResponse {
    let media = state.state.load();
    let views: Vec<OutputView> = media
        .outputs
        .iter()
        .map(|output| view_of(&state, output, (state.now)()))
        .collect();
    axum::Json(views)
}

pub(super) async fn output_state(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, (state.now)())).into_response())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreviewQuery {
    width: Option<u16>,
    height: Option<u16>,
}

/// Returns the latest composite frame from the same demand-driven source advertised over CITP.
/// A renderer seeds a valid black frame before the first GPU readback. The route still owns a
/// valid fallback for an API-only process, so an `<img>` can never receive an empty response.
pub(super) async fn output_preview(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    if state.state.load().output(id).is_none() {
        return Err(unknown_output(id));
    }
    let requested_size = query.width.zip(query.height);
    let frame = (state.preview)(id, None, requested_size)
        .unwrap_or_else(|| fallback_preview(requested_size, false));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_TYPE, frame.content_type)
        .header("x-tosklight-preview-sequence", frame.sequence)
        .header("x-tosklight-preview-width", frame.width)
        .header("x-tosklight-preview-height", frame.height)
        .body(axum::body::Body::from(frame.bytes))
        .expect("a preview response has valid static headers"))
}

/// Returns the latest isolated live frame for one layer.
pub(super) async fn layer_preview(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    if found.layer(layer).is_none() {
        return Err(ApiError::not_found(
            "layer-not-found",
            format!("output {id} has no layer {}", layer + 1),
        ));
    }
    let requested_size = query.width.zip(query.height);
    let frame = (state.preview)(id, Some(layer), requested_size)
        .unwrap_or_else(|| fallback_preview(requested_size, true));
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_TYPE, frame.content_type)
        .header("x-tosklight-preview-sequence", frame.sequence)
        .header("x-tosklight-preview-width", frame.width)
        .header("x-tosklight-preview-height", frame.height)
        .body(axum::body::Body::from(frame.bytes))
        .expect("a preview response has valid static headers"))
}

fn fallback_preview(size: Option<(u16, u16)>, transparent: bool) -> crate::OutputPreviewFrame {
    let (width, height) = size.unwrap_or((320, 180));
    let fill = if transparent { "none" } else { "black" };
    let bytes = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="100%" height="100%" fill="{fill}"/></svg>"#
    )
    .into_bytes();
    crate::OutputPreviewFrame {
        sequence: 0,
        width,
        height,
        content_type: "image/svg+xml",
        bytes,
    }
}

/// The stored settings that define one output.
///
/// This is separate from `/state`: state is what the output is drawing now, while configuration
/// is what surface, clock, personality, and DMX ingress the next process start will create.
pub(super) async fn output_configuration(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let configuration = state.configuration.load();
    let found = configuration.output(id).ok_or_else(|| unknown_output(id))?;
    let active = state.active_configuration.output(id).unwrap_or(found);
    Ok(axum::Json(OutputConfigurationView::of(
        found,
        active,
        (state.diagnostics.monitors)(),
        (state.diagnostics.output_devices)(),
    ))
    .into_response())
}

/// The canonical, absolute DMX map for one configured output.
pub(super) async fn dmx_map(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let configuration = state.configuration.load();
    let found = configuration.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(DmxMapView::of(found)).into_response())
}

/// Changes only the stated output settings, after validating the whole server configuration.
///
/// This edit uses the same written-then-live-then-answered path as every stored setting. The view
/// truthfully reports that all accepted fields take effect on restart; publishing the stored
/// configuration does not recreate an already-running output surface or ingress.
pub(super) async fn update_output_configuration(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    TolerantJson(body): TolerantJson<UpdateOutputConfiguration>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let id = parse_output(&output)?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let found = configuration
        .outputs
        .iter_mut()
        .find(|candidate| candidate.id == id)
        .ok_or_else(|| unknown_output(id))?;
    *found = body.applied(found).map_err(|error| {
        ApiError::bad_request("output-configuration-invalid", error.to_string())
    })?;
    let view = OutputConfigurationView::of(
        found,
        state.active_configuration.output(id).unwrap_or(found),
        (state.diagnostics.monitors)(),
        (state.diagnostics.output_devices)(),
    );

    // Loading what we would write exercises the one authoritative full-document validator. In
    // particular, this catches resolution and presentation errors, personality footprint bounds,
    // and overlaps with every other enabled output before persistence is attempted.
    load(&save(&configuration)).map_err(|error| {
        ApiError::bad_request("output-configuration-invalid", error.to_string())
    })?;

    edit::commit(&state, configuration, &body.request_id, &view)
}

pub(super) async fn update_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
    TolerantJson(body): TolerantJson<UpdateLayer>,
) -> Result<Response, ApiError> {
    update_layer_inner(state, output, layer, body, false).await
}

pub(super) async fn update_native_effects(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
    TolerantJson(body): TolerantJson<UpdateLayer>,
) -> Result<Response, ApiError> {
    if !body.changes_effect() || body.changes_non_effect() || body.effect_mix.is_some() {
        return Err(ApiError::bad_request(
            "native-effect-configuration-only",
            "native effect configuration accepts effect type, state, and typed parameters; control effect amount through DMX",
        ));
    }
    update_layer_inner(state, output, layer, body, true).await
}

async fn update_layer_inner(
    state: ApiState,
    output: String,
    layer: usize,
    body: UpdateLayer,
    native_effect_configuration: bool,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();

    let current = {
        let media = state.state.load();
        let found = media.output(id).ok_or_else(|| unknown_output(id))?;
        found
            .layer(layer)
            .ok_or_else(|| {
                ApiError::not_found("unknown-layer", format!("this output has no layer {layer}"))
            })?
            .clone()
    };
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
    ] {
        validate_range(name, value, minimum, maximum)?;
    }
    let scaling_mode = body
        .scaling_mode
        .as_deref()
        .map(parse_scaling_mode)
        .transpose()?;
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
    let effects = if body.changes_effect() {
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
        let effect = &mut effects[slot];
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
            if effect.effect_type.as_deref() != Some(BLUR_EFFECT) {
                return Err(ApiError::bad_request(
                    "blur-parameters-effect",
                    "choose the Blur effect before changing its controls",
                ));
            }
            effect.parameters = BlurParameters { amount }.as_array().to_vec();
        }
        if body.feedback_amount.is_some()
            || body.feedback_motion.is_some()
            || body.feedback_direction.is_some()
        {
            if effect.effect_type.as_deref() != Some(FEEDBACK_EFFECT) {
                return Err(ApiError::bad_request(
                    "feedback-parameters-effect",
                    "choose the Feedback effect before changing its controls",
                ));
            }
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
        if let Some(interval) = body.cycle_interval.as_deref() {
            if effect.effect_type.as_deref() != Some(OPACITY_CYCLE_EFFECT) {
                return Err(ApiError::bad_request(
                    "cycle-interval-effect",
                    "choose the layer opacity cycle effect before setting its interval",
                ));
            }
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
        }
        if body.beat_move_amount.is_some()
            || body.beat_move_direction.is_some()
            || body.beat_move_decay.is_some()
        {
            if effect.effect_type.as_deref() != Some(BEAT_MOVE_EFFECT) {
                return Err(ApiError::bad_request(
                    "beat-move-parameters-effect",
                    "choose the Beat Move effect before changing its controls",
                ));
            }
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
            if effect.effect_type.as_deref() != Some(KALEIDOSCOPE_EFFECT) {
                return Err(ApiError::bad_request(
                    "kaleidoscope-parameters-effect",
                    "choose the Kaleidoscope effect before changing its controls",
                ));
            }
            let mut parameters = KaleidoscopeParameters::from_parameters(&effect.parameters);
            if let Some(repetitions) = body.kaleidoscope_repetitions {
                if !(1..=16).contains(&repetitions) {
                    return Err(ApiError::bad_request(
                        "kaleidoscope-repetitions-range",
                        "kaleidoscopeRepetitions must be between 1 and 16",
                    ));
                }
                parameters.repetitions = repetitions;
            }
            parameters.angle_degrees = body.kaleidoscope_angle.unwrap_or(parameters.angle_degrees);
            effect.parameters = parameters.as_array().to_vec();
        }
        if body.rasterize_mode.is_some() || body.rasterize_dot_size.is_some() {
            if effect.effect_type.as_deref() != Some(RASTERIZE_EFFECT) {
                return Err(ApiError::bad_request(
                    "rasterize-parameters-effect",
                    "choose the Rasterized Print effect before changing its controls",
                ));
            }
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
                    parameters.chroma_damage =
                        body.chroma_damage.unwrap_or(parameters.chroma_damage);
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
        effect.normalize();
        Some(effects)
    } else {
        None
    };
    let command = if native_effect_configuration {
        CommandKind::ConfigureLayerEffects {
            output: id,
            layer,
            effects: Box::new(effects.expect("native effect configuration has effects")),
        }
    } else {
        CommandKind::SetLayerControls {
            output: id,
            layer,
            controls: Box::new(LayerControls {
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
            }),
        }
    };
    submit(&state, vec![command], now)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

pub(super) async fn update_master(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    TolerantJson(body): TolerantJson<UpdateMaster>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();
    let current = state
        .state
        .load()
        .output(id)
        .ok_or_else(|| unknown_output(id))?
        .master;
    for (name, value) in [
        ("dimmer", body.dimmer),
        ("volume", body.volume),
        ("tintRed", body.tint_red),
        ("tintGreen", body.tint_green),
        ("tintBlue", body.tint_blue),
    ] {
        validate_unit(name, value)?;
    }
    for (name, value, minimum, maximum) in [
        ("scaleX", body.scale_x, 0.0, 4.0),
        ("scaleY", body.scale_y, 0.0, 4.0),
        ("positionX", body.position_x, -2.0, 2.0),
        ("positionY", body.position_y, -2.0, 2.0),
        ("maskPositionX", body.mask_position_x, -2.0, 2.0),
        ("maskPositionY", body.mask_position_y, -2.0, 2.0),
        ("rotation", body.rotation, -180.0, 180.0),
        ("shaperLeft", body.shaper_left, 0.0, 1.0),
        ("shaperRight", body.shaper_right, 0.0, 1.0),
        ("shaperTop", body.shaper_top, 0.0, 1.0),
        ("shaperBottom", body.shaper_bottom, 0.0, 1.0),
        ("shaperLeftRotation", body.shaper_left_rotation, -45.0, 45.0),
        (
            "shaperRightRotation",
            body.shaper_right_rotation,
            -45.0,
            45.0,
        ),
        ("shaperTopRotation", body.shaper_top_rotation, -45.0, 45.0),
        (
            "shaperBottomRotation",
            body.shaper_bottom_rotation,
            -45.0,
            45.0,
        ),
        ("shaperRotation", body.shaper_rotation, -180.0, 180.0),
    ] {
        validate_range(name, value, minimum, maximum)?;
    }
    let tint = (body.tint_red.is_some() || body.tint_green.is_some() || body.tint_blue.is_some())
        .then(|| {
            Tint::new(
                body.tint_red.unwrap_or(current.tint.red),
                body.tint_green.unwrap_or(current.tint.green),
                body.tint_blue.unwrap_or(current.tint.blue),
            )
        });
    let flip_mirror = body
        .flip_mirror
        .as_deref()
        .map(parse_flip_mirror)
        .transpose()?;
    let mask = (body.mask_folder.is_some() || body.mask_file.is_some()).then(|| {
        MediaAddress::new(
            body.mask_folder.unwrap_or(current.mask.folder),
            body.mask_file.unwrap_or(current.mask.file),
        )
    });
    let scaling_mode = body
        .scaling_mode
        .as_deref()
        .map(parse_scaling_mode)
        .transpose()?;
    let shaper_changed = body.shaper_left.is_some()
        || body.shaper_right.is_some()
        || body.shaper_top.is_some()
        || body.shaper_bottom.is_some()
        || body.shaper_left_rotation.is_some()
        || body.shaper_right_rotation.is_some()
        || body.shaper_top_rotation.is_some()
        || body.shaper_bottom_rotation.is_some()
        || body.shaper_rotation.is_some();
    let shaper = shaper_changed.then(|| MasterShaper {
        left: body.shaper_left.unwrap_or(current.shaper.left),
        right: body.shaper_right.unwrap_or(current.shaper.right),
        top: body.shaper_top.unwrap_or(current.shaper.top),
        bottom: body.shaper_bottom.unwrap_or(current.shaper.bottom),
        left_rotation: body
            .shaper_left_rotation
            .unwrap_or(current.shaper.left_rotation),
        right_rotation: body
            .shaper_right_rotation
            .unwrap_or(current.shaper.right_rotation),
        top_rotation: body
            .shaper_top_rotation
            .unwrap_or(current.shaper.top_rotation),
        bottom_rotation: body
            .shaper_bottom_rotation
            .unwrap_or(current.shaper.bottom_rotation),
        rotation: body.shaper_rotation.unwrap_or(current.shaper.rotation),
    });
    submit(
        &state,
        vec![CommandKind::SetMasterControls {
            output: id,
            controls: Box::new(MasterControls {
                dimmer: body.dimmer,
                volume: body.volume,
                tint,
                flip_mirror,
                mask,
                mask_position_x: body.mask_position_x,
                mask_position_y: body.mask_position_y,
                scale_x: body.scale_x,
                scale_y: body.scale_y,
                scaling_mode,
                position_x: body.position_x,
                position_y: body.position_y,
                rotation: body.rotation,
                shaper,
            }),
        }],
        now,
    )?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

fn validate_unit(name: &str, value: Option<f32>) -> Result<(), ApiError> {
    if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
        return Err(ApiError::bad_request(
            if name == "dimmer" {
                "dimmer-out-of-range"
            } else {
                "control-out-of-range"
            },
            format!("{name} must be between 0 and 1"),
        ));
    }
    Ok(())
}

fn validate_range(
    name: &str,
    value: Option<f32>,
    minimum: f32,
    maximum: f32,
) -> Result<(), ApiError> {
    if value.is_some_and(|value| !value.is_finite() || !(minimum..=maximum).contains(&value)) {
        return Err(ApiError::bad_request(
            "control-out-of-range",
            format!("{name} must be between {minimum} and {maximum}"),
        ));
    }
    Ok(())
}

fn parse_scaling_mode(value: &str) -> Result<ScalingMode, ApiError> {
    match value {
        "fit" => Ok(ScalingMode::Fit),
        "fill" => Ok(ScalingMode::Fill),
        "original" => Ok(ScalingMode::Original),
        "stretch" => Ok(ScalingMode::Stretch),
        _ => Err(ApiError::bad_request(
            "unknown-scaling-mode",
            "use fit, fill, original, or stretch",
        )),
    }
}

fn parse_flip_mirror(value: &str) -> Result<FlipMirror, ApiError> {
    match value {
        "none" => Ok(FlipMirror::None),
        "horizontal" => Ok(FlipMirror::Horizontal),
        "vertical" => Ok(FlipMirror::Vertical),
        "both" => Ok(FlipMirror::Both),
        _ => Err(ApiError::bad_request(
            "unknown-flip-mirror",
            "use none, horizontal, vertical, or both",
        )),
    }
}

/// Restarts a layer's media. A live-control action with no payload, so it is a `GET` an
/// integrator can trigger from a URL bar or a microcontroller.
pub(super) async fn reset_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();
    submit(
        &state,
        vec![CommandKind::ResetLayer { output: id, layer }],
        now,
    )?;

    // A side-effecting GET must never be cached, or a proxy would swallow the second press.
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Explicitly changes playback ownership. Taking over makes subsequent network DMX read-only;
/// releasing immediately lets the next Art-Net or sACN frame drive the output again.
pub(super) async fn set_playback_takeover(
    State(state): State<ApiState>,
    Path((output, mode)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let take_over = match mode.as_str() {
        "take-over" => true,
        "release" => false,
        _ => {
            return Err(ApiError::not_found(
                "unknown-playback-control",
                "use take-over or release",
            ));
        }
    };
    let now = (state.now)();
    submit(
        &state,
        vec![CommandKind::TakeOverPlayback {
            output: id,
            take_over,
        }],
        now,
    )?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(view_of(&state, found, now)),
    )
        .into_response())
}

/// Applies commands through the reducer and publishes one new snapshot.
fn submit(state: &ApiState, commands: Vec<CommandKind>, now: Timestamp) -> Result<(), ApiError> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut next = MediaState::clone(&state.state.load());
    let mut published = false;

    for kind in commands {
        let command = Command::new(kind, CommandSource::Web, now);
        match apply(&mut next, &command) {
            Applied::Changed => published = true,
            Applied::Unchanged => {}
            Applied::RejectedNotOwner => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "playback-takeover-required",
                    "take over playback for this output before changing its live values",
                ));
            }
            Applied::RejectedUnknownOutput => {
                return Err(ApiError::not_found("unknown-output", "no such output"));
            }
            Applied::RejectedUnknownLayer => {
                return Err(ApiError::not_found("unknown-layer", "no such layer"));
            }
        }
    }

    if published {
        state.state.store(Arc::new(next));
    }
    Ok(())
}

fn view_of(state: &ApiState, output: &media_domain::OutputState, now: Timestamp) -> OutputView {
    let configuration = state.configuration.load();
    let name = configuration
        .output(output.id)
        .map(|configured| configured.name.to_string())
        .unwrap_or_else(|| output.id.to_string());

    OutputView::of(output, name, output.ownership.dmx_is_active(now))
}

fn parse_output(raw: &str) -> Result<OutputId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(OutputId::from_uuid)
        .map_err(|_| ApiError::bad_request("malformed-output-id", "that is not an output id"))
}

fn unknown_output(id: OutputId) -> ApiError {
    ApiError::not_found("unknown-output", format!("no output {id}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::{StatusCode, header};
    use http_body_util::BodyExt as _;
    use media_application::configuration::{MediaConfiguration, OutputConfiguration};
    use media_domain::{
        CommandSource, LayerPersonality, MediaAddress, MediaState, OutputId, Timestamp,
    };
    use tower::ServiceExt as _;

    use crate::routes::bench::{Bench, bench, get, post, send};

    async fn take_over(bench: &Bench) {
        let (status, _) = send(
            &bench.router,
            get(format!(
                "/api/v2/outputs/{}/playback/take-over",
                bench.output
            )),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn an_output_returns_its_whole_state() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Main");
        assert_eq!(body["layers"].as_array().unwrap().len(), 2);
        assert_eq!(body["layers"][0]["playMode"], "Loop");
        assert_eq!(body["layers"][0]["effects"].as_array().unwrap().len(), 4);
        assert_eq!(body["dmxActive"], false);
    }

    #[tokio::test]
    async fn an_output_returns_its_whole_editable_configuration() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/configuration", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], bench.output.to_string());
        assert_eq!(body["name"], "Main");
        assert_eq!(body["targetKind"], "off-screen");
        assert!(body["monitorBy"].is_null());
        assert!(body["monitorValue"].is_null());
        assert_eq!(body["fullscreen"], false);
        assert_eq!(body["width"], 1920);
        assert_eq!(body["height"], 1080);
        assert_eq!(body["presentation"], "display-synchronized");
        assert_eq!(body["personality"], "two-layers");
        assert_eq!(body["protocol"], "art-net");
        assert_eq!(body["universe"], 0);
        assert_eq!(body["startAddress"], 1);
        assert_eq!(body["takesEffectOnRestart"], true);
        assert!(
            body.get("statusOverlay").is_none(),
            "the retired setting stays dropped"
        );
        assert!(
            body.get("targetCodec").is_none(),
            "library policy is not an output setting"
        );
    }

    #[tokio::test]
    async fn an_output_returns_its_canonical_absolute_dmx_map() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/dmx-map", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["outputId"], bench.output.to_string());
        assert_eq!(body["outputName"], "Main");
        assert_eq!(body["personality"], "twoLayers");
        assert_eq!(body["layerCount"], 2);
        assert_eq!(body["channels"].as_array().unwrap().len(), 118);
        assert_eq!(body["channels"][0]["absoluteChannel"], 1);
        assert_eq!(body["channels"][0]["name"], "Folder");
        // The master section follows both 39-slot layer blocks.
        assert_eq!(body["channels"][78]["group"]["kind"], "master");
        assert_eq!(body["channels"][78]["absoluteChannel"], 79);

        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/dmx-map", OutputId::new())),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-output");
    }

    #[tokio::test]
    async fn an_output_configuration_edit_is_intent_shaped_written_and_restart_explicit() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"settings-1","targetKind":"monitor","monitorBy":"name","monitorValue":" Stage Right ","fullscreen":true,"width":1280,"height":720,"presentation":"fixed-fps","framesPerSecond":50,"personality":"eight-layers","protocol":"sacn","universe":7,"startAddress":10}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["targetKind"], "monitor");
        assert_eq!(body["monitorBy"], "name");
        assert_eq!(body["monitorValue"], "Stage Right");
        assert_eq!(body["fullscreen"], true);
        assert_eq!(body["width"], 1280);
        assert_eq!(body["height"], 720);
        assert_eq!(body["presentation"], "fixed-fps");
        assert_eq!(body["framesPerSecond"], 50.0);
        assert_eq!(body["personality"], "eight-layers");
        assert_eq!(body["protocol"], "sacn");
        assert_eq!(body["universe"], 7);
        assert_eq!(body["startAddress"], 10);
        assert_eq!(body["takesEffectOnRestart"], true);
        assert_eq!(body["active"]["width"], 1920);
        assert_eq!(body["active"]["protocol"], "art-net");
        assert_eq!(body["picturePendingRestart"], true);
        assert_eq!(body["soundPendingRestart"], false);
        assert_eq!(body["dmxPendingRestart"], true);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(
            stored.len(),
            1,
            "an accepted edit was persisted exactly once"
        );
        assert_eq!(stored[0].outputs[0].resolution.width, 1280);
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_output_configuration_edit_changes_only_what_it_carries() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"settings-2","universe":4,"newerClientField":{"keptByNewerServer":true}}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "unknown fields remain tolerated");
        assert_eq!(body["universe"], 4);
        assert_eq!(body["width"], 1920);
        assert_eq!(body["presentation"], "display-synchronized");
        assert_eq!(body["personality"], "two-layers");
    }

    #[tokio::test]
    async fn an_invalid_output_edit_is_neither_written_nor_published() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let (status, body) = send(
            &bench.router,
            post(uri, r#"{"requestId":"settings-3","height":0}"#),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "output-configuration-invalid");
        assert!(body["message"].as_str().unwrap().contains("height"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(
            bench.configuration.load().outputs[0].resolution.height,
            1080
        );
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_output_edit_validates_against_every_other_outputs_patch() {
        let bench = bench();
        let mut configuration = MediaConfiguration::clone(&bench.configuration.load());
        let mut second = OutputConfiguration::new("Second");
        second.personality = LayerPersonality::TwoLayers;
        second.start_address = 200;
        let second_id = second.id;
        configuration.outputs.push(second);
        bench.configuration.store(Arc::new(configuration));

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{second_id}/configuration/update"),
                r#"{"requestId":"overlap","startAddress":50}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "output-configuration-invalid");
        assert!(body["message"].as_str().unwrap().contains("both consume"));
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(bench.configuration.load().outputs[1].start_address, 200);
    }

    #[tokio::test]
    async fn a_retried_output_edit_is_answered_without_a_second_write() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/configuration/update", bench.output);
        let request = r#"{"requestId":"same-settings","startAddress":20}"#;

        let (_, first) = send(&bench.router, post(uri.clone(), request)).await;
        let (status, second) = send(&bench.router, post(uri, request)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_output_edit_the_disk_refuses_does_not_become_live() {
        let bench = bench();
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/configuration/update", bench.output),
                r#"{"requestId":"settings-4","universe":12}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");
        assert_eq!(bench.configuration.load().outputs[0].universe, 0);
        assert_eq!(bench.applied.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_update_carries_only_what_it_changes() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);

        let (status, body) =
            send(&bench.router, post(uri.clone(), r#"{"folder":3,"file":7}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["folder"], 3);
        assert_eq!(body["layers"][0]["address"]["file"], 7);
        assert_eq!(body["layers"][0]["dimmer"], 1.0);

        // A dimmer change must not disturb the selection.
        let (_, body) = send(&bench.router, post(uri, r#"{"dimmer":0.25}"#)).await;
        assert_eq!(body["layers"][0]["dimmer"], 0.25);
        assert_eq!(
            body["layers"][0]["address"]["folder"], 3,
            "the selection survived"
        );
        assert_eq!(body["layers"][0]["address"]["file"], 7);
    }

    #[tokio::test]
    async fn a_layer_and_master_accept_the_network_equivalent_controls() {
        let bench = bench();
        take_over(&bench).await;
        let (_, layer) = send(
            &bench.router,
            post(format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"playModeDmx":236,"scaleX":10,"scaleY":2,"scalingMode":"stretch","positionX":-2,"positionY":2,"rotation":360,"volume":0.4,"tintRed":0.2,"tintGreen":0.3,"tintBlue":0.4,"grayscale":0.5,"maskFolder":2,"maskFile":3,"maskScaleX":2,"maskScaleY":1.5,"maskPositionX":-0.75,"maskPositionY":1.25,"maskInvert":true,"maskOpacity":0.8,"speedMultiplierDmx":255,"playbackBpm":120}"#),
        ).await;
        assert_eq!(layer["layers"][0]["playMode"], "Pause");
        assert_eq!(layer["layers"][0]["scalingMode"], "stretch");
        assert_eq!(layer["layers"][0]["speedMultiplier"], "16×");
        assert_eq!(layer["layers"][0]["mask"]["address"]["file"], 3);
        assert_eq!(layer["layers"][0]["mask"]["positionX"], -0.75);
        assert_eq!(layer["layers"][0]["mask"]["positionY"], 1.25);

        let (_, master) = send(
            &bench.router,
            post(format!("/api/v2/outputs/{}/master/update", bench.output),
                r#"{"dimmer":0.4,"volume":0.5,"tintRed":0.6,"tintGreen":0.7,"tintBlue":0.8,"flipMirror":"both","maskFolder":4,"maskFile":5,"maskPositionX":0.75,"maskPositionY":-1.25,"scaleX":1.5,"scaleY":0.75,"scalingMode":"fill","positionX":0.5,"positionY":-0.5,"rotation":30,"shaperLeft":0.1,"shaperRight":0.2,"shaperTop":0.3,"shaperBottom":0.4,"shaperLeftRotation":10,"shaperRightRotation":-10,"shaperTopRotation":20,"shaperBottomRotation":-20,"shaperRotation":15}"#),
        ).await;
        assert_eq!(master["master"]["flipMirror"], "both");
        assert_eq!(master["master"]["mask"]["file"], 5);
        assert_eq!(master["master"]["maskPositionX"], 0.75);
        assert_eq!(master["master"]["maskPositionY"], -1.25);
        assert_eq!(master["master"]["scaleX"], 1.5);
        assert_eq!(master["master"]["scalingMode"], "fill");
        assert_eq!(master["master"]["positionY"], -0.5);
        assert_eq!(master["master"]["rotation"], 30.0);
        assert_eq!(master["master"]["shaperLeft"], 0.1);
        assert_eq!(master["master"]["shaperBottomRotation"], -20.0);
        assert_eq!(master["master"]["shaperRotation"], 15.0);
    }

    #[tokio::test]
    async fn analog_tv_is_a_typed_intent_shaped_effect_edit() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(uri.clone(), r#"{"effectSlot":1,"effectType":"analog-tv"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][1];
        assert_eq!(effect["effectType"], "analog-tv");
        assert_eq!(effect["label"], "Analog TV");
        assert_eq!(effect["supported"], true);
        assert_eq!(effect["parameters"][0]["id"], "tv-curvature");
        assert_eq!(effect["parameters"][0]["value"], 0.30);
        assert_eq!(effect["parameters"][1]["value"], 0.18);
        assert_eq!(effect["parameters"][2]["value"], 0.20);
        assert_eq!(effect["parameters"][3]["value"], 0.08);

        let (status, tuned) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":1,"tvCurvature":0,"effectDistortion":0.6,"imageGrain":0,"effectGlitching":1}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &tuned["layers"][0]["effects"][1];
        assert_eq!(effect["parameters"][0]["value"], 0.0);
        assert_eq!(effect["parameters"][1]["value"], 0.6);
        assert_eq!(effect["parameters"][2]["value"], 0.0);
        assert_eq!(effect["parameters"][3]["value"], 1.0);

        let (_, cleared) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":1,"effectType":"none"}"#),
        )
        .await;
        assert!(cleared["layers"][0]["effects"][1]["effectType"].is_null());
    }

    #[tokio::test]
    async fn opacity_cycle_persists_its_named_interval() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri,
                r#"{"effectSlot":0,"effectType":"opacity-cycle","cycleInterval":"every-half-beat"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "opacity-cycle");
        assert_eq!(effect["label"], "Layer opacity cycle");
        assert_eq!(effect["parameters"][0]["id"], "cycle-interval");
        assert_eq!(effect["parameters"][0]["value"], 1.0);
    }

    #[tokio::test]
    async fn blur_persists_its_live_amount_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"blur","blurAmount":0.8}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "blur");
        assert_eq!(effect["parameters"][0]["id"], "blur-amount");
        assert_eq!(effect["parameters"][0]["value"], 0.8);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn feedback_persists_all_motion_controls_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"feedback","feedbackAmount":0.7,"feedbackMotion":0.4,"feedbackDirection":"rotate-right"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "feedback");
        assert_eq!(effect["label"], "Feedback");
        assert_eq!(effect["parameters"][0]["value"], 0.7);
        assert_eq!(effect["parameters"][1]["value"], 0.4);
        assert_eq!(effect["parameters"][2]["value"], 5.0);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn beat_move_persists_amount_direction_return_time_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"beat-move","beatMoveAmount":0.3,"beatMoveDirection":"right","beatMoveDecay":0.8}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "beat-move");
        assert_eq!(effect["label"], "Beat Move");
        assert_eq!(effect["parameters"][0]["value"], 0.3);
        assert_eq!(effect["parameters"][1]["value"], 3.0);
        assert_eq!(effect["parameters"][2]["value"], 0.8);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn kaleidoscope_persists_live_repetitions_angle_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"kaleidoscope","kaleidoscopeRepetitions":8,"kaleidoscopeAngle":37}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "kaleidoscope");
        assert_eq!(effect["label"], "Kaleidoscope");
        assert_eq!(effect["parameters"][0]["value"], 8.0);
        assert_eq!(effect["parameters"][1]["value"], 37.0);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn rasterize_persists_print_mode_dot_size_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"rasterize","rasterizeMode":"cmyk","rasterizeDotSize":18}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "rasterize");
        assert_eq!(effect["label"], "Rasterized Print");
        assert_eq!(effect["parameters"][0]["value"], 1.0);
        assert_eq!(effect["parameters"][1]["value"], 18.0);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn beat_scan_persists_width_edge_falloff_duration_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"beat-scan","beatScanWidth":0.12,"beatScanEdge":"soft","beatScanFalloff":0.7,"beatScanDuration":2.25}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "beat-scan");
        assert_eq!(effect["label"], "Beat Scan");
        assert_eq!(effect["parameters"][0]["value"], 0.12);
        assert_eq!(effect["parameters"][1]["value"], 1.0);
        assert_eq!(effect["parameters"][2]["value"], 0.7);
        assert_eq!(effect["parameters"][3]["value"], 2.25);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn beat_scale_turn_persists_independent_turn_amounts_decay_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"beat-scale-turn","beatScaleAmount":0.22,"beatTurnEnabled":true,"beatTurnRotation":-7,"beatScaleDecay":0.8}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "beat-scale-turn");
        assert_eq!(effect["label"], "Beat Scale and Turn");
        assert_eq!(effect["parameters"][0]["value"], 0.22);
        assert_eq!(effect["parameters"][1]["value"], 1.0);
        assert_eq!(effect["parameters"][2]["value"], -7.0);
        assert_eq!(effect["parameters"][3]["value"], 0.8);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn beat_grid_wave_persists_origin_shape_colour_brightness_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"beat-grid-wave","beatGridDensity":36,"beatGridHeight":0.72,"beatGridDuration":1.8,"beatGridOrigin":"left","beatGridHue":280,"beatGridBrightness":1.4}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "beat-grid-wave");
        assert_eq!(effect["label"], "Beat Grid Wave");
        assert_eq!(effect["parameters"][0]["value"], 36.0);
        assert_eq!(effect["parameters"][1]["value"], 0.72);
        assert_eq!(effect["parameters"][2]["value"], 1.8);
        assert_eq!(effect["parameters"][3]["value"], 4.0);
        assert_eq!(effect["parameters"][4]["value"], 280.0);
        assert_eq!(effect["parameters"][5]["value"], 1.4);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn beat_form_flash_persists_size_lifetime_density_variation_and_bypass() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"beat-form-flash","beatFormEnlargement":2.4,"beatFormLifetime":1.6,"beatFormDensity":3,"beatFormVariation":0.65}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][0];
        assert_eq!(effect["effectType"], "beat-form-flash");
        assert_eq!(effect["label"], "Beat Form Flash");
        assert_eq!(effect["parameters"][0]["value"], 2.4);
        assert_eq!(effect["parameters"][1]["value"], 1.6);
        assert_eq!(effect["parameters"][2]["value"], 3.0);
        assert_eq!(effect["parameters"][3]["value"], 0.65);

        let (_, bypassed) = send(
            &bench.router,
            post(uri, r#"{"effectSlot":0,"effectEnabled":false}"#),
        )
        .await;
        assert_eq!(bypassed["layers"][0]["effects"][0]["enabled"], false);
    }

    #[tokio::test]
    async fn slot_one_persists_visualizer_parameters_only_for_a_visualizer_layer() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, _) = send(
            &bench.router,
            post(uri.clone(), r#"{"folder":250,"file":1}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let mut parameters = crate::wire::VisualizerParametersView::of(
            &media_domain::VisualizerParameters::default(),
        );
        parameters.size = 0.2;
        let body = serde_json::json!({
            "effectSlot": 0,
            "visualizerParameters": parameters,
        })
        .to_string();
        let (status, tuned) = send(&bench.router, post(uri.clone(), &body)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            tuned["layers"][0]["effects"][0]["visualizerParameters"]["size"],
            0.2
        );
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].effects[0]
                .visualizer_parameters
                .as_ref()
                .unwrap()
                .size,
            0.2
        );

        let (_, _) = send(&bench.router, post(uri.clone(), r#"{"folder":1,"file":1}"#)).await;
        let (status, rejected) = send(&bench.router, post(uri, &body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(rejected["code"], "visualizer-controls-source");
    }

    #[tokio::test]
    async fn invalid_or_unsupported_effect_edits_leave_the_chain_untouched() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        for (body, code) in [
            (r#"{"effectType":"analog-tv"}"#, "effect-slot-required"),
            (
                r#"{"effectSlot":4,"effectType":"analog-tv"}"#,
                "effect-slot-out-of-range",
            ),
            (
                r#"{"effectSlot":0,"effectType":"future-effect"}"#,
                "effect-unsupported",
            ),
            (
                r#"{"effectSlot":0,"effectType":"analog-tv","imageGrain":1.1}"#,
                "control-out-of-range",
            ),
        ] {
            let (status, response) = send(&bench.router, post(uri.clone(), body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(response["code"], code, "{body}");
            assert!(
                bench.state.load().output(bench.output).unwrap().layers[0].effects[0]
                    .effect_type
                    .is_none(),
                "the rejected edit published nothing"
            );
        }
    }

    #[tokio::test]
    async fn digital_tv_is_a_five_parameter_typed_intent_shaped_effect_edit() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        let (status, selected) = send(
            &bench.router,
            post(uri.clone(), r#"{"effectSlot":2,"effectType":"digital-tv"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let effect = &selected["layers"][0]["effects"][2];
        assert_eq!(effect["effectType"], "digital-tv");
        assert_eq!(effect["label"], "Digital TV");
        assert_eq!(effect["parameters"][0]["value"], 0.35);
        assert_eq!(effect["parameters"][1]["value"], 0.35);
        assert_eq!(effect["parameters"][2]["value"], 0.25);
        assert_eq!(effect["parameters"][3]["value"], 0.20);
        assert_eq!(effect["parameters"][4]["value"], 0.15);

        let (status, tuned) = send(
            &bench.router,
            post(
                uri,
                r#"{"effectSlot":2,"compressionDamage":0,"blockSize":1,"tileDisplacement":0.6,"chromaDamage":0.4,"effectGlitching":1}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let values = tuned["layers"][0]["effects"][2]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|parameter| parameter["value"].as_f64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0.0, 1.0, 0.6, 0.4, 1.0]);
    }

    #[tokio::test]
    async fn an_invalid_control_range_publishes_none_of_the_update() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.25,"scaleX":10.01}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "control-out-of-range");
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].dimmer,
            1.0
        );
    }

    #[tokio::test]
    async fn unknown_fields_are_accepted_rather_than_rejected() {
        let bench = bench();
        take_over(&bench).await;
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5,"somethingNewer":true,"nested":{"deep":1}}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "a newer client must not be refused");
        assert_eq!(body["layers"][0]["dimmer"], 0.5);
    }

    #[tokio::test]
    async fn a_malformed_body_names_the_problem_rather_than_crashing() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":"loud"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "body-invalid");
        assert!(body["message"].as_str().unwrap().contains("dimmer"));
    }

    #[tokio::test]
    async fn a_dimmer_outside_its_range_is_refused_with_a_stable_code() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":4.0}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "dimmer-out-of-range");
    }

    #[tokio::test]
    async fn a_reset_is_a_payload_free_get_that_must_not_be_cached() {
        let bench = bench();
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].reset_trigger_id,
            1
        );
    }

    #[tokio::test]
    async fn takeover_and_release_are_get_actions_that_must_not_be_cached() {
        let bench = bench();
        for (mode, expected) in [("take-over", true), ("release", false)] {
            let response = bench
                .router
                .clone()
                .oneshot(get(format!(
                    "/api/v2/outputs/{}/playback/{mode}",
                    bench.output
                )))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                bench
                    .state
                    .load()
                    .output(bench.output)
                    .unwrap()
                    .ownership
                    .web_takeover,
                expected
            );
        }
    }

    #[tokio::test]
    async fn playback_changes_require_explicit_takeover_even_before_dmx() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "playback-takeover-required");
    }

    #[tokio::test]
    async fn an_output_preview_returns_the_renderer_citp_frame_without_caching() {
        let bench = bench();
        *bench.preview_frame.lock().unwrap() = Some(crate::OutputPreviewFrame {
            sequence: 7,
            width: 320,
            height: 180,
            content_type: "image/jpeg",
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
        });
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/preview?width=320&height=180",
                bench.output
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(response.headers()["x-tosklight-preview-sequence"], "7");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            &[0xff, 0xd8, 0xff, 0xd9][..]
        );
    }

    #[tokio::test]
    async fn a_layer_preview_returns_an_isolated_live_renderer_frame() {
        let bench = bench();
        *bench.preview_frame.lock().unwrap() = Some(crate::OutputPreviewFrame {
            sequence: 8,
            width: 160,
            height: 90,
            content_type: "image/png",
            bytes: vec![0x89, b'P', b'N', b'G'],
        });
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/preview?width=160&height=90",
                bench.output
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()["x-tosklight-preview-sequence"], "8");
    }

    #[tokio::test]
    async fn preview_routes_always_return_valid_image_payloads_before_the_renderer_captures() {
        let bench = bench();
        for (path, expected_fill) in [
            (format!("/api/v2/outputs/{}/preview", bench.output), "black"),
            (
                format!("/api/v2/outputs/{}/layers/0/preview", bench.output),
                "none",
            ),
        ] {
            let response = bench.router.clone().oneshot(get(path)).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "image/svg+xml");
            let body = response.into_body().collect().await.unwrap().to_bytes();
            assert!(
                std::str::from_utf8(&body)
                    .unwrap()
                    .contains(&format!("fill=\"{expected_fill}\""))
            );
        }
    }

    #[tokio::test]
    async fn a_live_desk_keeps_the_web_ui_read_only_with_a_reason() {
        let bench = bench();
        // A desk starts sending.
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "playback-takeover-required");

        // Reading still works, and reports why.
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dmxActive"], true);
    }

    #[tokio::test]
    async fn a_live_native_desk_can_configure_an_effect_without_taking_over_dmx() {
        let bench = bench();
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));
        let uri = format!(
            "/api/v2/outputs/{}/layers/0/native-effects/update",
            bench.output
        );

        let (status, body) = send(
            &bench.router,
            post(
                uri.clone(),
                r#"{"effectSlot":0,"effectType":"blur","blurAmount":0.7}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dmxActive"], true);
        assert_eq!(body["playbackTakeover"], false);
        assert_eq!(body["layers"][0]["effects"][0]["effectType"], "blur");

        for payload in [
            r#"{"effectSlot":0,"effectMix":0.7}"#,
            r#"{"effectSlot":0,"effectEnabled":true,"folder":2}"#,
        ] {
            let (status, body) = send(&bench.router, post(uri.clone(), payload)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body["code"], "native-effect-configuration-only");
        }
    }

    #[tokio::test]
    async fn a_reset_still_works_while_a_desk_is_driving() {
        let bench = bench();
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "an administrative action stays available"
        );
    }

    #[tokio::test]
    async fn unknown_outputs_and_layers_are_reported_distinctly() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", OutputId::new())),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-output");

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/9/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-layer");
    }

    #[tokio::test]
    async fn a_malformed_output_id_is_a_bad_request_not_a_not_found() {
        let bench = bench();
        let (status, body) =
            send(&bench.router, get("/api/v2/outputs/nonsense/state".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "malformed-output-id");
    }

    #[tokio::test]
    async fn a_layer_reports_its_mask_even_when_the_mask_is_doing_nothing() {
        let bench = bench();
        let (_, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        let mask = &body["layers"][0]["mask"];
        assert_eq!(mask["address"]["class"], "blank");
        assert_eq!(mask["opacity"], 0.0);
        assert_eq!(mask["source"], "luminance");
        assert_eq!(
            mask["active"], false,
            "selected-but-faded and not-selected must be tellable apart"
        );
    }

    #[tokio::test]
    async fn selecting_a_blank_address_is_allowed_because_it_clears_a_layer() {
        let bench = bench();
        take_over(&bench).await;
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        send(&bench.router, post(uri.clone(), r#"{"folder":1,"file":1}"#)).await;

        let (status, body) = send(&bench.router, post(uri, r#"{"file":0}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["file"], 0);
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].address,
            MediaAddress::new(1, 0)
        );
    }
}
