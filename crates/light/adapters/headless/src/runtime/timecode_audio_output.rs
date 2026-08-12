//! Native, server-owned Timecode audio output.
//!
//! The device callback only takes a non-blocking snapshot of prepared voices and mixes samples.
//! Asset reads and WAV decoding happen on the API/scheduler side before playback begins.

use std::{
    collections::{BTreeMap, HashMap},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use light_application::timeline::TimecodeClock;
use light_application::{
    AssetChunkSink, AssetReference, ManagedAssetStore, TimecodeAudioCommand, TimecodeAudioOutput,
};
use light_core::FixtureId;
use light_playback::{TimecodeFrame, TimecodeFrameRate, TimecodeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OutputDeviceSelector {
    SystemDefault,
    Name(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeTimecodeAudioConfig {
    pub device: OutputDeviceSelector,
    /// Signed operator calibration added to the detected device-buffer baseline.
    pub latency_trim_micros: i64,
}

impl Default for NativeTimecodeAudioConfig {
    fn default() -> Self {
        Self {
            device: OutputDeviceSelector::SystemDefault,
            latency_trim_micros: 0,
        }
    }
}

const OUTPUT_DEVICE_PROBE_ARGUMENT: &str = "--probe-timecode-audio-outputs";
const OUTPUT_DEVICE_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) fn run_output_device_probe_from_process() -> anyhow::Result<bool> {
    if std::env::args().nth(1).as_deref() != Some(OUTPUT_DEVICE_PROBE_ARGUMENT) {
        return Ok(false);
    }
    let devices = enumerate_output_devices().map_err(anyhow::Error::msg)?;
    serde_json::to_writer(std::io::stdout().lock(), &devices)?;
    Ok(true)
}

pub(super) fn output_devices() -> Result<Vec<String>, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Timecode audio output probe could not be located: {error}"))?;
    let mut command = Command::new(executable);
    command.arg(OUTPUT_DEVICE_PROBE_ARGUMENT);
    output_devices_from_command(&mut command)
}

fn enumerate_output_devices() -> Result<Vec<String>, String> {
    cpal::default_host()
        .output_devices()
        .map(|devices| {
            devices
                .filter_map(|device| device.name().ok())
                .filter(|name| !name.trim().is_empty())
                .collect()
        })
        .map_err(|error| format!("audio output devices could not be enumerated: {error}"))
}

fn output_devices_from_command(command: &mut Command) -> Result<Vec<String>, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Timecode audio output probe could not start: {error}"))?;
    let deadline = Instant::now() + OUTPUT_DEVICE_PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().map_err(|error| {
                    format!("Timecode audio output probe result could not be read: {error}")
                })?;
                if !output.status.success() {
                    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                    return Err(if detail.is_empty() {
                        format!(
                            "Timecode audio output discovery stopped unexpectedly ({})",
                            output.status
                        )
                    } else {
                        format!("Timecode audio output discovery failed: {detail}")
                    });
                }
                return serde_json::from_slice(&output.stdout).map_err(|error| {
                    format!("Timecode audio output discovery returned invalid data: {error}")
                });
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Timecode audio output discovery did not finish within 3 seconds".into(),
                );
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "Timecode audio output probe could not be monitored: {error}"
                ));
            }
        }
    }
}

pub(super) struct NativeTimecodeAudioOutput {
    store: Arc<dyn ManagedAssetStore>,
    latency_micros: u64,
    worker: Arc<super::TimecodeAudioWorkerResource>,
}

