//! Owned background output and administration tasks.

use super::*;

pub(super) fn start_output_services(
    runtime: &tokio::runtime::Runtime,
    shared: &presentation::Shared,
    shutdown: &Shutdown,
    started: std::time::Instant,
    access: &media_http::automation::AutomationAccess,
    ingress: dmx::SharedDiagnostics,
) -> (
    Option<std::thread::JoinHandle<()>>,
    tokio::task::JoinHandle<()>,
) {
    let off_screen = off_screen::spawn(
        &shared.configuration.load(),
        shared.clone(),
        shutdown.clone(),
    );
    let automation = automation::spawn(
        runtime,
        shared.clone(),
        shutdown.clone(),
        started,
        access.clone(),
        ConfigurationSource::from_environment()
            .path()
            .with_extension("automation-runtime.json"),
        ingress,
    );
    (off_screen, automation)
}

pub(super) fn spawn_administration(
    runtime: &tokio::runtime::Runtime,
    shutdown: &Shutdown,
    services: Services,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    let shutdown = shutdown.clone();
    runtime.spawn(async move {
        let result = serve_with(services).await;
        shutdown.request(ShutdownReason::Requested);
        result
    })
}
