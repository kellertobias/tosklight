//! What the process can tell the API about itself, beyond state and configuration.
//!
//! Audio capture, the machine's input devices, and the log are all things only the running process
//! knows. None of them is an HTTP concern, and this adapter must not reach for a sound card or a
//! subscriber of its own — so each arrives as a function, the same way an accepted configuration
//! arrives as a writer rather than as a path.
//!
//! The records here are transport-neutral. The wire projections in [`crate::wire`] turn them into
//! JSON, so a change in how the process gathers a diagnostic is not a change to the API.

use std::sync::Arc;

use media_domain::{AssetId, MediaAddress, OutputId};

/// One instant of audio analysis, as the process publishes it.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioTelemetry {
    /// Whether an input device is actually open. False is a real state: a machine with no input
    /// runs on silence, and an operator watching a flat meter needs to know which of the two it is.
    pub capturing: bool,
    /// The device that is open, or what was tried.
    pub device: String,
    /// Why capture is not running, when it is not.
    pub detail: Option<String>,
    pub waveform: Vec<f32>,
    pub spectrum: Vec<f32>,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub energy: f32,
    pub peak: f32,
    pub beat: f32,
    pub bpm: f32,
    pub beat_phase: f32,
}

impl Default for AudioTelemetry {
    /// A machine with no input. Silence, and the reason for it.
    fn default() -> Self {
        Self {
            capturing: false,
            device: "none".to_owned(),
            detail: Some("this server is not capturing audio".to_owned()),
            waveform: Vec::new(),
            spectrum: Vec::new(),
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            energy: 0.0,
            peak: 0.0,
            beat: 0.0,
            bpm: 0.0,
            beat_phase: 0.0,
        }
    }
}

/// A file in the library that could be played once it has been imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingImport {
    pub destination: media_domain::MediaAddress,
    pub name: String,
    pub filename: String,
}

/// Where one import has got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    Queued,
    Running,
    Succeeded,
    Failed { reason: String },
    Cancelled,
}

/// One import, as the running process reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportJob {
    pub id: String,
    pub destination: media_domain::MediaAddress,
    pub filename: String,
    pub outcome: ImportOutcome,
    /// Absent when the source did not report a frame count. Nothing invents one.
    pub fraction: Option<f32>,
    pub frames_done: Option<u32>,
    pub frames_total: Option<u32>,
}

/// What the process can do about importing, and what it is doing.
///
/// Functions rather than the importer itself, for the same reason as the audio: the pool belongs
/// to the process, and the API only ever asks it questions and gives it work.
#[derive(Clone)]
pub struct Imports {
    /// What is waiting in the library, and every job this run has seen.
    pub state: Arc<dyn Fn() -> (Vec<PendingImport>, Vec<ImportJob>) + Send + Sync>,
    /// Queues everything waiting, or one address. Returns how many jobs were started.
    pub start: Arc<dyn Fn(Option<media_domain::MediaAddress>) -> usize + Send + Sync>,
    /// Stops one job. False when there was nothing to stop.
    pub cancel: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// Whether this machine can transcode at all.
    pub available: bool,
}

/// One durable edit to the operator's media library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryEdit {
    RenameItem {
        id: AssetId,
        name: String,
    },
    MoveItem {
        id: AssetId,
        destination: MediaAddress,
        /// When occupied, exchange the two addresses instead of overwriting either file.
        swap: bool,
    },
    RenameFolder {
        folder: u8,
        name: Option<String>,
    },
}

/// A streaming upload owned by the runtime/library adapter.
///
/// Multipart framing stays in HTTP, but no HTTP route opens a file or invents a temporary path.
pub trait UploadStream: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    /// Publishes the source and starts its import job. Returns that job's identity.
    fn finish(self: Box<Self>) -> Result<String, String>;
}

/// Mutations and files the running library exposes to the API.
#[derive(Clone)]
pub struct LibraryAccess {
    pub edit: Arc<dyn Fn(LibraryEdit) -> Result<(), String> + Send + Sync>,
    pub thumbnail: Arc<dyn Fn(MediaAddress) -> Result<Vec<u8>, String> + Send + Sync>,
    pub begin_upload: Arc<
        dyn Fn(MediaAddress, &str, &str) -> Result<Box<dyn UploadStream>, String> + Send + Sync,
    >,
}

