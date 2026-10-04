//! Runtime PSN ownership. Configuration/show changes are explicit boundaries; incoming motion
//! never edits the show. One resource lock makes install, ingress and tick atomic. Status reads
//! only project state and cannot advance zones or consume their macro transitions.
use light_core::ShowId;
#[cfg(test)]
use light_engine::TrackedOverride;
use light_psn_wire::{
    PSN_MAX_PACKET_BYTES, PsnAcceptedSample, PsnIngressDiagnostics, PsnObservation,
    PsnSourceHealth, PsnTracking,
};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(test)]
use super::bindings::placements;
use super::bindings::{BindingPlacement, placement_reports};
use super::config::PsnConfiguration;
use super::zones::{ZoneState, ZoneTransition, advance};

/// Reject newcomers at capacity instead of evicting an existing source which owns a held point.
/// An explicit show/network/enable boundary frees the table; silence never changes ownership.
const MAX_PSN_SOURCES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct TrackingSampleIdentity {
    pub source_generation: u64,
    pub source: SocketAddr,
    pub sample: PsnAcceptedSample,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct TrackerReport {
    pub tracker_id: u16,
    pub name: Option<String>,
    pub position_metres: Option<[f32; 3]>,
    pub age_millis: u64,
    pub stale: bool,
    pub source: SocketAddr,
    /// Identity of this tracker's last finite position, independent of newer metadata.
    pub accepted_sample: Option<TrackingSampleIdentity>,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct SourceReport {
    pub source: SocketAddr,
    pub accepted_sample: Option<TrackingSampleIdentity>,
    pub diagnostics: PsnIngressDiagnostics,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct PsnStatus {
    pub enabled: bool,
    pub listening_on: Option<String>,
    pub health: Option<PsnHealth>,
    pub system_names: Vec<String>,
    pub trackers: Vec<TrackerReport>,
    pub sources: Vec<SourceReport>,
    pub placements: Vec<BindingPlacement>,
    pub occupied_zones: Vec<Uuid>,
    pub frames: u64,
    pub ignored_datagrams: u64,
    pub rejected_source_datagrams: u64,
    pub source_capacity: usize,
    /// Raw finite input can overflow a valid but large calibration. These rows remain unavailable.
    pub invalid_calibrated_positions: usize,
    /// Stored rows withheld because their binding UUID is repeated, including disabled rows.
    pub conflicting_binding_rows: usize,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PsnHealth {
    Silent,
    Receiving,
    Stale { silent_for_millis: u64 },
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct BoundTrackingSample {
    pub binding_id: Uuid,
    pub point_fixture_id: Uuid,
    pub position_metres: [f32; 3],
    pub identity: TrackingSampleIdentity,
    pub position_received_at_millis: u64,
}

/// Coherent resource snapshot for later engine frame plumbing. Consumers combine it with their
/// retained output generation; they must not independently join a later status read to old DMX.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PsnTrackingSnapshot {
    pub show_id: Option<ShowId>,
    pub configuration_generation: u64,
    pub point_generation: u64,
    pub source_generation: u64,
    pub accepted_sequence: u64,
    pub sampled_at_millis: u64,
    pub bindings: Arc<[BoundTrackingSample]>,
}

pub(in crate::runtime) struct PsnTick {
    #[cfg(test)]
    pub overrides: Vec<TrackedOverride>,
    pub zone_transitions: Vec<(Uuid, ZoneTransition)>,
    pub status: PsnStatus,
    pub tracking: PsnTrackingSnapshot,
}

struct CompiledConfiguration {
    configuration: PsnConfiguration,
    bindings: Vec<light_application::PsnBinding>,
    binding_identities: HashMap<Uuid, (u16, Uuid)>,
    conflicting_binding_rows: usize,
    listening_on: Option<String>,
}
impl CompiledConfiguration {
    fn new(configuration: PsnConfiguration) -> Self {
        // Cold compilation preserves the stored body, including older malformed rows. A UUID
        // collision has no unambiguous owner, even if one row is disabled; withhold every row
        // sharing that identity rather than choosing a tracker or Point by storage order.
        let mut counts = HashMap::<Uuid, usize>::with_capacity(configuration.bindings.len());
        for binding in &configuration.bindings {
            *counts.entry(binding.id).or_default() += 1;
        }
        let conflicting_binding_rows = counts.values().filter(|count| **count > 1).sum();
        let bindings: Vec<_> = configuration
            .active_bindings()
            .filter(|binding| counts.get(&binding.id) == Some(&1))
            .copied()
            .collect();
        let binding_identities = bindings
            .iter()
            .map(|binding| (binding.id, (binding.tracker_id, binding.point_fixture_id)))
            .collect();
        let listening_on = configuration
            .enabled
            .then(|| format!("{}:{}", configuration.group, configuration.port));
        Self {
            configuration,
            bindings,
            binding_identities,
            conflicting_binding_rows,
            listening_on,
        }
    }
}

#[derive(Clone, Copy)]
struct SeenTracker {
    /// Exact finite PSN coordinates for this accepted position, before calibration.
    raw_position: [f32; 3],
    position: [f32; 3],
    /// A calibration edit could not reproject this sample; position remains the last finite hold.
    calibration_invalid: bool,
    received_at_millis: u64,
    identity: TrackingSampleIdentity,
}

impl SeenTracker {
    /// Candidate records already come from each sender's latest accepted positional sample.
    /// Equal receiver milliseconds must allow another accepted sample from the same sender
    /// (or reprojected calibration), while competing senders retain their stable address tie.
    fn supersedes(self, current: Self) -> bool {
        self.received_at_millis > current.received_at_millis
            || (self.received_at_millis == current.received_at_millis
                && self.identity.source <= current.identity.source)
    }
}

#[derive(Default)]
struct Held {
    positions: HashMap<Uuid, SeenTracker>,
    /// Also holds zones through missing/invalid calibrated samples.
    trackers: HashMap<u16, SeenTracker>,
    zones: HashMap<Uuid, ZoneState>,
}

#[derive(Clone)]
pub(in crate::runtime) struct PsnResource {
    inner: Arc<Mutex<PsnState>>,
}
struct PsnState {
    show_id: Option<ShowId>,
    configuration: Arc<CompiledConfiguration>,
    configuration_generation: u64,
    point_generation: u64,
    source_generation: u64,
    accepted_sequence: u64,
    sources: BTreeMap<SocketAddr, PsnTracking>,
    held: Held,
    error: Option<String>,
    point_locations: Arc<HashMap<Uuid, [f32; 3]>>,
    unowned_ignored_datagrams: u64,
    rejected_source_datagrams: u64,
}
impl Default for PsnResource {
    fn default() -> Self {
        Self::new()
    }
}
impl PsnResource {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PsnState {
                show_id: None,
                configuration: Arc::new(CompiledConfiguration::new(PsnConfiguration::default())),
                configuration_generation: 0,
                point_generation: 0,
                source_generation: 0,
                accepted_sequence: 0,
                sources: BTreeMap::new(),
                held: Held::default(),
                error: None,
                point_locations: Arc::new(HashMap::new()),
                unowned_ignored_datagrams: 0,
                rejected_source_datagrams: 0,
            })),
        }
    }

    pub(in crate::runtime) fn configuration(&self) -> PsnConfiguration {
        self.inner.lock().configuration.configuration.clone()
    }

    /// Same-show compatibility entry point. Activation/reload must use install_for_show even
    /// if the incoming configuration compares equal to the previous show's configuration.
    pub(in crate::runtime) fn install(&self, configuration: PsnConfiguration) {
        let mut state = self.inner.lock();
        let show_id = state.show_id;
        state.install(show_id, configuration, false);
    }

    pub(in crate::runtime) fn install_for_show(
        &self,
        show_id: Option<ShowId>,
        configuration: PsnConfiguration,
    ) {
        self.inner.lock().install(show_id, configuration, false);
    }

    /// Cold activation/reopen/overwrite, including the same show ID and equal configuration.
    /// This retires previous runtime samples, zones and Point cache; callers reinstall patch data.
    pub(in crate::runtime) fn reset_for_show(
        &self,
        show_id: Option<ShowId>,
        configuration: PsnConfiguration,
    ) {
        self.inner.lock().install(show_id, configuration, true);
    }

    /// Immediately retire ownership after a configuration edit, without sampling new input or
    /// advancing zones. The listener remains the only owner of zone/macro transitions.
    #[cfg(test)]
    pub(in crate::runtime) fn committed_overrides(&self) -> Vec<TrackedOverride> {
        let state = self.inner.lock();
        placements(
            &state.configuration.configuration,
            &state.bound_positions(),
            &state.point_locations,
        )
        .0
    }

    /// Pure input capture after an ownership edit. It retains accepted world positions and their
    /// original sample ages, without sampling packets or consuming listener-owned zone edges.
    pub(in crate::runtime) fn committed_tracking_frame(
        &self,
        now_millis: u64,
    ) -> PsnTrackingSnapshot {
        self.inner.lock().tracking_snapshot(now_millis)
    }

    /// Source epoch also identifies the socket generation, including equal-config show switches.
    pub(in crate::runtime) fn generation(&self) -> u64 {
        self.inner.lock().source_generation
    }

    pub(in crate::runtime) fn set_error(&self, error: Option<String>) {
        self.inner.lock().error = error;
    }

    pub(in crate::runtime) fn observe(&self, source: SocketAddr, datagram: &[u8], now_millis: u64) {
        self.observe_for_generation(self.generation(), source, datagram, now_millis);
    }

    /// Late data from a replaced socket cannot acquire ownership in the next show/source epoch.
    pub(in crate::runtime) fn observe_for_generation(
        &self,
        generation: u64,
        source: SocketAddr,
        datagram: &[u8],
        now_millis: u64,
    ) {
        let mut state = self.inner.lock();
        if generation != state.source_generation || !state.configuration.configuration.enabled {
            return;
        }
        if !state.sources.contains_key(&source) {
            // Validate before allocating a source. An arbitrary UDP sender must not fill this map.
            if datagram.len() > PSN_MAX_PACKET_BYTES || light_psn_wire::decode(datagram).is_err() {
                state.unowned_ignored_datagrams = state.unowned_ignored_datagrams.saturating_add(1);
                return;
            }
            if state.sources.len() >= MAX_PSN_SOURCES {
                state.unowned_ignored_datagrams = state.unowned_ignored_datagrams.saturating_add(1);
                state.rejected_source_datagrams = state.rejected_source_datagrams.saturating_add(1);
                return;
            }
            state.sources.insert(source, PsnTracking::new());
        }
        let observed = state
            .sources
            .get_mut(&source)
            .expect("validated source")
            .observe(datagram, now_millis);
        if matches!(observed, PsnObservation::Frame(_)) {
            state.accepted_sequence = state.accepted_sequence.wrapping_add(1);
            state.retire_replaced_sources(source, now_millis);
        }
    }

    /// Pure projection: never advances dwell timers or consumes a listener-owned zone edge.
    pub(in crate::runtime) fn status(&self, now_millis: u64) -> PsnStatus {
        let state = self.inner.lock();
        let mut status = state.initial_status();
        let (_, reports) = state.report_trackers(now_millis, &mut status);
        status.trackers = reports;
        if state.configuration.configuration.enabled {
            status.occupied_zones = occupied_zone_ids(&state.held.zones);
            let bound = state.bound_positions();
            status.placements = placement_reports(
                &state.configuration.configuration,
                &bound,
                &state.point_locations,
            );
        }
        status
    }

    pub(in crate::runtime) fn tick(&self, now_millis: u64) -> PsnTick {
        let mut state = self.inner.lock();
        // A cheap Arc clone keeps field borrows independent; the full configuration is compiled
        // once per edit, never cloned or recompiled for a packet/tick/status consumer.
        let compiled = Arc::clone(&state.configuration);
        let configuration = &compiled.configuration;
        let mut status = state.initial_status();
        let (positions, reports) = state.report_trackers(now_millis, &mut status);
        status.trackers = reports;
        if !configuration.enabled {
            state.held.positions.clear();
            return PsnTick {
                #[cfg(test)]
                overrides: Vec::new(),
                zone_transitions: Vec::new(),
                tracking: state.tracking_snapshot(now_millis),
                status,
            };
        }
        for (tracker_id, seen) in positions {
            state
                .held
                .trackers
                .entry(tracker_id)
                .and_modify(|current| {
                    // A newer raw position may overflow calibration. Another sender's older
                    // finite position must not displace the last valid committed hold.
                    if seen.supersedes(*current) {
                        *current = seen;
                    }
                })
                .or_insert(seen);
        }
        for binding in &compiled.bindings {
            if let Some(position) = state.held.trackers.get(&binding.tracker_id).copied() {
                state.held.positions.insert(binding.id, position);
            }
        }
        state
            .held
            .positions
            .retain(|id, _| compiled.binding_identities.contains_key(id));
        let zone_positions = state
            .held
            .trackers
            .iter()
            .map(|(id, seen)| (*id, seen.position))
            .collect();
        // After an explicit source change we await its first valid position. Missing input is
        // not an instruction to make all occupied zones empty.
        let zone_transitions = if state.held.trackers.is_empty() {
            Vec::new()
        } else {
            advance(
                configuration,
                &zone_positions,
                &mut state.held.zones,
                now_millis,
            )
        };
        status.occupied_zones = occupied_zone_ids(&state.held.zones);
        let bound = state.bound_positions();
        let placed = placement_reports(configuration, &bound, &state.point_locations);
        #[cfg(test)]
        let overrides = placements(configuration, &bound, &state.point_locations).0;
        let tracking = state.tracking_snapshot(now_millis);
        status.placements = placed;
        PsnTick {
            #[cfg(test)]
            overrides,
            zone_transitions,
            status,
            tracking,
        }
    }

    pub(in crate::runtime) fn install_point_locations(
        &self,
        mut locations: HashMap<Uuid, [f32; 3]>,
    ) {
        locations.retain(|_, position| position.iter().all(|axis| axis.is_finite()));
        let mut state = self.inner.lock();
        if *state.point_locations != locations {
            state.point_locations = Arc::new(locations);
            state.point_generation = state.point_generation.wrapping_add(1);
        }
    }
}

