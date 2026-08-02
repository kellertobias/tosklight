//! Authenticated Schedule snapshots, previews, and idempotent show-object intents.

use super::show_objects_v2::{active_entry, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use chrono::{DateTime, Local, SecondsFormat, Utc};
use light_show::{
    ScheduleOccurrenceClaim, ScheduleOccurrenceClaimResult, ScheduleOccurrenceRecord,
    ScheduleOccurrenceResolution, ScheduleOccurrenceStatus as StoredOccurrenceStatus,
};
use light_wire::v2::schedules as wire;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/schedules", get(snapshot))
        .route("/api/v2/schedules/preview", post(preview))
        .route("/api/v2/schedules/create", post(create))
        .route("/api/v2/schedules/{id}/update", post(update))
        .route("/api/v2/schedules/{id}/duplicate", post(duplicate))
        .route("/api/v2/schedules/{id}/delete", post(remove))
}

pub(super) async fn run_scheduler(
    state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<()> {
    interrupt_abandoned_claims(&state);
    let mut interval = tokio::time::interval(Duration::from_millis(250));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut active_show = None;
    let mut checked_after = Utc::now();
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            _ = interval.tick() => {
                let now = Utc::now();
                let current = state.active_show.current();
                let current_id = current.as_ref().map(|show| show.id);
                if current_id != active_show {
                    active_show = current_id;
                    checked_after = now;
                    continue;
                }
                let Some(show) = current else {
                    checked_after = now;
                    continue;
                };
                // A large forward wall-clock step is missed-time, not a catch-up window.
                if now.signed_duration_since(checked_after) > chrono::Duration::seconds(5) {
                    checked_after = now;
                    continue;
                }
                if let Err(error) = run_due_occurrences(&state, &show, checked_after, now).await {
                    tracing::error!(error = %error.message, "Schedule evaluation failed");
                }
                checked_after = now;
            }
        }
    }
}

fn interrupt_abandoned_claims(state: &AppState) {
    let Some(show) = state.active_show.current() else {
        return;
    };
    match ActiveShowRepository::open(&show.path).and_then(|store| {
        store.interrupt_claimed_schedule_occurrences(Utc::now(), "server restarted")
    }) {
        Ok(0) => {}
        Ok(count) => tracing::warn!(count, "interrupted abandoned Schedule claims"),
        Err(error) => tracing::error!(%error, "failed to interrupt abandoned Schedule claims"),
    }
}

async fn run_due_occurrences(
    state: &AppState,
    show: &ShowEntry,
    after: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), ApiError> {
    let store = ActiveShowRepository::open(&show.path).map_err(ApiError::store)?;
    let (_, objects) = store
        .objects_with_portable_revision("schedule")
        .map_err(ApiError::store)?;
    let recurrence = recurrence()?;
    for object in objects {
        let definition: light_application::ScheduleDefinition =
            decode(object.body, "stored Schedule")?;
        if !definition.enabled {
            continue;
        }
        let occurrences = match recurrence.preview(&definition, after, 2) {
            Ok(occurrences) => occurrences,
            Err(error) => {
                emit_schedule_runtime(
                    state,
                    show.id,
                    definition.id.0,
                    None,
                    Some(error.to_string()),
                );
                continue;
            }
        };
        for occurrence in occurrences
            .into_iter()
            .take_while(|occurrence| occurrence.scheduled_at <= now)
        {
            execute_occurrence(state, show, object.revision, &definition, occurrence).await?;
        }
    }
    Ok(())
}