pub(in crate::runtime) enum NativeCommand {
    Prepare {
        timecode_id: TimecodeId,
        decoded: DecodedWav,
        timeline_rate: TimecodeFrameRate,
    },
    Transport(TimecodeAudioCommand),
    PrepareInternal {
        fixture_id: FixtureId,
        decoded: DecodedWav,
    },
    InternalTransport {
        fixture_id: FixtureId,
        action: NativeInternalTransport,
    },
    InternalRepeat {
        fixture_id: FixtureId,
        enabled: bool,
    },
    InternalVolume {
        fixture_id: FixtureId,
        linear: f32,
    },
    InternalSeek {
        fixture_id: FixtureId,
        cursor_millis: u32,
    },
    RemoveInternal {
        fixture_id: FixtureId,
    },
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum NativeInternalTransport {
    Stop,
    Pause,
    Play,
    RestartPlay,
}

#[derive(Clone)]
pub(in crate::runtime) struct NativeInternalAudioOutput {
    worker: Arc<super::TimecodeAudioWorkerResource>,
}

impl NativeTimecodeAudioOutput {
    pub(in crate::runtime) fn internal_output(&self) -> NativeInternalAudioOutput {
        NativeInternalAudioOutput {
            worker: Arc::clone(&self.worker),
        }
    }
    pub(super) fn open_with_timeout(
        store: Arc<dyn ManagedAssetStore>,
        clock: Arc<dyn TimecodeClock>,
        configuration: &NativeTimecodeAudioConfig,
    ) -> Result<Self, String> {
        let configuration = configuration.clone();
        let (started, startup) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("timecode-audio-startup".into())
            .spawn(move || {
                let _ = started.send(Self::open(store, clock, &configuration));
            })
            .map_err(|error| format!("Timecode audio startup worker could not start: {error}"))?;
        receive_startup(
            &startup,
            Duration::from_secs(3),
            "Timecode audio initialization did not finish within 3 seconds",
            "Timecode audio startup worker stopped unexpectedly",
        )?
    }

    pub(super) fn open(
        store: Arc<dyn ManagedAssetStore>,
        clock: Arc<dyn TimecodeClock>,
        configuration: &NativeTimecodeAudioConfig,
    ) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = select_device(&host, &configuration.device)?;
        let supported = device
            .default_output_config()
            .map_err(|error| format!("Timecode audio output has no usable format: {error}"))?;
        let output_sample_rate = supported.sample_rate().0;
        let baseline = buffer_latency_micros(supported.buffer_size(), output_sample_rate);
        let latency_micros = add_signed(baseline, configuration.latency_trim_micros);
        let config = supported.config();
        let format = supported.sample_format();
        let (commands, receiver) = std::sync::mpsc::channel();
        let (started, startup) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("timecode-audio-output".into())
            .spawn(move || {
                run_device(device, config, format, receiver, clock, started);
            })
            .map_err(|error| format!("Timecode audio output worker could not start: {error}"))?;
        receive_startup(
            &startup,
            Duration::from_secs(3),
            "Timecode audio output worker did not start within 3 seconds",
            "Timecode audio output worker stopped during startup",
        )??;
        Ok(Self {
            store,
            latency_micros,
            worker: Arc::new(super::TimecodeAudioWorkerResource::new(commands, worker)),
        })
    }
}

impl NativeInternalAudioOutput {
    pub(in crate::runtime) fn prepare(
        &self,
        fixture_id: FixtureId,
        wav: &[u8],
    ) -> Result<(), String> {
        self.request(NativeCommand::PrepareInternal {
            fixture_id,
            decoded: decode_wav(wav)?,
        })
    }

    pub(in crate::runtime) fn transport(
        &self,
        fixture_id: FixtureId,
        action: NativeInternalTransport,
    ) -> Result<(), String> {
        self.request(NativeCommand::InternalTransport { fixture_id, action })
    }

    pub(in crate::runtime) fn repeat(
        &self,
        fixture_id: FixtureId,
        enabled: bool,
    ) -> Result<(), String> {
        self.request(NativeCommand::InternalRepeat {
            fixture_id,
            enabled,
        })
    }

    pub(in crate::runtime) fn volume(
        &self,
        fixture_id: FixtureId,
        linear: f32,
    ) -> Result<(), String> {
        self.request(NativeCommand::InternalVolume { fixture_id, linear })
    }

    pub(in crate::runtime) fn seek(
        &self,
        fixture_id: FixtureId,
        cursor_millis: u32,
    ) -> Result<(), String> {
        self.request(NativeCommand::InternalSeek {
            fixture_id,
            cursor_millis,
        })
    }

    pub(in crate::runtime) fn remove(&self, fixture_id: FixtureId) -> Result<(), String> {
        self.request(NativeCommand::RemoveInternal { fixture_id })
    }