impl PsnState {
    /// A sender restarted on the same host binds a new source port. Once the old socket has gone
    /// stale it is retired, so its silence no longer reads as the desk's PSN health; a second
    /// sender that is still receiving, or that names another PSN system, is kept.
    fn retire_replaced_sources(&mut self, current: SocketAddr, now_millis: u64) {
        let stale_after = self.configuration.configuration.stale_after_millis;
        let Some(name) = self
            .sources
            .get(&current)
            .map(|tracking| tracking.system_name().map(str::to_owned))
        else {
            return;
        };
        self.sources.retain(|source, tracking| {
            *source == current
                || source.ip() != current.ip()
                || matches!(
                    tracking.health(now_millis, stale_after),
                    PsnSourceHealth::Receiving
                )
                || matches!((&name, tracking.system_name()), (Some(new), Some(old)) if new != old)
        });
    }

    fn install(
        &mut self,
        show_id: Option<ShowId>,
        configuration: PsnConfiguration,
        force_reset: bool,
    ) {
        let current = &self.configuration.configuration;
        let show_changed = force_reset || self.show_id != show_id;
        if !show_changed && *current == configuration {
            return;
        }
        let moved = show_changed
            || current.group != configuration.group
            || current.port != configuration.port
            || current.interface != configuration.interface
            || current.enabled != configuration.enabled;
        let calibration_changed = current.calibration != configuration.calibration;
        let compiled = Arc::new(CompiledConfiguration::new(configuration));
        if moved {
            self.sources.clear();
            self.held.positions.clear();
            self.held.trackers.clear();
            self.error = None;
            self.source_generation = self.source_generation.wrapping_add(1);
            self.accepted_sequence = 0;
            self.unowned_ignored_datagrams = 0;
            self.rejected_source_datagrams = 0;
        } else {
            // Reusing a binding UUID for another tracker/Point must never reuse its old hold.
            self.held.positions.retain(|id, _| {
                compiled
                    .binding_identities
                    .get(id)
                    .is_some_and(|next| self.configuration.binding_identities.get(id) == Some(next))
            });
            if calibration_changed {
                // Reproject exactly the samples already committed to each hold. A newer packet
                // may be waiting in sources, but accepting it and advancing zones belongs to tick.
                for seen in self
                    .held
                    .trackers
                    .values_mut()
                    .chain(self.held.positions.values_mut())
                {
                    let position = compiled
                        .configuration
                        .calibration
                        .place_in_show(seen.raw_position);
                    seen.calibration_invalid = !position.iter().all(|axis| axis.is_finite());
                    if !seen.calibration_invalid {
                        seen.position = position;
                    }
                }
            }
        }
        if show_changed {
            self.held.zones.clear();
            self.point_locations = Arc::new(HashMap::new());
            self.point_generation = self.point_generation.wrapping_add(1);
        } else {
            self.held.zones.retain(|id, _| {
                compiled
                    .configuration
                    .zones
                    .iter()
                    .any(|zone| zone.id == *id)
            });
        }
        self.show_id = show_id;
        self.configuration = compiled;
        self.configuration_generation = self.configuration_generation.wrapping_add(1);
    }

