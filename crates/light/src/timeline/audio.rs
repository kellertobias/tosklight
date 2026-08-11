use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};

use crate::{
    AssetChunkSource, AssetDescriptor, AssetError, AssetErrorKind, AssetId, AssetNamespace,
    AssetReference, ImportAssetRequest, ManagedAssetStore,
};
use light_playback::{
    TimecodeFrame, TimecodeFrameRate, TimecodeId, TimecodeTransportAction, TimecodeTransportState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WavMetadata {
    pub encoding: WavEncoding,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub data_bytes: u64,
    pub sample_frames: u64,
}

impl WavMetadata {
    pub fn duration_frames(self, rate: TimecodeFrameRate) -> TimecodeFrame {
        let scaled = u128::from(self.sample_frames) * u128::from(rate.numerator());
        let divisor = u128::from(self.sample_rate) * u128::from(rate.denominator());
        TimecodeFrame(u64::try_from(scaled.div_ceil(divisor)).unwrap_or(u64::MAX))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WavEncoding {
    PcmInteger,
    IeeeFloat,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedWavAsset {
    pub descriptor: AssetDescriptor,
    pub metadata: WavMetadata,
}

/// Validates supported WAV structure before bytes enter the managed show asset store.
pub struct TimecodeWavImporter {
    store: Arc<dyn ManagedAssetStore>,
}

impl TimecodeWavImporter {
    pub fn new(store: Arc<dyn ManagedAssetStore>) -> Self {
        Self { store }
    }

    pub fn import(
        &self,
        identity: Option<AssetId>,
        namespace: AssetNamespace,
        name: String,
        source: &mut dyn AssetChunkSource,
    ) -> Result<ImportedWavAsset, AssetError> {
        let mut bytes = Vec::new();
        while let Some(chunk) = source.read_chunk(64 * 1024)? {
            bytes.extend_from_slice(&chunk);
        }
        let metadata = parse_wav_metadata(&bytes)?;
        self.store_wav(identity, namespace, name, bytes, metadata)
    }

    /// Decodes an MP3 completely during import and stores a portable PCM16 WAV revision.
    /// Runtime playback therefore never depends on an MP3 decoder, source seek tables, or
    /// platform codec availability.
    pub fn import_mp3(
        &self,
        identity: Option<AssetId>,
        namespace: AssetNamespace,
        name: String,
        source: &mut dyn AssetChunkSource,
    ) -> Result<ImportedWavAsset, AssetError> {
        let mut mp3 = Vec::new();
        while let Some(chunk) = source.read_chunk(64 * 1024)? {
            mp3.extend_from_slice(&chunk);
        }
        let bytes = normalize_mp3_to_wav(&mp3)?;
        let metadata = parse_wav_metadata(&bytes)?;
        let name = normalized_wav_name(&name);
        self.store_wav(identity, namespace, name, bytes, metadata)
    }

    fn store_wav(
        &self,
        identity: Option<AssetId>,
        namespace: AssetNamespace,
        name: String,
        bytes: Vec<u8>,
        metadata: WavMetadata,
    ) -> Result<ImportedWavAsset, AssetError> {
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let descriptor = self.store.import(
            ImportAssetRequest {
                identity,
                namespace,
                name,
                media_type: "audio/wav".into(),
                declared_length: bytes.len() as u64,
                declared_digest: digest,
            },
            &mut BytesSource::new(bytes),
        )?;
        Ok(ImportedWavAsset {
            descriptor,
            metadata,
        })
    }
}

fn normalized_wav_name(name: &str) -> String {
    let trimmed = name.trim();
    match trimmed.rsplit_once('.') {
        Some((stem, extension)) if extension.eq_ignore_ascii_case("mp3") => format!("{stem}.wav"),
        _ if trimmed.to_ascii_lowercase().ends_with(".wav") => trimmed.to_owned(),
        _ => format!("{trimmed}.wav"),
    }
}

fn normalize_mp3_to_wav(bytes: &[u8]) -> Result<Vec<u8>, AssetError> {
    use std::io::Cursor;
    use symphonia::core::{
        audio::SampleBuffer,
        codecs::{CODEC_TYPE_NULL, DecoderOptions},
        errors::Error as SymphoniaError,
        formats::FormatOptions,
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    };

    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(mp3_decode_error)?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| invalid_mp3("MP3 contains no decodable audio track"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(mp3_decode_error)?;
    let mut samples = Vec::<f32>::new();
    let mut sample_rate = None;
    let mut channels = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(mp3_decode_error(error)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(mp3_decode_error(error)),
        };
        let spec = *decoded.spec();
        let current_rate = spec.rate;
        let current_channels = u16::try_from(spec.channels.count())
            .map_err(|_| invalid_mp3("MP3 channel count is too large"))?;
        if sample_rate.is_some_and(|rate| rate != current_rate)
            || channels.is_some_and(|count| count != current_channels)
        {
            return Err(invalid_mp3(
                "MP3 changes sample rate or channel layout during the stream",
            ));
        }
        sample_rate = Some(current_rate);
        channels = Some(current_channels);
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buffer.samples());
    }
    let sample_rate = sample_rate.ok_or_else(|| invalid_mp3("MP3 decoded no audio samples"))?;
    let channels = channels.ok_or_else(|| invalid_mp3("MP3 decoded no channel layout"))?;
    encode_pcm16_wav(&samples, sample_rate, channels)
}

fn encode_pcm16_wav(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<u8>, AssetError> {
    if sample_rate == 0 || channels == 0 || samples.is_empty() {
        return Err(invalid_mp3("decoded MP3 audio must be non-empty"));
    }
    let data_length = samples
        .len()
        .checked_mul(2)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| invalid_mp3("decoded MP3 is too large for a WAV file"))?;
    let block_align = channels
        .checked_mul(2)
        .ok_or_else(|| invalid_mp3("decoded MP3 channel layout is too large"))?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(block_align))
        .ok_or_else(|| invalid_mp3("decoded MP3 byte rate overflows WAV metadata"))?;
    let riff_length = 36_u32
        .checked_add(data_length)
        .ok_or_else(|| invalid_mp3("decoded MP3 is too large for a WAV file"))?;
    let mut wav = Vec::with_capacity(riff_length as usize + 8);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_length.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_length.to_le_bytes());
    for sample in samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        wav.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(wav)
}

fn invalid_mp3(message: impl Into<String>) -> AssetError {
    AssetError::new(
        AssetErrorKind::Invalid,
        format!("invalid MP3: {}", message.into()),
    )
}

fn mp3_decode_error(error: symphonia::core::errors::Error) -> AssetError {
    invalid_mp3(error.to_string())
}

pub fn parse_wav_metadata(bytes: &[u8]) -> Result<WavMetadata, AssetError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(invalid_wav("expected a RIFF/WAVE header"));
    }
    let riff_length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if riff_length.saturating_add(8) > bytes.len() {
        return Err(invalid_wav("RIFF length exceeds the supplied bytes"));
    }
    let mut cursor = 12_usize;
    let mut format = None;
    let mut data_bytes = None;
    while cursor.saturating_add(8) <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let length = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        cursor += 8;
        let end = cursor
            .checked_add(length)
            .ok_or_else(|| invalid_wav("chunk length overflow"))?;
        if end > bytes.len() {
            return Err(invalid_wav("chunk length exceeds the supplied bytes"));
        }
        if id == b"fmt " {
            if length < 16 {
                return Err(invalid_wav("fmt chunk is shorter than 16 bytes"));
            }
            format = Some((
                u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap()),
                u16::from_le_bytes(bytes[cursor + 2..cursor + 4].try_into().unwrap()),
                u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()),
                u16::from_le_bytes(bytes[cursor + 12..cursor + 14].try_into().unwrap()),
                u16::from_le_bytes(bytes[cursor + 14..cursor + 16].try_into().unwrap()),
            ));
        } else if id == b"data" {
            data_bytes = Some(length as u64);
        }
        cursor = end.saturating_add(length & 1);
    }
    let (encoding, channels, sample_rate, block_align, bits_per_sample) =
        format.ok_or_else(|| invalid_wav("fmt chunk is missing"))?;
    let encoding = match encoding {
        1 => WavEncoding::PcmInteger,
        3 => WavEncoding::IeeeFloat,
        _ => {
            return Err(invalid_wav(
                "only PCM integer and IEEE float WAV are supported",
            ));
        }
    };
    let data_bytes = data_bytes.ok_or_else(|| invalid_wav("data chunk is missing"))?;
    if channels == 0 || sample_rate == 0 || block_align == 0 || bits_per_sample == 0 {
        return Err(invalid_wav("WAV format values must be non-zero"));
    }
    let expected_block_align = u32::from(channels) * u32::from(bits_per_sample).div_ceil(8);
    if u32::from(block_align) != expected_block_align || data_bytes % u64::from(block_align) != 0 {
        return Err(invalid_wav("WAV block alignment or data length is invalid"));
    }
    Ok(WavMetadata {
        encoding,
        channels,
        sample_rate,
        bits_per_sample,
        data_bytes,
        sample_frames: data_bytes / u64::from(block_align),
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimecodeAudioState {
    pub transport: TimecodeTransportState,
    pub frame: TimecodeFrame,
    pub volume: f64,
    pub looping: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimecodeAudioCommand {
    Prepare {
        timecode_id: TimecodeId,
        asset: AssetReference,
        sample_rate: u32,
        channels: u16,
        timeline_rate: TimecodeFrameRate,
    },
    Play {
        timecode_id: TimecodeId,
        source_frame: TimecodeFrame,
        audible_at_micros: u64,
    },
    Pause {
        timecode_id: TimecodeId,
    },
    Stop {
        timecode_id: TimecodeId,
    },
    Seek {
        timecode_id: TimecodeId,
        source_frame: TimecodeFrame,
        audible_at_micros: u64,
    },
    SetLoop {
        timecode_id: TimecodeId,
        enabled: bool,
        end_exclusive: TimecodeFrame,
    },
    SetVolume {
        timecode_id: TimecodeId,
        linear: f64,
    },
}

/// Server-owned output boundary. Implementations honor the supplied audible deadline rather than
/// introducing a second transport clock.
pub trait TimecodeAudioOutput: Send + Sync {
    fn output_latency_micros(&self) -> u64;
    fn apply(&self, command: TimecodeAudioCommand) -> Result<(), String>;
}

#[derive(Clone, Debug)]
struct PreparedAudio {
    duration: TimecodeFrame,
    state: TimecodeAudioState,
}

pub struct TimecodeAudioService {
    output: Arc<dyn TimecodeAudioOutput>,
    prepared: Mutex<BTreeMap<TimecodeId, PreparedAudio>>,
}

impl TimecodeAudioService {
    pub fn new(output: Arc<dyn TimecodeAudioOutput>) -> Self {
        Self {
            output,
            prepared: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn is_prepared(&self, timecode_id: TimecodeId) -> bool {
        self.prepared
            .lock()
            .expect("Timecode audio lock poisoned")
            .contains_key(&timecode_id)
    }

    pub fn prepare(
        &self,
        timecode_id: TimecodeId,
        asset: AssetReference,
        metadata: WavMetadata,
        rate: TimecodeFrameRate,
        looping: bool,
    ) -> Result<TimecodeFrame, String> {
        let duration = metadata.duration_frames(rate);
        if duration == TimecodeFrame::ZERO {
            return Err("Timecode audio duration must be positive".into());
        }
        self.output.apply(TimecodeAudioCommand::Prepare {
            timecode_id,
            asset,
            sample_rate: metadata.sample_rate,
            channels: metadata.channels,
            timeline_rate: rate,
        })?;
        self.output.apply(TimecodeAudioCommand::SetLoop {
            timecode_id,
            enabled: looping,
            end_exclusive: duration,
        })?;
        self.prepared
            .lock()
            .expect("Timecode audio lock poisoned")
            .insert(
                timecode_id,
                PreparedAudio {
                    duration,
                    state: TimecodeAudioState {
                        transport: TimecodeTransportState::Stopped,
                        frame: TimecodeFrame::ZERO,
                        volume: 1.0,
                        looping,
                    },
                },
            );
        Ok(duration)
    }

    pub fn handle(
        &self,
        timecode_id: TimecodeId,
        action: TimecodeTransportAction,
        now_micros: u64,
    ) -> Result<TimecodeAudioState, String> {
        let audible_at_micros = now_micros.saturating_add(self.output.output_latency_micros());
        let mut prepared = self.prepared.lock().expect("Timecode audio lock poisoned");
        let audio = prepared
            .get_mut(&timecode_id)
            .ok_or("Timecode audio is not prepared")?;
        match action {
            TimecodeTransportAction::Go | TimecodeTransportAction::Rewind => {
                audio.state.transport = TimecodeTransportState::Playing;
                audio.state.frame = TimecodeFrame::ZERO;
                self.output.apply(TimecodeAudioCommand::Play {
                    timecode_id,
                    source_frame: TimecodeFrame::ZERO,
                    audible_at_micros,
                })?;
            }
            TimecodeTransportAction::Pause => match audio.state.transport {
                TimecodeTransportState::Playing => {
                    audio.state.transport = TimecodeTransportState::Paused;
                    self.output
                        .apply(TimecodeAudioCommand::Pause { timecode_id })?;
                }
                TimecodeTransportState::Paused => {
                    audio.state.transport = TimecodeTransportState::Playing;
                    self.output.apply(TimecodeAudioCommand::Play {
                        timecode_id,
                        source_frame: audio.state.frame,
                        audible_at_micros,
                    })?;
                }
                TimecodeTransportState::Stopped => {}
            },
            TimecodeTransportAction::Stop => {
                audio.state.transport = TimecodeTransportState::Stopped;
                audio.state.frame = TimecodeFrame::ZERO;
                self.output
                    .apply(TimecodeAudioCommand::Stop { timecode_id })?;
            }
            TimecodeTransportAction::Seek { frame } => {
                audio.state.frame = TimecodeFrame(frame.0.min(audio.duration.0));
                self.output.apply(TimecodeAudioCommand::Seek {
                    timecode_id,
                    source_frame: audio.state.frame,
                    audible_at_micros,
                })?;
            }
        }
        Ok(audio.state)
    }

    pub fn synchronize(
        &self,
        timecode_id: TimecodeId,
        frame: TimecodeFrame,
        volume: f64,
        now_micros: u64,
    ) -> Result<TimecodeAudioState, String> {
        let audible_at_micros = now_micros.saturating_add(self.output.output_latency_micros());
        let mut prepared = self.prepared.lock().expect("Timecode audio lock poisoned");
        let audio = prepared
            .get_mut(&timecode_id)
            .ok_or("Timecode audio is not prepared")?;
        let frame = if audio.state.looping {
            TimecodeFrame(frame.0 % audio.duration.0)
        } else {
            TimecodeFrame(frame.0.min(audio.duration.0))
        };
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if frame != audio.state.frame {
            audio.state.frame = frame;
            self.output.apply(TimecodeAudioCommand::Seek {
                timecode_id,
                source_frame: frame,
                audible_at_micros,
            })?;
        }
        if volume != audio.state.volume {
            audio.state.volume = volume;
            self.output.apply(TimecodeAudioCommand::SetVolume {
                timecode_id,
                linear: volume,
            })?;
        }
        Ok(audio.state)
    }
}

struct BytesSource {
    bytes: Vec<u8>,
    cursor: usize,
}
impl BytesSource {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, cursor: 0 }
    }
}
impl AssetChunkSource for BytesSource {
    fn read_chunk(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, AssetError> {
        if self.cursor == self.bytes.len() {
            return Ok(None);
        }
        let end = self
            .cursor
            .saturating_add(maximum_bytes)
            .min(self.bytes.len());
        let chunk = self.bytes[self.cursor..end].to_vec();
        self.cursor = end;
        Ok(Some(chunk))
    }
}

fn invalid_wav(message: impl Into<String>) -> AssetError {
    AssetError::new(
        AssetErrorKind::Invalid,
        format!("invalid WAV: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    const TINY_MP3: &str = "SUQzBAAAAAAAI1RTU0UAAAAPAAADTGF2ZjYyLjEyLjEwMQAAAAAAAAAAAAAA/+NIwAAAAAAAAAAAAEluZm8AAAAPAAAAAwAABIAAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA////////////////////////////////////////////AAAAAExhdmM2Mi4yOAAAAAAAAAAAAAAAACQC8AAAAAAAAASAkuQGUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/+NIxAA5Yv5QAVoYAUA6Acu+XLLloB0V0x1jqCJCIBC5BcgvAg4oIsRU6EstmWTLJlx23vJzgIIkaZVecWyenqfv2fXWdmGakWpqydeBdwvAg4po1yWZyt/3Lct/43b05hAAAAEocAEbuegABgb/XDgYAAIiIgAAAYGLd3cDAwAAEIiBAAAAwMDd3DgYGAAAAiIEAAADAwMW7hwMDAAAAEIgQAAAMDAxbu4GLAAAAREQIIAwMW7u7iwAAREREQW7u7u7iEAAAAAw8PDw8AAAAAAw8PDw8AAAAAAw8PDw8AAAAAAw8PDw8AAAAAAw8PDw8AAFAMMMDhlKlSKwJASaLLJS/YCACAksiXhiRbEwECxCADA4MLaK8ftVR8QIDQaC/+NIxC1G67JwWZzAAYweEDDouAAVO4JIzmTkNDH4pO7qAy8HDBoMMCDkzQfzNhTChzQUxFUqSKSGV+yBaha9TeBwLA1gWc7TDkTnBXau19nKa1EX9dtTNa5axTSKOQ1hpLLZyUv61mJQ8/sqjUNVYzGa1NTOosd377vuXF3Eln15VGn+q2blWUxmrGYzWpqarS0uVNaa45ECO/P4OW/8XlnLdbKrWpqbfK1Wls1aWlrU1Nulx1llvF3H8nLerD/xu3yvbhyMa3VrU1r9VqtLj+PNZZ/jr/3r62ONXLKxhSTjvxu3TyufhyWcpJROQ/T1a2XdZVscf3jVyKhSgvICgAQkFouyldru2JTLYKS+LlJXKKmaJ0mjUDDmxpoCpFVY/+NIxCQ7BDn4A9hgAAJDSg0mQAvYwJIZQaHwsE2VO004gQc0FS5SOLJJ9JGmAasSqpd1B1dM40pL5Io1AsAEOyUSUQdABFklA2BsTrnIgg1jOSa60fPkkGqo5EkxdxcZGXNLvZW0OhKdaMj57WVrv1rjT2nJi8ytW9lrWnJ2tcXGXNLnvrXpmZta2rVvsrazb52Z2texpd1rWmvZaZta06yt+tem07X9rWs5a3Wt+1m1uy1vna1nazlprX9rXpy1ptaa1lpd1rW+a1rMzNraytAoKw7tTEFNRTMuMTAwqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";

    #[derive(Default)]
    struct RecordingOutput {
        commands: Mutex<Vec<TimecodeAudioCommand>>,
        latency: u64,
    }
    impl TimecodeAudioOutput for RecordingOutput {
        fn output_latency_micros(&self) -> u64 {
            self.latency
        }
        fn apply(&self, command: TimecodeAudioCommand) -> Result<(), String> {
            self.commands.lock().unwrap().push(command);
            Ok(())
        }
    }

    fn wav(sample_rate: u32, channels: u16, samples: u32) -> Vec<u8> {
        let data_length = samples * u32::from(channels) * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        bytes.resize(bytes.len() + data_length as usize, 0);
        bytes
    }

    #[test]
    fn wav_metadata_is_validated_and_duration_uses_rational_rate() {
        let metadata = parse_wav_metadata(&wav(48_000, 2, 48_000)).unwrap();
        assert_eq!(metadata.channels, 2);
        assert_eq!(metadata.sample_frames, 48_000);
        assert_eq!(
            metadata.duration_frames(TimecodeFrameRate::whole_frames(44).unwrap()),
            TimecodeFrame(44)
        );
        assert_eq!(
            parse_wav_metadata(b"not a wav").unwrap_err().kind,
            AssetErrorKind::Invalid
        );
    }

    #[test]
    fn mp3_normalizes_to_portable_pcm16_wav() {
        let mp3 = base64::engine::general_purpose::STANDARD
            .decode(TINY_MP3)
            .unwrap();
        let wav = normalize_mp3_to_wav(&mp3).unwrap();
        let metadata = parse_wav_metadata(&wav).unwrap();
        assert_eq!(metadata.encoding, WavEncoding::PcmInteger);
        assert_eq!(metadata.bits_per_sample, 16);
        assert_eq!(metadata.sample_rate, 8_000);
        assert_eq!(metadata.channels, 1);
        assert!(metadata.sample_frames > 0);
        assert_eq!(normalized_wav_name("walk-in.MP3"), "walk-in.wav");
    }

    #[test]
    fn transport_seek_loop_volume_and_latency_commands_are_deterministic() {
        let output = Arc::new(RecordingOutput {
            commands: Mutex::new(Vec::new()),
            latency: 12_500,
        });
        let service = TimecodeAudioService::new(output.clone());
        let id = TimecodeId(uuid::Uuid::from_u128(7));
        let asset = AssetReference {
            id: AssetId(uuid::Uuid::from_u128(8)),
            revision: crate::AssetRevision(2),
        };
        let metadata = parse_wav_metadata(&wav(48_000, 2, 48_000)).unwrap();
        service
            .prepare(
                id,
                asset,
                metadata,
                TimecodeFrameRate::whole_frames(44).unwrap(),
                true,
            )
            .unwrap();
        service
            .handle(id, TimecodeTransportAction::Go, 1_000_000)
            .unwrap();
        service
            .synchronize(id, TimecodeFrame(45), 0.4, 1_010_000)
            .unwrap();
        service
            .handle(id, TimecodeTransportAction::Pause, 1_020_000)
            .unwrap();
        service
            .handle(id, TimecodeTransportAction::Pause, 1_030_000)
            .unwrap();
        service
            .handle(id, TimecodeTransportAction::Stop, 1_040_000)
            .unwrap();
        let commands = output.commands.lock().unwrap();
        assert_eq!(
            commands[2],
            TimecodeAudioCommand::Play {
                timecode_id: id,
                source_frame: TimecodeFrame::ZERO,
                audible_at_micros: 1_012_500
            }
        );
        assert!(commands.contains(&TimecodeAudioCommand::Seek {
            timecode_id: id,
            source_frame: TimecodeFrame(1),
            audible_at_micros: 1_022_500
        }));
        assert!(commands.contains(&TimecodeAudioCommand::SetVolume {
            timecode_id: id,
            linear: 0.4
        }));
        assert_eq!(
            commands.last(),
            Some(&TimecodeAudioCommand::Stop { timecode_id: id })
        );
    }
}