    fn request(&self, command: NativeCommand) -> Result<(), String> {
        self.worker.request(command)
    }
}

fn receive_startup<T>(
    receiver: &std::sync::mpsc::Receiver<T>,
    timeout: Duration,
    timeout_error: &str,
    disconnected_error: &str,
) -> Result<T, String> {
    receiver.recv_timeout(timeout).map_err(|error| match error {
        std::sync::mpsc::RecvTimeoutError::Timeout => timeout_error.to_owned(),
        std::sync::mpsc::RecvTimeoutError::Disconnected => disconnected_error.to_owned(),
    })
}

impl TimecodeAudioOutput for NativeTimecodeAudioOutput {
    fn output_latency_micros(&self) -> u64 {
        self.latency_micros
    }

    fn apply(&self, command: TimecodeAudioCommand) -> Result<(), String> {
        let command = match command {
            TimecodeAudioCommand::Prepare {
                timecode_id,
                asset,
                sample_rate,
                channels,
                timeline_rate,
            } => {
                let bytes = read_asset(&*self.store, asset)?;
                let decoded = decode_wav(&bytes)?;
                if decoded.sample_rate != sample_rate || decoded.channels != channels {
                    return Err("Timecode WAV metadata changed between import and prepare".into());
                }
                NativeCommand::Prepare {
                    timecode_id,
                    decoded,
                    timeline_rate,
                }
            }
            other => NativeCommand::Transport(other),
        };
        self.worker.request(command)
    }
}

fn run_device(
    device: cpal::Device,
    config: cpal::StreamConfig,
    format: cpal::SampleFormat,
    receiver: std::sync::mpsc::Receiver<super::TimecodeAudioWorkerRequest>,
    clock: Arc<dyn TimecodeClock>,
    started: std::sync::mpsc::Sender<Result<(), String>>,
) {
    let voices = Arc::new(Mutex::new(DeviceVoices::default()));
    let stream = match format {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, Arc::clone(&voices), Arc::clone(&clock))
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, Arc::clone(&voices), Arc::clone(&clock))
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, &config, Arc::clone(&voices), clock)
        }
        format => Err(format!(
            "unsupported Timecode audio output sample format {format:?}"
        )),
    };
    let stream = match stream.and_then(|stream| {
        stream
            .play()
            .map_err(|error| format!("Timecode audio output could not start: {error}"))?;
        Ok(stream)
    }) {
        Ok(stream) => {
            let _ = started.send(Ok(()));
            stream
        }
        Err(error) => {
            let _ = started.send(Err(error));
            return;
        }
    };
    while let Ok(request) = receiver.recv() {
        if matches!(request.command, NativeCommand::Shutdown) {
            let _ = request.reply.send(Ok(()));
            break;
        }
        let result = voices
            .lock()
            .map_err(|_| "Timecode audio output lock is poisoned".to_owned())
            .and_then(|mut voices| {
                apply_native(
                    &mut voices,
                    request.command,
                    config.sample_rate.0,
                    usize::from(config.channels),
                )
            });
        let _ = request.reply.send(result);
    }
    drop(stream);
}

