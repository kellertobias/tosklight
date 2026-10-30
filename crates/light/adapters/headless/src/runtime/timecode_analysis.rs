//! Offline analysis of a Timecode's audio: a beat grid and suggested song sections.
//!
//! Analysing a song takes a few seconds, so it runs on its own thread, reports progress as a
//! `timecode_audio_analysis_changed` event, and can be cancelled. The result belongs to the audio
//! content, not to the show: it is cached under the desk's data directory by the audio's SHA-256,
//! so reopening a show, or linking the same file to another Timecode, never analyses it again.

use super::show_objects_v2::{active_entry, validate_request_id};
use super::timecode_v2::load_timecode;
use super::timecode_waveform::{WaveformSink, wav_pcm};
use super::*;
use light_wire::v2::timecode as wire;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

/// Timecode frames per second, as every Timecode stores time.
const FPS: f64 = 44.0;

#[derive(Clone)]
struct Job {
    snapshot: wire::TimecodeAudioAnalysisSnapshot,
    cancel: Arc<AtomicBool>,
}

type JobKey = (Uuid, Uuid);

fn jobs() -> &'static Mutex<HashMap<JobKey, Job>> {
    static JOBS: OnceLock<Mutex<HashMap<JobKey, Job>>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}

struct LinkedAudio {
    show_id: Uuid,
    timecode_id: Uuid,
    asset_id: Uuid,
    asset_revision: u64,
    digest: String,
}

fn linked_audio(
    state: &AppState,
    context: &ShowContext,
    timecode_id: Uuid,
) -> Result<LinkedAudio, ApiError> {
    let show_id = context.resolve(state)?;
    let entry = active_entry(state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let object = load_timecode(&store, timecode_id)?;
    let definition = serde_json::from_value::<light_playback::TimecodeDefinition>(object.body)
        .map_err(|error| ApiError::internal(format!("stored Timecode is invalid: {error}")))?;
    let audio = definition
        .audio
        .ok_or_else(|| ApiError::not_found("Timecode has no linked audio"))?;
    let digest = state
        .managed_assets
        .revisions(light_application::AssetId(audio.asset_id))
        .map_err(|error| ApiError::internal(error.message))?
        .into_iter()
        .find(|descriptor| descriptor.asset.revision.0 == audio.asset_revision)
        .map(|descriptor| descriptor.digest)
        .ok_or_else(|| ApiError::not_found("linked audio revision"))?;
    Ok(LinkedAudio {
        show_id: show_id.0,
        timecode_id,
        asset_id: audio.asset_id,
        asset_revision: audio.asset_revision,
        digest,
    })
}

fn cache_path(state: &AppState, digest: &str) -> Option<std::path::PathBuf> {
    // The digest is a hex string from the asset store; anything else never names a file.
    digest
        .chars()
        .all(|character| character.is_ascii_hexdigit())
        .then(|| {
            state
                .installation
                .data_dir()
                .join("audio-analysis")
                .join(format!("{digest}.json"))
        })
}

fn cached(state: &AppState, digest: &str) -> Option<wire::TimecodeAudioAnalysis> {
    let bytes = std::fs::read(cache_path(state, digest)?).ok()?;
    serde_json::from_slice::<wire::TimecodeAudioAnalysis>(&bytes)
        .ok()
        .filter(|analysis| analysis.digest == digest)
}

fn snapshot(
    audio: &LinkedAudio,
    state: wire::TimecodeAudioAnalysisState,
    progress: f32,
    error: Option<String>,
    analysis: Option<wire::TimecodeAudioAnalysis>,
) -> wire::TimecodeAudioAnalysisSnapshot {
    wire::TimecodeAudioAnalysisSnapshot {
        timecode_id: audio.timecode_id,
        asset_id: audio.asset_id,
        asset_revision: audio.asset_revision,
        state,
        progress,
        error,
        analysis,
    }
}

/// What is known about this Timecode's current audio, without starting anything.
fn current(state: &AppState, audio: &LinkedAudio) -> wire::TimecodeAudioAnalysisSnapshot {
    let key = (audio.show_id, audio.timecode_id);
    if let Some(job) = jobs().lock().unwrap().get(&key)
        && job.snapshot.asset_id == audio.asset_id
        && job.snapshot.asset_revision == audio.asset_revision
        && job.snapshot.state != wire::TimecodeAudioAnalysisState::Ready
    {
        return job.snapshot.clone();
    }
    match cached(state, &audio.digest) {
        Some(analysis) => snapshot(
            audio,
            wire::TimecodeAudioAnalysisState::Ready,
            1.0,
            None,
            Some(analysis),
        ),
        None => snapshot(
            audio,
            wire::TimecodeAudioAnalysisState::Idle,
            0.0,
            None,
            None,
        ),
    }
}

fn publish(state: &AppState, key: JobKey, update: wire::TimecodeAudioAnalysisSnapshot) {
    if let Some(job) = jobs().lock().unwrap().get_mut(&key) {
        job.snapshot = update.clone();
    }
    emit(
        state,
        "timecode_audio_analysis_changed",
        serde_json::to_value(&update).unwrap_or_default(),
    );
}

fn frames(seconds: &[f64]) -> Vec<u64> {
    seconds
        .iter()
        .map(|time| (time * FPS).round().max(0.0) as u64)
        .collect()
}

fn run(state: AppState, audio: LinkedAudio, cancel: Arc<AtomicBool>) {
    let key = (audio.show_id, audio.timecode_id);
    let fail = |message: String| {
        publish(
            &state,
            key,
            snapshot(
                &audio,
                wire::TimecodeAudioAnalysisState::Failed,
                0.0,
                Some(message),
                None,
            ),
        );
    };
    let mut sink = WaveformSink(Vec::new());
    if let Err(error) = state.managed_assets.stream(
        light_application::AssetReference {
            id: light_application::AssetId(audio.asset_id),
            revision: light_application::AssetRevision(audio.asset_revision),
        },
        &mut sink,
    ) {
        return fail(format!(
            "Could not read the linked audio: {}",
            error.message
        ));
    }
    let (samples, sample_rate) = match wav_pcm(&sink.0) {
        Ok(pcm) => (pcm.mono(), pcm.sample_rate),
        Err(message) => return fail(format!("Could not decode the linked audio: {message}")),
    };
    let mut reported = -1.0f32;
    let result = light_beat::analyse_song(&samples, sample_rate as f32, |fraction| {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        if fraction - reported >= 0.05 || fraction >= 1.0 {
            reported = fraction;
            publish(
                &state,
                key,
                snapshot(
                    &audio,
                    wire::TimecodeAudioAnalysisState::Running,
                    fraction,
                    None,
                    None,
                ),
            );
        }
        true
    });
    let Some(song) = result else {
        return publish(
            &state,
            key,
            snapshot(
                &audio,
                wire::TimecodeAudioAnalysisState::Cancelled,
                0.0,
                None,
                None,
            ),
        );
    };
    let analysis = wire::TimecodeAudioAnalysis {
        digest: audio.digest.clone(),
        bpm: song.bpm,
        beat_frames: frames(&song.beats_seconds),
        downbeat_frames: frames(&song.downbeats_seconds),
        sections: song
            .sections
            .iter()
            .map(|section| wire::TimecodeAudioSection {
                frame: (section.start_seconds * FPS).round().max(0.0) as u64,
                label: section.label.name().to_owned(),
                energy: section.energy,
            })
            .collect(),
    };
    if let Some(path) = cache_path(&state, &audio.digest) {
        let written = path
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(&path, serde_json::to_vec(&analysis).unwrap_or_default()));
        if let Err(error) = written {
            tracing::warn!(%error, "could not cache the audio analysis");
        }
    }
    publish(
        &state,
        key,
        snapshot(
            &audio,
            wire::TimecodeAudioAnalysisState::Ready,
            1.0,
            None,
            Some(analysis),
        ),
    );
}

