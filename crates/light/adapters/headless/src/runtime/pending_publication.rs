//! TL-610: immutable Pending native readouts and the Pending-episode publication gate.
//!
//! This is a pure prerequisite helper, not production Pending delivery. It acquires no
//! application lock, reads no Live state, starts no evaluation and owns no notification. TL-548
//! keeps the episode executor, lifecycle wiring, acceptance authority, locks and the
//! output/visualization cutover. Production keeps `SUPPORTED_PROGRAMMING_CONTRACT = 0`; nothing
//! here has a production caller yet.
//!
//! - [`PendingNativeReadout::prepare`] consumes only an accepted paired result and its exact
//!   retained capture. Rows, Point poses and native ownership masks come solely from
//!   `result.rendered.projection`; show revision and tracking come solely from the retained
//!   capture; generation and sample time come from the After Pending frame token; the sequence
//!   is the owner's attempt ticket. It never reads `PublishedVisualizationFrame`, the current
//!   engine or Live poses, invents no Live fallback and reruns no fitting.
//! - [`PendingPairBinding`] is the one capture/Programmer/token-lineage validation of an
//!   accepted pair. The native readout and the Pending Position readout
//!   (`position_readout::capture_pending_position_readouts`) both bind its frame identity.
//! - [`PendingPublicationGate`] admits a readout only under the current episode lease, for the
//!   lease's own identity, with a strictly newer ticket. A failure or gap retains the last
//!   accepted `Arc`; a new episode or reset exposes no earlier result.
//!
//! The helper cannot prove that speculative evaluation was accepted. The caller prepares and
//! publishes only after the paired evaluator succeeded and under actual episode authority.
//! Gate mutation is plain `&mut self`; the owner supplies any synchronization.
#![cfg_attr(not(test), allow(dead_code))]

use super::retained_preload_hybrid::PendingHybridResult;
use crate::runtime::dynamic_snapshot_publication::RetainedInputCapture;
use crate::runtime::preload::retained_history::paired::PendingPairResult;
use light_core::{FixtureId, ProgrammerId, ShowId};
use light_engine::{
    CapturedFrameLane, CapturedFrameToken, PhysicalForwardFrame, PreloadBranch,
    RenderedPreloadFrame, ResolvedPointPose, ResolvedValues,
};
use light_wire::v2::output_control::{
    OutputFrameIdentity, OutputNativeInstance, OutputNativeLane, OutputPointPose,
    OutputTrackingIdentity,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Owned identity of one Pending episode. `activation` is new for every show activation,
/// including a same-show reload; `episode` is new for every Pending episode of that activation
/// (for example after clear or GO). `show_id` is the actual show, carried independently of its
/// activation nonce. Session identity and preload-values generations are deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct PendingEpisodeIdentity {
    pub show_id: ShowId,
    pub activation: Uuid,
    pub programmer: ProgrammerId,
    pub episode: Uuid,
}

/// Owner-supplied attempt order within one episode. Never derived from Live clocks or capture
/// timestamps. It becomes the readout's frame `sequence`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::runtime) struct PendingAttemptTicket(u64);

impl PendingAttemptTicket {
    pub(in crate::runtime) const fn new(sequence: u64) -> Self {
        Self(sequence)
    }

    pub(in crate::runtime) const fn sequence(self) -> u64 {
        self.0
    }
}

/// Opaque authority for one begun episode. It is not `Clone`; its nonce is a fresh allocation,
/// so a lease from an earlier episode, another gate or a same-show reload never matches.
#[derive(Debug)]
pub(in crate::runtime) struct PendingEpisodeLease {
    identity: PendingEpisodeIdentity,
    nonce: Arc<()>,
}

impl PendingEpisodeLease {
    pub(in crate::runtime) fn identity(&self) -> PendingEpisodeIdentity {
        self.identity
    }
}

