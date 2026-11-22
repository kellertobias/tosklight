//! Durable automation edits and explicit live actions share the existing edit and control paths.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_application::configuration::validate_automation;
use media_domain::OutputId;
use media_domain::playlist::{AutomationOutput, Playlist, ScheduleEvent};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use super::ApiState;
use super::edit::{self, Proceed};
use crate::error::ApiError;
use crate::tolerant::TolerantJson;
use crate::wire::{
    AutomationConfigurationView, AutomationOutputEdit, AutomationView, DeleteAutomationObject,
    PlaylistEdit, ScheduleEventEdit, SetAutomationEnabled,
};

fn view(state: &ApiState, configuration: &MediaConfiguration) -> AutomationView {
    AutomationView {
        configuration: AutomationConfigurationView::of(&configuration.automation),
        runtime: (**state.diagnostics.automation.snapshot.load()).clone(),
    }
}

pub(super) async fn automation(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(view(&state, &state.configuration.load()))
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::bad_request("automation-invalid", message)
}

fn identity(value: &str, field: &str) -> Result<Uuid, ApiError> {
    let id = Uuid::parse_str(value).map_err(|_| invalid(format!("{field} must be a UUID")))?;
    if id.is_nil() {
        return Err(invalid(format!("{field} must not be empty")));
    }
    Ok(id)
}

fn convert<T: DeserializeOwned>(value: impl Serialize) -> Result<T, ApiError> {
    let value = serde_json::to_value(value)
        .map_err(|_| invalid("automation values cannot be serialized"))?;
    serde_json::from_value(value).map_err(|error| invalid(error.to_string()))
}

fn commit(
    state: &ApiState,
    configuration: MediaConfiguration,
    request_id: &str,
) -> Result<Response, ApiError> {
    validate_automation(&configuration).map_err(invalid)?;
    let result = view(state, &configuration);
    edit::commit(state, configuration, request_id, &result)
}

fn edit_playlist(playlist: &mut Playlist, body: &PlaylistEdit) -> Result<(), ApiError> {
    if let Some(name) = &body.name {
        playlist.name = name.trim().to_owned();
    }
    if let Some(enabled) = body.enabled {
        playlist.enabled = enabled;
    }
    if let Some(priority) = body.priority {
        playlist.priority = priority;
    }
    if let Some(layers) = body.layers {
        playlist.layers = layers;
    }
    if let Some(level) = body.master_dimmer {
        playlist.master_dimmer = level;
    }
    if let Some(volume) = body.master_volume {
        playlist.master_volume = volume;
    }
    if let Some(fade) = body.default_fade_millis {
        playlist.default_fade_millis = fade;
    }
    if let Some(entries) = &body.entries {
        playlist.entries = convert(entries)?;
    }
    if let Some(auxiliary) = &body.auxiliary_layers {
        playlist.auxiliary_layers = convert(auxiliary)?;
    }
    Ok(())
}

pub(super) async fn create_playlist(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<PlaylistEdit>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let mut playlist = Playlist {
        enabled: false,
        ..Playlist::default()
    };
    edit_playlist(&mut playlist, &body)?;
    configuration.automation.playlists.push(playlist);
    commit(&state, configuration, &body.request_id)
}

pub(super) async fn update_playlist(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<PlaylistEdit>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = identity(&id, "playlistId")?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let playlist = configuration
        .automation
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| ApiError::not_found("playlist-not-found", "the playlist does not exist"))?;
    edit_playlist(playlist, &body)?;
    commit(&state, configuration, &body.request_id)
}

pub(super) async fn delete_playlist(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<DeleteAutomationObject>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = identity(&id, "playlistId")?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    if !configuration
        .automation
        .playlists
        .iter()
        .any(|playlist| playlist.id == id)
    {
        return Err(ApiError::not_found(
            "playlist-not-found",
            "the playlist does not exist",
        ));
    }
    if configuration
        .automation
        .events
        .iter()
        .any(|event| event.playlist_id == id)
        || configuration
            .automation
            .outputs
            .iter()
            .any(|output| output.fallback_playlist_id == Some(id))
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "playlist-in-use",
            "remove or reassign this playlist's scheduled events and fallback references before deleting it",
        ));
    }
    configuration
        .automation
        .playlists
        .retain(|playlist| playlist.id != id);
    commit(&state, configuration, &body.request_id)
}

fn edit_event(event: &mut ScheduleEvent, body: &ScheduleEventEdit) -> Result<(), ApiError> {
    if let Some(name) = &body.name {
        event.name = name.trim().to_owned();
    }
    if let Some(enabled) = body.enabled {
        event.enabled = enabled;
    }
    if let Some(id) = &body.output_id {
        event.output_id = OutputId::from_uuid(identity(id, "outputId")?);
    }
    if let Some(id) = &body.playlist_id {
        event.playlist_id = identity(id, "playlistId")?;
    }
    if let Some(trigger) = &body.trigger {
        event.trigger = convert(trigger)?;
    }
    Ok(())
}