async fn execute_occurrence(
    state: &AppState,
    show: &ShowEntry,
    object_revision: u64,
    definition: &light_application::ScheduleDefinition,
    occurrence: light_application::ScheduleOccurrence,
) -> Result<(), ApiError> {
    let _activation = state.active_show.acquire().await;
    if state.active_show.current().as_ref().map(|active| active.id) != Some(show.id) {
        return Ok(());
    }
    let store = ActiveShowRepository::open(&show.path).map_err(ApiError::store)?;
    let occurrence_id = occurrence_id(&occurrence.identity.key);
    let target_action = serde_json::json!({
        "target": definition.target,
        "local_time": occurrence.local.format("%Y-%m-%dT%H:%M:%S").to_string(),
    });
    let claim = store
        .claim_schedule_occurrence(&ScheduleOccurrenceClaim {
            schedule_id: definition.id.0.to_string(),
            occurrence_id: occurrence_id.clone(),
            scheduled_for: occurrence.scheduled_at,
            target_action,
            claimed_at: Utc::now(),
        })
        .map_err(ApiError::store)?;
    if matches!(claim, ScheduleOccurrenceClaimResult::AlreadyRecorded(_)) {
        return Ok(());
    }
    let result = dispatch_schedule_target(state, show, definition);
    let (resolution, detail) = match result {
        Ok(()) => (ScheduleOccurrenceResolution::Completed, None),
        Err(error) => {
            let message = error.message;
            (
                ScheduleOccurrenceResolution::Failed {
                    reason: message.clone(),
                },
                Some(message),
            )
        }
    };
    let recorded = store
        .resolve_schedule_occurrence(
            &definition.id.0.to_string(),
            &occurrence_id,
            resolution,
            Utc::now(),
        )
        .map_err(ApiError::store)?;
    if detail.is_none()
        && matches!(
            &definition.trigger,
            light_application::ScheduleTrigger::OneTime { .. }
        )
    {
        disable_completed_one_time(state, show.id, object_revision, definition)?;
    }
    emit_schedule_runtime(
        state,
        show.id,
        definition.id.0,
        Some(stored_result(recorded)?),
        detail,
    );
    Ok(())
}

fn dispatch_schedule_target(
    state: &AppState,
    show: &ShowEntry,
    definition: &light_application::ScheduleDefinition,
) -> Result<(), ApiError> {
    let light_application::ScheduleTarget::Playback {
        page,
        slot,
        playback_number,
        action,
        master_transition,
    } = &definition.target
    else {
        return Err(ApiError::conflict(
            "Macro runtime is unavailable until refactoring item 30",
        ));
    };
    let (page, slot) = (
        u8::try_from(*page).map_err(|_| ApiError::bad_request("Schedule page is out of range"))?,
        u8::try_from(*slot).map_err(|_| ApiError::bad_request("Schedule slot is out of range"))?,
    );
    let store = ActiveShowRepository::open(&show.path).map_err(ApiError::store)?;
    let (_, page_object) = store
        .object_with_portable_revision("playback_page", &page.to_string())
        .map_err(ApiError::store)?;
    let page_definition: light_playback::PlaybackPage = decode(
        page_object
            .ok_or_else(|| ApiError::conflict("scheduled Playback page no longer exists"))?
            .body,
        "Playback page",
    )?;
    if page_definition.slots.get(&slot) != Some(playback_number) {
        return Err(ApiError::conflict(
            "scheduled Playback moved, was replaced, or no longer occupies its saved page slot",
        ));
    }
    let (_, playback_object) = store
        .object_with_portable_revision("playback", &playback_number.to_string())
        .map_err(ApiError::store)?;
    let playback: light_playback::PlaybackDefinition = decode(
        playback_object
            .ok_or_else(|| ApiError::conflict("scheduled Playback no longer exists"))?
            .body,
        "Playback",
    )?;
    validate_action_matrix(&playback.target, *action)?;
    let context = light_application::ActionContext::system(
        Uuid::nil(),
        light_application::ActionSource::Scheduler,
    );
    playback_service::execute(
        state,
        None,
        None,
        context.clone(),
        light_application::PlaybackCommand {
            address: light_application::PlaybackAddress::Pool(*playback_number),
            action: scheduled_action(state, &playback.target, *action)?,
            surface: light_application::PlaybackSurface::Virtual,
        },
    )?;
    if let Some(transition) = master_transition {
        playback_service::execute(
            state,
            None,
            None,
            context,
            light_application::PlaybackCommand {
                address: light_application::PlaybackAddress::Pool(*playback_number),
                action: light_application::PlaybackAction::MasterTransition {
                    level: light_application::PlaybackLevel::new(transition.level),
                    duration_millis: transition.fade_millis,
                },
                surface: light_application::PlaybackSurface::Virtual,
            },
        )?;
    }
    Ok(())
}

fn validate_action_matrix(
    target: &light_playback::PlaybackTarget,
    action: light_application::ScheduledPlaybackAction,
) -> Result<(), ApiError> {
    use light_application::ScheduledPlaybackAction::{Go, Pause};
    match target {
        light_playback::PlaybackTarget::CueList { .. }
        | light_playback::PlaybackTarget::Dynamic { .. } => Ok(()),
        light_playback::PlaybackTarget::Group { .. } if !matches!(action, Go | Pause) => Ok(()),
        light_playback::PlaybackTarget::Group { .. } => Err(ApiError::conflict(
            "Group Playbacks do not support scheduled Go or Pause",
        )),
        _ => Err(ApiError::conflict(
            "this Playback type cannot be used by a Schedule",
        )),
    }
}