/// Why an accepted pair could not become a Pending readout. No partial payload is produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingReadoutError {
    /// The pair's capture is not the exact supplied retained capture allocation.
    ForeignCapture,
    /// The retained capture belongs to another Programmer than the episode identity.
    ForeignProgrammer,
    /// A branch token names a different capture than the exact retained capture.
    TokenCapture(PreloadBranch),
    /// Tokens are not Before/After of one Preload bundle, state and revision (e.g. swapped).
    BranchLineage,
    /// A token's revision/generation/sample/tracking disagrees with its retained capture.
    FrameIdentity(PreloadBranch),
}

/// Projected rows deliberately not emitted. Diagnostic only; never a fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct PendingReadoutOmissions {
    /// Physical rows the projection did not complete.
    pub incomplete: usize,
    /// Complete rows without a projected native ownership mask: never a complete override.
    pub unowned: usize,
    /// Complete rows whose mask length differs from their native row, or owns no channel.
    pub invalid_mask: usize,
}

/// Immutable, owned Pending native readout. It holds only wire DTO data, owned value maps and
/// identities: no engine pools, physical lanes, adapter scratch, capture or sidecars.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PendingNativeReadout {
    identity: PendingEpisodeIdentity,
    ticket: PendingAttemptTicket,
    lane: OutputNativeLane,
    values: PendingLaneValues,
    omitted: PendingReadoutOmissions,
}

/// TL-594: the Preload visualization lane of the same accepted pair, owned so a retained
/// readout pins no engine pool. `values` are the After branch's resolved values,
/// `profile` their profile visualization projection, and the options are the ones captured
/// with the retained input. Readers present exactly these instead of re-evaluating Preload.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct PendingLaneValues {
    pub(in crate::runtime) values: Arc<ResolvedValues>,
    pub(in crate::runtime) profile: Arc<ResolvedValues>,
    pub(in crate::runtime) grand_master: f32,
    pub(in crate::runtime) blackout: bool,
}

/// One projected physical row, as consumed by the native instance emitter.
#[derive(Clone, Copy, Debug)]
struct NativeRow<'a> {
    fixture_id: FixtureId,
    instance_id: Uuid,
    native_identity: &'a str,
    raw: &'a [u32],
    complete: bool,
}

fn projected_rows(physical: &PhysicalForwardFrame) -> impl Iterator<Item = NativeRow<'_>> {
    physical.instances.iter().map(|instance| NativeRow {
        fixture_id: instance.fixture_id,
        instance_id: instance.instance_id,
        native_identity: &instance.native_identity,
        raw: &instance.native_raw,
        complete: instance.complete,
    })
}

/// Existing Preload native semantics: only complete rows, each carrying its fixture's exact
/// projected mask. A row without a valid mask is omitted, never promoted to a complete row.
fn emit_native_instances<'a>(
    rows: impl IntoIterator<Item = NativeRow<'a>>,
    masks: &HashMap<FixtureId, Box<[bool]>>,
) -> (Vec<OutputNativeInstance>, PendingReadoutOmissions) {
    let mut omitted = PendingReadoutOmissions::default();
    let mut instances = Vec::new();
    for row in rows {
        if !row.complete {
            omitted.incomplete += 1;
            continue;
        }
        let Some(mask) = masks.get(&row.fixture_id) else {
            omitted.unowned += 1;
            continue;
        };
        if mask.len() != row.raw.len() || !mask.iter().any(|owned| *owned) {
            omitted.invalid_mask += 1;
            continue;
        }
        instances.push(OutputNativeInstance {
            fixture_id: row.fixture_id.0,
            instance_id: row.instance_id,
            native_identity: row.native_identity.to_owned(),
            raw: row.raw.to_vec(),
            owned_channels: Some(mask.to_vec()),
        });
    }
    (instances, omitted)
}

/// Point poses exactly as projected for this Pending frame; never Live poses.
fn native_points(points: &[ResolvedPointPose]) -> Vec<OutputPointPose> {
    points
        .iter()
        .map(|pose| OutputPointPose {
            fixture_id: pose.fixture_id.0,
            offset_metres: pose.offset_metres,
            rotation_degrees: pose.rotation_degrees,
        })
        .collect()
}

