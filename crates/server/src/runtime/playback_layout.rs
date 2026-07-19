use super::*;

#[derive(Default, Deserialize)]
pub(super) struct PoolPlaybackInput {
    pub(super) value: Option<f32>,
    pub(super) cue_number: Option<f64>,
    pub(super) pressed: Option<bool>,
    pub(super) button: Option<u8>,
    pub(super) surface: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct PlaybackSlotUpsertInput {
    pub(super) playback: light_playback::PlaybackDefinition,
    #[serde(default)]
    pub(super) expected_playback_revision: u64,
    #[serde(default)]
    pub(super) expected_page_revision: u64,
}

#[derive(Deserialize)]
pub(super) struct PlaybackSlotClearInput {
    pub(super) expected_playback_revision: u64,
    pub(super) expected_page_revision: u64,
}

pub(super) async fn upsert_playback_slot(
    State(state): State<AppState>,
    Path((page_number, slot)): Path<(u8, u8)>,
    headers: HeaderMap,
    Json(input): Json<PlaybackSlotUpsertInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_playback_slot(page_number, slot)?;
    let activation = state.activation_lock.clone().lock_owned().await;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let plan = playback_layout_mutations::plan_playback_slot_upsert(
        &store,
        page_number,
        slot,
        input.playback,
        input.expected_playback_revision,
        input.expected_page_revision,
    )?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http),
        show.id,
        plan.mutations,
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let playback_revision = playback_layout_mutations::changed_revision(
        &result,
        light_application::ActiveShowObjectKind::Playback,
        &plan.number.to_string(),
    );
    let page_revision = playback_layout_mutations::changed_revision(
        &result,
        light_application::ActiveShowObjectKind::PlaybackPage,
        &page_number.to_string(),
    );
    emit(
        &state,
        "playback_slot_changed",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":plan.number}),
    );
    Ok(Json(serde_json::json!({
        "playback": plan.playback,
        "playback_revision": playback_revision,
        "page": plan.page,
        "page_revision": page_revision,
        "event_sequence": result.event_sequence
    })))
}

pub(super) async fn clear_playback_slot(
    State(state): State<AppState>,
    Path((page_number, slot)): Path<(u8, u8)>,
    headers: HeaderMap,
    Json(input): Json<PlaybackSlotClearInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_playback_slot(page_number, slot)?;
    let activation = state.activation_lock.clone().lock_owned().await;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let plan = playback_layout_mutations::plan_playback_slot_clear(
        &store,
        page_number,
        slot,
        input.expected_playback_revision,
        input.expected_page_revision,
    )?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http),
        show.id,
        plan.mutations,
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let page_revisions = result
        .changes
        .iter()
        .filter(|change| change.kind == light_application::ActiveShowObjectKind::PlaybackPage)
        .map(|change| change.object_revision)
        .collect::<Vec<_>>();
    emit(
        &state,
        "playback_slot_cleared",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":plan.number}),
    );
    Ok(Json(serde_json::json!({
        "cleared": true,
        "page": page_number,
        "slot": slot,
        "playback_number": plan.number,
        "page_revisions": page_revisions,
        "event_sequence": result.event_sequence
    })))
}

fn validate_playback_slot(page: u8, slot: u8) -> Result<(), ApiError> {
    if (1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
        && (1..=light_playback::MAX_PAGE_SLOTS).contains(&slot)
    {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "page and slot must each be within 1-127",
        ))
    }
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
        .playback()
        .read()
        .active()
        .into_iter()
        .find(|active| active.playback_number == Some(number));
    Ok(Json(
        serde_json::json!({"playback":definition,"runtime":runtime}),
    ))
}
#[derive(Deserialize)]
pub(super) struct DeskPageInput {
    pub(super) page: u8,
}
#[derive(Deserialize)]
pub(super) struct ControlDeskInput {
    pub(super) name: String,
    pub(super) osc_alias: String,
    pub(super) columns: u8,
    pub(super) rows: u8,
    pub(super) buttons: u8,
    #[serde(default)]
    pub(super) playback_layout: Option<light_show::PlaybackSurfaceLayout>,
}
pub(super) async fn update_control_desk(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ControlDeskInput>,
) -> Result<Json<ControlDesk>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let desk = state
        .desk
        .lock()
        .update_desk(
            id,
            &input.name,
            &input.osc_alias,
            input.columns,
            input.rows,
            input.buttons,
            input.playback_layout,
        )
        .map_err(ApiError::store)?;
    for session in state
        .sessions
        .write()
        .values_mut()
        .filter(|session| session.desk.id == id)
    {
        session.desk = desk.clone();
    }
    emit(
        &state,
        "control_desk_changed",
        serde_json::json!({"desk":desk}),
    );
    Ok(Json(desk))
}

pub(super) async fn update_desk_page(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DeskPageInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    if session.desk.id != id {
        return Err(ApiError::bad_request(
            "session is not attached to this desk",
        ));
    }
    let _activation = state.activation_lock.clone().lock_owned().await;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let event_sequence = {
        let _ordered = state.playback_service.operation_lock();
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        let availability = ensure_playback_page_for_advance(&state, &show, input.page, &context)?;
        if !availability.available() {
            return Err(ApiError::bad_request("playback page does not exist"));
        }
        state
            .desk
            .lock()
            .set_desk_page(id, show.id, input.page)
            .map_err(ApiError::store)?;
        availability.event_sequence()
    };
    emit(
        &state,
        "playback_page_changed",
        serde_json::json!({"desk_id":id,"show_id":show.id,"page":input.page}),
    );
    send_osc_feedback(&state, false);
    Ok(Json(serde_json::json!({
        "desk_id":id,
        "page":input.page,
        "event_sequence":event_sequence
    })))
}