pub(super) async fn create_event(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<ScheduleEventEdit>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let output = body
        .output_id
        .as_deref()
        .ok_or_else(|| invalid("outputId is required"))?;
    let playlist = body
        .playlist_id
        .as_deref()
        .ok_or_else(|| invalid("playlistId is required"))?;
    let trigger = body
        .trigger
        .as_ref()
        .ok_or_else(|| invalid("trigger is required"))?;
    let mut event = ScheduleEvent {
        id: Uuid::new_v4(),
        name: "Event".into(),
        enabled: false,
        output_id: OutputId::from_uuid(identity(output, "outputId")?),
        playlist_id: identity(playlist, "playlistId")?,
        trigger: convert(trigger)?,
    };
    edit_event(&mut event, &body)?;
    configuration.automation.events.push(event);
    commit(&state, configuration, &body.request_id)
}

pub(super) async fn update_event(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<ScheduleEventEdit>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = identity(&id, "eventId")?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let event = configuration
        .automation
        .events
        .iter_mut()
        .find(|event| event.id == id)
        .ok_or_else(|| {
            ApiError::not_found("event-not-found", "the scheduled event does not exist")
        })?;
    edit_event(event, &body)?;
    commit(&state, configuration, &body.request_id)
}

pub(super) async fn delete_event(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<DeleteAutomationObject>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let id = identity(&id, "eventId")?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    if !configuration
        .automation
        .events
        .iter()
        .any(|event| event.id == id)
    {
        return Err(ApiError::not_found(
            "event-not-found",
            "the scheduled event does not exist",
        ));
    }
    configuration
        .automation
        .events
        .retain(|event| event.id != id);
    commit(&state, configuration, &body.request_id)
}

fn output_id(state: &ApiState, value: &str) -> Result<OutputId, ApiError> {
    let id = OutputId::from_uuid(identity(value, "outputId")?);
    if state.configuration.load().output(id).is_none() {
        return Err(ApiError::not_found(
            "output-not-found",
            "the output does not exist",
        ));
    }
    Ok(id)
}

fn settings(configuration: &mut MediaConfiguration, output_id: OutputId) -> &mut AutomationOutput {
    let index = configuration
        .automation
        .outputs
        .iter()
        .position(|output| output.output_id == output_id)
        .unwrap_or_else(|| {
            configuration.automation.outputs.push(AutomationOutput {
                output_id,
                enabled: false,
                fallback_playlist_id: None,
                dmx_silence_seconds: 10,
            });
            configuration.automation.outputs.len() - 1
        });
    &mut configuration.automation.outputs[index]
}

pub(super) async fn update_output(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    TolerantJson(body): TolerantJson<AutomationOutputEdit>,
) -> Result<Response, ApiError> {
    let _guard = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let output = output_id(&state, &id)?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let settings = settings(&mut configuration, output);
    if let Some(fallback) = body.fallback_playlist_id {
        settings.fallback_playlist_id = fallback
            .map(|value| identity(&value, "fallbackPlaylistId"))
            .transpose()?;
    }
    if let Some(seconds) = body.dmx_silence_seconds {
        settings.dmx_silence_seconds = seconds;
    }
    commit(&state, configuration, &body.request_id)
}

/// Canonical payload action shared with the established telemetry WebSocket. Actions have no
/// request identity and are never replayed on reconnect.
pub(super) async fn set_enabled_action(
    state: &ApiState,
    output: &str,
    enabled: bool,
) -> Result<(), ApiError> {
    let _transaction = state.replays.transaction().await;
    let id = output_id(state, output)?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    if enabled
        && !configuration
            .output(id)
            .is_some_and(|output| output.enabled)
    {
        return Err(invalid(
            "the output must be enabled before scheduling can start",
        ));
    }
    settings(&mut configuration, id).enabled = enabled;
    validate_automation(&configuration).map_err(invalid)?;
    edit::store(state, configuration)
}

pub(super) async fn set_enabled(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    TolerantJson(body): TolerantJson<SetAutomationEnabled>,
) -> Result<Response, ApiError> {
    set_enabled_action(&state, &output, body.enabled).await?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(view(&state, &state.configuration.load())),
    )
        .into_response())
}

pub(super) fn apply_now_action(state: &ApiState, output: &str) -> Result<(), ApiError> {
    let id = output_id(state, output)?;
    let configuration = state.configuration.load();
    if !configuration
        .automation
        .outputs
        .iter()
        .any(|output| output.output_id == id && output.enabled)
    {
        return Err(invalid(
            "start scheduling on this output before applying playlist edits",
        ));
    }
    let active = state.state.load();
    let output = active
        .output(id)
        .ok_or_else(|| invalid("this output is not running"))?;
    if output.ownership.web_takeover {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "automation-manual-control",
            "release manual playback takeover before applying a playlist",
        ));
    }
    let silence = configuration
        .automation
        .outputs
        .iter()
        .find(|output| output.output_id == id)
        .map_or(10_000, |output| output.dmx_silence_seconds * 1000);
    if output.ownership.dmx.is_some_and(|activity| {
        (state.now)().since(activity.last_seen).as_millis() < u128::from(silence)
    }) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "automation-dmx-control",
            "DMX is controlling this output; wait for the configured silence timeout",
        ));
    }
    state
        .diagnostics
        .automation
        .apply_now
        .lock()
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "automation-unavailable",
                "the playlist runner is unavailable",
            )
        })?
        .insert(id);
    Ok(())
}

pub(super) async fn apply_now(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    apply_now_action(&state, &output)?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(view(&state, &state.configuration.load())),
    )
        .into_response())
}

#[cfg(test)]
mod tests;