/// The shared Preload bundle/state/revision of a token, if it is the expected branch.
fn preload_lineage(
    token: &CapturedFrameToken,
    expected: PreloadBranch,
) -> Option<(&Arc<()>, &Arc<()>, u64)> {
    match token.lane() {
        CapturedFrameLane::Preload {
            bundle,
            state,
            revision,
            branch,
        } if *branch == expected => Some((bundle, state, *revision)),
        _ => None,
    }
}

/// The one identity an accepted Pending pair binds. Pending native and Position readouts both
/// derive it here, so they can never disagree about which accepted frame they describe.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct PendingPairBinding {
    /// Generation and sample time of the After token, the owner's ticket as `sequence`, and the
    /// retained capture's tracking.
    pub(in crate::runtime) frame: OutputFrameIdentity,
    /// The retained capture's show snapshot revision.
    pub(in crate::runtime) show_revision: u64,
}

impl PendingPairBinding {
    /// Validate an accepted pair against the exact retained capture and episode identity.
    /// The caller must already hold actual episode authority and a successful paired result.
    pub(in crate::runtime) fn accepted<S>(
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        accepted: &PendingPairResult<PendingHybridResult<S>>,
        exact: &Arc<RetainedInputCapture>,
    ) -> Result<Self, PendingReadoutError> {
        Self::validate(
            identity,
            ticket,
            &accepted.capture,
            exact,
            &accepted.value.before.frame_token,
            &accepted.value.after.frame_token,
        )
    }

    /// Capture, Programmer, per-branch frame identity and shared Before/After lineage checks.
    fn validate(
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        accepted_capture: &Arc<RetainedInputCapture>,
        exact: &Arc<RetainedInputCapture>,
        before: &CapturedFrameToken,
        after: &CapturedFrameToken,
    ) -> Result<Self, PendingReadoutError> {
        if !Arc::ptr_eq(accepted_capture, exact) {
            return Err(PendingReadoutError::ForeignCapture);
        }
        let frame = &exact.frame;
        if frame.programmer().identity != Some(identity.programmer) {
            return Err(PendingReadoutError::ForeignProgrammer);
        }
        let live = frame.frame_token();
        let snapshot_revision = frame.snapshot().revision;
        let tracking = frame.tracking();
        for (branch, token) in [
            (PreloadBranch::BeforeRelease, before),
            (PreloadBranch::AfterRelease, after),
        ] {
            if !token.same_capture(&live) {
                return Err(PendingReadoutError::TokenCapture(branch));
            }
            if token.show_revision() != snapshot_revision
                || token.generation() != frame.generation()
                || token.sampled_at() != frame.sampled_at()
                || token.tracking_sequence() != tracking.accepted_sequence
            {
                return Err(PendingReadoutError::FrameIdentity(branch));
            }
        }
        let (Some(before_lineage), Some(after_lineage)) = (
            preload_lineage(before, PreloadBranch::BeforeRelease),
            preload_lineage(after, PreloadBranch::AfterRelease),
        ) else {
            return Err(PendingReadoutError::BranchLineage);
        };
        if !Arc::ptr_eq(before_lineage.0, after_lineage.0)
            || !Arc::ptr_eq(before_lineage.1, after_lineage.1)
            || before_lineage.2 != after_lineage.2
        {
            return Err(PendingReadoutError::BranchLineage);
        }
        Ok(Self {
            frame: OutputFrameIdentity {
                generation: after.generation(),
                sequence: ticket.sequence(),
                sampled_at: after.sampled_at().to_rfc3339(),
                tracking: Some(OutputTrackingIdentity {
                    show_id: tracking.show_id.map(|id| id.0),
                    configuration_generation: tracking.configuration_generation,
                    point_generation: tracking.point_generation,
                    source_generation: tracking.source_generation,
                    accepted_sequence: tracking.accepted_sequence,
                    sampled_at_millis: tracking.sampled_at_millis,
                }),
            },
            show_revision: snapshot_revision,
        })
    }
}

