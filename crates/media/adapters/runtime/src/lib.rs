#![forbid(unsafe_code)]

//! The Media Server lifecycle adapter.
//!
//! Startup order is fixed and observable: read and validate configuration, install logging, start
//! subsystems, then wait for a shutdown signal. Configuration parses before any subsystem starts,
//! so an unusable configuration stops the process with an actionable error instead of bringing
//! half a server up.

mod beat_form_flash;
mod beat_grid_wave;
mod beat_move;
mod beat_scale_turn;
mod beat_scan;
mod citp;
mod citp_console_presence;
mod dmx;
mod layer_pipeline;
mod layer_sources;
pub mod log_buffer;
mod logging;
pub mod off_screen;
mod opacity_cycle;
pub mod presentation;
pub mod preview;
mod shutdown;
mod standby;
mod startup;
mod text_sources;
mod tray;

pub use dmx::SharedState;
pub use layer_sources::LayerSources;
pub use log_buffer::LogBuffer;
pub use logging::{InstalledLogging, install_logging};
pub use presentation::{Diagnostics, SharedConfiguration};
pub use shutdown::{Shutdown, ShutdownReason};
pub use startup::{
    ConfigurationSource, StartupError, load_configuration, running_from_macos_app_bundle,
};

use media_application::MediaConfiguration;
use media_domain::{MediaState, OutputState, Timestamp};
use std::sync::Arc;

/// The argument that reads, migrates, and validates configuration, then exits.
///
/// Packaging smoke tests use it to prove a real executable on a real platform gets as far as a
/// valid configuration, without needing a display, a network, or an audio device.
pub const CHECK_CONFIGURATION_ARGUMENT: &str = "--check-configuration";

/// The argument that fills each output with a flat diagnostic colour, so an operator can confirm
/// an output is on the monitor they meant, at the size they meant, the right way up.
pub const TEST_PATTERN_ARGUMENT: &str = "--test-pattern";

/// The argument that runs the server without any desktop presence: no event loop, no menu bar
/// item, no outputs. For a machine that has no window server to talk to, or a service manager that
/// wants a plain process.
pub const HEADLESS_ARGUMENT: &str = "--headless";

/// Plays one `.toskclip` on layer one of every output. A development affordance for exercising
/// the whole path — import, residency, session, upload, composite — without a desk or a catalog.
pub const PLAY_ARGUMENT: &str = "--play";

/// Runs the Media Server until it is asked to stop.
///
/// This is synchronous, and deliberately so. Windowed outputs need the platform event loop on the
/// main thread, so the asynchronous services get a background runtime and the main thread belongs
/// to the outputs. A process whose outputs are all off-screen never builds an event loop and
/// simply blocks on the services.
pub fn run() -> anyhow::Result<()> {
    let result = run_inner();
    if let Err(error) = &result {
        show_startup_error(error);
    }
    result
}

