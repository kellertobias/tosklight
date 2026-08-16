//! Opening an input device and keeping it fed into the worker.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use crossbeam_queue::ArrayQueue;
use media_application::configuration::{AudioConfiguration, AudioDeviceSelector};
use media_domain::audio::Tuning;

use crate::QUEUE_CAPACITY;
use crate::snapshot::{AnalysisSnapshot, SharedAnalysis, Worker};

/// CoreAudio device discovery can block indefinitely when the host audio service is unhealthy.
/// Media is still useful without an audio input, so startup must not wait forever for it.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);

/// Why audio could not be captured.
///
/// Every one of these is a real capability of the machine being absent, which a feature is allowed
/// to report. None of them is a platform adapter that was left unwritten.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("this machine has no audio input device")]
    NoDevice,
    #[error("no audio input device is named {name}")]
    NoSuchDevice { name: String },
    #[error("the input device offers no format this server can read: {detail}")]
    NoUsableFormat { detail: String },
    #[error("the input device could not be opened: {detail}")]
    NotOpened { detail: String },
}

/// The tuning the analysis worker is using, swapped when an operator changes it.
///
/// Shared rather than passed at construction because gain, the bands, and beat sensitivity are
/// things an operator turns *while listening*. Waiting for a restart to hear a gain change would
/// make them unusable.
pub type SharedTuning = Arc<arc_swap::ArcSwap<Tuning>>;

/// The thread-safe control surface for a running capture.
///
/// CPAL's CoreAudio stream is not `Send`, so the platform stream remains on the device thread
/// that opened it. The rest of Media holds only the published analysis, live tuning, and shutdown
/// control.
pub struct AudioService {
    published: SharedAnalysis,
    tuning: SharedTuning,
    /// The device that is open, as the machine names it.
    device: String,
    stop: Option<std::sync::mpsc::Sender<()>>,
    device_thread: Option<std::thread::JoinHandle<()>>,
}

struct Opened {
    published: SharedAnalysis,
    tuning: SharedTuning,
    device: String,
}

