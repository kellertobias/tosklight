//! What the desk knows about the tracking system between one packet and the next.
//!
//! Two entry points, and the split between them is deliberate. `observe` takes a datagram and
//! remembers it; `tick` decides what that means — which 3D Points are held where, which zones
//! changed, and what to tell the operator. Neither touches a socket, so the whole path from a
//! sender's bytes to a held point can be tested by handing it packets, which is what the tests
//! beside this file do.
//!
//! Two decisions are worth reading twice:
//!
//! **A tracker that has gone quiet keeps its last position.** Not just for the point it drives —
//! for the zones too. A tracking system dropping off the network is not everybody walking off
//! stage, and firing every leave macro at once because a switch rebooted would be the worst
//! possible reading of silence. The operator is told the source is stale; the show is not changed
//! on the desk's own initiative.
//!
//! **Turning PSN off releases every point.** That is the difference between silence and an
//! instruction: switching the source off is something an operator did on purpose, and afterwards
//! the points are the show's again.

use light_engine::TrackedOverride;
use light_psn_wire::{PsnSourceHealth, PsnTracking};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;

use super::bindings::{BindingPlacement, placements};
use super::config::PsnConfiguration;
use super::zones::{ZoneState, ZoneTransition, advance};

/// One tracker as the operator should see it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct TrackerReport {
    pub tracker_id: u16,
    /// What the sender calls it, once an info packet has said so. A data packet carries only the
    /// number, so a source that has been heard for less than a second has no name yet.
    pub name: Option<String>,
    /// Where it is in the show's own stage space, calibration applied, in metres.
    pub position_metres: Option<[f32; 3]>,
    pub age_millis: u64,
    pub stale: bool,
    pub source: SocketAddr,
}

/// Everything the PSN tab shows, resolved at one moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct PsnStatus {
    pub enabled: bool,
    pub listening_on: Option<String>,
    pub health: Option<PsnHealth>,
    pub system_names: Vec<String>,
    pub trackers: Vec<TrackerReport>,
    pub placements: Vec<BindingPlacement>,
    pub occupied_zones: Vec<Uuid>,
    pub frames: u64,
    pub ignored_datagrams: u64,
    /// Why the desk is not listening, when it should be but cannot.
    pub error: Option<String>,
}

/// The source's condition in the words an operator needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PsnHealth {
    /// Nothing has ever arrived. A sender that is switched off looks exactly like a desk on the
    /// wrong network, so this is stated rather than diagnosed.
    Silent,
    Receiving,
    Stale {
        silent_for_millis: u64,
    },
}

impl From<PsnSourceHealth> for PsnHealth {
    fn from(health: PsnSourceHealth) -> Self {
        match health {
            PsnSourceHealth::Silent => Self::Silent,
            PsnSourceHealth::Receiving => Self::Receiving,
            PsnSourceHealth::Stale { silent_for_millis } => Self::Stale { silent_for_millis },
        }
    }
}

/// What one tick decided.
pub(in crate::runtime) struct PsnTick {
    pub overrides: Vec<TrackedOverride>,
    pub zone_transitions: Vec<(Uuid, ZoneTransition)>,
    pub status: PsnStatus,
}

#[derive(Default)]
struct Held {
    /// Per binding, the last show-space position the desk is willing to stand behind.
    positions: HashMap<Uuid, [f32; 3]>,
    zones: HashMap<Uuid, ZoneState>,
}

/// The desk's PosiStageNet state, shared between the listener and everything that reads it.
#[derive(Clone)]
pub(in crate::runtime) struct PsnResource {
    inner: Arc<PsnState>,
}

struct PsnState {
    configuration: RwLock<PsnConfiguration>,
    sources: Mutex<HashMap<SocketAddr, PsnTracking>>,
    held: Mutex<Held>,
    error: RwLock<Option<String>>,
    /// Where every 3D Point in the show was patched, in metres. The server installs this from the
    /// snapshot it owns, so a tick never reaches into the engine while holding its own locks.
    point_locations: RwLock<HashMap<Uuid, [f32; 3]>>,
    /// Bumped whenever the configuration changes, so a listener notices that the group, port or
    /// interface it bound to is no longer the one the show asks for.
    generation: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for PsnResource {
    fn default() -> Self {
        Self::new()
    }
}

impl PsnResource {
    #[must_use]
    pub(in crate::runtime) fn new() -> Self {
        Self {
            inner: Arc::new(PsnState {
                configuration: RwLock::new(PsnConfiguration::default()),
                sources: Mutex::new(HashMap::new()),
                held: Mutex::new(Held::default()),
                error: RwLock::new(None),
                point_locations: RwLock::new(HashMap::new()),
                generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            }),
        }
    }

    pub(in crate::runtime) fn configuration(&self) -> PsnConfiguration {
        self.inner.configuration.read().clone()
    }

