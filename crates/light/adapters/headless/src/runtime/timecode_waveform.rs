//! Waveform peaks for the audio a Timecode is linked to.
//!
//! The audio lane is the surface an operator aligns markers against, so this resolves far more
//! detail than a lane-wide overview would need and lets a caller ask for more still.

use super::show_objects_v2::active_entry;
use super::timecode_v2::load_timecode;
use super::*;
use light_wire::v2::timecode as wire;

pub(super) struct WaveformSink(pub(super) Vec<u8>);

impl light_application::AssetChunkSink for WaveformSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

/// Waveform buckets returned when the caller does not ask for a resolution.
///
/// Markers are aligned against this waveform, so it carries far more detail than a lane-width
/// overview would need; the lane stretches one fetched waveform across every zoom level. At a
/// deep zoom a coarse envelope reads as straight lines between distant peaks, so the resolution
/// is set for the zoomed-in lane rather than for the whole track at once.
pub(super) const AUDIO_WAVEFORM_BUCKETS: usize = 98_304;

/// Upper bound on requested waveform buckets, so one request cannot pin the runtime.
pub(super) const AUDIO_WAVEFORM_BUCKETS_MAXIMUM: usize = 196_608;

#[derive(Deserialize)]
pub(super) struct AudioWaveformQuery {
    pub(super) resolution: Option<usize>,
}

pub(super) async fn audio_waveform(
    State(state): State<AppState>,
    context: ShowContext,
    Path(timecode_id): Path<Uuid>,
    Query(query): Query<AudioWaveformQuery>,
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
    let buckets = query
        .resolution
        .unwrap_or(AUDIO_WAVEFORM_BUCKETS)
        .clamp(1, AUDIO_WAVEFORM_BUCKETS_MAXIMUM);
    let peaks = pcm_waveform_peaks(&sink.0, buckets)
        .map_err(|message| ApiError::bad_request(format!("linked audio waveform: {message}")))?;
    Ok(Json(wire::TimecodeAudioWaveform { peaks }))
}

/// The PCM data of a stored Timecode WAV: what the waveform and the analysis both read.
pub(super) struct WavPcm<'a> {
    format: u16,
    channels: usize,
    bits: u16,
    pub(super) sample_rate: u32,
    samples: &'a [u8],
}

pub(super) fn wav_pcm(bytes: &[u8]) -> Result<WavPcm<'_>, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("asset is not a RIFF/WAVE file".into());
    }
    let mut cursor = 12usize;
    let mut format = None;
    let mut channels = None;
    let mut bits = None;
    let mut sample_rate = None;
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
            sample_rate = Some(u32::from_le_bytes(
                bytes[start + 4..start + 8].try_into().unwrap(),
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
    if channels == 0 || !matches!((format, bits), (1, 16) | (3, 32)) {
        return Err("only PCM16 or IEEE-float32 WAV waveform data is supported".into());
    }
    Ok(WavPcm {
        format,
        channels,
        bits,
        sample_rate: sample_rate.unwrap_or(48_000),
        samples,
    })
}

impl WavPcm<'_> {
    fn frame_count(&self) -> usize {
        self.samples.len() / (self.channels * usize::from(self.bits / 8))
    }

    fn sample(&self, frame: usize, channel: usize) -> f32 {
        let bytes_per_sample = usize::from(self.bits / 8);
        let offset = (frame * self.channels + channel) * bytes_per_sample;
        if self.format == 1 {
            f32::from(i16::from_le_bytes(
                self.samples[offset..offset + 2].try_into().unwrap(),
            )) / f32::from(i16::MAX)
        } else {
            f32::from_le_bytes(self.samples[offset..offset + 4].try_into().unwrap())
        }
    }

    /// Every frame averaged across channels.
    pub(super) fn mono(&self) -> Vec<f32> {
        (0..self.frame_count())
            .map(|frame| {
                (0..self.channels)
                    .map(|channel| self.sample(frame, channel))
                    .sum::<f32>()
                    / self.channels as f32
            })
            .collect()
    }
}

pub(super) fn pcm_waveform_peaks(bytes: &[u8], bucket_count: usize) -> Result<Vec<f32>, String> {
    let pcm = wav_pcm(bytes)?;
    let (channels, frame_count) = (pcm.channels, pcm.frame_count());
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
                *peak = peak.max(pcm.sample(frame, channel).abs().min(1.0));
            }
        }
    }
    Ok(peaks)
}