fn run_inner() -> anyhow::Result<()> {
    let logging = install_logging();
    let arguments: Vec<String> = std::env::args().collect();

    if !arguments_ask_to_run(&arguments)? {
        return Ok(());
    }

    let configuration = prepare_configuration()?;

    if arguments
        .iter()
        .any(|argument| argument == CHECK_CONFIGURATION_ARGUMENT)
    {
        tracing::info!(
            outputs = configuration.outputs.len(),
            "configuration is valid"
        );
        return Ok(());
    }

    let diagnostics_arguments = diagnostics_asked_for(&arguments);
    let shutdown = Shutdown::new();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let started = std::time::Instant::now();
    let state: dmx::SharedState = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
        initial_state(&configuration),
    ));
    // One catalog, read by the API and by the outputs. A second copy is a second truth, and the
    // picker would eventually offer something the compositor could not resolve.
    let catalog: presentation::SharedCatalog =
        std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            media_library::discover(&configuration.library.root).unwrap_or_default(),
        ));

    let importer = start_importer(&configuration, &catalog);
    let (audio, analysis) = start_audio(&configuration);
    let dmx_diagnostics = dmx::diagnostics();
    let console_identity = citp::ConsoleIdentity::default();
    let available_monitors = std::sync::Arc::new(std::sync::RwLock::new(Vec::new()));

    // One configuration document, read by the outputs and written by the API. A second copy is a
    // second truth: an operator would edit one and watch the other.
    let live: SharedConfiguration =
        std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(configuration.clone()));
    let diagnostics = diagnostics_of(
        audio.as_ref(),
        &logging,
        &importer,
        &configuration.library.root,
        &catalog,
        &dmx_diagnostics,
        &console_identity,
        &available_monitors,
        started,
    );
    let apply = applies_to(audio.as_ref());

    // What a subscribed console sees. Shared between the outputs, which capture, and the CITP
    // connections, which send.
    let previews = preview::SharedPreviews::configured(&configuration);

    // The desk drives the outputs, so the listeners come up before anything presents.
    runtime.block_on(async {
        dmx::spawn(
            &configuration,
            state.clone(),
            shutdown.clone(),
            started,
            dmx_diagnostics,
        )?;
        citp::spawn(
            &configuration,
            state.clone(),
            catalog.clone(),
            live.clone(),
            previews.clone(),
            shutdown.clone(),
            console_identity,
        );
        anyhow::Ok(())
    })?;

    // Off-screen outputs render on their own thread with their own device, so they run whether or
    // not this process also hosts a window. A rack server with no display is still a media server.
    let off_screen = {
        let configuration = configuration.clone();
        let shared = presentation::Shared {
            state: state.clone(),
            catalog: catalog.clone(),
            configuration: live.clone(),
            analysis: analysis.clone(),
            previews: previews.clone(),
        };
        let shutdown = shutdown.clone();
        std::thread::Builder::new()
            .name("media-off-screen".into())
            .spawn(move || off_screen::run(&configuration, shared, shutdown))
            .ok()
    };

    let services = Services {
        configuration: live.clone(),
        shutdown: shutdown.clone(),
        state: Some(state.clone()),
        catalog: Some(catalog.clone()),
        diagnostics,
        apply,
        previews: Some(previews.clone()),
    };
    // What decides this is whether a desktop is reachable, not whether an output is configured.
    // The Media Server is an application with a menu bar item; a server with nothing assigned to a
    // monitor is still that application, and an operator has to be able to see and stop it.
    if !desktop_is_available(&arguments) {
        return runtime.block_on(serve_with(services));
    }

    // The services run on the background runtime; the main thread hosts the outputs. Shutdown
    // reaches both through the same handle, whichever of them starts it.
    let serving = runtime.spawn(async move { serve_with(services).await });

    let presented = presentation::run_event_loop(
        &configuration,
        presentation::Shared {
            state,
            catalog,
            configuration: live,
            analysis,
            previews,
        },
        shutdown.clone(),
        diagnostics_arguments,
        available_monitors,
        started,
        administration_endpoint(&configuration),
    );
    shutdown.request(ShutdownReason::Requested);
    let _ = runtime.block_on(serving);
    if let Some(thread) = off_screen {
        let _ = thread.join();
    }
    importer.stop();
    // Closing the device before the process ends keeps the operating system from logging a
    // stream that vanished.
    drop(audio);
    presented
}

fn prepare_configuration() -> Result<MediaConfiguration, StartupError> {
    let app_mode = running_from_macos_app_bundle();
    let source = ConfigurationSource::from_environment();
    let mut configuration = load_configuration(&source)?;
    let first_run = startup::is_first_run(&source);

    if app_mode && first_run {
        startup::apply_macos_app_defaults(&mut configuration, &source.path());
    }
    // A first run inherits legacy text once, then the resulting document belongs to this server.
    if first_run {
        startup::adopt_legacy_text(&mut configuration, true, unix_millis());
        if (app_mode || !configuration.text.slots.is_empty())
            && let Err(error) = startup::write_configuration(&source.path(), &configuration)
        {
            tracing::error!(%error, "the adopted text sources could not be stored");
        }
    }
    Ok(configuration)
}

/// Makes a Finder-launch failure actionable even though no Terminal window exists.
fn show_startup_error(error: &anyhow::Error) {
    tracing::error!(%error, "ToskLight Media could not start");
    #[cfg(target_os = "macos")]
    if running_from_macos_app_bundle() {
        let message = format!("ToskLight Media could not start.\n\n{error}");
        let _ = std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "display alert \"ToskLight Media\" message (item 1 of argv) as critical buttons {\"OK\"}",
                "-e",
                "end run",
                "--",
                &message,
            ])
            .status();
    }
}

fn administration_endpoint(configuration: &MediaConfiguration) -> String {
    let listen = configuration.network.resolved().http_listen;
    let ip = if listen.ip().is_unspecified() {
        primary_ipv4().unwrap_or(media_application::configuration::LOOPBACK)
    } else {
        match listen.ip() {
            std::net::IpAddr::V4(ip) => ip,
            std::net::IpAddr::V6(_) => media_application::configuration::LOOPBACK,
        }
    };
    format!("{ip}:{}", listen.port())
}

fn primary_ipv4() -> Option<std::net::Ipv4Addr> {
    let socket = std::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket
        .connect((std::net::Ipv4Addr::new(192, 0, 2, 1), 9))
        .ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// Milliseconds since the Unix epoch, for a migration that has to resolve a time of day.
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as i64)
        .unwrap_or_default()
}

