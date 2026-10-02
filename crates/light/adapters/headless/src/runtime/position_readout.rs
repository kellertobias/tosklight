//! Derived Position feedback and first-edit seed from one retained accepted output source.
//! No fitting, Point sampling, calibration, mutation or output acceptance occurs here.
//! Projection/gesture consumers must supply their own lane's accepted source and retain this
//! minimal capture under their control-context lifetime; a sequence is not a history lookup.
//!
//! Pending (Preload) readouts come only from an accepted Pending pair of the caller's own
//! episode, bound through the same [`PendingPairBinding`] as the Pending native readout. Their
//! requested intent is the After branch's; Before and the Live publication are never read.
use super::dynamic_snapshot_publication::RetainedInputCapture;
use super::output_scheduler::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingHybridResult, PendingPairBinding,
    PendingReadoutError,
};
use super::preload::retained_history::paired::PendingPairResult;
use super::visualization_frame::PublishedVisualizationFrame;
use light_core::programming::{JointAngles, ProgrammingOwner, ScalarIntent, ZoomIntent};
use light_core::{AttributeValue, FixtureId, ProgrammerId};
use light_engine::{Engine, FrameValues, PositionCommandedOwnerReadout};
use light_fixture::forward::ZoomForwardValue;
use light_wire::v2::{output_control::OutputFrameIdentity, visualization::VisualizationScope};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct CapturedPositionOwner {
    pub(crate) owner: FixtureId,
    /// Requested intent and achieved commands come from the same accepted frame. A native
    /// source can have no typed Position request; its complete common commands are a seed.
    pub(crate) requested: Option<AttributeValue>,
    /// Distinct copies/emitter joints stay distinct. Unavailable is passive, not root fallback.
    pub(crate) readout: PositionCommandedOwnerReadout,
    /// TL-637 follow-up: the first-edit Zoom seed of the SAME accepted frame. Its typed Zoom
    /// request when it has one, else the opening its native output measures through the compiled
    /// optics model in the profile's convention. None when neither is known: never a percentage
    /// reinterpreted as degrees.
    pub(crate) zoom: Option<AttributeValue>,
}
impl CapturedPositionOwner {
    pub(crate) fn common_angles(&self) -> Option<JointAngles> {
        self.readout.common_angles()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CapturedPositionReadouts {
    pub(crate) identity: OutputFrameIdentity,
    pub(crate) scope: VisualizationScope,
    pub(crate) show_revision: u64,
    pub(crate) owners: Vec<CapturedPositionOwner>,
}

/// Capture only requested owners, preserving their order and unavailable records. One bulk
/// physical lookup avoids scanning every fixture for each visible owner. The engine validates
/// the current generation/layout and every owned copy/emitter. These guards do not establish
/// lane acceptance or authorize a gesture; the retained publication and caller do that.
pub(crate) fn capture_position_readouts(
    engine: &Engine,
    source: &PublishedVisualizationFrame,
    owners: &[FixtureId],
) -> Option<CapturedPositionReadouts> {
    if !Arc::ptr_eq(&engine.snapshot(), &source.source_snapshot) {
        return None;
    }
    let readouts = engine.position_commanded_readouts_from_physical(
        source.generation,
        &source.physical,
        owners,
    )?;
    let zooms = engine.zoom_readouts_from_physical(source.generation, &source.physical, owners)?;
    Some(CapturedPositionReadouts {
        identity: source.identity(),
        scope: source.scope,
        show_revision: source.show_revision,
        owners: requested_owners(readouts, zooms, &source.values),
    })
}

/// Join each readout to the requested intent of the SAME accepted frame's values.
fn requested_owners(
    readouts: Vec<PositionCommandedOwnerReadout>,
    zooms: Vec<Option<ZoomForwardValue>>,
    values: &FrameValues,
) -> Vec<CapturedPositionOwner> {
    let attribute = ProgrammingOwner::Position.key();
    readouts
        .into_iter()
        .zip(zooms)
        .map(|(readout, measured)| CapturedPositionOwner {
            owner: readout.owner,
            requested: values.value(readout.owner, &attribute).cloned(),
            zoom: zoom_seed(
                values.value(readout.owner, &ProgrammingOwner::Zoom.key()),
                measured,
            ),
            readout,
        })
        .collect()
}

/// A typed Zoom request of the frame wins; otherwise the measured opening, only in a known
/// convention and inside the open Zoom domain (exactly the Zoom adapter's adoption rule).
fn zoom_seed(
    requested: Option<&AttributeValue>,
    measured: Option<ZoomForwardValue>,
) -> Option<AttributeValue> {
    if let Some(requested @ AttributeValue::Zoom(_)) = requested {
        return Some(requested.clone());
    }
    let measured = measured?;
    let convention = measured.convention?;
    (measured.degrees > 0. && measured.degrees < 180.).then(|| {
        AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(measured.degrees as f32),
            convention,
        }))
    })
}

