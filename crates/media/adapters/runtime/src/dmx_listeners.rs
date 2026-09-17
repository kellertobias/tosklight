//! The Art-Net and sACN listeners, kept in step with the published configuration.
//!
//! Routing — which universe and start address feed which output — is a table the listeners read
//! for every frame, so an accepted edit reaches the next frame without touching a socket. A socket
//! is rebound only when what it has to be changes: its listen address, whether the protocol is
//! needed at all, or, for sACN, the multicast groups it has to join. A protocol whose binding did
//! not change keeps its socket and never drops a frame.
//!
//! A listener that cannot bind is reported as a network warning and leaves everything else
//! running, at startup and after an edit alike.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use media_application::configuration::{DmxProtocol, MediaConfiguration};
use media_domain::Timestamp;
use media_net::{ArtNetListener, SacnListener, UniverseFrame};
use tokio::task::JoinHandle;

use crate::dmx::{
    Route, SharedDiagnostics, SharedState, SharedUniverseInputs, apply_frame_with_diagnostics,
    capture_handoff_frame, routes,
};
use crate::live_settings::Follower;
use crate::shutdown::Shutdown;

/// Listener problems the operator sees on the Network settings.
pub type SharedWarnings = Arc<Mutex<Vec<String>>>;

/// What every received frame is routed by.
#[derive(Debug, Default, PartialEq)]
struct Plan {
    routes: Vec<Route>,
    handoffs: Vec<(DmxProtocol, u16)>,
}

impl Plan {
    fn of(configuration: &MediaConfiguration) -> Self {
        let mut handoffs: Vec<(DmxProtocol, u16)> = configuration
            .outputs
            .iter()
            .filter(|output| output.enabled)
            .flat_map(|output| output.pixel_map.handoffs.iter())
            .map(|handoff| (handoff.protocol, handoff.input_universe))
            .collect();
        handoffs
            .sort_by_key(|(protocol, universe)| (matches!(protocol, DmxProtocol::Sacn), *universe));
        handoffs.dedup();
        Self {
            routes: routes(configuration),
            handoffs,
        }
    }

    fn uses(&self, protocol: DmxProtocol) -> bool {
        self.routes.iter().any(|route| route.protocol == protocol)
            || self.handoffs.iter().any(|(wanted, _)| *wanted == protocol)
    }

    fn sacn_universes(&self) -> Vec<u16> {
        let mut universes: Vec<u16> = self
            .routes
            .iter()
            .filter(|route| route.protocol == DmxProtocol::Sacn)
            .map(|route| route.universe)
            .chain(
                self.handoffs
                    .iter()
                    .filter(|(protocol, _)| *protocol == DmxProtocol::Sacn)
                    .map(|(_, universe)| *universe),
            )
            .collect();
        universes.sort_unstable();
        universes.dedup();
        universes
    }
}

/// What each protocol's socket has to be. `None` means the protocol is not needed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Bindings {
    art_net: Option<SocketAddr>,
    sacn: Option<(SocketAddr, Vec<u16>)>,
}

impl Bindings {
    fn of(configuration: &MediaConfiguration, plan: &Plan) -> Self {
        // Each protocol is bound only if something uses it, so a show that speaks only Art-Net
        // never holds the sACN port and cannot collide with something else that wants it.
        let resolved = configuration.network.resolved();
        Self {
            art_net: plan
                .uses(DmxProtocol::ArtNet)
                .then_some(resolved.art_net_listen),
            sacn: plan
                .uses(DmxProtocol::Sacn)
                .then(|| (resolved.sacn_listen, plan.sacn_universes())),
        }
    }
}

/// Everything a receiving task needs.
#[derive(Clone)]
struct Context {
    state: SharedState,
    diagnostics: SharedDiagnostics,
    inputs: SharedUniverseInputs,
    plan: Arc<ArcSwap<Plan>>,
    shutdown: Shutdown,
    started: std::time::Instant,
}

impl Context {
    fn receive(&self, frame: &UniverseFrame) {
        let plan = self.plan.load();
        capture_handoff_frame(frame, &plan.handoffs, &self.inputs);
        apply_frame_with_diagnostics(&self.state, &plan.routes, frame, &self.diagnostics);
    }

    fn clock(&self) -> impl Fn() -> Timestamp + Send + 'static {
        let started = self.started;
        move || Timestamp::from_micros(started.elapsed().as_micros() as u64)
    }
}