impl PendingNativeReadout {
    /// Build a readout from one accepted pair and the exact retained capture it consumed.
    /// The caller must already hold actual episode authority and a successful paired result.
    pub(in crate::runtime) fn prepare<S>(
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        accepted: &PendingPairResult<PendingHybridResult<S>>,
        exact: &Arc<RetainedInputCapture>,
    ) -> Result<Self, PendingReadoutError> {
        Self::prepare_parts(
            identity,
            ticket,
            &accepted.capture,
            exact,
            &accepted.value.before.frame_token,
            &accepted.value.after.frame_token,
            &accepted.value.rendered,
        )
    }

    fn prepare_parts(
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        accepted_capture: &Arc<RetainedInputCapture>,
        exact: &Arc<RetainedInputCapture>,
        before: &CapturedFrameToken,
        after: &CapturedFrameToken,
        rendered: &RenderedPreloadFrame,
    ) -> Result<Self, PendingReadoutError> {
        let binding =
            PendingPairBinding::validate(identity, ticket, accepted_capture, exact, before, after)?;
        let projection = &rendered.projection;
        let (instances, omitted) = emit_native_instances(
            projected_rows(&projection.physical),
            &projection.native_ownership,
        );
        let points = native_points(&projection.points);
        let options = exact.frame.options();
        let values = PendingLaneValues {
            values: Arc::new(rendered.source.values().values().clone()),
            profile: Arc::new(projection.values.clone()),
            grand_master: options.grand_master,
            blackout: options.blackout,
        };
        Ok(Self {
            identity,
            ticket,
            values,
            lane: OutputNativeLane {
                show_id: Some(identity.show_id.0),
                revision: binding.show_revision,
                frame: Some(binding.frame),
                instances,
                points,
            },
            omitted,
        })
    }

    pub(in crate::runtime) fn identity(&self) -> PendingEpisodeIdentity {
        self.identity
    }

    pub(in crate::runtime) fn ticket(&self) -> PendingAttemptTicket {
        self.ticket
    }

    /// The existing Preload native lane DTO; no wire or schema change.
    pub(in crate::runtime) fn lane(&self) -> &OutputNativeLane {
        &self.lane
    }

    /// The Preload visualization values of the same accepted pair (TL-594).
    pub(in crate::runtime) fn values(&self) -> &PendingLaneValues {
        &self.values
    }

    /// A gate-only readout with explicit values, for reader tests that need a known ticket.
    #[cfg(test)]
    pub(in crate::runtime) fn for_tests(
        identity: PendingEpisodeIdentity,
        ticket: PendingAttemptTicket,
        lane: OutputNativeLane,
        values: PendingLaneValues,
    ) -> Self {
        Self {
            identity,
            ticket,
            lane,
            values,
            omitted: PendingReadoutOmissions::default(),
        }
    }

    pub(in crate::runtime) fn omitted(&self) -> PendingReadoutOmissions {
        self.omitted
    }
}

/// Why the gate refused a publication or gap. The last accepted readout is always retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PendingPublicationRejection {
    NoEpisode,
    /// The lease is not the current episode's lease (ended, superseded or another gate's).
    StaleLease,
    /// The readout was prepared for a different episode identity than the current lease.
    ForeignIdentity,
    /// The ticket does not follow the last accepted or failed ticket of this episode.
    StaleTicket {
        ticket: PendingAttemptTicket,
        floor: PendingAttemptTicket,
    },
}

#[derive(Debug)]
struct EpisodeSlot {
    identity: PendingEpisodeIdentity,
    nonce: Arc<()>,
    floor: Option<PendingAttemptTicket>,
    latest: Option<Arc<PendingNativeReadout>>,
}