/// Starts the import pool.
///
/// A finished import republishes the catalog, because the picker and the compositor read the same
/// snapshot: a clip that has arrived has to appear in it without anyone restarting anything.
fn start_importer(
    configuration: &MediaConfiguration,
    catalog: &presentation::SharedCatalog,
) -> media_library::Importer {
    let catalog = catalog.clone();
    let root = configuration.library.root.clone();
    media_library::Importer::start(
        media_library::LibraryStorage::new(configuration.library.root.clone()),
        IMPORT_CONCURRENCY,
        std::sync::Arc::new(move || {
            if let Ok(published) = media_library::discover(&root) {
                catalog.store(std::sync::Arc::new(published));
            }
        }),
    )
}

/// Opens the configured audio input, and the analysis whatever happened publishes into.
///
/// Audio capture is a real capability of the machine: when there is no input device the server
/// says so once and runs on silence, rather than refusing to start.
fn start_audio(
    configuration: &MediaConfiguration,
) -> (
    Option<media_audio::AudioService>,
    media_audio::SharedAnalysis,
) {
    let audio = match media_audio::AudioService::start_bounded(&configuration.audio) {
        Ok(service) => Some(service),
        Err(error) => {
            tracing::warn!(%error, "no audio input; generated sources will run on silence");
            None
        }
    };
    let analysis = audio.as_ref().map_or_else(
        || {
            std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
                media_audio::AnalysisSnapshot::default(),
            ))
        },
        media_audio::AudioService::analysis,
    );
    (audio, analysis)
}

/// How many clips are converted at once.
///
/// Import is CPU-bound compression and a show may be running on the same machine, so this is
/// deliberately small: a library still drains, and the outputs still get their frames.
const IMPORT_CONCURRENCY: usize = 2;

/// The diagnostics an operator asked for on the command line.
/// Whether this process can own a desktop presence.
///
/// Asked for explicitly, or on a Unix desktop that names no display server, the answer is no and
/// the server runs as a plain process. macOS and Windows always have one when a user launches the
/// application; a daemon on either that does not gets `--headless`.
fn desktop_is_available(arguments: &[String]) -> bool {
    if arguments
        .iter()
        .any(|argument| argument == HEADLESS_ARGUMENT)
    {
        return false;
    }
    if cfg!(all(unix, not(target_os = "macos"))) {
        return std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
    }
    true
}

/// What the command line asked for, before any of it is acted on.
enum Understanding {
    /// Every argument is one this server acts on.
    Run,
    /// Usage was asked for.
    Explain,
    /// An argument nobody here recognises. Named so the message can say which.
    Reject(String),
}

/// Answer the command line before anything is read or bound, and say whether to carry on.
///
/// An unrecognised argument used to fall through to a normal startup, so `--help` began a server
/// that took the Art-Net port with it and, having no window, left nothing on screen to say so.
fn arguments_ask_to_run(arguments: &[String]) -> anyhow::Result<bool> {
    match arguments_are_understood(arguments) {
        Understanding::Run => Ok(true),
        Understanding::Explain => {
            println!("{USAGE}");
            Ok(false)
        }
        Understanding::Reject(argument) => {
            anyhow::bail!("{argument} is not an argument this server understands.\n\n{USAGE}")
        }
    }
}

/// Decide what to do with the command line.
///
/// Unknown arguments are refused rather than ignored, because the alternative response to a typo
/// is starting a server that holds the Art-Net port for as long as nobody notices it.
fn arguments_are_understood(arguments: &[String]) -> Understanding {
    let mut remaining = arguments.iter().skip(1); // The executable's own path.
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--help" | "-h" => return Understanding::Explain,
            CHECK_CONFIGURATION_ARGUMENT | TEST_PATTERN_ARGUMENT | HEADLESS_ARGUMENT => {}
            // The one argument that carries a value; the value is not an argument itself.
            PLAY_ARGUMENT => {
                if remaining.next().is_none() {
                    return Understanding::Reject(format!("{PLAY_ARGUMENT} without a file"));
                }
            }
            other => return Understanding::Reject(other.to_owned()),
        }
    }
    Understanding::Run
}

/// What `--help` prints. Written out rather than generated: there are five arguments, and a
/// dependency on an argument parser would be the larger thing to keep in sync.
const USAGE: &str = concat!(
    "ToskLight Media\n",
    "\n",
    "Runs the media server. With no arguments it serves, opens the outputs its configuration\n",
    "assigns to monitors, and shows an icon in the menu bar or notification area.\n",
    "\n",
    "  --headless             Serve without any desktop presence: no event loop, no menu bar\n",
    "                         item, no output windows. For a machine with no window server.\n",
    "  --check-configuration  Read, migrate, and validate the configuration, then exit.\n",
    "  --test-pattern         Fill each output with a flat colour, to confirm it is on the\n",
    "                         monitor you meant, at the size you meant, the right way up.\n",
    "  --play <file>          Play one file directly, instead of what the layers resolve to.\n",
    "  -h, --help             Print this and exit.\n",
    "\n",
    "The configuration file is named by MEDIA_CONFIG; MEDIA_LOG sets the logging level.",
);