    /// Take a new configuration, as accepted from the show.
    ///
    /// Everything heard so far is forgotten when the source moves, because positions from the old
    /// group say nothing about the new one; a change that only edits bindings or zones keeps what
    /// is already arriving.
    pub(in crate::runtime) fn install(&self, configuration: PsnConfiguration) {
        let moved = {
            let current = self.inner.configuration.read();
            current.group != configuration.group
                || current.port != configuration.port
                || current.interface != configuration.interface
                || current.enabled != configuration.enabled
        };
        *self.inner.configuration.write() = configuration;
        if moved {
            self.inner.sources.lock().clear();
            self.inner.held.lock().positions.clear();
            *self.inner.error.write() = None;
        }
        self.inner
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub(in crate::runtime) fn generation(&self) -> u64 {
        self.inner
            .generation
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Say why the desk cannot listen — a group it could not join, a port already taken.
    pub(in crate::runtime) fn set_error(&self, error: Option<String>) {
        *self.inner.error.write() = error;
    }

    /// One datagram from one sender.
    pub(in crate::runtime) fn observe(&self, source: SocketAddr, datagram: &[u8], now_millis: u64) {
        let mut sources = self.inner.sources.lock();
        sources
            .entry(source)
            .or_insert_with(PsnTracking::new)
            .observe(datagram, now_millis);
    }

    /// Work out what is held and what changed, as of `now_millis`.
    pub(in crate::runtime) fn tick(&self, now_millis: u64) -> PsnTick {
        let configuration = self.configuration();
        let mut status = PsnStatus {
            enabled: configuration.enabled,
            listening_on: configuration
                .enabled
                .then(|| format!("{}:{}", configuration.group, configuration.port)),
            error: self.inner.error.read().clone(),
            ..PsnStatus::default()
        };
        let (positions, reports) = self.report_trackers(&configuration, now_millis, &mut status);
        status.trackers = reports;
        let mut held = self.inner.held.lock();
        if !configuration.enabled {
            // An operator switched the source off, so the points go back to the show. Zone state
            // is kept rather than reset: switching PSN on again should not replay every enter.
            held.positions.clear();
            return PsnTick {
                overrides: Vec::new(),
                zone_transitions: Vec::new(),
                status,
            };
        }
        let mut bound = HashMap::new();
        for binding in configuration.active_bindings() {
            if let Some(position) = positions.get(&binding.tracker_id) {
                held.positions.insert(binding.id, position.position);
            }
            if let Some(position) = held.positions.get(&binding.id) {
                bound.insert(binding.id, *position);
            }
        }
        // A binding the operator deleted stops being held in the same tick the others move.
        held.positions.retain(|id, _| bound.contains_key(id));
        let zone_positions = positions
            .iter()
            .map(|(tracker_id, seen)| (*tracker_id, seen.position))
            .collect();
        let zone_transitions =
            advance(&configuration, &zone_positions, &mut held.zones, now_millis);
        status.occupied_zones = held
            .zones
            .iter()
            .filter_map(|(id, state)| state.occupied.then_some(*id))
            .collect();
        status.occupied_zones.sort();
        drop(held);
        let (overrides, placed) = placements(&configuration, &bound, &self.point_locations());
        status.placements = placed;
        PsnTick {
            overrides,
            zone_transitions,
            status,
        }
    }

    fn point_locations(&self) -> HashMap<Uuid, [f32; 3]> {
        self.inner.point_locations.read().clone()
    }

    pub(in crate::runtime) fn install_point_locations(&self, locations: HashMap<Uuid, [f32; 3]>) {
        *self.inner.point_locations.write() = locations;
    }

    /// Every tracker any source has reported, freshest first when two sources claim the same id.
    fn report_trackers(
        &self,
        configuration: &PsnConfiguration,
        now_millis: u64,
        status: &mut PsnStatus,
    ) -> (HashMap<u16, SeenTracker>, Vec<TrackerReport>) {
        let sources = self.inner.sources.lock();
        let mut latest: HashMap<u16, SeenTracker> = HashMap::new();
        let mut reports = Vec::new();
        let mut health: Option<PsnHealth> = None;
        for (source, tracking) in sources.iter() {
            status.frames = status.frames.saturating_add(tracking.frames());
            status.ignored_datagrams = status.ignored_datagrams.saturating_add(tracking.ignored());
            if let Some(name) = tracking.system_name() {
                status.system_names.push(name.to_owned());
            }
            health = Some(worse(
                health,
                tracking
                    .health(now_millis, configuration.stale_after_millis)
                    .into(),
            ));
            for tracked in tracking.trackers() {
                let age_millis = tracked.age_millis(now_millis);
                let position = tracked.position().map(|position| {
                    configuration
                        .calibration
                        .place_in_show([position.x, position.y, position.z])
                });
                reports.push(TrackerReport {
                    tracker_id: tracked.id,
                    name: tracked.name.clone(),
                    position_metres: position,
                    age_millis,
                    stale: age_millis > configuration.stale_after_millis,
                    source: *source,
                });
                if let Some(position) = position {
                    let seen = SeenTracker {
                        position,
                        age_millis,
                    };
                    latest
                        .entry(tracked.id)
                        .and_modify(|current| {
                            if seen.age_millis < current.age_millis {
                                *current = seen;
                            }
                        })
                        .or_insert(seen);
                }
            }
        }
        status.health = health.or(configuration.enabled.then_some(PsnHealth::Silent));
        status.system_names.sort();
        status.system_names.dedup();
        reports.sort_by_key(|report| (report.tracker_id, report.source));
        (latest, reports)
    }
}

#[derive(Clone, Copy)]
struct SeenTracker {
    position: [f32; 3],
    age_millis: u64,
}

/// The condition an operator should be told about when sources disagree: the unhappiest one.
fn worse(current: Option<PsnHealth>, candidate: PsnHealth) -> PsnHealth {
    let rank = |health: PsnHealth| match health {
        PsnHealth::Receiving => 0,
        PsnHealth::Stale { .. } => 1,
        PsnHealth::Silent => 2,
    };
    match current {
        Some(current) if rank(current) >= rank(candidate) => current,
        _ => candidate,
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
