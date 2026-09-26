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
    Applied, Command, CommandKind, CommandSource, FlipMirror, MasterControls, MasterShaper,
    MediaAddress, MediaState, OutputId, ScalingMode, Timestamp, Tint, apply,
};

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::routes::layer_controls;
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
    let active_configuration = state.active_configuration.load();
    let active = active_configuration.output(id).unwrap_or(found);
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
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };

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
    let active_configuration = state.active_configuration.load();
    let view = OutputConfigurationView::of(
        found,
        active_configuration.output(id).unwrap_or(found),
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
    layer_controls::validate_layer_ranges(&body)?;
    let scaling_mode = body
        .scaling_mode
        .as_deref()
        .map(parse_scaling_mode)
        .transpose()?;
    let effects = super::output_effects::updated_effects(&body, &current, layer)?;
    let visualizer_tuning = visualizer_tuning(&state, &body, &current)?;
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
            controls: Box::new(media_domain::LayerControls {
                visualizer_tuning,
                ..layer_controls::layer_controls(&body, &current, scaling_mode, effects)?
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
        // A negative master scale mirrors the composite along that axis.
        ("scaleX", body.scale_x, -4.0, 4.0),
        ("scaleY", body.scale_y, -4.0, 4.0),
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
                opacity_cycle: body
                    .opacity_cycle_dmx
                    .map(media_domain::BeatRatio::from_dmx),
            }),
        }],
        now,
    )?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

pub(super) fn validate_unit(name: &str, value: Option<f32>) -> Result<(), ApiError> {
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

pub(super) fn validate_range(
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
    submit_to(&state.state, commands, now)
}

fn submit_to(
    state: &arc_swap::ArcSwap<MediaState>,
    commands: Vec<CommandKind>,
    now: Timestamp,
) -> Result<(), ApiError> {
    if commands.is_empty() {
        return Ok(());
    }
    loop {
        let current = state.load_full();
        let mut next = MediaState::clone(&current);
        let mut published = false;

        for kind in &commands {
            let command = Command::new(kind.clone(), CommandSource::Web, now);
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

        if !published {
            return Ok(());
        }
        let previous = state.compare_and_swap(&current, Arc::new(next));
        if Arc::ptr_eq(&previous, &current) {
            return Ok(());
        }
        // Ownership and output existence must be checked again against the winning snapshot.
    }
}

/// The layer's own visualizer tuning edit. It belongs to the layer and the address it shows,
/// never to an effect slot, so the banks stay ordinary effects while a visualizer is shown.
fn visualizer_tuning(
    state: &ApiState,
    body: &UpdateLayer,
    current: &media_domain::LayerState,
) -> Result<Option<Option<media_domain::VisualizerTuning>>, ApiError> {
    if body.reset_visualizer_parameters == Some(true) {
        return Ok(Some(None));
    }
    let Some(parameters) = body.visualizer_parameters else {
        return Ok(None);
    };
    let address = body.address(current.address);
    if state
        .configuration
        .load()
        .visualizers
        .resolve(address)
        .is_none()
    {
        return Err(ApiError::bad_request(
            "visualizer-controls-source",
            "select a generated visualizer on this layer first",
        ));
    }
    Ok(Some(Some(media_domain::VisualizerTuning {
        address,
        parameters: parameters.into_parameters(),
    })))
}

fn view_of(state: &ApiState, output: &media_domain::OutputState, now: Timestamp) -> OutputView {
    let configuration = state.configuration.load();
    let name = configuration
        .output(output.id)
        .map(|configured| configured.name.to_string())
        .unwrap_or_else(|| output.id.to_string());

    let mut view = OutputView::of(
        output,
        name,
        output.ownership.dmx_is_active(now),
        configuration.playback.frame_rate,
    );
    for (layer_view, layer) in view.layers.iter_mut().zip(&output.layers) {
        if let Some(visualizer) = configuration.visualizers.resolve(layer.address) {
            let parameters = media_domain::VisualizerTuning::resolve(
                layer.visualizer_tuning.as_ref(),
                layer.address,
                &visualizer.parameters,
            );
            layer_view.visualizer_channels = crate::wire::VisualizerChannelView::all(
                visualizer.kind,
                parameters,
                &layer.visualizer_controls,
            );
        }
    }
    if output.layers.iter().any(|layer| !layer.model.is_flat()) {
        let failures = (state.diagnostics.models.failures)();
        for (layer_view, layer) in view.layers.iter_mut().zip(&output.layers) {
            layer_view.model_status =
                media_domain::ModelStatus::of(layer.model, &configuration.models, |slot| {
                    failures.iter().any(|(failed, _)| *failed == slot)
                })
                .label()
                .to_owned();
        }
    }
    view
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
#[path = "outputs_tests.rs"]
mod tests;