pub(super) async fn analysis_snapshot(
    State(state): State<AppState>,
    context: ShowContext,
    Path(timecode_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeAudioAnalysisSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let audio = linked_audio(&state, &context, timecode_id)?;
    Ok(Json(current(&state, &audio)))
}

pub(super) async fn start_analysis(
    State(state): State<AppState>,
    context: ShowContext,
    Path(timecode_id): Path<Uuid>,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::TimecodeAudioAnalysisRequest>,
) -> Result<Json<wire::TimecodeAudioAnalysisSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    validate_request_id(&request.request_id)?;
    let audio = linked_audio(&state, &context, timecode_id)?;
    let existing = current(&state, &audio);
    if matches!(
        existing.state,
        wire::TimecodeAudioAnalysisState::Ready | wire::TimecodeAudioAnalysisState::Running
    ) {
        return Ok(Json(existing));
    }
    let started = snapshot(
        &audio,
        wire::TimecodeAudioAnalysisState::Running,
        0.0,
        None,
        None,
    );
    let cancel = Arc::new(AtomicBool::new(false));
    jobs().lock().unwrap().insert(
        (audio.show_id, audio.timecode_id),
        Job {
            snapshot: started.clone(),
            cancel: cancel.clone(),
        },
    );
    emit(
        &state,
        "timecode_audio_analysis_changed",
        serde_json::to_value(&started).unwrap_or_default(),
    );
    let worker = state.clone();
    std::thread::Builder::new()
        .name("timecode-audio-analysis".into())
        .spawn(move || run(worker, audio, cancel))
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(started))
}

pub(super) async fn cancel_analysis(
    State(state): State<AppState>,
    context: ShowContext,
    Path(timecode_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<wire::TimecodeAudioAnalysisSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let audio = linked_audio(&state, &context, timecode_id)?;
    if let Some(job) = jobs()
        .lock()
        .unwrap()
        .get(&(audio.show_id, audio.timecode_id))
    {
        job.cancel.store(true, Ordering::Relaxed);
    }
    Ok(Json(current(&state, &audio)))
}