/// The non-Send CoreAudio stream and its analysis worker, both owned by the device thread.
struct DeviceCapture {
    running: Arc<AtomicBool>,
    /// Held for as long as capture should continue. Dropping it closes the device.
    _stream: cpal::Stream,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl AudioService {
    /// Opens the configured device without allowing a platform audio service to hold up Media.
    pub fn start_bounded(configuration: &AudioConfiguration) -> Result<Self, AudioError> {
        let configuration = configuration.clone();
        let (started, startup) = std::sync::mpsc::channel();
        let (stop, stopping) = std::sync::mpsc::channel();
        let device_thread = std::thread::Builder::new()
            .name("media-audio-device".into())
            .spawn(move || {
                let (capture, opened) = match DeviceCapture::open(&configuration) {
                    Ok(opened) => opened,
                    Err(error) => {
                        let _ = started.send(Err(error));
                        return;
                    }
                };
                if started.send(Ok(opened)).is_err() {
                    return;
                }
                let _ = stopping.recv();
                drop(capture);
            })
            .map_err(|error| AudioError::NotOpened {
                detail: format!("audio startup worker could not start: {error}"),
            })?;

        let opened = receive_startup(&startup, STARTUP_TIMEOUT)??;
        Ok(Self {
            published: opened.published,
            tuning: opened.tuning,
            device: opened.device,
            stop: Some(stop),
            device_thread: Some(device_thread),
        })
    }

    /// The newest analysis, for whoever is drawing this frame.
    pub fn analysis(&self) -> SharedAnalysis {
        Arc::clone(&self.published)
    }

    /// The device this capture is reading, as the machine names it.
    pub fn device(&self) -> &str {
        &self.device
    }

    /// The tuning the worker reads, for whoever accepts an operator's edit.
    ///
    /// Handed out rather than reached through this service, because the service owns a platform
    /// stream that belongs to one thread while an edit arrives on another.
    pub fn tuning(&self) -> SharedTuning {
        Arc::clone(&self.tuning)
    }

    /// Applies an operator's tuning to the running analysis.
    ///
    /// The device is not reopened: choosing a different input is a different stream, and the
    /// platform event loop owns the one that is open. What changes here is what the worker does
    /// with the samples it is already receiving.
    pub fn retune(&self, configuration: &AudioConfiguration) {
        self.tuning.store(Arc::new(tuning_of(configuration)));
    }
}

impl DeviceCapture {
    /// Opens the configured device and starts analysing on the thread that retains the stream.
    fn open(configuration: &AudioConfiguration) -> Result<(Self, Opened), AudioError> {
        let host = cpal::default_host();
        let device = select(&host, &configuration.device)?;
        let name = device.name().unwrap_or_else(|_| "an input".to_owned());
        let config = device
            .default_input_config()
            .map_err(|error| AudioError::NoUsableFormat {
                detail: error.to_string(),
            })?;
        let sample_rate = config.sample_rate().0 as f32;
        let channels = usize::from(config.channels().max(1));

        let queue = Arc::new(ArrayQueue::new(QUEUE_CAPACITY));
        let published: SharedAnalysis =
            Arc::new(arc_swap::ArcSwap::from_pointee(AnalysisSnapshot::default()));

        // Everything the callback needs is captured by value. It allocates nothing, takes no lock
        // that can block, logs nothing, and never touches the device list.
        let filling = Arc::clone(&queue);
        let stream = device
            .build_input_stream(
                &config.config(),
                move |samples: &[f32], _| {
                    // Mono: a stereo input is averaged, so a source panned to one side is not
                    // analysed as half as loud.
                    for frame in samples.chunks(channels) {
                        let mono = frame.iter().sum::<f32>() / channels as f32;
                        // A full queue means the worker is behind. Dropping the newest sample is
                        // the only option that never blocks the device.
                        let _ = filling.push(mono);
                    }
                },
                |error| tracing::warn!(%error, "the audio input reported an error"),
                None,
            )
            .map_err(|error| AudioError::NotOpened {
                detail: error.to_string(),
            })?;
        stream.play().map_err(|error| AudioError::NotOpened {
            detail: error.to_string(),
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let tuning: SharedTuning =
            Arc::new(arc_swap::ArcSwap::from_pointee(tuning_of(configuration)));
        let worker = std::thread::Builder::new()
            .name("media-audio-analysis".into())
            .spawn({
                let running = Arc::clone(&running);
                let published = Arc::clone(&published);
                let tuning = Arc::clone(&tuning);
                let started = std::time::Instant::now();
                move || {
                    let mut worker = Worker::new(**tuning.load(), sample_rate, published);
                    let mut current = **tuning.load();
                    while running.load(Ordering::Relaxed) {
                        // Retuning keeps the beat history, so a gain change does not make the
                        // detector relearn what loud means in this room.
                        let wanted = **tuning.load();
                        if wanted != current {
                            worker.retune(wanted);
                            current = wanted;
                        }
                        let now = started.elapsed().as_millis() as u64;
                        if worker.drain(&queue, now) == 0 {
                            // Nothing completed a window; wait rather than spin a core.
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                    }
                }
            })
            .map_err(|error| AudioError::NotOpened {
                detail: error.to_string(),
            })?;

        tracing::info!(device = %name, sample_rate, channels, "capturing audio");
        let opened = Opened {
            published: Arc::clone(&published),
            tuning: Arc::clone(&tuning),
            device: name,
        };
        Ok((
            Self {
                running,
                _stream: stream,
                worker: Some(worker),
            },
            opened,
        ))
    }
}

fn receive_startup<T>(
    receiver: &std::sync::mpsc::Receiver<T>,
    timeout: Duration,
) -> Result<T, AudioError> {
    receiver.recv_timeout(timeout).map_err(|error| {
        let detail = match error {
            std::sync::mpsc::RecvTimeoutError::Timeout => {
                format!(
                    "audio input initialization did not finish within {} seconds",
                    timeout.as_secs()
                )
            }
            std::sync::mpsc::RecvTimeoutError::Disconnected => {
                "audio startup worker stopped unexpectedly".to_owned()
            }
        };
        AudioError::NotOpened { detail }
    })
}

/// This machine's audio inputs, by name.
///
/// A platform capability, so it is read here rather than guessed at anywhere else. A host that
/// cannot be asked reports no inputs, which is what a settings panel then shows.
pub fn input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| device.name().ok())
        .filter(|name| !name.trim().is_empty())
        .collect()
}

/// This machine's audio outputs, by name.
pub fn output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let Ok(devices) = host.output_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| device.name().ok())
        .filter(|name| !name.trim().is_empty())
        .collect()
}

impl Drop for AudioService {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(device_thread) = self.device_thread.take() {
            let _ = device_thread.join();
        }
    }
}

impl Drop for DeviceCapture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The tuning an operator configured.
pub fn tuning_of(configuration: &AudioConfiguration) -> Tuning {
    Tuning {
        input_gain: configuration.input_gain,
        eq_bass: configuration.eq_bass,
        eq_mid: configuration.eq_mid,
        eq_treble: configuration.eq_treble,
        beat_sensitivity: configuration.beat_sensitivity,
    }
}

