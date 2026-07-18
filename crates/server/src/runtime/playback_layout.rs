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
    if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page_number)
        || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot)
    {
        return Err(ApiError::bad_request(
            "page and slot must each be within 1-127",
        ));
    }
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let playback_objects = store.objects("playback").map_err(ApiError::store)?;
    let page_objects = store.objects("playback_page").map_err(ApiError::store)?;
    let stored_page = page_objects
        .iter()
        .find(|object| object.id == page_number.to_string());
    let mut page = stored_page
        .map(|object| {
            serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))
        })
        .transpose()?
        .unwrap_or(light_playback::PlaybackPage {
            number: page_number,
            name: format!("Page {page_number}"),
            slots: HashMap::new(),
        });
    let current_page_revision = stored_page.map_or(0, |object| object.revision);
    if current_page_revision != input.expected_page_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_page_revision,
            current: current_page_revision,
        }));
    }
    let existing_number = page.slots.get(&slot).copied();
    let number = if let Some(number) = existing_number {
        number
    } else {
        let used = playback_objects
            .iter()
            .filter_map(|object| object.id.parse::<u16>().ok())
            .collect::<std::collections::HashSet<_>>();
        (1..=light_playback::MAX_PLAYBACKS)
            .find(|number| !used.contains(number))
            .ok_or_else(|| ApiError::bad_request("playback pool is full"))?
    };
    let existing_playback = playback_objects
        .iter()
        .find(|object| object.id == number.to_string());
    let current_playback_revision = existing_playback.map_or(0, |object| object.revision);
    if current_playback_revision != input.expected_playback_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_playback_revision,
            current: current_playback_revision,
        }));
    }
    let mut playback = input.playback;
    playback.number = number;
    playback.validate().map_err(ApiError::bad_request)?;
    page.number = page_number;
    page.slots.insert(slot, number);
    page.validate().map_err(ApiError::bad_request)?;

    let mut candidate = (*state.engine.snapshot()).clone();
    candidate
        .playbacks
        .retain(|definition| definition.number != number);
    candidate.playbacks.push(playback.clone());
    if let Some(candidate_page) = candidate
        .playback_pages
        .iter_mut()
        .find(|candidate| candidate.number == page_number)
    {
        *candidate_page = page.clone();
    } else {
        candidate.playback_pages.push(page.clone());
    }
    state
        .engine
        .validate_snapshot_for_runtime(&candidate)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let playback_body =
        serde_json::to_value(&playback).map_err(|error| ApiError::internal(error.to_string()))?;
    let page_body =
        serde_json::to_value(&page).map_err(|error| ApiError::internal(error.to_string()))?;
    let playback_id = number.to_string();
    let page_id = page_number.to_string();
    backup_show(&state, &show)?;
    let revisions = store
        .mutate_objects_atomically(
            &[
                AtomicObjectWrite {
                    kind: "playback",
                    id: &playback_id,
                    body: &playback_body,
                    expected: current_playback_revision,
                },
                AtomicObjectWrite {
                    kind: "playback_page",
                    id: &page_id,
                    body: &page_body,
                    expected: current_page_revision,
                },
            ],
            &[],
        )
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        &state,
        "playback_slot_changed",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":number}),
    );
    Ok(Json(serde_json::json!({
        "playback": playback,
        "playback_revision": revisions[0],
        "page": page,
        "page_revision": revisions[1]
    })))
}

pub(super) async fn clear_playback_slot(
    State(state): State<AppState>,
    Path((page_number, slot)): Path<(u8, u8)>,
    headers: HeaderMap,
    Json(input): Json<PlaybackSlotClearInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let store = ShowStore::open(&show.path).map_err(ApiError::store)?;
    let playback_objects = store.objects("playback").map_err(ApiError::store)?;
    let page_objects = store.objects("playback_page").map_err(ApiError::store)?;
    let primary_page = page_objects
        .iter()
        .find(|object| object.id == page_number.to_string())
        .ok_or_else(|| ApiError::not_found("playback page"))?;
    if primary_page.revision != input.expected_page_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_page_revision,
            current: primary_page.revision,
        }));
    }
    let primary_definition: light_playback::PlaybackPage =
        serde_json::from_value(primary_page.body.clone())
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let number = primary_definition
        .slots
        .get(&slot)
        .copied()
        .ok_or_else(|| ApiError::not_found("paged playback"))?;
    let playback_object = playback_objects
        .iter()
        .find(|object| object.id == number.to_string())
        .ok_or_else(|| ApiError::not_found("playback"))?;
    if playback_object.revision != input.expected_playback_revision {
        return Err(ApiError::store(light_show::StoreError::RevisionConflict {
            expected: input.expected_playback_revision,
            current: playback_object.revision,
        }));
    }

    let mut page_updates = Vec::new();
    for object in page_objects {
        let mut definition: light_playback::PlaybackPage =
            serde_json::from_value(object.body.clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let before = definition.slots.len();
        definition.slots.retain(|_, playback| *playback != number);
        if definition.slots.len() != before {
            page_updates.push((
                object.id,
                serde_json::to_value(definition)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
                object.revision,
            ));
        }
    }
    let writes = page_updates
        .iter()
        .map(|(id, body, expected)| AtomicObjectWrite {
            kind: "playback_page",
            id,
            body,
            expected: *expected,
        })
        .collect::<Vec<_>>();
    let playback_id = number.to_string();
    let deletes = [AtomicObjectDelete {
        kind: "playback",
        id: &playback_id,
        expected: playback_object.revision,
    }];
    backup_show(&state, &show)?;
    let revisions = store
        .mutate_objects_atomically(&writes, &deletes)
        .map_err(ApiError::store)?;
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&show).map_err(ApiError::internal)?)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    emit(
        &state,
        "playback_slot_cleared",
        serde_json::json!({"session_id":session.id,"page":page_number,"slot":slot,"playback_number":number}),
    );
    Ok(Json(serde_json::json!({
        "cleared": true,
        "page": page_number,
        "slot": slot,
        "playback_number": number,
        "page_revisions": revisions
    })))
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
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    {
        let _ordered = state.playback_service.operation_lock();
        if !ensure_playback_page_for_advance(&state, &show, input.page)? {
            return Err(ApiError::bad_request("playback page does not exist"));
        }
        state
            .desk
            .lock()
            .set_desk_page(id, show.id, input.page)
            .map_err(ApiError::store)?;
    }
    emit(
        &state,
        "playback_page_changed",
        serde_json::json!({"desk_id":id,"show_id":show.id,"page":input.page}),
    );
    send_osc_feedback(&state, false);
    Ok(Json(serde_json::json!({"desk_id":id,"page":input.page})))
}