fn apply_native(
    voices: &mut DeviceVoices,
    command: NativeCommand,
    output_sample_rate: u32,
    output_channels: usize,
) -> Result<(), String> {
    match command {
        NativeCommand::Prepare {
            timecode_id,
            decoded,
            timeline_rate,
        } => {
            voices.timecodes.insert(
                timecode_id,
                Voice::new(decoded, timeline_rate, output_sample_rate, output_channels),
            );
        }
        NativeCommand::Transport(TimecodeAudioCommand::Play {
            timecode_id,
            source_frame,
            audible_at_micros,
        }) => {
            let voice = timecode_voice(voices, timecode_id)?;
            voice.seek(source_frame);
            voice.play_at_micros = Some(audible_at_micros);
            voice.playing = true;
        }
        NativeCommand::Transport(TimecodeAudioCommand::Pause { timecode_id }) => {
            let voice = timecode_voice(voices, timecode_id)?;
            voice.playing = false;
            voice.play_at_micros = None;
        }
        NativeCommand::Transport(TimecodeAudioCommand::Stop { timecode_id }) => {
            let voice = timecode_voice(voices, timecode_id)?;
            voice.playing = false;
            voice.play_at_micros = None;
            voice.position = 0.0;
        }
        NativeCommand::Transport(TimecodeAudioCommand::Seek {
            timecode_id,
            source_frame,
            audible_at_micros,
        }) => {
            let voice = timecode_voice(voices, timecode_id)?;
            voice.seek(source_frame);
            if voice.playing {
                voice.play_at_micros = Some(audible_at_micros);
            }
        }
        NativeCommand::Transport(TimecodeAudioCommand::SetLoop {
            timecode_id,
            enabled,
            end_exclusive,
        }) => {
            let voice = timecode_voice(voices, timecode_id)?;
            voice.looping = enabled;
            voice.loop_end = voice
                .sample_at_frame(end_exclusive)
                .min(voice.sample_frames());
        }
        NativeCommand::Transport(TimecodeAudioCommand::SetVolume {
            timecode_id,
            linear,
        }) => {
            timecode_voice(voices, timecode_id)?.volume = linear as f32;
        }
        NativeCommand::Transport(TimecodeAudioCommand::Prepare { .. }) => unreachable!(),
        NativeCommand::PrepareInternal {
            fixture_id,
            decoded,
        } => {
            voices.internal.insert(
                fixture_id,
                Voice::new(
                    decoded,
                    TimecodeFrameRate::whole_frames(1).expect("one fps is valid"),
                    output_sample_rate,
                    output_channels,
                ),
            );
        }
        NativeCommand::InternalTransport { fixture_id, action } => {
            let voice = internal_voice(voices, fixture_id)?;
            match action {
                NativeInternalTransport::Stop => {
                    voice.playing = false;
                    voice.play_at_micros = None;
                    voice.position = 0.0;
                }
                NativeInternalTransport::Pause => {
                    voice.playing = false;
                    voice.play_at_micros = None;
                }
                NativeInternalTransport::Play => {
                    voice.playing = true;
                    voice.play_at_micros = None;
                }
                NativeInternalTransport::RestartPlay => {
                    voice.position = 0.0;
                    voice.playing = true;
                    voice.play_at_micros = None;
                }
            }
        }
        NativeCommand::InternalRepeat {
            fixture_id,
            enabled,
        } => {
            internal_voice(voices, fixture_id)?.looping = enabled;
        }
        NativeCommand::InternalVolume { fixture_id, linear } => {
            internal_voice(voices, fixture_id)?.volume = linear.clamp(0.0, 1.0);
        }
        NativeCommand::InternalSeek {
            fixture_id,
            cursor_millis,
        } => {
            internal_voice(voices, fixture_id)?.seek_millis(cursor_millis);
        }
        NativeCommand::RemoveInternal { fixture_id } => {
            voices.internal.remove(&fixture_id);
        }
        NativeCommand::Shutdown => unreachable!(),
    }
    Ok(())
}

fn timecode_voice(voices: &mut DeviceVoices, id: TimecodeId) -> Result<&mut Voice, String> {
    voices
        .timecodes
        .get_mut(&id)
        .ok_or_else(|| "Timecode audio is not prepared on the output device".into())
}

fn internal_voice(voices: &mut DeviceVoices, id: FixtureId) -> Result<&mut Voice, String> {
    voices
        .internal
        .get_mut(&id)
        .ok_or_else(|| format!("Audio Player {} is not prepared on the output device", id.0))
}

fn select_device(
    host: &cpal::Host,
    selector: &OutputDeviceSelector,
) -> Result<cpal::Device, String> {
    if selector == &OutputDeviceSelector::SystemDefault {
        return host
            .default_output_device()
            .ok_or_else(|| "this machine has no audio output device".into());
    }
    let OutputDeviceSelector::Name(wanted) = selector else {
        unreachable!();
    };
    host.output_devices()
        .map_err(|error| format!("audio output devices could not be enumerated: {error}"))?
        .find(|device| device.name().is_ok_and(|name| name == *wanted))
        .ok_or_else(|| format!("no audio output device is named {wanted}"))
}

