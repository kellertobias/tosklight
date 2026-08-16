//! Authoritative Timecode transport routes over the application runtime service.

use super::show_object_intents_v2::{ReplayAction, ReplayKey};
use super::show_objects_v2::{active_entry, object_record, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use light_application::timeline::{
    TimecodeChangePublisher, TimecodeRuntimeChange, TimecodeRuntimeService,
};
use light_playback::{TimecodeFrame, TimecodeFrameRate, TimecodeId};
use light_wire::v2::show_objects::ShowObjectActionOutcome;
use light_wire::v2::timecode as wire;

struct RoutePublisher(light_application::EventBus);

impl TimecodeChangePublisher for RoutePublisher {
    fn publish(&self, change: &TimecodeRuntimeChange) {
        self.0
            .publish(light_application::EventDraft::timecode_runtime_changed(
                change.clone(),
            ));
    }
}

pub(super) fn new_service_with_clock(
    clock: Arc<dyn light_application::timeline::TimecodeClock>,
    output: Option<Arc<dyn light_application::TimecodeAudioOutput>>,
    events: light_application::EventBus,
) -> TimecodeRuntimeService {
    let service = TimecodeRuntimeService::new(
        clock,
        Arc::new(RoutePublisher(events)),
        TimecodeFrameRate::whole_frames(44).expect("44 fps Timecode rate is valid"),
    );
    output.map_or(service.clone(), |output| {
        service.with_audio(Arc::new(light_application::TimecodeAudioService::new(
            output,
        )))
    })
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/timecodes", get(timecode_objects))
        .route("/api/v2/timecodes/actions", post(object_action))
        .route("/api/v2/timecodes/runtime", get(runtime_snapshots))
        .route("/api/v2/timecodes/audio/outputs", get(audio_outputs))
        .route("/api/v2/timecodes/audio/import", post(import_audio))
        .route(
            "/api/v2/timecodes/{timecode_id}/audio/waveform",
            get(audio_waveform),
        )
        .route(
            "/api/v2/timecodes/{timecode_id}/runtime",
            get(runtime_snapshot),
        )
        .route(
            "/api/v2/timecodes/{timecode_id}/transport",
            post(transport_action),
        )
}

struct WaveformSink(Vec<u8>);

impl light_application::AssetChunkSink for WaveformSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

async fn audio_waveform(
    State(state): State<AppState>,
    context: ShowContext,
    Path(timecode_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeAudioWaveform>, ApiError> {
    authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let object = load_timecode(&store, timecode_id)?;
    let definition = serde_json::from_value::<light_playback::TimecodeDefinition>(object.body)
        .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    let audio = definition
        .audio
        .ok_or_else(|| ApiError::not_found("Timecode has no linked audio"))?;
    let mut sink = WaveformSink(Vec::new());
    state
        .managed_assets
        .stream(
            light_application::AssetReference {
                id: light_application::AssetId(audio.asset_id),
                revision: light_application::AssetRevision(audio.asset_revision),
            },
            &mut sink,
        )
        .map_err(|error| ApiError::internal(error.message))?;
    let peaks = pcm_waveform_peaks(&sink.0, 220)
        .map_err(|message| ApiError::bad_request(format!("linked audio waveform: {message}")))?;
    Ok(Json(wire::TimecodeAudioWaveform { peaks }))
}

fn pcm_waveform_peaks(bytes: &[u8], bucket_count: usize) -> Result<Vec<f32>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("asset is not a RIFF/WAVE file".into());
    }
    let mut cursor = 12usize;
    let mut format = None;
    let mut channels = None;
    let mut bits = None;
    let mut samples = None;
    while cursor.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let id = &bytes[cursor..cursor + 4];
        let length = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .ok_or_else(|| "chunk length overflow".to_string())?;
        if end > bytes.len() {
            return Err("truncated WAV chunk".into());
        }
        if id == b"fmt " && length >= 16 {
            format = Some(u16::from_le_bytes(
                bytes[start..start + 2].try_into().unwrap(),
            ));
            channels = Some(u16::from_le_bytes(
                bytes[start + 2..start + 4].try_into().unwrap(),
            ));
            bits = Some(u16::from_le_bytes(
                bytes[start + 14..start + 16].try_into().unwrap(),
            ));
        } else if id == b"data" {
            samples = Some(&bytes[start..end]);
        }
        cursor = end + (length & 1);
    }
    let format = format.ok_or_else(|| "missing fmt chunk".to_string())?;
    let channels = usize::from(channels.ok_or_else(|| "missing channel count".to_string())?);
    let bits = bits.ok_or_else(|| "missing sample width".to_string())?;
    let samples = samples.ok_or_else(|| "missing data chunk".to_string())?;
    let bytes_per_sample = usize::from(bits / 8);
    if channels == 0 || !matches!((format, bits), (1, 16) | (3, 32)) {
        return Err("only PCM16 or IEEE-float32 WAV waveform data is supported".into());
    }
    let frame_bytes = channels * bytes_per_sample;
    let frame_count = samples.len() / frame_bytes;
    if frame_count == 0 {
        return Err("audio contains no samples".into());
    }
    let count = bucket_count.clamp(1, frame_count);
    let mut peaks = vec![0.0f32; count];
    for (bucket, peak) in peaks.iter_mut().enumerate() {
        let first = bucket * frame_count / count;
        let last = ((bucket + 1) * frame_count / count).max(first + 1);
        for frame in first..last {
            for channel in 0..channels {
                let offset = frame * frame_bytes + channel * bytes_per_sample;
                let value = if format == 1 {
                    f32::from(i16::from_le_bytes(
                        samples[offset..offset + 2].try_into().unwrap(),
                    )) / f32::from(i16::MAX)
                } else {
                    f32::from_le_bytes(samples[offset..offset + 4].try_into().unwrap())
                };
                *peak = peak.max(value.abs().min(1.0));
            }
        }
    }
    Ok(peaks)
}