/// One protocol's running socket, or why it is not running.
#[derive(Default)]
struct Slot {
    task: Option<JoinHandle<()>>,
    warning: Option<String>,
}

impl Slot {
    /// Closes the socket before the caller binds its successor, which may want the same port.
    async fn stop(&mut self) {
        self.warning = None;
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }

    fn unavailable(protocol: &str, address: SocketAddr, error: impl std::fmt::Display) -> Self {
        let warning = format!(
            "{protocol} is unavailable at {address}. Pixel is running without {protocol} input: {error}"
        );
        tracing::warn!(%warning);
        Self {
            task: None,
            warning: Some(warning),
        }
    }
}

fn art_net(context: &Context, address: SocketAddr) -> Slot {
    let mut listener = match ArtNetListener::bind_for_console(address) {
        Ok(listener) => listener,
        Err(error) => return Slot::unavailable("Art-Net", address, error),
    };
    tracing::info!(%address, "listening for Art-Net");
    let (context, mut watcher) = (context.clone(), context.shutdown.watcher());
    let now = context.clock();
    Slot {
        task: Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = watcher.wait() => break,
                    frame = listener.receive(&now) => context.receive(&frame),
                }
            }
        })),
        warning: None,
    }
}

fn sacn(context: &Context, address: SocketAddr, universes: &[u16]) -> Slot {
    let mut listener = match SacnListener::bind(address, universes) {
        Ok(listener) => listener,
        Err(error) => return Slot::unavailable("sACN", address, error),
    };
    tracing::info!(%address, ?universes, "listening for sACN");
    let (context, mut watcher) = (context.clone(), context.shutdown.watcher());
    let now = context.clock();
    Slot {
        task: Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = watcher.wait() => break,
                    frame = listener.receive(&now) => context.receive(&frame),
                }
            }
        })),
        warning: None,
    }
}

/// The listeners this process is running now.
struct Listeners {
    context: Context,
    bound: Bindings,
    art_net: Slot,
    sacn: Slot,
    warnings: SharedWarnings,
}

impl Listeners {
    async fn follow(&mut self, configuration: &MediaConfiguration) {
        let plan = Plan::of(configuration);
        let wanted = Bindings::of(configuration, &plan);
        // Routing first: a socket that stays keeps receiving, now under the new table.
        self.context.plan.store(Arc::new(plan));
        if wanted.art_net != self.bound.art_net {
            self.art_net.stop().await;
            if let Some(address) = wanted.art_net {
                self.art_net = art_net(&self.context, address);
            }
        }
        if wanted.sacn != self.bound.sacn {
            self.sacn.stop().await;
            if let Some((address, universes)) = &wanted.sacn {
                self.sacn = sacn(&self.context, *address, universes);
            }
        }
        self.bound = wanted;
        *self
            .warnings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = self
            .art_net
            .warning
            .iter()
            .chain(self.sacn.warning.iter())
            .cloned()
            .collect();
    }
}

/// Starts the listeners the configuration calls for, and keeps them in step with later edits.
///
/// Returns once the startup listeners are bound, so their warnings are known before anything else
/// starts.
#[allow(clippy::too_many_arguments)]
pub async fn spawn(
    live: Arc<ArcSwap<MediaConfiguration>>,
    mut follower: Follower,
    state: SharedState,
    shutdown: Shutdown,
    started: std::time::Instant,
    diagnostics: SharedDiagnostics,
    inputs: SharedUniverseInputs,
    warnings: SharedWarnings,
) {
    let mut watcher = shutdown.watcher();
    let mut listeners = Listeners {
        context: Context {
            state,
            diagnostics,
            inputs,
            plan: Arc::new(ArcSwap::from_pointee(Plan::default())),
            shutdown,
            started,
        },
        bound: Bindings::default(),
        art_net: Slot::default(),
        sacn: Slot::default(),
        warnings,
    };
    listeners.follow(&live.load()).await;
    tokio::spawn(async move {
        loop {
            let generation = tokio::select! {
                _ = watcher.wait() => break,
                next = follower.next() => match next {
                    Some(generation) => generation,
                    None => break,
                },
            };
            listeners.follow(&live.load()).await;
            follower.applied(generation);
        }
    });
}

#[cfg(test)]
#[path = "dmx_listeners_tests.rs"]
mod tests;