fn diagnostics_asked_for(arguments: &[String]) -> Diagnostics {
    Diagnostics {
        test_pattern: arguments
            .iter()
            .any(|argument| argument == TEST_PATTERN_ARGUMENT),
        play: arguments
            .iter()
            .position(|argument| argument == PLAY_ARGUMENT)
            .and_then(|at| arguments.get(at + 1))
            .map(std::path::PathBuf::from),
    }
}

/// What a running subsystem honours as soon as an edit is stored.
///
/// The analysis tuning is the one an operator turns while listening, so it reaches the worker
/// immediately. Everything else about audio — which device is open — is a stream, and a stream is
/// opened at startup.
fn applies_to(audio: Option<&media_audio::AudioService>) -> media_http::ApplyConfiguration {
    match audio {
        Some(service) => {
            let tuning = service.tuning();
            std::sync::Arc::new(move |configuration: &MediaConfiguration| {
                tuning.store(std::sync::Arc::new(media_audio::tuning_of(
                    &configuration.audio,
                )));
            })
        }
        None => media_http::applies_nothing(),
    }
}

/// What this process can tell the API about itself.
///
/// Each of these is knowledge only the running process has: whether a device is open, which inputs
/// this machine offers, and what has been logged. The API is handed functions rather than any of
/// the objects behind them, because a platform stream belongs to the thread that opened it while a
/// request arrives on another.
fn diagnostics_of(
    audio: Option<&media_audio::AudioService>,
    logging: &logging::InstalledLogging,
    importer: &media_library::Importer,
    library_root: &std::path::Path,
    catalog: &presentation::SharedCatalog,
    dmx_diagnostics: &dmx::SharedDiagnostics,
    console_identity: &citp::ConsoleIdentity,
    available_monitors: &std::sync::Arc<std::sync::RwLock<Vec<media_http::MonitorDevice>>>,
    started: std::time::Instant,
) -> media_http::Diagnostics {
    let log = logging.window.clone();
    let dmx_diagnostics = dmx_diagnostics.clone();
    let console_identity = console_identity.clone();
    let available_monitors = available_monitors.clone();
    media_http::Diagnostics {
        audio: match audio {
            Some(service) => {
                let analysis = service.analysis();
                let device = service.device().to_owned();
                std::sync::Arc::new(move || {
                    let heard = analysis.load();
                    media_http::AudioTelemetry {
                        capturing: true,
                        device: device.clone(),
                        detail: None,
                        waveform: heard.analysis.waveform.clone(),
                        spectrum: heard.analysis.spectrum.clone(),
                        bass: heard.analysis.bass,
                        mid: heard.analysis.mid,
                        treble: heard.analysis.treble,
                        energy: heard.analysis.energy,
                        peak: heard.analysis.peak,
                        beat: heard.beat,
                        bpm: heard.bpm,
                        beat_phase: heard.beat_phase,
                    }
                })
            }
            // No device, which is a real state and not a failure: the visualizers run on silence
            // and the monitor says why the meter is flat.
            None => std::sync::Arc::new(media_http::AudioTelemetry::default),
        },
        audio_devices: std::sync::Arc::new(media_audio::input_devices),
        output_devices: std::sync::Arc::new(media_audio::output_devices),
        monitors: std::sync::Arc::new(move || {
            available_monitors
                .read()
                .map(|monitors| monitors.clone())
                .unwrap_or_default()
        }),
        logs: std::sync::Arc::new(move |query| log.page(query)),
        log_level: logging.control(),
        imports: imports_of(importer, library_root),
        library: library_access(importer, library_root, catalog),
        dmx: std::sync::Arc::new(move || {
            dmx::diagnostic_snapshot(
                &dmx_diagnostics,
                Timestamp::from_micros(started.elapsed().as_micros() as u64),
            )
        }),
        desk_identity: std::sync::Arc::new(move || console_identity.snapshot()),
    }
}

