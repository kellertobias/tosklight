//! TL-594 C4: lease stamps and the readout claim on the visualization WebSocket.
//!
//! - The Normal lane message of each publication carries a fresh session lease of exactly the
//!   accepted Live source it was projected from; a readout claim is answered from that same
//!   source with the same lease, so encoders show and then name one coherent captured token.
//! - While the family-adapter gate is on, the Preload lane is built from ONE read of the
//!   accepted Pending publication. Its message carries that publication's episode and frame
//!   (the gate ticket as `source_frame`), and its lease binds exactly that identity. With
//!   nothing published it carries the passive status and no lease. A Live stamp is never
//!   borrowed. With the gate off the Preload lane is the legacy projection, unstamped.
//!
//! Slow clients keep only the newest pending batch (the existing latest-batch queue), and the
//! readout capture is shared through the desk-wide Live cache, so neither slow nor duplicate
//! clients multiply readout work.

use super::output_readouts::{DisplayedSource, live_readouts, readout_snapshot};
use super::pending_preload_readers::{PublishedPreload, visualization_content};
use super::visualization_frame::PublishedVisualizationFrame;
use super::{AppState, Session};
use light_core::FixtureId;
use light_wire::v2::{
    output_readouts::{
        OutputReadoutUnavailable, VisualizationPendingStamp, VisualizationReadoutClaim,
    },
    visualization::{VisualizationLane, VisualizationServerMessage},
};
use std::{sync::Arc, time::Instant};

/// What one lane message is stamped with.
#[derive(Default)]
pub(super) struct LaneStamp {
    pub(super) lease: Option<u64>,
    pub(super) pending: Option<VisualizationPendingStamp>,
    /// Replaces the hub projection sequence for a published Pending lane: the gate ticket.
    pub(super) source_frame: Option<u64>,
}

/// Gated Preload content from one read of the publication; the read is recorded for this
/// session and hub source so the message is stamped with exactly what the content shows.
pub(super) fn gated_preload_content(
    state: &AppState,
    session: &Session,
    source: &PublishedVisualizationFrame,
    published: PublishedPreload,
) -> serde_json::Value {
    let content = visualization_content(state, &published);
    state
        .sessions
        .displayed_sources()
        .record_preload_projection(session.id, source.sequence, published);
    content
}

/// Owners per WebSocket readout claim; the claim is truncated beyond it.
const MAX_CLAIMED_OWNERS: usize = 512;

pub(super) fn claimed_owners(claim: Option<VisualizationReadoutClaim>) -> Option<Vec<FixtureId>> {
    claim.map(|claim| {
        claim
            .fixture_ids
            .into_iter()
            .take(MAX_CLAIMED_OWNERS)
            .map(FixtureId)
            .collect()
    })
}

/// Stamp one lane message of `source`. Normal: a Live lease. Gated Preload: the recorded
/// publication's identity and a lease bound to it. Legacy Preload: nothing.
pub(super) fn lane_stamp(
    state: &AppState,
    session: &Session,
    lane: VisualizationLane,
    source: &Arc<PublishedVisualizationFrame>,
) -> LaneStamp {
    let sources = state.sessions.displayed_sources();
    if lane == VisualizationLane::Normal {
        let lease = sources.leases().issue(
            session.id,
            lane,
            DisplayedSource::Live(Arc::clone(source)),
            Instant::now(),
        );
        return LaneStamp {
            lease: Some(lease),
            ..LaneStamp::default()
        };
    }
    let Some(published) = sources.preload_projection(session.id, source.sequence) else {
        return LaneStamp::default();
    };
    let pending = VisualizationPendingStamp {
        status: published.status(),
        frame: published.frame().cloned(),
    };
    let PublishedPreload::Published(readout) = published else {
        return LaneStamp {
            pending: Some(pending),
            ..LaneStamp::default()
        };
    };
    let Some(frame) = readout.lane().frame.clone() else {
        return LaneStamp::default();
    };
    let lease = sources.leases().issue(
        session.id,
        lane,
        DisplayedSource::PendingFrame {
            frame: frame.clone(),
            episode: readout.identity().episode,
            snapshot: state.output.engine().snapshot(),
        },
        Instant::now(),
    );
    LaneStamp {
        lease: Some(lease),
        source_frame: Some(frame.sequence),
        pending: Some(pending),
    }
}

/// Readouts of the claimed owners from the same source and lease as the Normal lane message.
/// The message takes the next outgoing sequence, so the stream stays strictly consecutive.
pub(super) fn readouts_message(
    state: &AppState,
    claim: Option<&[FixtureId]>,
    source: &PublishedVisualizationFrame,
    lease: u64,
    outgoing_sequence: &mut u64,
) -> Option<VisualizationServerMessage> {
    let owners = claim?;
    *outgoing_sequence += 1;
    let sequence = *outgoing_sequence;
    let readouts = match live_readouts(state, source, owners) {
        Some(captured) => readout_snapshot(VisualizationLane::Normal, &captured, Some(lease)),
        None => light_wire::v2::output_readouts::OutputReadoutSnapshot {
            lane: VisualizationLane::Normal,
            scope: source.scope,
            frame: Some(source.identity()),
            lease: None,
            revision: source.show_revision,
            unavailable: Some(OutputReadoutUnavailable::StaleGeneration),
            owners: Vec::new(),
        },
    };
    Some(VisualizationServerMessage::Readouts {
        sequence,
        source_frame: source.sequence,
        readouts,
    })
}

/// A scope/show change makes every retained Live delivery of this session meaningless.
pub(super) fn evict_on_invalidation(state: &AppState, session: &Session, lane: VisualizationLane) {
    state
        .sessions
        .displayed_sources()
        .leases()
        .evict_lane(session.id, lane);
}
