//! Waveform peaks for the audio a Timecode is linked to.
//!
//! The audio lane is the surface an operator aligns markers against, so this resolves far more
//! detail than a lane-wide overview would need and lets a caller ask for more still.

use super::show_objects_v2::active_entry;
use super::timecode_v2::load_timecode;
use super::*;
use light_wire::v2::timecode as wire;

pub(super) struct WaveformSink(Vec<u8>);

impl light_application::AssetChunkSink for WaveformSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

/// Waveform buckets returned when the caller does not ask for a resolution.
///
/// Markers are aligned against this waveform, so it carries far more detail than a lane-width
/// overview would need; the lane stretches one fetched waveform across every zoom level.
pub(super) const AUDIO_WAVEFORM_BUCKETS: usize = 4_096;

/// Upper bound on requested waveform buckets, so one request cannot pin the runtime.
pub(super) const AUDIO_WAVEFORM_BUCKETS_MAXIMUM: usize = 16_384;

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

pub(super) fn pcm_waveform_peaks(bytes: &[u8], bucket_count: usize) -> Result<Vec<f32>, String> {
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

#[cfg(test)]
mod waveform_tests {
    use super::{AUDIO_WAVEFORM_BUCKETS, AUDIO_WAVEFORM_BUCKETS_MAXIMUM, pcm_waveform_peaks};

    /// Builds a mono PCM16 WAV whose amplitude ramps across `frames` samples.
    fn ramp_wav(frames: usize) -> Vec<u8> {
        let data = (0..frames)
            .flat_map(|frame| {
                let value = ((frame % 128) as f32 / 127.0 * f32::from(i16::MAX)) as i16;
                value.to_le_bytes()
            })
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
        wav
    }

    #[test]
    fn the_default_resolution_resolves_far_more_than_a_lane_wide_overview() {
        // One minute at 44.1 kHz, which is shorter than most show audio.
        let wav = ramp_wav(44_100 * 60);
        let peaks = pcm_waveform_peaks(&wav, AUDIO_WAVEFORM_BUCKETS).unwrap();

        assert_eq!(peaks.len(), AUDIO_WAVEFORM_BUCKETS);
        // The previous fixed 220 buckets left roughly a quarter-second per bucket at this length.
        assert!(AUDIO_WAVEFORM_BUCKETS > 220 * 10);
        assert!(peaks.iter().all(|peak| (0.0..=1.0).contains(peak)));
    }

    #[test]
    fn a_request_cannot_ask_for_more_buckets_than_the_audio_has_frames() {
        let wav = ramp_wav(64);
        let peaks = pcm_waveform_peaks(&wav, AUDIO_WAVEFORM_BUCKETS_MAXIMUM).unwrap();

        assert_eq!(
            peaks.len(),
            64,
            "buckets are capped at one per sample frame"
        );
    }

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