/// Durable library work stays in the runtime/library boundary. The HTTP adapter carries typed
/// intent and bytes, but it never receives the library root or opens an operator-owned path.
fn library_access(
    importer: &media_library::Importer,
    library_root: &std::path::Path,
    catalog: &presentation::SharedCatalog,
) -> media_http::LibraryAccess {
    let storage = media_library::LibraryStorage::new(library_root.to_path_buf());
    let editing = storage.clone();
    let reading = storage.clone();
    let uploading_root = library_root.to_path_buf();
    let published = catalog.clone();
    let edit_lock = std::sync::Arc::new(std::sync::Mutex::new(()));
    let upload_importer = importer.clone();
    let folder_storage = storage.clone();
    let folder_reading = storage.clone();
    let folder_picture_reading = storage.clone();
    let folder_bytes_reading = storage.clone();
    let folder_edit_lock = edit_lock.clone();
    let folder_picture_lock = edit_lock.clone();
    let folder_remove_lock = edit_lock.clone();
    let folder_published = catalog.clone();
    let folder_picture_published = catalog.clone();
    let folder_remove_published = catalog.clone();

    media_http::LibraryAccess {
        edit: std::sync::Arc::new(move |operation| {
            let _guard = edit_lock
                .lock()
                .map_err(|_| "the library edit lock is unavailable".to_owned())?;
            let mut next = (*published.load_full()).clone();
            match operation {
                media_http::LibraryEdit::RenameItem { id, name } => {
                    editing.rename_item(&mut next, id, &name)
                }
                media_http::LibraryEdit::MoveItem {
                    id,
                    destination,
                    swap,
                } => {
                    let occupant = next
                        .folder(destination.folder)
                        .and_then(|folder| folder.item(destination.file))
                        .map(|item| item.id);
                    match (occupant, swap) {
                        (Some(other), true) => editing.swap_items(&mut next, id, other),
                        _ => editing.move_item(&mut next, id, destination),
                    }
                }
                media_http::LibraryEdit::SetItemBpm { id, bpm } => {
                    editing.set_intrinsic_bpm(&mut next, id, bpm)
                }
                media_http::LibraryEdit::RenameFolder { folder, name } => {
                    editing.rename_folder(&mut next, folder, name.as_deref())
                }
                media_http::LibraryEdit::SetFolderIcon { folder, icon } => {
                    editing.set_folder_icon(&mut next, folder, icon.as_deref())
                }
                media_http::LibraryEdit::SwapFolders { first, second } => {
                    editing.swap_folders(&mut next, first, second)
                }
            }
            .map_err(|error| error.to_string())?;
            published.store(std::sync::Arc::new(next));
            Ok(())
        }),
        thumbnail: std::sync::Arc::new(move |address| {
            let path = reading.thumbnail_path(address);
            std::fs::read(&path)
                .map_err(|error| format!("cannot read thumbnail {}: {error}", path.display()))
        }),
        begin_upload: std::sync::Arc::new(move |address, name, filename, replace| {
            let upload = if replace {
                media_library::Upload::begin_replacement(&uploading_root, address, name, filename)
            } else {
                media_library::Upload::begin(&uploading_root, address, name, filename)
            }
            .map_err(|error| error.to_string())?;
            Ok(Box::new(RuntimeUpload {
                upload: Some(upload),
                importer: upload_importer.clone(),
                address,
                name: name.to_owned(),
            }) as Box<dyn media_http::UploadStream>)
        }),
        folder_presentations: std::sync::Arc::new(move || {
            (1..=255)
                .map(|folder| {
                    folder_reading
                        .folder_presentation(folder)
                        .map(folder_presentation_of)
                        .map_err(|error| error.to_string())
                })
                .collect()
        }),
        update_folder_presentation: std::sync::Arc::new(move |folder, name, icon| {
            let _guard = folder_edit_lock
                .lock()
                .map_err(|_| "the folder presentation edit lock is unavailable".to_owned())?;
            if media_domain::catalog::is_storage_folder(folder) {
                let mut next = (*folder_published.load_full()).clone();
                match (name, icon) {
                    (Some(name), None) => {
                        folder_storage.rename_folder(&mut next, folder, name.as_deref())
                    }
                    (None, Some(icon)) => {
                        folder_storage.set_folder_icon(&mut next, folder, icon.as_deref())
                    }
                    _ => unreachable!("the HTTP route validates one presentation intent"),
                }
                .map_err(|error| error.to_string())?;
                folder_published.store(std::sync::Arc::new(next));
            } else {
                folder_storage
                    .update_generated_folder_presentation(
                        folder,
                        name.as_ref().map(|value| value.as_deref()),
                        icon.as_ref().map(|value| value.as_deref()),
                    )
                    .map_err(|error| error.to_string())?;
            }
            folder_storage
                .folder_presentation(folder)
                .map(folder_presentation_of)
                .map_err(|error| error.to_string())
        }),
        set_folder_picture: std::sync::Arc::new(move |folder, content_type, bytes| {
            let _guard = folder_picture_lock
                .lock()
                .map_err(|_| "the folder picture edit lock is unavailable".to_owned())?;
            let mut next = (*folder_picture_published.load_full()).clone();
            folder_picture_reading
                .write_folder_picture(&mut next, folder, content_type, bytes)
                .map_err(|error| error.to_string())?;
            if media_domain::catalog::is_storage_folder(folder) {
                folder_picture_published.store(std::sync::Arc::new(next));
            }
            folder_picture_reading
                .folder_presentation(folder)
                .map(folder_presentation_of)
                .map_err(|error| error.to_string())
        }),
        remove_folder_picture: std::sync::Arc::new(move |folder| {
            let _guard = folder_remove_lock
                .lock()
                .map_err(|_| "the folder picture edit lock is unavailable".to_owned())?;
            let mut next = (*folder_remove_published.load_full()).clone();
            storage
                .remove_folder_picture(&mut next, folder)
                .map_err(|error| error.to_string())?;
            if media_domain::catalog::is_storage_folder(folder) {
                folder_remove_published.store(std::sync::Arc::new(next));
            }
            storage
                .folder_presentation(folder)
                .map(folder_presentation_of)
                .map_err(|error| error.to_string())
        }),
        folder_picture: std::sync::Arc::new(move |folder| {
            folder_bytes_reading
                .read_folder_picture(folder)
                .map_err(|error| error.to_string())
        }),
    }
}