    fn initial_status(&self) -> PsnStatus {
        PsnStatus {
            enabled: self.configuration.configuration.enabled,
            listening_on: self.configuration.listening_on.clone(),
            error: self.error.clone(),
            ignored_datagrams: self.unowned_ignored_datagrams,
            rejected_source_datagrams: self.rejected_source_datagrams,
            source_capacity: MAX_PSN_SOURCES,
            conflicting_binding_rows: self.configuration.conflicting_binding_rows,
            invalid_calibrated_positions: self
                .held
                .trackers
                .values()
                .filter(|seen| seen.calibration_invalid)
                .count(),
            ..Default::default()
        }
    }

    fn bound_positions(&self) -> HashMap<Uuid, [f32; 3]> {
        self.held
            .positions
            .iter()
            .map(|(id, seen)| (*id, seen.position))
            .collect()
    }

    fn tracking_snapshot(&self, now_millis: u64) -> PsnTrackingSnapshot {
        let bindings = self
            .configuration
            .bindings
            .iter()
            .filter_map(|binding| {
                self.held
                    .positions
                    .get(&binding.id)
                    .map(|seen| BoundTrackingSample {
                        binding_id: binding.id,
                        point_fixture_id: binding.point_fixture_id,
                        // The engine applies reach limits against its own captured patch generation.
                        // Status placements above remain the receiver's clamped diagnostic projection.
                        position_metres: seen.position,
                        identity: seen.identity,
                        position_received_at_millis: seen.received_at_millis,
                    })
            })
            .collect::<Vec<_>>();
        PsnTrackingSnapshot {
            show_id: self.show_id,
            configuration_generation: self.configuration_generation,
            point_generation: self.point_generation,
            source_generation: self.source_generation,
            accepted_sequence: self.accepted_sequence,
            sampled_at_millis: now_millis,
            bindings: Arc::from(bindings),
        }
    }

