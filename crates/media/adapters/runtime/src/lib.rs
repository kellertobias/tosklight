#![forbid(unsafe_code)]

//! The Media Server lifecycle adapter.
//!
//! Startup order is fixed and observable: read and validate configuration, install logging, start
//! subsystems, then wait for a shutdown signal. Configuration parses before any subsystem starts,
//! so an unusable configuration stops the process with an actionable error instead of bringing
//! half a server up.

mod logging;
mod shutdown;
mod startup;

pub use logging::install_logging;
pub use shutdown::{Shutdown, ShutdownReason};
pub use startup::{ConfigurationSource, StartupError, load_configuration};

use media_application::MediaConfiguration;

/// The argument that reads, migrates, and validates configuration, then exits.
///
/// Packaging smoke tests use it to prove a real executable on a real platform gets as far as a
/// valid configuration, without needing a display, a network, or an audio device.
pub const CHECK_CONFIGURATION_ARGUMENT: &str = "--check-configuration";

/// Runs the Media Server until it is asked to stop.
pub async fn run() -> anyhow::Result<()> {
    install_logging();
    let check_only = std::env::args().any(|argument| argument == CHECK_CONFIGURATION_ARGUMENT);
    let configuration = load_configuration(&ConfigurationSource::from_environment())?;

    if check_only {
        tracing::info!(
            outputs = configuration.outputs.len(),
            "configuration is valid"
        );
        return Ok(());
    }

    serve(configuration, Shutdown::new()).await
}

/// Brings the configured subsystems up, waits for shutdown, and takes them back down in order.
///
/// The subsystems themselves arrive with their slices. What is already contractual is the
/// sequence: nothing starts before configuration is valid, and everything stops through one
/// structured path rather than by dropping the process. The caller owns the [`Shutdown`] handle
/// so an administrative request and an operating-system signal reach the same path.
pub async fn serve(configuration: MediaConfiguration, shutdown: Shutdown) -> anyhow::Result<()> {
    let outputs = configuration.outputs.len();
    tracing::info!(
        instance = configuration.instance_id.as_str(),
        outputs,
        http = %configuration.network.resolved().http_listen,
        "media server starting"
    );

    let reason = shutdown.wait_for_signal().await;

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
}