fn buffer_latency_micros(size: &cpal::SupportedBufferSize, sample_rate: u32) -> u64 {
    let frames = match size {
        cpal::SupportedBufferSize::Range { min, .. } => u64::from(*min),
        cpal::SupportedBufferSize::Unknown => 0,
    };
    frames.saturating_mul(1_000_000) / u64::from(sample_rate.max(1))
}

fn add_signed(value: u64, adjustment: i64) -> u64 {
    if adjustment >= 0 {
        value.saturating_add(adjustment as u64)
    } else {
        value.saturating_sub(adjustment.unsigned_abs())
    }
}

trait OutputSample: cpal::SizedSample {
    fn from_mix(value: f32) -> Self;
}
impl OutputSample for f32 {
    fn from_mix(value: f32) -> Self {
        value
    }
}
impl OutputSample for i16 {
    fn from_mix(value: f32) -> Self {
        (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
    }
}
impl OutputSample for u16 {
    fn from_mix(value: f32) -> Self {
        ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * f32::from(u16::MAX)) as u16
    }
}

fn build_stream<T: OutputSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    voices: Arc<Mutex<DeviceVoices>>,
    clock: Arc<dyn TimecodeClock>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                output.fill_with(|| T::from_mix(0.0));
                let Ok(mut voices) = voices.try_lock() else {
                    return;
                };
                let now = clock.now_micros();
                for frame in output.chunks_mut(channels) {
                    let mut mix = vec![0.0_f32; channels];
                    for voice in voices.timecodes.values_mut() {
                        voice.mix_frame(&mut mix, now);
                    }
                    for voice in voices.internal.values_mut() {
                        voice.mix_frame(&mut mix, now);
                    }
                    for (target, sample) in frame.iter_mut().zip(mix) {
                        *target = T::from_mix(sample.clamp(-1.0, 1.0));
                    }
                }
            },
            |error| tracing::warn!(%error, "Timecode audio output reported an error"),
            None,
        )
        .map_err(|error| format!("Timecode audio output could not be opened: {error}"))
}

pub(in crate::runtime) struct DecodedWav {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

#[derive(Default)]
struct DeviceVoices {
    timecodes: BTreeMap<TimecodeId, Voice>,
    internal: HashMap<FixtureId, Voice>,
}

struct Voice {
    samples: Arc<Vec<f32>>,
    source_rate: u32,
    source_channels: usize,
    timeline_rate: TimecodeFrameRate,
    output_rate: u32,
    output_channels: usize,
    position: f64,
    volume: f32,
    playing: bool,
    play_at_micros: Option<u64>,
    looping: bool,
    loop_end: usize,
}

impl Voice {
    fn new(
        decoded: DecodedWav,
        timeline_rate: TimecodeFrameRate,
        output_rate: u32,
        output_channels: usize,
    ) -> Self {
        let sample_frames = decoded.samples.len() / usize::from(decoded.channels);
        Self {
            samples: Arc::new(decoded.samples),
            source_rate: decoded.sample_rate,
            source_channels: usize::from(decoded.channels),
            timeline_rate,
            output_rate,
            output_channels,
            position: 0.0,
            volume: 1.0,
            playing: false,
            play_at_micros: None,
            looping: false,
            loop_end: sample_frames,
        }
    }

    fn sample_frames(&self) -> usize {
        self.samples.len() / self.source_channels
    }

    fn sample_at_frame(&self, frame: TimecodeFrame) -> usize {
        let numerator = u128::from(frame.0)
            * u128::from(self.source_rate)
            * u128::from(self.timeline_rate.denominator());
        let denominator = u128::from(self.timeline_rate.numerator());
        usize::try_from(numerator / denominator).unwrap_or(usize::MAX)
    }

    fn seek(&mut self, frame: TimecodeFrame) {
        self.position = self.sample_at_frame(frame).min(self.sample_frames()) as f64;
    }

