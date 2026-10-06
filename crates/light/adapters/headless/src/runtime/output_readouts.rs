//! TL-594 C2/C3: typed readouts of one accepted source and the displayed-source binding of the
//! first Angle edit.
//!
//! - Live readouts come from one retained [`PublishedVisualizationFrame`]: identity, requested
//!   intent and every commanded copy/emitter pair are read from that same frame.
//! - Preload readouts come only from the session Programmer's accepted Pending publication
//!   (the injected TL-548 source). Live is never a Preload fallback, and no fresh evaluation is
//!   ever labelled as a readout.
//! - Every delivered readout is leased per session ([`DisplayedSourceLeases`]). An edit that
//!   names the lease adopts exactly that source or holds quietly; it never substitutes latest.
//! - Duplicate and hidden clients cannot multiply readout work: Live captures are cached per
//!   (frame, owners) and shared by every session that reads the same accepted frame.

use super::displayed_source_leases::{DisplayedSourceLeases, LeasedSource};
use super::pending_preload_readers::PublishedPreload;
use super::position_readout::{CapturedPositionReadouts, capture_position_readouts};
use super::visualization_frame::PublishedVisualizationFrame;
use light_application::{ProgrammingDisplayedLane, ProgrammingDisplayedSource};
use light_core::{FixtureId, SessionId};
use light_engine::{Engine, EngineSnapshot};
use light_wire::v2::{
    output_control::OutputFrameIdentity,
    output_readouts as wire,
    visualization::{VisualizationLane, VisualizationScope},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

/// Live readout captures kept for sharing between clients of the same accepted frame.
const LIVE_READOUT_CACHE: usize = 4;

/// One leased source exactly as it was delivered.
#[derive(Clone)]
pub(in crate::runtime) enum DisplayedSource {
    /// An accepted Live output frame (the hub publication).
    Live(Arc<PublishedVisualizationFrame>),
    /// An accepted Pending (Preload) capture of the session Programmer's episode, with the
    /// engine snapshot it was captured against.
    Pending {
        readouts: Arc<CapturedPositionReadouts>,
        snapshot: Arc<EngineSnapshot>,
    },
    /// A gated Preload stream delivery: the accepted Pending publication's identity. An edit
    /// adopts it only while the session's Pending source still serves exactly this frame of
    /// this episode; otherwise it holds (the source retains only its newest accepted pair).
    PendingFrame {
        frame: OutputFrameIdentity,
        episode: uuid::Uuid,
        snapshot: Arc<EngineSnapshot>,
    },
}

impl LeasedSource for DisplayedSource {
    /// Live: the same retained hub frame. Pending stream: the same publication of the same
    /// episode against the same engine snapshot. Pending capture: the same accepted frame and
    /// snapshot; owners delivered by either read are merged, so one lease covers every owner
    /// this session was shown from that frame.
    fn absorb(&mut self, delivered: &Self) -> bool {
        match (self, delivered) {
            (Self::Live(retained), Self::Live(delivered)) => Arc::ptr_eq(retained, delivered),
            (
                Self::PendingFrame {
                    frame,
                    episode,
                    snapshot,
                },
                Self::PendingFrame {
                    frame: delivered_frame,
                    episode: delivered_episode,
                    snapshot: delivered_snapshot,
                },
            ) => {
                frame == delivered_frame
                    && episode == delivered_episode
                    && Arc::ptr_eq(snapshot, delivered_snapshot)
            }
            (
                Self::Pending { readouts, snapshot },
                Self::Pending {
                    readouts: delivered,
                    snapshot: delivered_snapshot,
                },
            ) => {
                if !Arc::ptr_eq(snapshot, delivered_snapshot)
                    || readouts.identity != delivered.identity
                    || readouts.scope != delivered.scope
                {
                    return false;
                }
                let added = delivered
                    .owners
                    .iter()
                    .filter(|owner| !readouts.owners.iter().any(|kept| kept.owner == owner.owner))
                    .cloned()
                    .collect::<Vec<_>>();
                if !added.is_empty() {
                    let mut merged = CapturedPositionReadouts::clone(readouts);
                    merged.owners.extend(added);
                    *readouts = Arc::new(merged);
                }
                true
            }
            _ => false,
        }
    }
}

type LiveReadoutEntry = (u64, Vec<FixtureId>, Option<Arc<CapturedPositionReadouts>>);

#[derive(Default)]
struct LiveReadoutCache {
    entries: parking_lot::Mutex<VecDeque<LiveReadoutEntry>>,
    computed: AtomicU64,
}

impl LiveReadoutCache {
    /// Computed once per (frame, owners) under the cache lock, so concurrent duplicate readers
    /// wait for and share one capture instead of repeating it.
    fn capture(
        &self,
        engine: &Engine,
        frame: &PublishedVisualizationFrame,
        owners: &[FixtureId],
    ) -> Option<Arc<CapturedPositionReadouts>> {
        let mut entries = self.entries.lock();
        if let Some((_, _, captured)) = entries
            .iter()
            .find(|(sequence, cached, _)| *sequence == frame.sequence && cached == owners)
        {
            return captured.clone();
        }
        self.computed.fetch_add(1, Ordering::Relaxed);
        let captured = capture_position_readouts(engine, frame, owners).map(Arc::new);
        if entries.len() >= LIVE_READOUT_CACHE {
            entries.pop_front();
        }
        entries.push_back((frame.sequence, owners.to_vec(), captured.clone()));
        captured
    }
}

/// Desk-wide registry: per-session leases plus the shared Live readout cache.
#[derive(Default)]
pub(in crate::runtime) struct DisplayedSources {
    leases: DisplayedSourceLeases<DisplayedSource>,
    live: LiveReadoutCache,
    /// The Pending publication each session's gated Preload projection was built from, per
    /// hub source: the stamp of the lane message is exactly what its content shows.
    preload_projections: parking_lot::Mutex<HashMap<SessionId, (u64, PublishedPreload)>>,
}

impl DisplayedSources {
    pub(in crate::runtime) fn leases(&self) -> &DisplayedSourceLeases<DisplayedSource> {
        &self.leases
    }

    pub(in crate::runtime) fn close_session(&self, session: SessionId) {
        self.leases.close_session(session);
        self.preload_projections.lock().remove(&session);
    }

    pub(in crate::runtime) fn record_preload_projection(
        &self,
        session: SessionId,
        hub_sequence: u64,
        published: PublishedPreload,
    ) {
        self.preload_projections
            .lock()
            .insert(session, (hub_sequence, published));
    }

    pub(in crate::runtime) fn preload_projection(
        &self,
        session: SessionId,
        hub_sequence: u64,
    ) -> Option<PublishedPreload> {
        self.preload_projections
            .lock()
            .get(&session)
            .filter(|(sequence, _)| *sequence == hub_sequence)
            .map(|(_, published)| published.clone())
    }

    /// Live captures actually computed (not served from the shared cache). Diagnostics/tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::runtime) fn live_captures_computed(&self) -> u64 {
        self.live.computed.load(Ordering::Relaxed)
    }
}

