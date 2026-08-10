//! Authoritative Timecode transport routes over the application runtime service.

use super::show_object_intents_v2::{ReplayAction, ReplayKey};
use super::show_objects_v2::{active_entry, object_record, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_application::timeline::{
    SystemTimecodeClock, TimecodeChangePublisher, TimecodeRuntimeChange, TimecodeRuntimeService,
};
use light_playback::{TimecodeFrame, TimecodeFrameRate, TimecodeId};
use light_wire::v2::show_objects::ShowObjectActionOutcome;
use light_wire::v2::timecode as wire;

struct RoutePublisher;

impl TimecodeChangePublisher for RoutePublisher {
    fn publish(&self, _change: &TimecodeRuntimeChange) {
        // The route records a capability event after each accepted action. Scheduler-owned ticks
        // will move to the typed application event bus when the output scheduler installs them.
    }
}

pub(super) fn new_service() -> TimecodeRuntimeService {
    TimecodeRuntimeService::new(
        Arc::new(SystemTimecodeClock::default()),
        Arc::new(RoutePublisher),
        TimecodeFrameRate::whole_frames(44).expect("44 fps Timecode rate is valid"),
    )
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/timecodes", get(timecode_objects))
        .route("/api/v2/timecodes/actions", post(object_action))
        .route("/api/v2/timecodes/runtime", get(runtime_snapshots))
        .route(
            "/api/v2/timecodes/{timecode_id}/runtime",
            get(runtime_snapshot),
        )
        .route(
            "/api/v2/timecodes/{timecode_id}/transport",
            post(transport_action),
        )
}

async fn timecode_objects(
    State(state): State<AppState>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeCollectionSnapshot>, ApiError> {
    let _session = session_for_desk(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let (show_revision, objects) = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .objects_with_portable_revision("timecode")
        .map_err(ApiError::store)?;
    let mut objects = objects
        .into_iter()
        .map(|object| {
            let definition = serde_json::from_value::<light_playback::TimecodeDefinition>(
                object.body,
            )
            .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
            Ok(wire::TimecodeObjectRecord {
                revision: object.revision,
                definition: wire_definition(definition),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    objects.sort_by_key(|object| object.definition.number);
    Ok(Json(wire::TimecodeCollectionSnapshot {
        show_revision: show_revision.value(),
        objects,
    }))
}

async fn object_action(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::TimecodeObjectActionRequest>,
) -> Result<Json<ShowObjectActionOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let show_id = context.resolve(&state)?;
    let replay_action = ReplayAction::Timecode(request.action.clone());
    let key = ReplayKey::new(&session, show_id, &request.request_id);
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_show_object_intent(&key, &replay_action)
        .await?
    {
        return Ok(Json(outcome));
    }
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (mutation, previous) = timecode_mutation(&store, &request.action)?;
    let action = active_show_object_action(
        operator_action_context(&session, light_application::ActionSource::Http)
            .with_request_id(&request.request_id),
        show_id,
        vec![mutation],
    );
    let (result, _activation) =
        run_active_show_object_action_async(&state, activation, action).await?;
    let change = result
        .changes
        .first()
        .ok_or_else(|| ApiError::internal("Timecode mutation returned no object change"))?;
    let object = if change.deleted {
        state.timecodes.uninstall(TimecodeId(
            Uuid::parse_str(&change.object_id)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        ));
        previous.ok_or_else(|| ApiError::internal("deleted Timecode had no previous object"))?
    } else {
        let object = ActiveShowRepository::open(&entry.path)
            .map_err(ApiError::store)?
            .object_with_portable_revision("timecode", &change.object_id)
            .map_err(ApiError::store)?
            .1
            .ok_or_else(|| ApiError::internal("committed Timecode is missing"))?;
        install_object_if_runnable(&state, object.clone())?;
        object
    };
    let outcome = ShowObjectActionOutcome {
        request_id: request.request_id,
        replayed: false,
        show_id: show_id.0,
        show_revision: result.show_revision.value(),
        object: object_record(object),
        event_sequence: Some(result.event_sequence),
    };
    emit(
        &state,
        "timecode_object_changed",
        serde_json::json!({
            "show_id": show_id,
            "timecode_id": change.object_id,
            "object_revision": change.object_revision,
            "deleted": change.deleted,
        }),
    );
    state
        .replay
        .insert_show_object_intent(key, replay_action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn timecode_mutation(
    store: &ActiveShowRepository,
    action: &wire::TimecodeObjectAction,
) -> Result<
    (
        light_application::ActiveShowObjectMutation,
        Option<light_show::VersionedObject>,
    ),
    ApiError,
> {
    match action {
        wire::TimecodeObjectAction::Create { definition } => {
            let definition = domain_definition(definition.clone())?;
            let id = definition.id.0.to_string();
            let previous = store
                .object_with_portable_revision("timecode", &id)
                .map_err(ApiError::store)?
                .1;
            if previous.is_some() {
                return Err(ApiError::conflict("Timecode id already exists"));
            }
            Ok((
                put_active_show_object(
                    light_application::ActiveShowObjectKind::Timecode,
                    id,
                    0,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?,
                None,
            ))
        }
        wire::TimecodeObjectAction::Update {
            timecode_id,
            expected_revision,
            patch,
        } => {
            let previous = load_timecode(store, *timecode_id)?;
            if previous.revision != *expected_revision {
                return Err(ApiError::conflict(format!(
                    "Timecode revision conflict: expected {expected_revision}, current {}",
                    previous.revision
                )));
            }
            let mut definition =
                serde_json::from_value::<light_playback::TimecodeDefinition>(previous.body.clone())
                    .map_err(|error| {
                        ApiError::internal(format!("stored Timecode is invalid: {error}"))
                    })?;
            if let Some(number) = patch.number {
                definition.number = number;
            }
            if let Some(name) = &patch.name {
                definition.name = name.trim().to_owned();
            }
            if let Some(duration) = patch.duration_frame {
                definition.duration = Some(TimecodeFrame(duration));
            }
            if let Some(offset) = patch.transport_offset_frame {
                definition.transport_offset = TimecodeFrame(offset);
            }
            if let Some(auto_start) = patch.auto_start {
                definition.auto_start = auto_start;
            }
            if let Some(markers) = &patch.markers {
                definition.markers = markers.iter().cloned().map(domain_marker).collect();
            }
            if let Some(lanes) = &patch.lanes {
                definition.lanes = lanes.iter().cloned().map(domain_lane).collect();
            }
            definition
                .validate()
                .map_err(|error| ApiError::bad_request(error.message))?;
            Ok((
                put_active_show_object(
                    light_application::ActiveShowObjectKind::Timecode,
                    timecode_id.to_string(),
                    *expected_revision,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?,
                Some(previous),
            ))
        }
        wire::TimecodeObjectAction::Delete {
            timecode_id,
            expected_revision,
        } => {
            let previous = load_timecode(store, *timecode_id)?;
            if previous.revision != *expected_revision {
                return Err(ApiError::conflict(format!(
                    "Timecode revision conflict: expected {expected_revision}, current {}",
                    previous.revision
                )));
            }
            Ok((
                delete_active_show_object(
                    light_application::ActiveShowObjectKind::Timecode,
                    timecode_id.to_string(),
                    *expected_revision,
                ),
                Some(previous),
            ))
        }
    }
}

fn load_timecode(
    store: &ActiveShowRepository,
    id: Uuid,
) -> Result<light_show::VersionedObject, ApiError> {
    store
        .object_with_portable_revision("timecode", &id.to_string())
        .map_err(ApiError::store)?
        .1
        .ok_or_else(|| ApiError::not_found("Timecode does not exist"))
}

fn domain_definition(
    definition: wire::TimecodeDefinition,
) -> Result<light_playback::TimecodeDefinition, ApiError> {
    let definition = light_playback::TimecodeDefinition {
        id: TimecodeId(definition.id),
        number: definition.number,
        name: definition.name.trim().to_owned(),
        duration: definition.duration_frame.map(TimecodeFrame),
        transport_offset: TimecodeFrame(definition.transport_offset_frame),
        auto_start: definition.auto_start,
        audio: definition.audio.map(|audio| light_playback::TimecodeAudio {
            asset_id: audio.asset_id,
            asset_revision: audio.asset_revision,
            end_fade_frames: audio.end_fade_frames,
        }),
        markers: definition.markers.into_iter().map(domain_marker).collect(),
        lanes: definition.lanes.into_iter().map(domain_lane).collect(),
    };
    definition
        .validate()
        .map_err(|error| ApiError::bad_request(error.message))?;
    Ok(definition)
}

fn domain_marker(marker: wire::TimecodeMarker) -> light_playback::TimecodeMarker {
    light_playback::TimecodeMarker {
        id: light_playback::TimecodeMarkerId(marker.id),
        frame: TimecodeFrame(marker.frame),
        name: marker.name,
        color: marker.color,
    }
}

fn domain_lane(lane: wire::TimecodeLane) -> light_playback::TimecodeLane {
    use light_playback as domain;
    let content = match lane.content {
        wire::TimecodeLaneContent::CueList { cue_list_id, clips } => {
            domain::TimecodeLaneContent::CueList {
                cue_list_id: light_core::CueListId(cue_list_id),
                clips: clips
                    .into_iter()
                    .map(|clip| domain::TimecodeCueListClip {
                        id: domain::TimecodeClipId(clip.id),
                        start_frame: TimecodeFrame(clip.start_frame),
                        end_frame: TimecodeFrame(clip.end_frame),
                        start_cue_id: clip.start_cue_id,
                        end_cue_id: clip.end_cue_id,
                        start_behavior: match clip.start_behavior {
                            wire::TimecodeClipStart::State => domain::TimecodeClipStart::State,
                            wire::TimecodeClipStart::Cue => domain::TimecodeClipStart::Cue,
                        },
                        end_behavior: match clip.end_behavior {
                            wire::TimecodeClipEnd::Release => domain::TimecodeClipEnd::Release,
                            wire::TimecodeClipEnd::Hold => domain::TimecodeClipEnd::Hold,
                        },
                    })
                    .collect(),
            }
        }
        wire::TimecodeLaneContent::SpeedGroup { group, keyframes } => {
            domain::TimecodeLaneContent::SpeedGroup {
                group,
                keyframes: keyframes
                    .into_iter()
                    .map(|keyframe| domain::TimecodeSpeedKeyframe {
                        id: domain::TimecodeKeyframeId(keyframe.id),
                        frame: TimecodeFrame(keyframe.frame),
                        bpm: keyframe.bpm,
                        phase: keyframe.phase,
                    })
                    .collect(),
            }
        }
        wire::TimecodeLaneContent::AudioVolume { keyframes } => {
            domain::TimecodeLaneContent::AudioVolume {
                keyframes: keyframes
                    .into_iter()
                    .map(|keyframe| domain::TimecodeVolumeKeyframe {
                        id: domain::TimecodeKeyframeId(keyframe.id),
                        frame: TimecodeFrame(keyframe.frame),
                        value: keyframe.value,
                        fade_frames: keyframe.fade_frames,
                        curve: match keyframe.curve {
                            wire::TimecodeCurve::Linear => domain::TimecodeCurve::Linear,
                            wire::TimecodeCurve::EaseIn => domain::TimecodeCurve::EaseIn,
                            wire::TimecodeCurve::EaseOut => domain::TimecodeCurve::EaseOut,
                            wire::TimecodeCurve::EaseInOut => domain::TimecodeCurve::EaseInOut,
                        },
                    })
                    .collect(),
            }
        }
    };
    domain::TimecodeLane {
        id: domain::TimecodeLaneId(lane.id),
        name: lane.name,
        content,
    }
}

fn wire_definition(definition: light_playback::TimecodeDefinition) -> wire::TimecodeDefinition {
    wire::TimecodeDefinition {
        id: definition.id.0,
        number: definition.number,
        name: definition.name,
        duration_frame: definition.duration.map(|frame| frame.0),
        transport_offset_frame: definition.transport_offset.0,
        auto_start: definition.auto_start,
        audio: definition.audio.map(|audio| wire::TimecodeAudio {
            asset_id: audio.asset_id,
            asset_revision: audio.asset_revision,
            end_fade_frames: audio.end_fade_frames,
        }),
        markers: definition
            .markers
            .into_iter()
            .map(|marker| wire::TimecodeMarker {
                id: marker.id.0,
                frame: marker.frame.0,
                name: marker.name,
                color: marker.color,
            })
            .collect(),
        lanes: definition.lanes.into_iter().map(wire_lane).collect(),
    }
}

fn wire_lane(lane: light_playback::TimecodeLane) -> wire::TimecodeLane {
    use light_playback as domain;
    let content = match lane.content {
        domain::TimecodeLaneContent::CueList { cue_list_id, clips } => {
            wire::TimecodeLaneContent::CueList {
                cue_list_id: cue_list_id.0,
                clips: clips
                    .into_iter()
                    .map(|clip| wire::TimecodeCueListClip {
                        id: clip.id.0,
                        start_frame: clip.start_frame.0,
                        end_frame: clip.end_frame.0,
                        start_cue_id: clip.start_cue_id,
                        end_cue_id: clip.end_cue_id,
                        start_behavior: match clip.start_behavior {
                            domain::TimecodeClipStart::State => wire::TimecodeClipStart::State,
                            domain::TimecodeClipStart::Cue => wire::TimecodeClipStart::Cue,
                        },
                        end_behavior: match clip.end_behavior {
                            domain::TimecodeClipEnd::Release => wire::TimecodeClipEnd::Release,
                            domain::TimecodeClipEnd::Hold => wire::TimecodeClipEnd::Hold,
                        },
                    })
                    .collect(),
            }
        }
        domain::TimecodeLaneContent::SpeedGroup { group, keyframes } => {
            wire::TimecodeLaneContent::SpeedGroup {
                group,
                keyframes: keyframes
                    .into_iter()
                    .map(|keyframe| wire::TimecodeSpeedKeyframe {
                        id: keyframe.id.0,
                        frame: keyframe.frame.0,
                        bpm: keyframe.bpm,
                        phase: keyframe.phase,
                    })
                    .collect(),
            }
        }
        domain::TimecodeLaneContent::AudioVolume { keyframes } => {
            wire::TimecodeLaneContent::AudioVolume {
                keyframes: keyframes
                    .into_iter()
                    .map(|keyframe| wire::TimecodeVolumeKeyframe {
                        id: keyframe.id.0,
                        frame: keyframe.frame.0,
                        value: keyframe.value,
                        fade_frames: keyframe.fade_frames,
                        curve: match keyframe.curve {
                            domain::TimecodeCurve::Linear => wire::TimecodeCurve::Linear,
                            domain::TimecodeCurve::EaseIn => wire::TimecodeCurve::EaseIn,
                            domain::TimecodeCurve::EaseOut => wire::TimecodeCurve::EaseOut,
                            domain::TimecodeCurve::EaseInOut => wire::TimecodeCurve::EaseInOut,
                        },
                    })
                    .collect(),
            }
        }
    };
    wire::TimecodeLane {
        id: lane.id.0,
        name: lane.name,
        content,
    }
}

async fn runtime_snapshots(
    State(state): State<AppState>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<Vec<wire::TimecodeTransportSnapshot>>, ApiError> {
    let _session = session_for_desk(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    install_show_timecodes(&state, show_id)?;
    Ok(Json(
        state
            .timecodes
            .snapshots()
            .into_iter()
            .map(wire_snapshot)
            .collect(),
    ))
}

async fn runtime_snapshot(
    State(state): State<AppState>,
    Path(timecode_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeTransportSnapshot>, ApiError> {
    let _session = session_for_desk(&state, &headers, &desk)?;
    let show_id = context.resolve(&state)?;
    ensure_installed(&state, show_id, TimecodeId(timecode_id))?;
    state
        .timecodes
        .snapshot(TimecodeId(timecode_id))
        .map(wire_snapshot)
        .map(Json)
        .map_err(|error| ApiError::not_found(error.message))
}

async fn transport_action(
    State(state): State<AppState>,
    Path(timecode_id): Path<Uuid>,
    context: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::TimecodeTransportActionRequest>,
) -> Result<Json<wire::TimecodeTransportSnapshot>, ApiError> {
    let session = command_http::authenticate_desk_mutation(&state, &headers, &desk)?;
    if request.timecode_id != timecode_id {
        return Err(ApiError::bad_request(
            "Timecode request identity does not match the route",
        ));
    }
    let show_id = context.resolve(&state)?;
    let id = TimecodeId(timecode_id);
    ensure_installed(&state, show_id, id)?;
    let outcome = state
        .timecodes
        .handle(id, domain_action(request.action))
        .map_err(|error| ApiError::bad_request(error.message))?;
    emit(
        &state,
        "timecode_runtime_changed",
        serde_json::json!({
            "desk_id": session.desk.id,
            "timecode_id": timecode_id,
            "revision": outcome.snapshot.revision,
            "state": transport_name(outcome.snapshot.transport),
            "frame": outcome.snapshot.frame.0,
        }),
    );
    Ok(Json(wire_snapshot(outcome.snapshot)))
}

fn install_show_timecodes(state: &AppState, show_id: light_core::ShowId) -> Result<(), ApiError> {
    let entry = super::show_objects_v2::active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (_, objects) = store
        .objects_with_portable_revision("timecode")
        .map_err(ApiError::store)?;
    for object in objects {
        let id = Uuid::parse_str(&object.id)
            .map(TimecodeId)
            .map_err(|error| {
                ApiError::internal(format!("stored Timecode id is invalid: {error}"))
            })?;
        if state.timecodes.snapshot(id).is_ok() {
            continue;
        }
        install_object(state, object)?;
    }
    Ok(())
}

fn ensure_installed(
    state: &AppState,
    show_id: light_core::ShowId,
    id: TimecodeId,
) -> Result<(), ApiError> {
    if state.timecodes.snapshot(id).is_ok() {
        return Ok(());
    }
    let entry = super::show_objects_v2::active_entry(state, show_id)?;
    let object = ActiveShowRepository::open(&entry.path)
        .map_err(ApiError::store)?
        .object_with_portable_revision("timecode", &id.0.to_string())
        .map_err(ApiError::store)?
        .1
        .ok_or_else(|| ApiError::not_found("Timecode does not exist"))?;
    install_object(state, object)
}

fn install_object(state: &AppState, object: light_show::VersionedObject) -> Result<(), ApiError> {
    let definition: light_playback::TimecodeDefinition = serde_json::from_value(object.body)
        .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    state
        .timecodes
        .install(definition, None)
        .map(|_| ())
        .map_err(|error| ApiError::bad_request(error.message))
}

fn install_object_if_runnable(
    state: &AppState,
    object: light_show::VersionedObject,
) -> Result<(), ApiError> {
    let definition: light_playback::TimecodeDefinition = serde_json::from_value(object.body)
        .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    if definition.duration.is_none() {
        state.timecodes.uninstall(definition.id);
        return Ok(());
    }
    state
        .timecodes
        .install(definition, None)
        .map(|_| ())
        .map_err(|error| ApiError::bad_request(error.message))
}

fn domain_action(action: wire::TimecodeTransportAction) -> light_playback::TimecodeTransportAction {
    match action {
        wire::TimecodeTransportAction::Go => light_playback::TimecodeTransportAction::Go,
        wire::TimecodeTransportAction::Pause => light_playback::TimecodeTransportAction::Pause,
        wire::TimecodeTransportAction::Stop => light_playback::TimecodeTransportAction::Stop,
        wire::TimecodeTransportAction::Rewind => light_playback::TimecodeTransportAction::Rewind,
        wire::TimecodeTransportAction::Seek { frame } => {
            light_playback::TimecodeTransportAction::Seek {
                frame: TimecodeFrame(frame),
            }
        }
    }
}

fn wire_snapshot(
    snapshot: light_application::timeline::TimecodeRuntimeSnapshot,
) -> wire::TimecodeTransportSnapshot {
    wire::TimecodeTransportSnapshot {
        timecode_id: snapshot.timecode_id.0,
        state: match snapshot.transport {
            light_playback::TimecodeTransportState::Stopped => {
                wire::TimecodeTransportState::Stopped
            }
            light_playback::TimecodeTransportState::Playing => {
                wire::TimecodeTransportState::Playing
            }
            light_playback::TimecodeTransportState::Paused => wire::TimecodeTransportState::Paused,
        },
        frame: snapshot.frame.0,
        duration_frame: snapshot.duration.0,
        audio_linked: false,
    }
}

fn transport_name(state: light_playback::TimecodeTransportState) -> &'static str {
    match state {
        light_playback::TimecodeTransportState::Stopped => "stopped",
        light_playback::TimecodeTransportState::Playing => "playing",
        light_playback::TimecodeTransportState::Paused => "paused",
    }
}