/// Finds the configured input.
fn select(host: &cpal::Host, selector: &AudioDeviceSelector) -> Result<cpal::Device, AudioError> {
    if matches!(selector, AudioDeviceSelector::SystemDefault) {
        return host.default_input_device().ok_or(AudioError::NoDevice);
    }

    let devices: Vec<cpal::Device> = host
        .input_devices()
        .map_err(|error| AudioError::NoUsableFormat {
            detail: error.to_string(),
        })?
        .collect();
    let names: Vec<String> = devices
        .iter()
        .map(|device| device.name().unwrap_or_default())
        .collect();
    let chosen = choose(selector, &names)?;
    devices.into_iter().nth(chosen).ok_or(AudioError::NoDevice)
}

/// Which of the machine's inputs the configuration names.
///
/// Separated from the device list so the rule can be tested without a sound card: a device named
/// in configuration and not present is an *error*, never a silent fall back to the default. An
/// operator who named a desk feed does not want the laptop microphone instead.
fn choose(selector: &AudioDeviceSelector, names: &[String]) -> Result<usize, AudioError> {
    match selector {
        // Handled before the list is built; the default input is the host's own answer.
        AudioDeviceSelector::SystemDefault => Ok(0),
        AudioDeviceSelector::Name(name) => names
            .iter()
            .position(|candidate| candidate == name)
            .ok_or_else(|| AudioError::NoSuchDevice { name: name.clone() }),
        // The legacy application's device index. Kept so migrated configuration still opens
        // something rather than failing on a number this host does not use.
        AudioDeviceSelector::Index(index) => {
            let at = usize::try_from(*index).unwrap_or(0);
            (at < names.len()).then_some(at).ok_or(AudioError::NoDevice)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_startup_reports_a_timeout_instead_of_waiting_forever() {
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        let error = receive_startup(&receiver, Duration::ZERO).unwrap_err();
        assert_eq!(
            error,
            AudioError::NotOpened {
                detail: "audio input initialization did not finish within 0 seconds".into()
            }
        );
    }

    #[test]
    fn bounded_startup_reports_a_worker_that_stopped() {
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        drop(sender);
        let error = receive_startup(&receiver, Duration::from_secs(1)).unwrap_err();
        assert_eq!(
            error,
            AudioError::NotOpened {
                detail: "audio startup worker stopped unexpectedly".into()
            }
        );
    }

    #[test]
    fn the_operators_tuning_reaches_the_analysis() {
        let configuration = AudioConfiguration {
            input_gain: 2.0,
            eq_bass: 1.5,
            eq_mid: 0.5,
            eq_treble: 0.25,
            beat_sensitivity: 3.0,
            ..Default::default()
        };
        let tuning = tuning_of(&configuration);

        assert_eq!(tuning.input_gain, 2.0);
        assert_eq!(tuning.eq_bass, 1.5);
        assert_eq!(tuning.eq_mid, 0.5);
        assert_eq!(tuning.eq_treble, 0.25);
        assert_eq!(tuning.beat_sensitivity, 3.0);
    }

    fn machine() -> Vec<String> {
        vec!["Built-in Microphone".into(), "Desk feed".into()]
    }

    #[test]
    fn a_named_device_is_found_by_its_name() {
        assert_eq!(
            choose(&AudioDeviceSelector::Name("Desk feed".into()), &machine()),
            Ok(1)
        );
    }

    #[test]
    fn a_named_device_that_is_not_present_is_an_error_rather_than_the_default() {
        assert_eq!(
            choose(
                &AudioDeviceSelector::Name("Desk feed".into()),
                &["Built-in Microphone".to_owned()]
            ),
            Err(AudioError::NoSuchDevice {
                name: "Desk feed".into()
            }),
            "an operator who named a desk feed must not silently get the laptop microphone"
        );
    }

    #[test]
    fn a_migrated_device_index_beyond_this_machine_says_so() {
        assert_eq!(choose(&AudioDeviceSelector::Index(1), &machine()), Ok(1));
        assert_eq!(
            choose(&AudioDeviceSelector::Index(9), &machine()),
            Err(AudioError::NoDevice)
        );
        assert_eq!(
            choose(&AudioDeviceSelector::Index(-1), &machine()),
            Ok(0),
            "a negative index from the legacy document means the first input"
        );
    }

    #[test]
    fn no_input_at_all_is_reported_rather_than_pretended_away() {
        assert_eq!(
            choose(&AudioDeviceSelector::Index(0), &[]),
            Err(AudioError::NoDevice)
        );
    }
}