    fn seek_millis(&mut self, millis: u32) {
        let sample = u128::from(millis) * u128::from(self.source_rate) / 1_000;
        self.position = usize::try_from(sample)
            .unwrap_or(usize::MAX)
            .min(self.sample_frames()) as f64;
    }

    fn mix_frame(&mut self, output: &mut [f32], now: u64) {
        if !self.playing || self.play_at_micros.is_some_and(|deadline| now < deadline) {
            return;
        }
        self.play_at_micros = None;
        let mut source_frame = self.position.floor() as usize;
        let end = self.loop_end.min(self.sample_frames());
        if source_frame >= end {
            if self.looping && end > 0 {
                self.position %= end as f64;
                source_frame = self.position.floor() as usize;
            } else {
                self.playing = false;
                self.position = 0.0;
                return;
            }
        }
        for channel in 0..self.output_channels.min(output.len()) {
            let source_channel = channel.min(self.source_channels - 1);
            let sample = self.samples[source_frame * self.source_channels + source_channel];
            // CPAL owns the sole mutable output frame, so mixing converts through f32 here.
            // Current callers use one Timecode audio lane; clamping is still safe if that expands.
            output[channel] += sample * self.volume;
        }
        self.position += f64::from(self.source_rate) / f64::from(self.output_rate.max(1));
    }
}

#[derive(Default)]
struct ByteSink(Vec<u8>);
impl AssetChunkSink for ByteSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), light_application::AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn read_asset(store: &dyn ManagedAssetStore, asset: AssetReference) -> Result<Vec<u8>, String> {
    let mut sink = ByteSink::default();
    store
        .stream(asset, &mut sink)
        .map_err(|error| error.message)?;
    Ok(sink.0)
}