fn folder_presentation_of(
    presentation: media_library::FolderPresentation,
) -> media_http::FolderPresentation {
    media_http::FolderPresentation {
        folder: presentation.folder,
        name: presentation.name,
        icon: presentation.icon,
        picture_content_type: presentation.picture_content_type,
    }
}

struct RuntimeUpload {
    upload: Option<media_library::Upload>,
    importer: media_library::Importer,
    address: media_domain::MediaAddress,
    name: String,
}

impl media_http::UploadStream for RuntimeUpload {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.upload
            .as_mut()
            .ok_or_else(|| "the upload is already complete".to_owned())?
            .write(bytes)
            .map_err(|error| error.to_string())
    }

    fn finish(mut self: Box<Self>) -> Result<String, String> {
        let source = self
            .upload
            .take()
            .ok_or_else(|| "the upload is already complete".to_owned())?
            .finish()
            .map_err(|error| error.to_string())?;
        Ok(self
            .importer
            .submit(source, self.address, &self.name)
            .to_string())
    }
}

/// What the API can ask and tell the import pool.
fn imports_of(
    importer: &media_library::Importer,
    library_root: &std::path::Path,
) -> media_http::Imports {
    let reading = importer.clone();
    let starting = importer.clone();
    let cancelling = importer.clone();
    let root = library_root.to_path_buf();
    let start_root = root.clone();

    media_http::Imports {
        state: std::sync::Arc::new(move || {
            let pending = media_library::pending_imports(&root)
                .into_iter()
                .map(|item| media_http::PendingImport {
                    destination: item.destination,
                    name: item.name,
                    filename: filename_of(&item.source),
                })
                .collect();
            let jobs = reading.jobs().iter().map(job_of).collect();
            (pending, jobs)
        }),
        start: std::sync::Arc::new(move |address| {
            media_library::pending_imports(&start_root)
                .into_iter()
                .filter(|item| address.is_none_or(|wanted| item.destination == wanted))
                .map(|item| {
                    starting.submit(item.source, item.destination, &item.name);
                })
                .count()
        }),
        cancel: std::sync::Arc::new(move |id| {
            cancelling
                .jobs()
                .iter()
                .find(|job| job.id.to_string() == id)
                .is_some_and(|job| cancelling.cancel(job.id))
        }),
        // Import shells out to FFmpeg. A machine without it should say so before an operator
        // queues a whole library that will fail one clip at a time.
        available: media_codec::import::ffmpeg_available(),
    }
}

fn filename_of(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

fn job_of(job: &media_library::Job) -> media_http::ImportJob {
    use media_library::JobState;
    let (outcome, frames_done, frames_total) = match &job.state {
        JobState::Queued => (media_http::ImportOutcome::Queued, None, None),
        JobState::Running {
            frames_done,
            frames_total,
        } => (
            media_http::ImportOutcome::Running,
            Some(*frames_done),
            *frames_total,
        ),
        JobState::Succeeded { frames } => {
            (media_http::ImportOutcome::Succeeded, Some(*frames), None)
        }
        JobState::Failed { reason } => (
            media_http::ImportOutcome::Failed {
                reason: reason.clone(),
            },
            None,
            None,
        ),
        JobState::Cancelled => (media_http::ImportOutcome::Cancelled, None, None),
    };
    media_http::ImportJob {
        id: job.id.to_string(),
        destination: job.destination,
        filename: filename_of(&job.source),
        outcome,
        fraction: job.state.fraction(),
        frames_done,
        frames_total,
    }
}

/// The authoritative state one configuration describes, before anything has driven it.
pub fn initial_state(configuration: &MediaConfiguration) -> MediaState {
    MediaState::with_outputs(
        configuration
            .outputs
            .iter()
            .filter(|output| output.enabled)
            .map(|output| OutputState::new(output.id, output.personality))
            .collect(),
    )
}

/// Brings the configured subsystems up, waits for shutdown, and takes them back down in order.
///
/// The subsystems themselves arrive with their slices. What is already contractual is the
/// sequence: nothing starts before configuration is valid, and everything stops through one
/// structured path rather than by dropping the process. The caller owns the [`Shutdown`] handle
/// so an administrative request and an operating-system signal reach the same path.
pub async fn serve(configuration: MediaConfiguration, shutdown: Shutdown) -> anyhow::Result<()> {
    serve_with(Services {
        configuration: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(configuration)),
        shutdown,
        state: None,
        catalog: None,
        diagnostics: media_http::Diagnostics::default(),
        apply: media_http::applies_nothing(),
        previews: None,
    })
    .await
}