/// Read, lease and project the readouts of `owners` from `lane`'s accepted source.
pub(in crate::runtime) fn read_readouts(
    state: &super::AppState,
    session: SessionId,
    lane: VisualizationLane,
    owners: &[FixtureId],
) -> wire::OutputReadoutSnapshot {
    let active = state.active_show.current().map(|show| show.id.0);
    let sources = state.sessions.displayed_sources();
    let engine = state.output.engine();
    let (captured, source) = match lane {
        VisualizationLane::Normal => {
            let Some(frame) = state.output.latest_visualization_frame() else {
                return unavailable(
                    lane,
                    active,
                    wire::OutputReadoutUnavailable::NoAcceptedFrame,
                );
            };
            if frame.scope.show_id != active {
                return unavailable(lane, active, wire::OutputReadoutUnavailable::ShowChanged);
            }
            let Some(captured) = sources.live.capture(engine, &frame, owners) else {
                return unavailable(
                    lane,
                    active,
                    wire::OutputReadoutUnavailable::StaleGeneration,
                );
            };
            (captured, DisplayedSource::Live(frame))
        }
        VisualizationLane::Preload => {
            let Some(captured) = pending_readouts(state, session, owners) else {
                return unavailable(
                    lane,
                    active,
                    wire::OutputReadoutUnavailable::NoAcceptedPreload,
                );
            };
            if captured.scope.show_id != active {
                return unavailable(lane, active, wire::OutputReadoutUnavailable::ShowChanged);
            }
            let source = DisplayedSource::Pending {
                readouts: Arc::clone(&captured),
                snapshot: engine.snapshot(),
            };
            (captured, source)
        }
    };
    let lease = sources.leases.issue(session, lane, source, Instant::now());
    readout_snapshot(lane, &captured, Some(lease))
}

/// The session Programmer's accepted Pending capture. None without an installed source
/// (production until TL-548 gates it on) or without an accepted pair: never Live.
fn pending_readouts(
    state: &super::AppState,
    session: SessionId,
    owners: &[FixtureId],
) -> Option<Arc<CapturedPositionReadouts>> {
    let source = state.output.pending_position_readouts().source()?;
    let programmer = state.programming.get(session)?.id;
    source.capture(programmer, owners).map(Arc::new)
}

/// TL-594 C4 helper: the shared Live capture of `frame` for `owners`.
pub(in crate::runtime) fn live_readouts(
    state: &super::AppState,
    frame: &PublishedVisualizationFrame,
    owners: &[FixtureId],
) -> Option<Arc<CapturedPositionReadouts>> {
    state
        .sessions
        .displayed_sources()
        .live
        .capture(state.output.engine(), frame, owners)
}