fn decode_wav(bytes: &[u8]) -> Result<DecodedWav, String> {
    let metadata = light_application::parse_wav_metadata(bytes).map_err(|error| error.message)?;
    let mut cursor = 12;
    let mut data = None;
    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let length = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        cursor += 8;
        let end = cursor.saturating_add(length);
        if end > bytes.len() {
            return Err("invalid WAV: chunk length exceeds bytes".into());
        }
        if id == b"data" {
            data = Some(&bytes[cursor..end]);
            break;
        }
        cursor = end.saturating_add(length & 1);
    }
    let data = data.ok_or_else(|| "invalid WAV: data chunk is missing".to_owned())?;
    let samples = match (metadata.encoding, metadata.bits_per_sample) {
        (light_application::WavEncoding::PcmInteger, 16) => data
            .chunks_exact(2)
            .map(|bytes| f32::from(i16::from_le_bytes(bytes.try_into().unwrap())) / 32768.0)
            .collect(),
        (light_application::WavEncoding::IeeeFloat, 32) => data
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()).clamp(-1.0, 1.0))
            .collect(),
        _ => {
            return Err(format!(
                "WAV encoding {:?}/{}-bit is valid but unsupported for native playback",
                metadata.encoding, metadata.bits_per_sample
            ));
        }
    };
    Ok(DecodedWav {
        samples,
        sample_rate: metadata.sample_rate,
        channels: metadata.channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(samples: &[f32]) -> DecodedWav {
        DecodedWav {
            samples: samples.to_vec(),
            sample_rate: 1,
            channels: 1,
        }
    }

    fn fixture(value: u128) -> FixtureId {
        FixtureId(uuid::Uuid::from_u128(value))
    }

    #[test]
    fn latency_trim_is_signed_and_saturating() {
        assert_eq!(add_signed(10_000, 2_500), 12_500);
        assert_eq!(add_signed(10_000, -2_500), 7_500);
        assert_eq!(add_signed(1_000, -2_500), 0);
    }

    #[test]
    fn startup_wait_reports_timeout_without_blocking_server_startup() {
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();

        assert_eq!(
            receive_startup(
                &receiver,
                Duration::ZERO,
                "startup timed out",
                "startup disconnected",
            ),
            Err("startup timed out".to_owned())
        );
    }

    #[test]
    fn startup_wait_distinguishes_a_stopped_worker() {
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        drop(sender);

        assert_eq!(
            receive_startup(
                &receiver,
                Duration::from_secs(1),
                "startup timed out",
                "startup disconnected",
            ),
            Err("startup disconnected".to_owned())
        );
    }

    #[test]
    fn internal_players_mix_without_voice_stealing() {
        let mut voices = DeviceVoices::default();
        for (fixture_id, samples) in [(fixture(1), &[0.25][..]), (fixture(2), &[0.5][..])] {
            apply_native(
                &mut voices,
                NativeCommand::PrepareInternal {
                    fixture_id,
                    decoded: decoded(samples),
                },
                1,
                1,
            )
            .unwrap();
            apply_native(
                &mut voices,
                NativeCommand::InternalTransport {
                    fixture_id,
                    action: NativeInternalTransport::Play,
                },
                1,
                1,
            )
            .unwrap();
        }

        let mut output = [0.0];
        for voice in voices.internal.values_mut() {
            voice.mix_frame(&mut output, 0);
        }

        assert_eq!(voices.internal.len(), 2);
        assert_eq!(output, [0.75]);
    }

    #[test]
    fn internal_transport_resets_on_stop_and_non_repeating_end() {
        let fixture_id = fixture(3);
        let mut voices = DeviceVoices::default();
        apply_native(
            &mut voices,
            NativeCommand::PrepareInternal {
                fixture_id,
                decoded: decoded(&[0.5]),
            },
            1,
            1,
        )
        .unwrap();
        apply_native(
            &mut voices,
            NativeCommand::InternalTransport {
                fixture_id,
                action: NativeInternalTransport::Play,
            },
            1,
            1,
        )
        .unwrap();

        let voice = voices.internal.get_mut(&fixture_id).unwrap();
        voice.mix_frame(&mut [0.0], 0);
        voice.mix_frame(&mut [0.0], 1);
        assert!(!voice.playing);
        assert_eq!(voice.position, 0.0);

        voice.position = 1.0;
        voice.playing = true;
        apply_native(
            &mut voices,
            NativeCommand::InternalTransport {
                fixture_id,
                action: NativeInternalTransport::Stop,
            },
            1,
            1,
        )
        .unwrap();
        let voice = &voices.internal[&fixture_id];
        assert!(!voice.playing);
        assert_eq!(voice.position, 0.0);
    }

    #[test]
    fn restart_play_is_an_edge_action_and_repeat_wraps() {
        let fixture_id = fixture(4);
        let mut voices = DeviceVoices::default();
        apply_native(
            &mut voices,
            NativeCommand::PrepareInternal {
                fixture_id,
                decoded: decoded(&[0.25, 0.75]),
            },
            1,
            1,
        )
        .unwrap();
        apply_native(
            &mut voices,
            NativeCommand::InternalRepeat {
                fixture_id,
                enabled: true,
            },
            1,
            1,
        )
        .unwrap();
        apply_native(
            &mut voices,
            NativeCommand::InternalTransport {
                fixture_id,
                action: NativeInternalTransport::RestartPlay,
            },
            1,
            1,
        )
        .unwrap();
        let voice = voices.internal.get_mut(&fixture_id).unwrap();
        let mut first = [0.0];
        voice.mix_frame(&mut first, 0);
        voice.mix_frame(&mut [0.0], 1);
        let mut wrapped = [0.0];
        voice.mix_frame(&mut wrapped, 2);
        assert_eq!(first, [0.25]);
        assert_eq!(wrapped, [0.25]);

        apply_native(
            &mut voices,
            NativeCommand::InternalTransport {
                fixture_id,
                action: NativeInternalTransport::RestartPlay,
            },
            1,
            1,
        )
        .unwrap();
        assert_eq!(voices.internal[&fixture_id].position, 0.0);
    }

    #[cfg(unix)]
    #[test]
    fn failed_device_probe_is_reported_without_terminating_the_server_process() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "kill -SEGV $$"]);

        let error = output_devices_from_command(&mut command).unwrap_err();

        assert!(error.contains("stopped unexpectedly"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn malformed_device_probe_output_is_actionable() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf not-json"]);

        let error = output_devices_from_command(&mut command).unwrap_err();

        assert!(error.contains("returned invalid data"), "{error}");
    }
}
