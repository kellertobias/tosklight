use super::*;

#[derive(Default, Deserialize)]
pub(super) struct PoolPlaybackInput {
    pub(super) value: Option<f32>,
    pub(super) cue_number: Option<f64>,
    pub(super) pressed: Option<bool>,
    pub(super) button: Option<u8>,
    pub(super) surface: Option<String>,
}

pub(super) async fn pool_playback_state(
    State(state): State<AppState>,
    Path(number): Path<u16>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let definition = snapshot
        .playbacks
        .iter()
        .find(|playback| playback.number == number)
        .ok_or_else(|| ApiError::not_found("playback"))?;
    let runtime = state
        .engine
        .active_playbacks()
        .into_iter()
        .find(|active| active.playback_number == Some(number));
    Ok(Json(
        serde_json::json!({"playback":definition,"runtime":runtime}),
    ))
}