fn scheduled_action(
    state: &AppState,
    target: &light_playback::PlaybackTarget,
    action: light_application::ScheduledPlaybackAction,
) -> Result<light_application::PlaybackAction, ApiError> {
    if let light_playback::PlaybackTarget::Group { group_id, .. } = target {
        let level = match action {
            light_application::ScheduledPlaybackAction::On => 1.0,
            light_application::ScheduledPlaybackAction::Off
            | light_application::ScheduledPlaybackAction::Release => 0.0,
            light_application::ScheduledPlaybackAction::Toggle => {
                if state.output.group_master(group_id).unwrap_or(0.0) > 0.0 {
                    0.0
                } else {
                    1.0
                }
            }
            light_application::ScheduledPlaybackAction::Go
            | light_application::ScheduledPlaybackAction::Pause => {
                return Err(ApiError::conflict(
                    "Group Playbacks do not support scheduled Go or Pause",
                ));
            }
        };
        return Ok(light_application::PlaybackAction::Master(
            light_application::PlaybackLevel::new(level),
        ));
    }
    Ok(match action {
        light_application::ScheduledPlaybackAction::Go => {
            light_application::PlaybackAction::Go { pressed: true }
        }
        light_application::ScheduledPlaybackAction::Pause => {
            light_application::PlaybackAction::Pause { pressed: true }
        }
        light_application::ScheduledPlaybackAction::On => {
            light_application::PlaybackAction::On { pressed: true }
        }
        light_application::ScheduledPlaybackAction::Off => {
            light_application::PlaybackAction::Off { pressed: true }
        }
        light_application::ScheduledPlaybackAction::Release => {
            light_application::PlaybackAction::Release
        }
        light_application::ScheduledPlaybackAction::Toggle => {
            light_application::PlaybackAction::Toggle { pressed: true }
        }
    })
}

fn disable_completed_one_time(
    state: &AppState,
    show_id: light_core::ShowId,
    object_revision: u64,
    definition: &light_application::ScheduleDefinition,
) -> Result<(), ApiError> {
    let mut disabled = definition.clone();
    disabled.enabled = false;
    let action = active_show_object_action(
        light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::Scheduler,
        ),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Schedule,
            definition.id.0.to_string(),
            object_revision,
            serde_json::to_value(disabled)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    run_active_show_object_action(state, action)?;
    Ok(())
}

fn emit_schedule_runtime(
    state: &AppState,
    show_id: light_core::ShowId,
    schedule_id: Uuid,
    last_result: Option<wire::ScheduleOccurrenceResult>,
    validation_error: Option<String>,
) {
    let projection = schedule_snapshot(state, show_id, Utc::now())
        .ok()
        .and_then(|snapshot| {
            snapshot
                .schedules
                .into_iter()
                .find(|schedule| schedule.definition.id == schedule_id)
        });
    let next_occurrence = projection
        .as_ref()
        .and_then(|schedule| schedule.next_occurrence.clone());
    let last_result = last_result.or_else(|| {
        projection
            .as_ref()
            .and_then(|schedule| schedule.last_result.clone())
    });
    let validation_error = validation_error.or_else(|| {
        projection
            .as_ref()
            .and_then(|schedule| schedule.validation_error.clone())
    });
    emit(
        state,
        "schedule_runtime_changed",
        serde_json::json!({
            "show_id": show_id,
            "schedule_id": schedule_id,
            "next_occurrence": next_occurrence,
            "last_result": last_result,
            "validation_error": validation_error,
        }),
    );
    state
        .events
        .publish(light_application::EventDraft::schedule_runtime_changed(
            application_runtime_change(
                show_id,
                schedule_id,
                next_occurrence,
                last_result,
                validation_error,
            ),
        ));
}