#[derive(Deserialize)]
struct AudioImportQuery {
    name: String,
}

struct RequestAudioSource {
    bytes: Option<Vec<u8>>,
}

impl light_application::AssetChunkSource for RequestAudioSource {
    fn read_chunk(
        &mut self,
        _maximum_bytes: usize,
    ) -> Result<Option<Vec<u8>>, light_application::AssetError> {
        Ok(self.bytes.take())
    }
}

async fn import_audio(
    State(state): State<AppState>,
    context: ShowContext,
    Query(query): Query<AudioImportQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<wire::TimecodeAudioImportResult>, ApiError> {
    authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    if query.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "Timecode audio name must not be empty",
        ));
    }
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let mut source = RequestAudioSource {
        bytes: Some(body.to_vec()),
    };
    let importer = light_application::TimecodeWavImporter::new(Arc::clone(&state.managed_assets));
    let namespace = light_application::AssetNamespace(format!("show:{}", show_id.0));
    let imported = if matches!(content_type, "audio/mpeg" | "audio/mp3")
        || query.name.to_ascii_lowercase().ends_with(".mp3")
    {
        importer.import_mp3(None, namespace, query.name, &mut source)
    } else if matches!(content_type, "audio/wav" | "audio/x-wav")
        || query.name.to_ascii_lowercase().ends_with(".wav")
    {
        importer.import(None, namespace, query.name, &mut source)
    } else {
        return Err(ApiError::bad_request(
            "Timecode audio import accepts MP3 or WAV",
        ));
    }
    .map_err(|error| ApiError::bad_request(error.message))?;
    Ok(Json(wire::TimecodeAudioImportResult {
        asset_id: imported.descriptor.asset.id.0,
        asset_revision: imported.descriptor.asset.revision.0,
        name: imported.descriptor.name,
        media_type: imported.descriptor.media_type,
        sample_rate: imported.metadata.sample_rate,
        channels: imported.metadata.channels,
        sample_frames: imported.metadata.sample_frames,
    }))
}

