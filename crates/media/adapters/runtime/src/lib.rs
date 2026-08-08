#![forbid(unsafe_code)]

//! The Media Server lifecycle adapter.
//!
//! Startup order is fixed and observable: read and validate configuration, install logging, start
//! subsystems, then wait for a shutdown signal. Configuration parses before any subsystem starts,
//! so an unusable configuration stops the process with an actionable error instead of bringing
//! half a server up.

mod citp;
mod dmx;
mod layer_pipeline;
mod layer_sources;
pub mod log_buffer;
mod logging;
pub mod off_screen;
pub mod presentation;
pub mod preview;
mod shutdown;
mod startup;
mod text_sources;

pub use dmx::SharedState;
pub use layer_sources::LayerSources;
pub use log_buffer::LogBuffer;
pub use logging::install_logging;
pub use presentation::{Diagnostics, SharedConfiguration};
pub use shutdown::{Shutdown, ShutdownReason};
pub use startup::{ConfigurationSource, StartupError, load_configuration};

use media_application::MediaConfiguration;
use media_domain::{MediaState, OutputState, Timestamp};

/// The argument that reads, migrates, and validates configuration, then exits.
///
/// Packaging smoke tests use it to prove a real executable on a real platform gets as far as a
/// valid configuration, without needing a display, a network, or an audio device.
pub const CHECK_CONFIGURATION_ARGUMENT: &str = "--check-configuration";

/// The argument that fills each output with a flat diagnostic colour, so an operator can confirm
/// an output is on the monitor they meant, at the size they meant, the right way up.
pub const TEST_PATTERN_ARGUMENT: &str = "--test-pattern";

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
    let log = install_logging();
    let arguments: Vec<String> = std::env::args().collect();
    let source = ConfigurationSource::from_environment();
    let mut configuration = load_configuration(&source)?;

    // A first run in an existing installation inherits what the previous Media Server had: its
    // text sources are operator data, and cutover must not lose them. Once this server has a
    // document of its own, the operator's catalog is theirs.
    if startup::is_first_run(&source) {
        startup::adopt_legacy_text(&mut configuration, true, unix_millis());
        if !configuration.text.slots.is_empty()
            && let Err(error) = startup::write_configuration(&source.path(), &configuration)
        {
            tracing::error!(%error, "the adopted text sources could not be stored");
        }
    }

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

    // Audio capture is a real capability of the machine: when there is no input device the
    // server says so once and runs on silence, rather than refusing to start.
    let audio = match media_audio::AudioService::start(&configuration.audio) {
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

    // One configuration document, read by the outputs and written by the API. A second copy is a
    // second truth: an operator would edit one and watch the other.
    let live: SharedConfiguration =
        std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(configuration.clone()));
    let diagnostics = diagnostics_of(audio.as_ref(), &log);
    let apply = applies_to(audio.as_ref());

    // What a subscribed console sees. Shared between the outputs, which capture, and the CITP
    // connections, which send.
    let preview: preview::SharedPreview = std::sync::Arc::new(preview::Preview::new());

    // The desk drives the outputs, so the listeners come up before anything presents.
    runtime.block_on(async {
        dmx::spawn(&configuration, state.clone(), shutdown.clone(), started)?;
        citp::spawn(
            &configuration,
            state.clone(),
            catalog.clone(),
            preview.clone(),
            shutdown.clone(),
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
            preview: preview.clone(),
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
    };
    if !presentation::needs_a_window(&configuration) {
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
            preview,
        },
        shutdown.clone(),
        diagnostics_arguments,
        started,
    );
    shutdown.request(ShutdownReason::Requested);
    let _ = runtime.block_on(serving);
    if let Some(thread) = off_screen {
        let _ = thread.join();
    }
    // Closing the device before the process ends keeps the operating system from logging a
    // stream that vanished.
    drop(audio);
    presented
}

/// Milliseconds since the Unix epoch, for a migration that has to resolve a time of day.
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as i64)
        .unwrap_or_default()
}

/// The diagnostics an operator asked for on the command line.
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
    log: &LogBuffer,
) -> media_http::Diagnostics {
    let log = log.clone();
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
        logs: std::sync::Arc::new(move |query| log.page(query)),
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
        diagnostics,
        replays: std::sync::Arc::new(media_http::Replays::new()),
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_empty_media_application_starts_and_shuts_down() {
        let configuration = load_configuration(&ConfigurationSource::Defaults).unwrap();
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
}