impl EpisodeSlot {
    fn admit(
        &self,
        lease: &PendingEpisodeLease,
        ticket: PendingAttemptTicket,
    ) -> Result<(), PendingPublicationRejection> {
        if !Arc::ptr_eq(&self.nonce, &lease.nonce) || self.identity != lease.identity {
            return Err(PendingPublicationRejection::StaleLease);
        }
        match self.floor {
            Some(floor) if ticket <= floor => {
                Err(PendingPublicationRejection::StaleTicket { ticket, floor })
            }
            _ => Ok(()),
        }
    }
}

/// Owned single-episode publication state. `latest` is a cheap read of the same immutable
/// allocation until a newer readout is accepted or the episode ends.
#[derive(Debug, Default)]
pub(in crate::runtime) struct PendingPublicationGate {
    episode: Option<EpisodeSlot>,
}

impl PendingPublicationGate {
    /// Start a new episode, invalidating every earlier lease and hiding any earlier readout,
    /// even when the identity is unchanged.
    pub(in crate::runtime) fn begin_episode(
        &mut self,
        identity: PendingEpisodeIdentity,
    ) -> PendingEpisodeLease {
        let nonce = Arc::new(());
        self.episode = Some(EpisodeSlot {
            identity,
            nonce: Arc::clone(&nonce),
            floor: None,
            latest: None,
        });
        PendingEpisodeLease { identity, nonce }
    }

    /// End the episode only when `lease` is the current one. Returns whether it ended.
    pub(in crate::runtime) fn end_episode(&mut self, lease: &PendingEpisodeLease) -> bool {
        let current = self
            .episode
            .as_ref()
            .is_some_and(|slot| Arc::ptr_eq(&slot.nonce, &lease.nonce));
        if current {
            self.episode = None;
        }
        current
    }

    /// Accept a readout prepared after a successful pair. Rejections keep the previous `Arc`.
    pub(in crate::runtime) fn publish(
        &mut self,
        lease: &PendingEpisodeLease,
        readout: PendingNativeReadout,
    ) -> Result<Arc<PendingNativeReadout>, PendingPublicationRejection> {
        let slot = self
            .episode
            .as_mut()
            .ok_or(PendingPublicationRejection::NoEpisode)?;
        slot.admit(lease, readout.ticket)?;
        if readout.identity != slot.identity {
            return Err(PendingPublicationRejection::ForeignIdentity);
        }
        slot.floor = Some(readout.ticket);
        let readout = Arc::new(readout);
        slot.latest = Some(Arc::clone(&readout));
        Ok(readout)
    }

    /// Record a stopped or failed attempt. Its ticket is consumed, so an older successful
    /// result cannot arrive later; the last accepted readout stays visible.
    pub(in crate::runtime) fn record_gap(
        &mut self,
        lease: &PendingEpisodeLease,
        ticket: PendingAttemptTicket,
    ) -> Result<(), PendingPublicationRejection> {
        let slot = self
            .episode
            .as_mut()
            .ok_or(PendingPublicationRejection::NoEpisode)?;
        slot.admit(lease, ticket)?;
        slot.floor = Some(ticket);
        Ok(())
    }

    /// The current episode's identity, if one is active.
    pub(in crate::runtime) fn current(&self) -> Option<PendingEpisodeIdentity> {
        self.episode.as_ref().map(|slot| slot.identity)
    }

    /// The last accepted readout of the current episode. Never an earlier episode's result.
    pub(in crate::runtime) fn latest(&self) -> Option<&Arc<PendingNativeReadout>> {
        self.episode.as_ref()?.latest.as_ref()
    }
}

/// Owned readouts and the gate cross threads without interior synchronization.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PendingNativeReadout>();
    assert_send_sync::<Arc<PendingNativeReadout>>();
    assert_send_sync::<PendingEpisodeLease>();
    assert_send_sync::<PendingPublicationGate>();
};

#[cfg(test)]
#[path = "pending_publication/tests.rs"]
mod tests;