fn application_runtime_change(
    show_id: light_core::ShowId,
    schedule_id: Uuid,
    next_occurrence: Option<wire::ScheduleOccurrenceProjection>,
    last_result: Option<wire::ScheduleOccurrenceResult>,
    validation_error: Option<String>,
) -> light_application::ScheduleRuntimeChange {
    light_application::ScheduleRuntimeChange {
        show_id,
        schedule_id,
        next_occurrence: next_occurrence.map(application_occurrence),
        last_result: last_result.map(|result| light_application::ScheduleOccurrenceResult {
            occurrence: application_occurrence(result.occurrence),
            status: match result.status {
                wire::ScheduleOccurrenceStatus::Claimed => {
                    light_application::ScheduleOccurrenceStatus::Claimed
                }
                wire::ScheduleOccurrenceStatus::Completed => {
                    light_application::ScheduleOccurrenceStatus::Completed
                }
                wire::ScheduleOccurrenceStatus::Failed => {
                    light_application::ScheduleOccurrenceStatus::Failed
                }
                wire::ScheduleOccurrenceStatus::Skipped => {
                    light_application::ScheduleOccurrenceStatus::Skipped
                }
                wire::ScheduleOccurrenceStatus::Interrupted => {
                    light_application::ScheduleOccurrenceStatus::Interrupted
                }
            },
            recorded_at: result.recorded_at,
            message: result.message,
        }),
        validation_error,
    }
}

fn application_occurrence(
    occurrence: wire::ScheduleOccurrenceProjection,
) -> light_application::ScheduleOccurrenceProjection {
    light_application::ScheduleOccurrenceProjection {
        occurrence_id: occurrence.occurrence_id,
        scheduled_for: occurrence.scheduled_for,
        local_time: occurrence.local_time,
    }
}

async fn snapshot(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::ScheduleSnapshot>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    Ok(Json(schedule_snapshot(&state, show_id, Utc::now())?))
}

async fn preview(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::SchedulePreviewRequest>,
) -> Result<Json<wire::SchedulePreview>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let _show_id = context.resolve(&state)?;
    let now = Utc::now();
    let recurrence = recurrence()?;
    let definition = light_application::ScheduleDefinition {
        id: light_application::ScheduleId::new(),
        name: "Preview".into(),
        enabled: false,
        trigger: decode(request.trigger, "Schedule trigger")?,
        target: preview_target(),
    };
    let occurrences = recurrence
        .preview(&definition, now, usize::from(request.count))
        .map_err(schedule_validation)?
        .iter()
        .map(occurrence_projection)
        .collect();
    Ok(Json(wire::SchedulePreview {
        timezone: recurrence.timezone_name().into(),
        server_now: timestamp(now),
        occurrences,
    }))
}