    fn report_trackers(
        &self,
        now_millis: u64,
        status: &mut PsnStatus,
    ) -> (HashMap<u16, SeenTracker>, Vec<TrackerReport>) {
        let configuration = &self.configuration.configuration;
        let mut latest: HashMap<u16, SeenTracker> = HashMap::new();
        let mut reports = Vec::new();
        let mut health: Option<PsnHealth> = None;
        for (source, tracking) in &self.sources {
            status.frames = status.frames.saturating_add(tracking.frames());
            status.ignored_datagrams = status.ignored_datagrams.saturating_add(tracking.ignored());
            if let Some(name) = tracking.system_name() {
                status.system_names.push(name.to_owned());
            }
            let identity = |sample| TrackingSampleIdentity {
                source_generation: self.source_generation,
                source: *source,
                sample,
            };
            status.sources.push(SourceReport {
                source: *source,
                accepted_sample: tracking.accepted_sample().map(identity),
                diagnostics: tracking.diagnostics(),
            });
            health = Some(worse(
                health,
                tracking
                    .health(now_millis, configuration.stale_after_millis)
                    .into(),
            ));
            for tracked in tracking.trackers() {
                let age_millis = tracked.age_millis(now_millis);
                let raw = tracked
                    .position()
                    .map(|position| [position.x, position.y, position.z]);
                let position = raw
                    .map(|position| configuration.calibration.place_in_show(position))
                    .filter(|position| position.iter().all(|axis| axis.is_finite()));
                if raw.is_some() && position.is_none() {
                    // initial_status already counted an invalid held calibration for this source
                    // and tracker; status remains passive even before the next listener tick.
                    if !self.held.trackers.get(&tracked.id).is_some_and(|seen| {
                        seen.calibration_invalid && seen.identity.source == *source
                    }) {
                        status.invalid_calibrated_positions += 1;
                    }
                }
                let accepted_sample = tracked.position_sample.map(identity);
                reports.push(TrackerReport {
                    tracker_id: tracked.id,
                    name: tracked.name.clone(),
                    position_metres: position,
                    age_millis,
                    stale: age_millis > configuration.stale_after_millis,
                    source: *source,
                    accepted_sample,
                });
                if let (Some(raw_position), Some(position), Some(identity)) =
                    (raw, position, accepted_sample)
                {
                    let seen = SeenTracker {
                        raw_position,
                        position,
                        calibration_invalid: false,
                        received_at_millis: tracked.updated_at_millis,
                        identity,
                    };
                    latest
                        .entry(tracked.id)
                        .and_modify(|current| {
                            // Compare arrival, not saturated age: future/equal caller clocks cannot
                            // collapse distinct arrivals. SocketAddr gives deterministic equal-time ties.
                            if seen.supersedes(*current) {
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

fn occupied_zone_ids(zones: &HashMap<Uuid, ZoneState>) -> Vec<Uuid> {
    let mut occupied: Vec<_> = zones
        .iter()
        .filter_map(|(id, state)| state.occupied.then_some(*id))
        .collect();
    occupied.sort();
    occupied
}
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