/// TL-552: the Fixture Sheet's commanded Pan/Tilt, the same achieved pose the Pan/Tilt encoders
/// read, for the requested owners of the latest accepted Live frame of the running generation and
/// active show. Only owners with one common commanded pair are listed; divergent copies and
/// owners without a pose are left out, so the sheet never shows a guessed or averaged angle. No
/// lease is issued: this is a display projection, never a gesture's displayed source.
pub(crate) fn fixture_sheet_commanded_positions(
    state: &super::AppState,
    owners: &std::collections::HashSet<uuid::Uuid>,
) -> Vec<serde_json::Value> {
    let Some(frame) = state.output.latest_visualization_frame() else {
        return Vec::new();
    };
    if frame.scope.show_id != state.active_show.current().map(|show| show.id.0) {
        return Vec::new();
    }
    let owners: Vec<FixtureId> = owners.iter().map(|id| FixtureId(*id)).collect();
    let Some(captured) = capture_position_readouts(state.output.engine(), &frame, &owners) else {
        return Vec::new();
    };
    captured
        .owners
        .iter()
        .filter_map(|owner| {
            let pose = owner.common_angles()?;
            Some(serde_json::json!({
                "fixture_id": owner.owner.0,
                "pan_degrees": pose.pan_degrees,
                "tilt_degrees": pose.tilt_degrees,
            }))
        })
        .collect()
}

/// One owner's first-edit seed while no accepted Live frame of the running generation exists.
#[derive(Clone, Debug)]
pub(crate) struct UnpublishedPositionSeed {
    pub(crate) owner: FixtureId,
    /// The Position the next frame would resolve for the owner, when anything programs one.
    pub(crate) requested: Option<AttributeValue>,
    /// Otherwise the declared default pose the idle fixture outputs. None stays unknown.
    pub(crate) declared: Option<JointAngles>,
}

/// TL-552: the Live first-edit seed before the output scheduler has published a frame of the
/// running generation (straight after show open or a patch change). The accepted-frame capture
/// cannot answer then, so a first Position edit would silently be `no_change`. Instead each
/// owner adopts what that next frame resolves: a programmed Position from the observational
/// projection of the current sources, else the fixture's declared default pose. Empty while an
/// accepted frame of the current generation exists: that frame alone decides, so an owner it
/// reports without a pose stays passive (TL-637).
pub(crate) fn unpublished_position_seeds(
    state: &super::AppState,
    owners: &[FixtureId],
) -> Vec<UnpublishedPositionSeed> {
    let engine = state.output.engine();
    let snapshot = engine.snapshot();
    if state
        .output
        .latest_visualization_frame()
        .is_some_and(|frame| Arc::ptr_eq(&frame.source_snapshot, &snapshot))
    {
        return Vec::new();
    }
    let (resolved, _, _) = state.output.visualization_dynamic_projection(&[], false);
    let key = ProgrammingOwner::Position.key();
    owners
        .iter()
        .map(|&owner| {
            let requested = resolved.get(&(owner, key.clone())).cloned();
            UnpublishedPositionSeed {
                owner,
                declared: requested
                    .is_none()
                    .then(|| engine.declared_default_position(&snapshot, owner))
                    .flatten(),
                requested,
            }
        })
        .collect()
}