impl Default for LibraryAccess {
    fn default() -> Self {
        Self {
            edit: Arc::new(|_| Err("library editing is unavailable in this process".to_owned())),
            thumbnail: Arc::new(|_| Err("no thumbnail exists at that address".to_owned())),
            begin_upload: Arc::new(|_, _, _| {
                Err("library upload is unavailable in this process".to_owned())
            }),
        }
    }
}

impl std::fmt::Debug for LibraryAccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryAccess")
            .finish_non_exhaustive()
    }
}

/// The latest accepted DMX footprint for one configured output.
#[derive(Debug, Clone, PartialEq)]
pub struct DmxTelemetry {
    pub output: OutputId,
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
    pub source: String,
    pub frames_per_second: f32,
    pub age_millis: u64,
    pub active: bool,
    /// Exact bytes in this output's configured footprint, starting at `start_address`.
    pub slots: Vec<u8>,
}

pub type DmxSource = Arc<dyn Fn() -> Vec<DmxTelemetry> + Send + Sync>;

impl Default for Imports {
    /// A process that imports nothing: it reports nothing waiting, and says it cannot import.
    fn default() -> Self {
        Self {
            state: Arc::new(|| (Vec::new(), Vec::new())),
            start: Arc::new(|_| 0),
            cancel: Arc::new(|_| false),
            available: false,
        }
    }
}

impl std::fmt::Debug for Imports {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Imports")
            .field("available", &self.available)
            .finish_non_exhaustive()
    }
}

/// One emitted log record, held in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Monotonically increasing. A viewer asks for everything after the last one it holds, so a
    /// poll cannot miss a record or show one twice.
    pub sequence: u64,
    pub millis_since_start: u64,
    /// `error`, `warn`, `info`, `debug`, or `trace`.
    pub level: String,
    pub target: String,
    pub message: String,
}

/// What a log viewer is asking for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogQuery {
    /// Only records after this sequence number. Absent means the newest window.
    pub after: Option<u64>,
    /// Only records at this level or more severe.
    pub level: Option<String>,
    pub limit: usize,
}

/// What a log holds, and what it had to drop to stay bounded.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogPage {
    pub entries: Vec<LogEntry>,
    /// The newest sequence the buffer holds, whether or not this page reached it.
    pub newest: u64,
    /// How many records the buffer has discarded since the process started. A log viewer that
    /// silently loses records is worse than one that says it did.
    pub dropped: u64,
    pub capacity: usize,
}

/// The newest audio analysis.
pub type AudioSource = Arc<dyn Fn() -> AudioTelemetry + Send + Sync>;
/// The machine's audio inputs, by name. A platform capability, so the process owns it.
pub type DeviceLister = Arc<dyn Fn() -> Vec<String> + Send + Sync>;
/// Recent log records.
pub type LogSource = Arc<dyn Fn(&LogQuery) -> LogPage + Send + Sync>;

/// The diagnostics the API can read from the running process.
#[derive(Clone)]
pub struct Diagnostics {
    pub audio: AudioSource,
    pub audio_devices: DeviceLister,
    pub logs: LogSource,
    pub imports: Imports,
    pub library: LibraryAccess,
    pub dmx: DmxSource,
}

impl Default for Diagnostics {
    /// A process that reports no audio, no devices, and no log.
    ///
    /// This is what a test gets, and what a diagnostic build that starts no subsystems gets. Each
    /// route still answers — with the truth, which is that nothing is being captured or recorded.
    fn default() -> Self {
        Self {
            audio: Arc::new(AudioTelemetry::default),
            audio_devices: Arc::new(Vec::new),
            logs: Arc::new(|_| LogPage::default()),
            imports: Imports::default(),
            library: LibraryAccess::default(),
            dmx: Arc::new(Vec::new),
        }
    }
}

impl std::fmt::Debug for Diagnostics {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Diagnostics")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_diagnostics_answer_rather_than_pretending() {
        let diagnostics = Diagnostics::default();
        let audio = (diagnostics.audio)();

        assert!(!audio.capturing);
        assert!(
            audio.detail.is_some(),
            "silence needs a reason, or an operator cannot tell it from a dead meter"
        );
        assert!((diagnostics.audio_devices)().is_empty());
        assert_eq!((diagnostics.logs)(&LogQuery::default()), LogPage::default());
        assert!(
            !diagnostics.imports.available,
            "a process that cannot transcode says so before an operator queues a library"
        );
        assert_eq!((diagnostics.imports.state)(), (Vec::new(), Vec::new()));
    }
}
