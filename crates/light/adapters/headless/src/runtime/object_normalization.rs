use super::*;

pub(super) fn normalize_object_body(
    state: &AppState,
    kind: &str,
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let normalized = match kind {
        "patched_fixture" => normalize_fixture(body)?,
        "cue_list" => normalize_cue_list(state, object_id, body)?,
        "group" => normalize_group(object_id, body)?,
        "preset" => normalize_preset(object_id, body)?,
        "playback" => normalize_playback(object_id, body)?,
        "playback_page" => normalize_playback_page(object_id, body)?,
        "route" => normalize_route(body)?,
        _ => return Ok(body),
    };
    Ok(normalized)
}

fn normalize_fixture(body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    light_fixture::migrate_patched_fixture_to_v2(&mut fixture).map_err(ApiError::fixture)?;
    serde_json::to_value(fixture).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_cue_list(
    state: &AppState,
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let mut cue_list = serde_json::from_value::<light_playback::CueList>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if let Ok(id) = Uuid::parse_str(object_id) {
        cue_list.id = light_core::CueListId(id);
    }
    cue_list.migrate_legacy_chaser_xfade(&state.configuration.read().speed_groups_bpm);
    cue_list.validate().map_err(ApiError::bad_request)?;
    serde_json::to_value(cue_list).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_group(
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let mut group = serde_json::from_value::<light_programmer::GroupDefinition>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    group.id = object_id.to_owned();
    serde_json::to_value(group).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_preset(
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let address =
        light_programmer::PresetAddress::parse(object_id).map_err(ApiError::bad_request)?;
    let mut preset = serde_json::from_value::<light_programmer::Preset>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if preset.family != address.family {
        return Err(ApiError::bad_request(
            "preset family must match its pool address",
        ));
    }
    if preset.number != 0 && preset.number != address.number {
        return Err(ApiError::bad_request(
            "preset number must match its pool-local address",
        ));
    }
    preset.number = address.number;
    serde_json::to_value(preset).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_playback(
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let playback = serde_json::from_value::<light_playback::PlaybackDefinition>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if object_id != playback.number.to_string() {
        return Err(ApiError::bad_request(
            "playback object id must match its playback number",
        ));
    }
    playback.validate().map_err(ApiError::bad_request)?;
    serde_json::to_value(playback).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_playback_page(
    object_id: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let page = serde_json::from_value::<light_playback::PlaybackPage>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if object_id != page.number.to_string() {
        return Err(ApiError::bad_request(
            "playback page object id must match its page number",
        ));
    }
    page.validate().map_err(ApiError::bad_request)?;
    serde_json::to_value(page).map_err(|error| ApiError::internal(error.to_string()))
}

fn normalize_route(body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let mut route = serde_json::from_value::<light_output::OutputRoute>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if route.delivery_mode.is_none() {
        route.delivery_mode = Some(route.resolved_delivery_mode());
    }
    route.validate().map_err(ApiError::bad_request)?;
    serde_json::to_value(route).map_err(|error| ApiError::internal(error.to_string()))
}