async fn create(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScheduleCreateRequest>,
) -> Result<Json<wire::ScheduleMutationOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let action = ReplayAction::Create(request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state.replay.lookup_schedule(&key, &action).await? {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state.replay.lookup_schedule(&key, &action).await? {
        return Ok(Json(outcome));
    }
    let now = Utc::now();
    let id = Uuid::new_v4();
    let mut definition = light_application::ScheduleDefinition {
        id: light_application::ScheduleId(id),
        name: request.definition.name,
        enabled: request.definition.enabled,
        trigger: decode(request.definition.trigger, "Schedule trigger")?,
        target: decode(request.definition.target, "Schedule target")?,
    };
    normalize_new_anchor(&mut definition, now);
    recurrence()?
        .validate(&definition, now, false)
        .map_err(schedule_validation)?;
    if definition.enabled {
        validate_persisted_target(&state, show_id, &definition.target)?;
    }
    let action_envelope = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Schedule,
            id.to_string(),
            0,
            serde_json::to_value(&definition)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action_envelope).await?;
    let outcome = mutation_outcome(
        &state,
        show_id,
        Some(id),
        request.request_id,
        result.show_revision.value(),
        result.event_sequence,
    )?;
    state
        .replay
        .insert_schedule(key, action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScheduleUpdateRequest>,
) -> Result<Json<wire::ScheduleMutationOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Update(id, request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let (revision, mut definition) = load_schedule(&state, show_id, id)?;
    if revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Schedule revision conflict: expected {}, current {revision}",
            request.expected_revision
        )));
    }
    let was_enabled = definition.enabled;
    let interval_changed = request.patch.trigger.as_ref().is_some_and(|trigger| {
        let Ok(trigger) =
            decode::<_, light_application::ScheduleTrigger>(trigger.clone(), "Schedule trigger")
        else {
            return false;
        };
        match (&definition.trigger, trigger) {
            (
                light_application::ScheduleTrigger::Interval {
                    every_seconds: before,
                    ..
                },
                light_application::ScheduleTrigger::Interval {
                    every_seconds: after,
                    ..
                },
            ) => *before != after,
            _ => true,
        }
    });
    if let Some(name) = request.patch.name {
        definition.name = name;
    }
    if let Some(enabled) = request.patch.enabled {
        definition.enabled = enabled;
    }
    if let Some(trigger) = request.patch.trigger {
        definition.trigger = decode(trigger, "Schedule trigger")?;
    }
    if let Some(target) = request.patch.target {
        definition.target = decode(target, "Schedule target")?;
    }
    let now = Utc::now();
    if definition.enabled && (!was_enabled || interval_changed) {
        normalize_new_anchor(&mut definition, now);
    }
    recurrence()?
        .validate(&definition, now, false)
        .map_err(schedule_validation)?;
    if definition.enabled {
        validate_persisted_target(&state, show_id, &definition.target)?;
    }
    let action_envelope = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Schedule,
            id.to_string(),
            revision,
            serde_json::to_value(&definition)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action_envelope).await?;
    let outcome = mutation_outcome(
        &state,
        show_id,
        Some(id),
        request.request_id,
        result.show_revision.value(),
        result.event_sequence,
    )?;
    state
        .replay
        .insert_schedule(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn duplicate(
    State(state): State<AppState>,
    Path(source_id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScheduleDuplicateRequest>,
) -> Result<Json<wire::ScheduleMutationOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Duplicate(source_id, request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let (revision, mut definition) = load_schedule(&state, show_id, source_id)?;
    if revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Schedule revision conflict: expected {}, current {revision}",
            request.expected_revision
        )));
    }
    let id = Uuid::new_v4();
    definition.id = light_application::ScheduleId(id);
    definition.name = request
        .name
        .unwrap_or_else(|| format!("{} Copy", definition.name));
    normalize_new_anchor(&mut definition, Utc::now());
    recurrence()?
        .validate(&definition, Utc::now(), false)
        .map_err(schedule_validation)?;
    if definition.enabled {
        validate_persisted_target(&state, show_id, &definition.target)?;
    }
    let action_envelope = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![put_active_show_object(
            light_application::ActiveShowObjectKind::Schedule,
            id.to_string(),
            0,
            serde_json::to_value(&definition)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        )?],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action_envelope).await?;
    let outcome = mutation_outcome(
        &state,
        show_id,
        Some(id),
        request.request_id,
        result.show_revision.value(),
        result.event_sequence,
    )?;
    state
        .replay
        .insert_schedule(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

async fn remove(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::ScheduleDeleteRequest>,
) -> Result<Json<wire::ScheduleMutationOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Delete(id, request.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state.replay.lookup_schedule(&key, &replay_action).await? {
        return Ok(Json(outcome));
    }
    let (revision, _) = load_schedule(&state, show_id, id)?;
    if revision != request.expected_revision {
        return Err(ApiError::conflict(format!(
            "Schedule revision conflict: expected {}, current {revision}",
            request.expected_revision
        )));
    }
    let action_envelope = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![delete_active_show_object(
            light_application::ActiveShowObjectKind::Schedule,
            id.to_string(),
            revision,
        )],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action_envelope).await?;
    let outcome = mutation_outcome(
        &state,
        show_id,
        None,
        request.request_id,
        result.show_revision.value(),
        result.event_sequence,
    )?;
    state
        .replay
        .insert_schedule(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn schedule_snapshot(
    state: &AppState,
    show_id: light_core::ShowId,
    now: DateTime<Utc>,
) -> Result<wire::ScheduleSnapshot, ApiError> {
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (show_revision, objects) = store
        .objects_with_portable_revision("schedule")
        .map_err(ApiError::store)?;
    let recurrence = recurrence()?;
    let schedules = objects
        .into_iter()
        .map(|object| schedule_projection(&store, &recurrence, object, now))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(wire::ScheduleSnapshot {
        show_id: show_id.0,
        show_revision: show_revision.value(),
        timezone: recurrence.timezone_name().into(),
        server_now: timestamp(now),
        event_sequence: state.events.latest_sequence(),
        schedules,
    })
}

fn mutation_outcome(
    state: &AppState,
    show_id: light_core::ShowId,
    schedule_id: Option<Uuid>,
    request_id: String,
    show_revision: u64,
    event_sequence: u64,
) -> Result<wire::ScheduleMutationOutcome, ApiError> {
    let schedule = schedule_id
        .map(|id| {
            let entry = active_entry(state, show_id)?;
            let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
            let (_, object) = store
                .object_with_portable_revision("schedule", &id.to_string())
                .map_err(ApiError::store)?;
            schedule_projection(
                &store,
                &recurrence()?,
                object.ok_or_else(|| ApiError::internal("committed Schedule is missing"))?,
                Utc::now(),
            )
        })
        .transpose()?;
    Ok(wire::ScheduleMutationOutcome {
        request_id,
        replayed: false,
        show_id: show_id.0,
        show_revision,
        schedule,
        event_sequence,
    })
}

fn schedule_projection(
    store: &ActiveShowRepository,
    recurrence: &light_application::ScheduleRecurrence,
    object: light_show::VersionedObject,
    now: DateTime<Utc>,
) -> Result<wire::ScheduleProjection, ApiError> {
    let definition: light_application::ScheduleDefinition = decode(object.body, "stored Schedule")?;
    let validation = recurrence.validate(&definition, now, false);
    let next_occurrence = if definition.enabled && validation.is_ok() {
        recurrence
            .next_occurrence(&definition, now)
            .map_err(schedule_validation)?
            .as_ref()
            .map(occurrence_projection)
    } else {
        None
    };
    let history = store
        .schedule_occurrence_history(&definition.id.0.to_string())
        .map_err(ApiError::store)?
        .into_iter()
        .map(stored_result)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(wire::ScheduleProjection {
        definition: encode(definition, "stored Schedule")?,
        object_revision: object.revision,
        next_occurrence,
        last_result: history.first().cloned(),
        history,
        validation_error: validation.err().map(|error| error.to_string()),
    })
}

fn load_schedule(
    state: &AppState,
    show_id: light_core::ShowId,
    id: Uuid,
) -> Result<(u64, light_application::ScheduleDefinition), ApiError> {
    let entry = active_entry(state, show_id)?;
    let (_, object) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision("schedule", &id.to_string())
        .map_err(ApiError::store)?;
    let object = object.ok_or_else(|| ApiError::not_found("Schedule does not exist"))?;
    Ok((object.revision, decode(object.body, "stored Schedule")?))
}

fn validate_persisted_target(
    state: &AppState,
    show_id: light_core::ShowId,
    target: &light_application::ScheduleTarget,
) -> Result<(), ApiError> {
    let light_application::ScheduleTarget::Playback {
        page,
        slot,
        playback_number,
        action,
        ..
    } = target
    else {
        return Err(ApiError::conflict(
            "Macro runtime is unavailable until refactoring item 30",
        ));
    };
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let page =
        u8::try_from(*page).map_err(|_| ApiError::bad_request("Schedule page is out of range"))?;
    let slot =
        u8::try_from(*slot).map_err(|_| ApiError::bad_request("Schedule slot is out of range"))?;
    let (_, page_object) = store
        .object_with_portable_revision("playback_page", &page.to_string())
        .map_err(ApiError::store)?;
    let page_definition: light_playback::PlaybackPage = decode(
        page_object
            .ok_or_else(|| ApiError::conflict("scheduled Playback page does not exist"))?
            .body,
        "Playback page",
    )?;
    if page_definition.slots.get(&slot) != Some(playback_number) {
        return Err(ApiError::conflict(
            "scheduled Playback does not occupy its selected page slot",
        ));
    }
    let (_, playback_object) = store
        .object_with_portable_revision("playback", &playback_number.to_string())
        .map_err(ApiError::store)?;
    let playback: light_playback::PlaybackDefinition = decode(
        playback_object
            .ok_or_else(|| ApiError::conflict("scheduled Playback does not exist"))?
            .body,
        "Playback",
    )?;
    validate_action_matrix(&playback.target, *action)
}

fn recurrence() -> Result<light_application::ScheduleRecurrence, ApiError> {
    let system = jiff::tz::TimeZone::system();
    let timezone = system.iana_name().unwrap_or("UTC");
    light_application::ScheduleRecurrence::new(timezone).map_err(schedule_validation)
}

fn normalize_new_anchor(
    definition: &mut light_application::ScheduleDefinition,
    now: DateTime<Utc>,
) {
    match &mut definition.trigger {
        light_application::ScheduleTrigger::Interval { enabled_at, .. } => *enabled_at = now,
        light_application::ScheduleTrigger::Calendar {
            rule: light_application::CalendarRule::EveryNDays { anchor, .. },
        } => *anchor = Local::now().date_naive(),
        _ => {}
    }
}

fn preview_target() -> light_application::ScheduleTarget {
    light_application::ScheduleTarget::Playback {
        page: 1,
        slot: 1,
        playback_number: 1,
        action: light_application::ScheduledPlaybackAction::Go,
        master_transition: None,
    }
}

fn occurrence_projection(
    occurrence: &light_application::ScheduleOccurrence,
) -> wire::ScheduleOccurrenceProjection {
    wire::ScheduleOccurrenceProjection {
        occurrence_id: occurrence_id(&occurrence.identity.key),
        scheduled_for: timestamp(occurrence.scheduled_at),
        local_time: occurrence.local.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }
}

fn occurrence_id(key: &light_application::ScheduleOccurrenceKey) -> String {
    match key {
        light_application::ScheduleOccurrenceKey::Interval {
            enabled_at,
            ordinal,
        } => format!("interval:{}:{ordinal}", timestamp(*enabled_at)),
        light_application::ScheduleOccurrenceKey::Calendar { local } => {
            format!("calendar:{}", local.format("%Y-%m-%dT%H:%M:%S"))
        }
        light_application::ScheduleOccurrenceKey::OneTime { local } => {
            format!("one_time:{}", local.format("%Y-%m-%dT%H:%M:%S"))
        }
    }
}

fn stored_result(
    record: ScheduleOccurrenceRecord,
) -> Result<wire::ScheduleOccurrenceResult, ApiError> {
    let local_time = record
        .target_action
        .get("local_time")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(wire::ScheduleOccurrenceResult {
        occurrence: wire::ScheduleOccurrenceProjection {
            occurrence_id: record.occurrence_id,
            scheduled_for: timestamp(record.scheduled_for),
            local_time,
        },
        status: match record.status {
            StoredOccurrenceStatus::Claimed => wire::ScheduleOccurrenceStatus::Claimed,
            StoredOccurrenceStatus::Completed => wire::ScheduleOccurrenceStatus::Completed,
            StoredOccurrenceStatus::Failed => wire::ScheduleOccurrenceStatus::Failed,
            StoredOccurrenceStatus::Interrupted => wire::ScheduleOccurrenceStatus::Interrupted,
            StoredOccurrenceStatus::Skipped => wire::ScheduleOccurrenceStatus::Skipped,
        },
        recorded_at: timestamp(record.resolved_at.unwrap_or(record.recorded_at)),
        message: record.result_detail,
    })
}

fn decode<T: Serialize, U: serde::de::DeserializeOwned>(
    value: T,
    label: &str,
) -> Result<U, ApiError> {
    let value =
        serde_json::to_value(value).map_err(|error| ApiError::bad_request(error.to_string()))?;
    serde_json::from_value(value)
        .map_err(|error| ApiError::bad_request(format!("invalid {label}: {error}")))
}

fn encode<T: Serialize, U: serde::de::DeserializeOwned>(
    value: T,
    label: &str,
) -> Result<U, ApiError> {
    let value =
        serde_json::to_value(value).map_err(|error| ApiError::internal(error.to_string()))?;
    serde_json::from_value(value)
        .map_err(|error| ApiError::internal(format!("cannot encode {label}: {error}")))
}

fn schedule_validation(error: light_application::ScheduleValidationError) -> ApiError {
    ApiError::bad_request(error.to_string())
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ReplayAction {
    Create(wire::ScheduleCreateRequest),
    Update(Uuid, wire::ScheduleUpdateRequest),
    Duplicate(Uuid, wire::ScheduleDuplicateRequest),
    Delete(Uuid, wire::ScheduleDeleteRequest),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    show_id: light_core::ShowId,
    request_id: String,
}

impl ReplayKey {
    fn new(session: &Session, show_id: light_core::ShowId, request_id: &str) -> Self {
        Self {
            session_id: session.id.0,
            show_id,
            request_id: request_id.into(),
        }
    }
}

struct ReplayEntry {
    action: ReplayAction,
    outcome: wire::ScheduleMutationOutcome,
}

#[derive(Default)]
pub(super) struct ScheduleReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ScheduleReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &ReplayAction,
    ) -> Result<Option<wire::ScheduleMutationOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different Schedule action",
            ));
        }
        let mut outcome = entry.outcome.clone();
        outcome.replayed = true;
        Ok(Some(outcome))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        action: ReplayAction,
        outcome: wire::ScheduleMutationOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { action, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