/// Why an accepted Pending pair produced no Position readouts. Never a Live fallback.
/// No production caller until TL-548 installs an episode-backed source.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingPositionReadoutError {
    /// The shared Pending identity validation refused the pair (see `PendingNativeReadout`).
    Pair(PendingReadoutError),
    /// The engine no longer runs the pair's snapshot/generation/layout. Passive, not an error
    /// to the operator: the next accepted pair supplies a current readout.
    StaleGeneration,
}

/// Capture `owners` from one ACCEPTED Pending pair of the caller's episode. The identity is
/// the same [`PendingPairBinding`] the Pending native readout publishes; requested intent and
/// commands both come from the After branch (`rendered.source` is the After observation and
/// `rendered.projection` is projected from it). Before is never read. No fitting, sampling,
/// calibration or mutation occurs; the engine validates current generation and layout.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::runtime) fn capture_pending_position_readouts<S>(
    engine: &Engine,
    identity: PendingEpisodeIdentity,
    ticket: PendingAttemptTicket,
    accepted: &PendingPairResult<PendingHybridResult<S>>,
    exact: &Arc<RetainedInputCapture>,
    owners: &[FixtureId],
) -> Result<CapturedPositionReadouts, PendingPositionReadoutError> {
    let binding = PendingPairBinding::accepted(identity, ticket, accepted, exact)
        .map_err(PendingPositionReadoutError::Pair)?;
    if !Arc::ptr_eq(&engine.snapshot(), &exact.frame.snapshot()) {
        return Err(PendingPositionReadoutError::StaleGeneration);
    }
    let after = &accepted.value.rendered;
    let readouts = engine
        .position_commanded_readouts_from_physical(
            binding.frame.generation,
            &after.projection.physical,
            owners,
        )
        .ok_or(PendingPositionReadoutError::StaleGeneration)?;
    let zooms = engine
        .zoom_readouts_from_physical(binding.frame.generation, &after.projection.physical, owners)
        .ok_or(PendingPositionReadoutError::StaleGeneration)?;
    Ok(CapturedPositionReadouts {
        identity: binding.frame,
        scope: VisualizationScope {
            show_id: Some(identity.show_id.0),
        },
        show_revision: binding.show_revision,
        owners: requested_owners(readouts, zooms, after.source.values()),
    })
}

/// TL-548 hook: the desk's accepted Pending Position source. An implementation holds its own
/// engine handle and each Programmer's current episode, and captures with
/// [`capture_pending_position_readouts`] from that episode's last accepted pair and the exact
/// retained capture it consumed. It returns None when the Programmer has no current episode or
/// no accepted pair; it must never substitute the Live publication.
pub(in crate::runtime) trait PendingPositionReadoutSource: Send + Sync {
    fn capture(
        &self,
        programmer: ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<CapturedPositionReadouts>;
}

/// Injected Pending readout source. Empty by default, so production Preload stays quiet until
/// TL-548 installs an episode-backed source. Clones share one slot.
#[derive(Clone, Default)]
pub(in crate::runtime) struct PendingPositionReadoutSlot(
    Arc<parking_lot::RwLock<Option<Arc<dyn PendingPositionReadoutSource>>>>,
);

impl PendingPositionReadoutSlot {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::runtime) fn install(&self, source: Arc<dyn PendingPositionReadoutSource>) {
        *self.0.write() = Some(source);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::runtime) fn clear(&self) {
        *self.0.write() = None;
    }

    pub(in crate::runtime) fn source(&self) -> Option<Arc<dyn PendingPositionReadoutSource>> {
        self.0.read().clone()
    }
}

#[cfg(test)]
#[path = "position_readout/pending_tests.rs"]
mod pending_tests;