async fn audio_outputs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeAudioOutputDevices>, ApiError> {
    authenticate(&state, &headers)?;
    let devices = tokio::task::spawn_blocking(super::timecode_audio_output::output_devices)
        .await
        .map_err(|error| {
            ApiError::unavailable(format!("Timecode audio output discovery stopped: {error}"))
        })?
        .map_err(ApiError::unavailable)?;
    Ok(Json(wire::TimecodeAudioOutputDevices { devices }))
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
            validate_timecode_graph(store, Some(&definition), None)?;
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
            if let Some(audio) = &patch.audio {
                definition.audio = Some(light_playback::TimecodeAudio {
                    asset_id: audio.asset_id,
                    asset_revision: audio.asset_revision,
                    end_fade_frames: audio.end_fade_frames,
                });
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
            validate_timecode_graph(store, Some(&definition), None)?;
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
            validate_timecode_graph(store, None, Some(TimecodeId(*timecode_id)))?;
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

fn validate_timecode_graph(
    store: &ActiveShowRepository,
    replacement: Option<&light_playback::TimecodeDefinition>,
    removed: Option<TimecodeId>,
) -> Result<(), ApiError> {
    let cue_lists = store
        .objects_with_portable_revision("cue_list")
        .map_err(ApiError::store)?
        .1
        .into_iter()
        .map(|object| {
            serde_json::from_value(object.body)
                .map_err(|error| ApiError::internal(error.to_string()))
        })
        .collect::<Result<Vec<light_playback::CueList>, _>>()?;
    let mut timecodes = store
        .objects_with_portable_revision("timecode")
        .map_err(ApiError::store)?
        .1
        .into_iter()
        .map(|object| {
            serde_json::from_value(object.body)
                .map_err(|error| ApiError::internal(error.to_string()))
        })
        .collect::<Result<Vec<light_playback::TimecodeDefinition>, _>>()?;
    if let Some(replacement) = replacement {
        timecodes.retain(|value| value.id != replacement.id);
        timecodes.push(replacement.clone());
    }
    if let Some(removed) = removed {
        timecodes.retain(|value| value.id != removed);
    }
    light_application::validate_cue_timecode_graph(&cue_lists, &timecodes)
        .map_err(ApiError::bad_request)
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
        wire::TimecodeLaneContent::AudioPlayer { fixture_id, clips } => {
            domain::TimecodeLaneContent::AudioPlayer {
                fixture_id: light_core::FixtureId(fixture_id),
                clips: clips
                    .into_iter()
                    .map(|clip| domain::TimecodeAudioPlayerClip {
                        id: domain::TimecodeClipId(clip.id),
                        start_frame: TimecodeFrame(clip.start_frame),
                        end_frame: TimecodeFrame(clip.end_frame),
                        folder: clip.folder,
                        file: clip.file,
                        repeat: clip.repeat,
                        volume_keyframes: clip
                            .volume_keyframes
                            .into_iter()
                            .map(domain_volume_keyframe)
                            .collect(),
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

fn domain_volume_keyframe(
    keyframe: wire::TimecodeVolumeKeyframe,
) -> light_playback::TimecodeVolumeKeyframe {
    light_playback::TimecodeVolumeKeyframe {
        id: light_playback::TimecodeKeyframeId(keyframe.id),
        frame: TimecodeFrame(keyframe.frame),
        value: keyframe.value,
        fade_frames: keyframe.fade_frames,
        curve: match keyframe.curve {
            wire::TimecodeCurve::Linear => light_playback::TimecodeCurve::Linear,
            wire::TimecodeCurve::EaseIn => light_playback::TimecodeCurve::EaseIn,
            wire::TimecodeCurve::EaseOut => light_playback::TimecodeCurve::EaseOut,
            wire::TimecodeCurve::EaseInOut => light_playback::TimecodeCurve::EaseInOut,
        },
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
        domain::TimecodeLaneContent::AudioPlayer { fixture_id, clips } => {
            wire::TimecodeLaneContent::AudioPlayer {
                fixture_id: fixture_id.0,
                clips: clips
                    .into_iter()
                    .map(|clip| wire::TimecodeAudioPlayerClip {
                        id: clip.id.0,
                        start_frame: clip.start_frame.0,
                        end_frame: clip.end_frame.0,
                        folder: clip.folder,
                        file: clip.file,
                        repeat: clip.repeat,
                        volume_keyframes: clip
                            .volume_keyframes
                            .into_iter()
                            .map(wire_volume_keyframe)
                            .collect(),
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

fn wire_volume_keyframe(
    keyframe: light_playback::TimecodeVolumeKeyframe,
) -> wire::TimecodeVolumeKeyframe {
    wire::TimecodeVolumeKeyframe {
        id: keyframe.id.0,
        frame: keyframe.frame.0,
        value: keyframe.value,
        fade_frames: keyframe.fade_frames,
        curve: match keyframe.curve {
            light_playback::TimecodeCurve::Linear => wire::TimecodeCurve::Linear,
            light_playback::TimecodeCurve::EaseIn => wire::TimecodeCurve::EaseIn,
            light_playback::TimecodeCurve::EaseOut => wire::TimecodeCurve::EaseOut,
            light_playback::TimecodeCurve::EaseInOut => wire::TimecodeCurve::EaseInOut,
        },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandLineTimecodeAction {
    Run,
    Arm,
    Disarm,
    Edit,
}

#[allow(private_interfaces)]
pub(crate) fn timecode_command(
    state: &AppState,
    session: &Session,
    number: u32,
    action: CommandLineTimecodeAction,
    context: &light_application::ActionContext,
) -> Result<String, ApiError> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .map(|show| show.id)
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let (_, objects) = store
        .objects_with_portable_revision("timecode")
        .map_err(ApiError::store)?;
    let object = objects
        .into_iter()
        .find(|object| {
            serde_json::from_value::<light_playback::TimecodeDefinition>(object.body.clone())
                .is_ok_and(|definition| definition.number == number)
        })
        .ok_or_else(|| ApiError::not_found(format!("Timecode {number} does not exist")))?;
    let mut definition =
        serde_json::from_value::<light_playback::TimecodeDefinition>(object.body.clone())
            .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    match action {
        CommandLineTimecodeAction::Run => {
            ensure_installed(state, show_id, definition.id)?;
            apply_transport_action(
                state,
                definition.id,
                light_playback::TimecodeTransportAction::Go,
            )?;
            Ok(format!("Started Timecode {number}"))
        }
        CommandLineTimecodeAction::Edit => {
            emit(
                state,
                "desk_action",
                serde_json::json!({
                    "action": "open-object-editor",
                    "control": "timecode",
                    "value": object.id,
                    "desk_id": session.desk.id,
                }),
            );
            Ok(format!("Opened Timecode {number} editor"))
        }
        CommandLineTimecodeAction::Arm | CommandLineTimecodeAction::Disarm => {
            let armed = matches!(action, CommandLineTimecodeAction::Arm);
            if definition.auto_start != armed {
                definition.auto_start = armed;
                definition
                    .validate()
                    .map_err(|error| ApiError::bad_request(error.message))?;
                let mutation = put_active_show_object(
                    light_application::ActiveShowObjectKind::Timecode,
                    object.id.clone(),
                    object.revision,
                    serde_json::to_value(definition)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                )?;
                let action = active_show_object_action(context.clone(), show_id, vec![mutation]);
                let result =
                    run_active_show_object_action_in_programming_interaction(state, action)?;
                let change = result.changes.first().ok_or_else(|| {
                    ApiError::internal("Timecode autoplay mutation returned no object change")
                })?;
                let stored = ActiveShowRepository::open(&entry.path)
                    .map_err(ApiError::store)?
                    .object_with_portable_revision("timecode", &change.object_id)
                    .map_err(ApiError::store)?
                    .1
                    .ok_or_else(|| ApiError::internal("committed Timecode is missing"))?;
                install_object_if_runnable(state, stored)?;
                emit(
                    state,
                    "timecode_object_changed",
                    serde_json::json!({
                        "show_id": show_id,
                        "timecode_id": change.object_id,
                        "object_revision": change.object_revision,
                        "deleted": false,
                    }),
                );
            }
            Ok(format!(
                "{} Timecode {number} autoplay",
                if armed { "Armed" } else { "Disarmed" }
            ))
        }
    }
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
    context.resolve(&state)?;
    let snapshot = apply_transport_request(&state, session.desk.id, request)?;
    Ok(Json(snapshot))
}

pub(super) fn apply_transport_request(
    state: &AppState,
    desk_id: Uuid,
    request: wire::TimecodeTransportActionRequest,
) -> Result<wire::TimecodeTransportSnapshot, ApiError> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?
        .id;
    let timecode_id = request.timecode_id;
    let id = TimecodeId(timecode_id);
    ensure_installed(state, show_id, id)?;
    let outcome = apply_transport_action(state, id, domain_action(request.action))?;
    emit(
        state,
        "timecode_runtime_changed",
        serde_json::json!({
            "desk_id": desk_id,
            "timecode_id": timecode_id,
            "revision": outcome.snapshot.revision,
            "state": transport_name(outcome.snapshot.transport),
            "frame": outcome.snapshot.frame.0,
        }),
    );
    Ok(wire_snapshot(outcome.snapshot))
}

pub(super) fn apply_transport_action(
    state: &AppState,
    id: TimecodeId,
    action: light_playback::TimecodeTransportAction,
) -> Result<light_application::timeline::TimecodeRuntimeOutcome, ApiError> {
    state
        .timecodes
        .handle(id, action)
        .map_err(|error| ApiError::bad_request(error.message))
}

pub(super) fn apply_cue_action(
    state: &AppState,
    action: &light_playback::CueAction,
) -> Result<Option<u64>, ApiError> {
    let show_id = state
        .active_show
        .current()
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?
        .id;
    let id = match action {
        light_playback::CueAction::Jump { .. } => return Ok(None),
        light_playback::CueAction::TimecodeStart { timecode_id, .. }
        | light_playback::CueAction::TimecodeStop { timecode_id } => *timecode_id,
    };
    ensure_installed(state, show_id, id)?;
    let entry = active_entry(state, show_id)?;
    apply_installed_cue_action(&state.timecodes, &entry.path, action)
}

pub(super) fn apply_installed_cue_action(
    timecodes: &light_application::timeline::TimecodeRuntimeService,
    show_path: &str,
    action: &light_playback::CueAction,
) -> Result<Option<u64>, ApiError> {
    let (id, transport, completion) = match action {
        light_playback::CueAction::Jump { .. } => return Ok(None),
        light_playback::CueAction::TimecodeStop { timecode_id } => (
            *timecode_id,
            light_playback::TimecodeTransportAction::Stop,
            None,
        ),
        light_playback::CueAction::TimecodeStart { timecode_id, start } => {
            let frame = match start {
                light_playback::CueTimecodeStart::Frame { frame } => *frame,
                light_playback::CueTimecodeStart::Marker { marker_id } => {
                    let object = ActiveShowRepository::open(show_path)
                        .map_err(ApiError::store)?
                        .object_with_portable_revision("timecode", &timecode_id.0.to_string())
                        .map_err(ApiError::store)?
                        .1
                        .ok_or_else(|| ApiError::not_found("Timecode does not exist"))?;
                    let definition: light_playback::TimecodeDefinition =
                        serde_json::from_value(object.body)
                            .map_err(|error| ApiError::internal(error.to_string()))?;
                    definition
                        .markers
                        .iter()
                        .find(|marker| marker.id == *marker_id)
                        .map(|marker| marker.frame)
                        .ok_or_else(|| ApiError::bad_request("Timecode marker does not exist"))?
                }
            };
            timecodes
                .handle(*timecode_id, light_playback::TimecodeTransportAction::Go)
                .map_err(|error| ApiError::bad_request(error.message))?;
            let completion = timecodes
                .remaining_millis(*timecode_id, frame)
                .map_err(|error| ApiError::bad_request(error.message))?;
            (
                *timecode_id,
                light_playback::TimecodeTransportAction::Seek { frame },
                Some(completion),
            )
        }
    };
    timecodes
        .handle(id, transport)
        .map_err(|error| ApiError::bad_request(error.message))?;
    Ok(completion)
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
    let audio_duration = prepare_audio_if_present(state, &definition)?;
    state
        .timecodes
        .install(definition, audio_duration)
        .map(|_| ())
        .map_err(|error| ApiError::bad_request(error.message))
}

fn install_object_if_runnable(
    state: &AppState,
    object: light_show::VersionedObject,
) -> Result<(), ApiError> {
    let definition: light_playback::TimecodeDefinition = serde_json::from_value(object.body)
        .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    let audio_duration = prepare_audio_if_present(state, &definition)?;
    if definition.duration.is_none() && audio_duration.is_none() {
        state.timecodes.uninstall(definition.id);
        return Ok(());
    }
    state
        .timecodes
        .install(definition, audio_duration)
        .map(|_| ())
        .map_err(|error| ApiError::bad_request(error.message))
}

#[derive(Default)]
struct AudioBytes(Vec<u8>);

impl light_application::AssetChunkSink for AudioBytes {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn prepare_audio_if_present(
    state: &AppState,
    definition: &light_playback::TimecodeDefinition,
) -> Result<Option<TimecodeFrame>, ApiError> {
    let Some(audio) = &definition.audio else {
        return Ok(None);
    };
    let asset = light_application::AssetReference {
        id: light_application::AssetId(audio.asset_id),
        revision: light_application::AssetRevision(audio.asset_revision),
    };
    let mut bytes = AudioBytes::default();
    state
        .managed_assets
        .stream(asset, &mut bytes)
        .map_err(|error| ApiError::bad_request(error.message))?;
    let metadata = light_application::parse_wav_metadata(&bytes.0)
        .map_err(|error| ApiError::bad_request(error.message))?;
    match state
        .timecodes
        .prepare_audio(definition.id, asset, metadata, true)
    {
        Ok(duration) => Ok(Some(duration)),
        Err(error) if error.message == "native Timecode audio output is unavailable" => {
            tracing::warn!(
                timecode_id = %definition.id.0,
                "Timecode audio is valid but this server has no native output device"
            );
            Ok(None)
        }
        Err(error) => Err(ApiError::bad_request(error.message)),
    }
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

pub(super) fn wire_snapshot(
    snapshot: light_application::timeline::TimecodeRuntimeSnapshot,
) -> wire::TimecodeTransportSnapshot {
    wire::TimecodeTransportSnapshot {
        timecode_id: snapshot.timecode_id.0,
        revision: snapshot.revision,
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
        audio_linked: snapshot.audio_linked,
    }
}

fn transport_name(state: light_playback::TimecodeTransportState) -> &'static str {
    match state {
        light_playback::TimecodeTransportState::Stopped => "stopped",
        light_playback::TimecodeTransportState::Playing => "playing",
        light_playback::TimecodeTransportState::Paused => "paused",
    }
}

#[cfg(test)]
mod waveform_tests {
    use super::pcm_waveform_peaks;

    #[test]
    fn waveform_uses_actual_pcm_samples_and_bounds_the_projection() {
        let samples = [0i16, i16::MAX, i16::MIN, 0];
        let data = samples
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&88_200u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);

        let peaks = pcm_waveform_peaks(&wav, 2).unwrap();
        assert_eq!(peaks.len(), 2);
        assert!((peaks[0] - 1.0).abs() < 0.0001);
        assert!((peaks[1] - 1.0).abs() < 0.0001);
        assert!(pcm_waveform_peaks(b"not wav", 2).is_err());
    }
}