/// Resolve the displayed source an edit named into readouts of exactly `members`, pinning it
/// for `gesture`. `None` means hold quietly: unknown/expired/foreign lease, a lease of the
/// other lane (Preload never uses Live and Live never uses Preload), a source the engine no
/// longer runs, or a member the leased source did not deliver. Latest is never consulted.
pub(in crate::runtime) fn displayed_position_readouts(
    state: &super::AppState,
    session: SessionId,
    preload: bool,
    displayed: ProgrammingDisplayedSource,
    gesture: Option<&str>,
    members: &[FixtureId],
) -> Option<CapturedPositionReadouts> {
    let lane = match displayed.lane {
        ProgrammingDisplayedLane::Normal => VisualizationLane::Normal,
        ProgrammingDisplayedLane::Preload => VisualizationLane::Preload,
    };
    if preload != (lane == VisualizationLane::Preload) {
        return None;
    }
    let leases = state.sessions.displayed_sources().leases();
    let now = Instant::now();
    let source = match gesture {
        Some(gesture) => leases.pin(session, lane, displayed.lease, gesture, now),
        None => leases.resolve(session, lane, displayed.lease, now),
    }?;
    let engine = state.output.engine();
    match (lane, source) {
        (VisualizationLane::Normal, DisplayedSource::Live(frame)) => {
            capture_position_readouts(engine, &frame, members)
        }
        (VisualizationLane::Preload, DisplayedSource::Pending { readouts, snapshot }) => {
            if !Arc::ptr_eq(&engine.snapshot(), &snapshot) {
                return None;
            }
            select_members(&readouts, members)
        }
        (
            VisualizationLane::Preload,
            DisplayedSource::PendingFrame {
                frame,
                episode,
                snapshot,
            },
        ) => pending_frame_readouts(state, session, &frame, episode, &snapshot, members),
        _ => None,
    }
}

/// Readouts of a leased Pending stream frame: only while the desk still publishes this
/// episode and the session's Pending source serves exactly this frame. Never Live, never newer.
fn pending_frame_readouts(
    state: &super::AppState,
    session: SessionId,
    frame: &OutputFrameIdentity,
    episode: uuid::Uuid,
    snapshot: &Arc<EngineSnapshot>,
    members: &[FixtureId],
) -> Option<CapturedPositionReadouts> {
    if !Arc::ptr_eq(&state.output.engine().snapshot(), snapshot) {
        return None;
    }
    match super::pending_preload_readers::published_preload(state)? {
        PublishedPreload::Published(readout) if readout.identity().episode == episode => {}
        _ => return None,
    }
    let captured = pending_readouts(state, session, members)?;
    (captured.identity == *frame).then(|| select_members(&captured, members))?
}

/// The delivered owners in member order. A member the source never delivered holds the edit.
fn select_members(
    readouts: &CapturedPositionReadouts,
    members: &[FixtureId],
) -> Option<CapturedPositionReadouts> {
    let owners = members
        .iter()
        .map(|member| {
            readouts
                .owners
                .iter()
                .find(|owner| owner.owner == *member)
                .cloned()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CapturedPositionReadouts {
        identity: readouts.identity.clone(),
        scope: readouts.scope,
        show_revision: readouts.show_revision,
        owners,
    })
}

pub(in crate::runtime) fn readout_snapshot(
    lane: VisualizationLane,
    captured: &CapturedPositionReadouts,
    lease: Option<u64>,
) -> wire::OutputReadoutSnapshot {
    wire::OutputReadoutSnapshot {
        lane,
        scope: captured.scope,
        frame: Some(captured.identity.clone()),
        lease,
        revision: captured.show_revision,
        unavailable: None,
        owners: captured
            .owners
            .iter()
            .map(|owner| wire::OutputOwnerReadout {
                fixture_id: owner.owner.0,
                requested: owner
                    .requested
                    .as_ref()
                    .map(super::command_http::preload_attribute_value_wire),
                color: owner
                    .color
                    .as_ref()
                    .map(super::command_http::preload_attribute_value_wire),
                position: wire::OutputPositionReadout {
                    available: owner.readout.commands.is_some(),
                    commands: owner
                        .readout
                        .commands
                        .iter()
                        .flatten()
                        .map(|command| wire::OutputPositionCommandReadout {
                            destination: command.destination.0,
                            emitter_id: command.emitter_id,
                            pan_degrees: command.pan_degrees,
                            tilt_degrees: command.tilt_degrees,
                        })
                        .collect(),
                    common: owner
                        .common_angles()
                        .map(|angles| wire::OutputCommonAngles {
                            pan_degrees: angles.pan_degrees,
                            tilt_degrees: angles.tilt_degrees,
                        }),
                },
            })
            .collect(),
    }
}

fn unavailable(
    lane: VisualizationLane,
    show_id: Option<uuid::Uuid>,
    reason: wire::OutputReadoutUnavailable,
) -> wire::OutputReadoutSnapshot {
    wire::OutputReadoutSnapshot {
        lane,
        scope: VisualizationScope { show_id },
        frame: None,
        lease: None,
        revision: 0,
        unavailable: Some(reason),
        owners: Vec::new(),
    }
}

#[cfg(test)]
#[path = "output_readouts_tests.rs"]
mod tests;