/// Everything the services need from the process that started them.
///
/// A value rather than a parameter list, because the outputs, the API, and the diagnostics all
/// share the same handles and adding a sixth argument to a function nobody can read is not an
/// improvement.
pub struct Services {
    /// The live configuration document, shared with the outputs.
    pub configuration: SharedConfiguration,
    pub shutdown: Shutdown,
    /// The state the outputs present, when this process has any.
    pub state: Option<dmx::SharedState>,
    pub catalog: Option<presentation::SharedCatalog>,
    /// What the API can learn about the running process.
    pub diagnostics: media_http::Diagnostics,
    /// What a running subsystem does when an edit is accepted.
    pub apply: media_http::ApplyConfiguration,
    /// The compositor/CITP preview slots, when this process owns a renderer.
    pub previews: Option<preview::SharedPreviews>,
}

/// Brings the API up, waits for shutdown, and takes it back down.
///
/// The API reads and writes exactly the state the renderer presents and the configuration the
/// outputs read; there is no second copy for the web to diverge from.
pub async fn serve_with(services: Services) -> anyhow::Result<()> {
    let Services {
        configuration: live,
        shutdown,
        state,
        catalog,
        diagnostics,
        apply,
        previews,
    } = services;
    let configuration = live.load_full();
    let resolved = configuration.network.resolved();
    let outputs = configuration.outputs.len();
    tracing::info!(
        instance = configuration.instance_id.as_str(),
        outputs,
        http = %resolved.http_listen,
        "media server starting"
    );

    let state = state.unwrap_or_else(|| {
        std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(initial_state(
            &configuration,
        )))
    });
    let catalog = catalog.unwrap_or_else(|| {
        std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
            media_library::discover(&configuration.library.root).unwrap_or_default(),
        ))
    });
    tracing::info!(
        items = catalog.load().item_count(),
        root = %configuration.library.root.display(),
        "library discovered"
    );

    let started = std::time::Instant::now();
    // Where an accepted edit is written. The API adapter never touches the filesystem itself; it
    // is handed the one path this run was started from, so a saved edit lands where the next
    // start will read it.
    let configuration_path = ConfigurationSource::from_environment().path();
    let api = media_http::ApiState {
        configuration: live,
        active_configuration: Arc::clone(&configuration),
        administration_endpoint: administration_endpoint(&configuration),
        state,
        catalog,
        now: std::sync::Arc::new(move || {
            Timestamp::from_micros(started.elapsed().as_micros() as u64)
        }),
        persist: std::sync::Arc::new(move |configuration| {
            startup::write_configuration(&configuration_path, configuration)
                .map_err(|error| error.to_string())
        }),
        apply,
        preview: std::sync::Arc::new(move |output, layer, size| {
            let previews = previews.as_ref()?;
            let preview = match layer {
                Some(layer) => previews.for_layer(output, layer)?,
                None => previews.for_output(output)?,
            };
            preview.requested_for_browser(size);
            let (sequence, frame) = preview.latest_web()?;
            Some(media_http::OutputPreviewFrame {
                sequence,
                width: frame.width,
                height: frame.height,
                content_type: frame.content_type,
                bytes: frame.bytes.clone(),
            })
        }),
        diagnostics,
        replays: std::sync::Arc::new(media_http::Replays::new()),
        // Multipart framing adds a small amount around the library's authoritative payload bound.
        upload_body_limit: media_library::MAX_UPLOAD_BYTES as usize + 1024 * 1024,
    };

    let listener = tokio::net::TcpListener::bind(resolved.http_listen).await.map_err(|error| {
        anyhow::anyhow!(
            "cannot bind the administration interface to {}: {error}. Another process already              holds it.",
            resolved.http_listen
        )
    })?;
    tracing::info!(address = %resolved.http_listen, "administration interface listening");

    let serving = shutdown.clone();
    axum::serve(listener, media_http::router(api))
        .with_graceful_shutdown(async move {
            let _ = serving.watcher().wait().await;
        })
        .await?;

    let reason = shutdown.reason().unwrap_or(ShutdownReason::Requested);

    tracing::info!(reason = reason.as_str(), "media server stopping");
    Ok(())
}

