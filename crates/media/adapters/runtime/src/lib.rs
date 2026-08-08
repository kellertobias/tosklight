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
mod logging;
pub mod presentation;
mod shutdown;
mod startup;
mod text_sources;

pub use dmx::SharedState;
pub use layer_sources::LayerSources;
pub use logging::install_logging;
pub use presentation::Diagnostics;
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
    install_logging();
    let arguments: Vec<String> = std::env::args().collect();
    let configuration = load_configuration(&ConfigurationSource::from_environment())?;

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

    let diagnostics = Diagnostics {
        test_pattern: arguments
            .iter()
            .any(|argument| argument == TEST_PATTERN_ARGUMENT),
        play: arguments
            .iter()
            .position(|argument| argument == PLAY_ARGUMENT)
            .and_then(|at| arguments.get(at + 1))
            .map(std::path::PathBuf::from),
    };
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

    // The desk drives the outputs, so the listeners come up before anything presents.
    runtime.block_on(async {
        dmx::spawn(&configuration, state.clone(), shutdown.clone(), started)?;
        citp::spawn(
            &configuration,
            state.clone(),
            catalog.clone(),
            shutdown.clone(),
        );
        anyhow::Ok(())
    })?;

    if !presentation::needs_a_window(&configuration) {
        return runtime.block_on(serve_with(
            configuration,
            shutdown,
            Some(state),
            Some(catalog),
        ));
    }

    // The services run on the background runtime; the main thread hosts the outputs. Shutdown
    // reaches both through the same handle, whichever of them starts it.
    let services = runtime.spawn({
        let configuration = configuration.clone();
        let shutdown = shutdown.clone();
        let state = state.clone();
        let catalog = catalog.clone();
        async move { serve_with(configuration, shutdown, Some(state), Some(catalog)).await }
    });

    let presented = presentation::run_event_loop(
        &configuration,
        state,
        catalog,
        analysis,
        shutdown.clone(),
        diagnostics,
        started,
    );
    shutdown.request(ShutdownReason::Requested);
    let _ = runtime.block_on(services);
    // Closing the device before the process ends keeps the operating system from logging a
    // stream that vanished.
    drop(audio);
    presented
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
    serve_with(configuration, shutdown, None, None).await
}

/// The same, sharing state with the outputs when there are any.
///
/// The API reads and writes exactly the state the renderer presents; there is no second copy for
/// the web to diverge from.
pub async fn serve_with(
    configuration: MediaConfiguration,
    shutdown: Shutdown,
    state: Option<dmx::SharedState>,
    catalog: Option<presentation::SharedCatalog>,
) -> anyhow::Result<()> {
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
        configuration: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(configuration)),
        state,
        catalog,
        now: std::sync::Arc::new(move || {
            Timestamp::from_micros(started.elapsed().as_micros() as u64)
        }),
        persist: std::sync::Arc::new(move |configuration| {
            startup::write_configuration(&configuration_path, configuration)
                .map_err(|error| error.to_string())
        }),
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