#[cfg(test)]
mod desktop_presence_tests {
    use super::*;

    #[test]
    fn asking_for_usage_does_not_start_a_server() {
        for asked in ["--help", "-h"] {
            let arguments = vec!["media-server".to_owned(), asked.to_owned()];

            assert!(matches!(
                arguments_are_understood(&arguments),
                Understanding::Explain
            ));
        }
    }

    #[test]
    fn an_unknown_argument_is_refused_rather_than_ignored() {
        // Ignoring it starts a server that holds the Art-Net port until somebody goes looking.
        let arguments = vec!["media-server".to_owned(), "--headles".to_owned()];

        let understanding = arguments_are_understood(&arguments);

        assert!(matches!(understanding, Understanding::Reject(named) if named == "--headles"));
    }

    #[test]
    fn the_file_after_play_is_not_read_as_an_argument() {
        let arguments = vec![
            "media-server".to_owned(),
            PLAY_ARGUMENT.to_owned(),
            "--not-an-argument.mp4".to_owned(),
            HEADLESS_ARGUMENT.to_owned(),
        ];

        assert!(matches!(
            arguments_are_understood(&arguments),
            Understanding::Run
        ));
    }

    #[test]
    fn play_without_a_file_is_refused() {
        let arguments = vec!["media-server".to_owned(), PLAY_ARGUMENT.to_owned()];

        assert!(matches!(
            arguments_are_understood(&arguments),
            Understanding::Reject(_)
        ));
    }

    #[test]
    fn every_documented_argument_is_one_the_server_acts_on() {
        // Usage that names an argument the parser refuses would be worse than no usage at all.
        for argument in [
            HEADLESS_ARGUMENT,
            CHECK_CONFIGURATION_ARGUMENT,
            TEST_PATTERN_ARGUMENT,
        ] {
            assert!(USAGE.contains(argument), "{argument} is undocumented");
            assert!(matches!(
                arguments_are_understood(&["media-server".to_owned(), argument.to_owned()]),
                Understanding::Run
            ));
        }
        assert!(USAGE.contains(PLAY_ARGUMENT));
    }

    #[test]
    fn asking_for_headless_runs_without_a_desktop_presence() {
        let arguments = vec!["media-server".to_owned(), HEADLESS_ARGUMENT.to_owned()];

        assert!(!desktop_is_available(&arguments));
    }

    #[test]
    fn a_desktop_platform_runs_the_event_loop_for_its_menu_bar_item() {
        // Not conditioned on the outputs: a server with nothing assigned to a monitor is still an
        // application, and an operator still has to be able to see and stop it.
        let arguments = vec!["media-server".to_owned()];

        let available = desktop_is_available(&arguments);

        if cfg!(all(unix, not(target_os = "macos"))) {
            assert_eq!(
                available,
                std::env::var_os("DISPLAY").is_some()
                    || std::env::var_os("WAYLAND_DISPLAY").is_some(),
                "a Unix desktop is only available when a display server is named"
            );
        } else {
            assert!(available);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_empty_media_application_starts_and_shuts_down() {
        let mut configuration = load_configuration(&ConfigurationSource::Defaults).unwrap();
        configuration.network.http_listen = "127.0.0.1:0".parse().unwrap();
        let shutdown = Shutdown::new();

        let requester = shutdown.clone();
        tokio::spawn(async move { requester.request(ShutdownReason::Requested) });

        serve(configuration, shutdown.clone()).await.unwrap();
        assert_eq!(shutdown.reason(), Some(ShutdownReason::Requested));
    }

    #[test]
    fn the_initial_state_mirrors_the_enabled_outputs() {
        let mut configuration = MediaConfiguration::default();
        let personality = configuration.outputs[0].personality;
        let state = initial_state(&configuration);
        assert_eq!(state.outputs.len(), 1);
        assert_eq!(
            state.outputs[0].layers.len(),
            usize::from(personality.layer_count())
        );

        configuration.outputs[0].enabled = false;
        assert!(
            initial_state(&configuration).outputs.is_empty(),
            "a disabled output holds no state"
        );
    }

    #[test]
    fn the_standby_address_is_a_literal_usable_ip_and_port() {
        let mut configuration = MediaConfiguration::default();
        configuration.network.http_listen = "10.42.0.8:9090".parse().unwrap();

        assert_eq!(administration_endpoint(&configuration), "10.42.0.8:9090");
    }
}
