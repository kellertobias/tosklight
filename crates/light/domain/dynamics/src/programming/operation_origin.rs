//! Opaque producer-operation provenance for Required keyframe transitions and controller Size.
//!
//! One immutable [`DynamicEmissionWitness`] is pinned per genuine instance/controller sampling
//! emission, before the target loop. Target-specific operation origins are attached at the
//! actual producer sites (keyframe segment, Size operation) before any per-target
//! normalization. Shared identity is the retained witness object itself: equal rank, progress,
//! values, controller fields or node positions never correlate two independent emissions.
//!
//! Nothing here can be manufactured from scalar metadata. Origins are created only by the
//! sampler, and reconstructed only from a validated retained-tape object table, where every
//! reference names one table entry and all references to that entry share one `Arc`.
//! A restored witness proves historical correspondence only; it carries no sample boundary,
//! frame capture or solver authority.
use crate::{DynamicController, DynamicDefinition, DynamicLaneBody, ProgrammingLaneConfiguration};
use light_core::{FixtureId, programming::IntentError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Evidence of one genuine pinned sampling emission. Read-only; no public constructor.
/// `Clone` exists only for copy-on-write normalization of one table entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicEmissionWitness {
    instance_id: Uuid,
    /// The definition pinned by the frame, not a later hot-edited replacement.
    definition: Arc<DynamicDefinition>,
    /// The controller snapshot pinned by the frame (rank, Size, phase, speed).
    controller: DynamicController,
    /// Ordered target membership of the pinned frame.
    targets: Arc<[FixtureId]>,
    elapsed_millis: u64,
    cycle_duration_millis: u64,
    /// Process-local: deserialized witnesses are always historical.
    #[serde(skip, default = "restored_witness")]
    restored: bool,
}

fn restored_witness() -> bool {
    true
}

/// Value evidence only. Identity is never value equality; see [`DynamicOperationHandle`].
impl PartialEq for DynamicEmissionWitness {
    fn eq(&self, other: &Self) -> bool {
        self.instance_id == other.instance_id
            && self.definition == other.definition
            && self.controller == other.controller
            && self.targets == other.targets
            && self.elapsed_millis == other.elapsed_millis
            && self.cycle_duration_millis == other.cycle_duration_millis
    }
}

impl DynamicEmissionWitness {
    /// Genuine producer emission only: one per pinned instance/controller frame.
    pub(crate) fn pin(
        instance_id: Uuid,
        definition: &Arc<DynamicDefinition>,
        controller: &DynamicController,
        targets: &Arc<[FixtureId]>,
        elapsed_millis: u64,
        cycle_duration_millis: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            instance_id,
            definition: Arc::clone(definition),
            controller: controller.clone(),
            targets: Arc::clone(targets),
            elapsed_millis,
            cycle_duration_millis,
            restored: false,
        })
    }

    pub fn instance_id(&self) -> Uuid {
        self.instance_id
    }
    pub fn definition(&self) -> &DynamicDefinition {
        &self.definition
    }
    pub fn controller(&self) -> &DynamicController {
        &self.controller
    }
    pub fn targets(&self) -> &[FixtureId] {
        &self.targets
    }
    pub fn elapsed_millis(&self) -> u64 {
        self.elapsed_millis
    }
    pub fn cycle_duration_millis(&self) -> u64 {
        self.cycle_duration_millis
    }
    /// True for witnesses reconstructed from a checkpoint. They prove historical operation
    /// correspondence only and never authorize a new frame capture or fitting boundary.
    pub fn is_restored(&self) -> bool {
        self.restored
    }

    /// Cold structural validation of one object-table entry. Producer witnesses were pinned
    /// from a validated instance and skip the membership scan on the frame path.
    pub(crate) fn validate(&self) -> Result<(), IntentError> {
        if !self.restored {
            return Ok(());
        }
        let mut seen = std::collections::HashSet::with_capacity(self.targets.len());
        if self.instance_id.is_nil()
            || self.targets.is_empty()
            || !self.targets.iter().all(|target| seen.insert(*target))
            || !self.controller.size.is_finite()
            || self.controller.size < 0.0
        {
            return Err(IntentError(
                "Dynamic emission witness has invalid membership or controller".into(),
            ));
        }
        Ok(())
    }

    /// Restore materializes one historical object per validated table entry, also when the
    /// snapshot was never serialized, so a restored runtime never shares live authority.
    pub(crate) fn restored_copy(&self) -> Self {
        Self {
            restored: true,
            ..self.clone()
        }
    }

    /// Programmer identity normalization rewrites the one shared controller reference.
    pub(crate) fn rekey_controller(&mut self, new_id: Uuid) {
        let authored_link = self.controller.id;
        self.controller.id = new_id;
        if let crate::DynamicControllerSource::Programmer { instance_link, .. } =
            &mut self.controller.source
            && instance_link.is_none()
        {
            *instance_link = Some(authored_link);
        }
    }
}

/// The original operation site, before simplification or per-target normalization.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicOperationSite {
    /// A keyframe transition between `segment_index` and its successor, as returned by the
    /// lane's keyframe segment selection.
    KeyframeTransition { segment_index: u32 },
    /// The controller Size operation of one lane.
    ControllerSize { role: DynamicControllerSizeRole },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicControllerSizeRole {
    /// Whole-family `Scale` expression from the ordinary Size path.
    FamilyScale,
    /// `ScaleFrom` around Current in an optimized numeric Angle program.
    AngleNumericScaleFrom,
}

/// A target-specific origin attached at a producer site. Opaque; no public constructor.
#[derive(Clone, Debug)]
pub struct DynamicOperationOrigin {
    emission: Arc<DynamicEmissionWitness>,
    target: FixtureId,
    lane_id: Uuid,
    site: DynamicOperationSite,
}

impl DynamicOperationOrigin {
    pub(crate) fn emission(&self) -> &Arc<DynamicEmissionWitness> {
        &self.emission
    }
    pub(crate) fn target(&self) -> FixtureId {
        self.target
    }
    pub(crate) fn lane_id(&self) -> Uuid {
        self.lane_id
    }
    pub(crate) fn site(&self) -> DynamicOperationSite {
        self.site
    }

    /// Validated object-table reconstruction only.
    pub(crate) fn restore(
        emission: &Arc<DynamicEmissionWitness>,
        target: FixtureId,
        lane_id: Uuid,
        site: DynamicOperationSite,
    ) -> Self {
        Self {
            emission: Arc::clone(emission),
            target,
            lane_id,
            site,
        }
    }

    /// Membership, lane and keyframe-segment checks against the witness's own pinned evidence.
    pub(crate) fn validate(&self) -> Result<(), IntentError> {
        if !self.emission.targets.contains(&self.target) {
            return Err(IntentError(
                "Dynamic operation target is not a member of its emission".into(),
            ));
        }
        let lane = self
            .emission
            .definition
            .lanes
            .iter()
            .find(|lane| lane.id == self.lane_id)
            .ok_or_else(|| {
                IntentError("Dynamic operation lane is absent from its pinned definition".into())
            })?;
        if let DynamicOperationSite::KeyframeTransition { segment_index } = self.site {
            let DynamicLaneBody::Programming(body) = &lane.body else {
                return Err(IntentError(
                    "Dynamic keyframe operation requires a programming lane".into(),
                ));
            };
            let ProgrammingLaneConfiguration::Keyframes(keyframes) = &body.configuration else {
                return Err(IntentError(
                    "Dynamic keyframe operation requires a keyframe lane".into(),
                ));
            };
            if segment_index as usize >= keyframes.points.len() {
                return Err(IntentError(
                    "Dynamic keyframe operation names an absent segment".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Producer-side context for one target/lane of one pinned emission.
#[derive(Clone, Copy)]
pub(crate) struct DynamicOperationContext<'a> {
    pub emission: &'a Arc<DynamicEmissionWitness>,
    pub target: FixtureId,
    pub lane_id: Uuid,
}

impl DynamicOperationContext<'_> {
    pub(crate) fn origin(&self, site: DynamicOperationSite) -> DynamicOperationOrigin {
        DynamicOperationOrigin {
            emission: Arc::clone(self.emission),
            target: self.target,
            lane_id: self.lane_id,
            site,
        }
    }
}

/// Inline origins of an optimized numeric Angle program: `(program node, origin)` pairs.
/// Transparent to value equality and serialization; retained tapes move these into their
/// object table, so a tape never stores a second inline copy.
#[derive(Clone, Debug, Default)]
pub struct AngleNumericOperationOrigins(pub(crate) [Option<(u32, DynamicOperationOrigin)>; 2]);

impl PartialEq for AngleNumericOperationOrigins {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl AngleNumericOperationOrigins {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &(u32, DynamicOperationOrigin)> {
        self.0.iter().flatten()
    }
}

/// Read-only handle to one attributed producer operation.
#[derive(Clone, Debug)]
pub struct DynamicOperationHandle {
    origin: DynamicOperationOrigin,
    program_node: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicOperationCorrespondence {
    /// The same genuine emission object, lane and original site, with both targets validated
    /// members. `historical` is true when the witness was restored from a checkpoint: such a
    /// correspondence never establishes a new frame capture or fitting authority.
    Shared {
        historical: bool,
    },
    Uncorrelated(DynamicOperationMismatch),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicOperationMismatch {
    /// Independent emissions, even with equal values, rank, progress or controller fields.
    DifferentEmission,
    DifferentLane,
    DifferentSite,
    NonMemberTarget,
}

impl DynamicOperationHandle {
    pub(crate) fn new(origin: DynamicOperationOrigin, program_node: Option<u32>) -> Self {
        Self {
            origin,
            program_node,
        }
    }
    pub fn site(&self) -> DynamicOperationSite {
        self.origin.site
    }
    pub fn lane_id(&self) -> Uuid {
        self.origin.lane_id
    }
    pub fn target(&self) -> FixtureId {
        self.origin.target
    }
    /// Locator inside an optimized numeric Angle program; not part of operation identity.
    pub fn program_node(&self) -> Option<u32> {
        self.program_node
    }
    pub fn emission(&self) -> &DynamicEmissionWitness {
        &self.origin.emission
    }
    pub fn is_historical(&self) -> bool {
        self.origin.emission.restored
    }

    /// Guarded correspondence: the same emission object, original site and lane, and both
    /// targets validated members of the pinned ordered membership.
    pub fn correspondence(&self, other: &Self) -> DynamicOperationCorrespondence {
        use DynamicOperationCorrespondence::*;
        use DynamicOperationMismatch::*;
        if !Arc::ptr_eq(&self.origin.emission, &other.origin.emission) {
            return Uncorrelated(DifferentEmission);
        }
        if self.origin.lane_id != other.origin.lane_id {
            return Uncorrelated(DifferentLane);
        }
        if self.origin.site != other.origin.site {
            return Uncorrelated(DifferentSite);
        }
        if self.origin.validate().is_err() || other.origin.validate().is_err() {
            return Uncorrelated(NonMemberTarget);
        }
        Shared {
            historical: self.origin.emission.restored,
        }
    }
}

/// Explicit provenance of every reachable Required/Size operation of one expression.
/// `unattributed` counts Required transitions, whole-family Size nodes and numeric keyframe
/// transitions without a retained origin (legacy checkpoints, synthetic assembly).
#[derive(Clone, Debug, Default)]
pub struct DynamicOperationProvenance {
    pub(crate) handles: Vec<DynamicOperationHandle>,
    pub(crate) unattributed: usize,
}

impl DynamicOperationProvenance {
    pub fn handles(&self) -> &[DynamicOperationHandle] {
        &self.handles
    }
    pub fn unattributed_operations(&self) -> usize {
        self.unattributed
    }
    pub fn is_complete(&self) -> bool {
        self.unattributed == 0
    }
}

impl crate::DynamicSampleExpression {
    /// The annotated node itself. Provenance wrappers are transparent to every view; callers
    /// that match a root variant directly should match this instead.
    pub fn unannotated(&self) -> &Self {
        let mut expression = self;
        while let Self::Operation { value, .. } = expression {
            expression = value;
        }
        expression
    }

    /// Every reachable attributed Required/Size operation, plus an explicit count of such
    /// operations without retained provenance. Walks shared history once; never samples.
    pub fn operation_provenance(&self) -> Result<DynamicOperationProvenance, IntentError> {
        use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
        use crate::{AngleNumericNode, DynamicTransitionReason};
        let mut result = DynamicOperationProvenance::default();
        for node in ExpressionNodeRef::new(self).postorder(false)? {
            let view = node.node()?;
            let first = result.handles.len();
            match node {
                ExpressionNodeRef::Tape(tape, id) => {
                    result.handles.extend(tape.operation_handles(id)?);
                }
                ExpressionNodeRef::Tree(_) => {
                    if let Some(origin) = node.operation_origin() {
                        let matches = match (&view, origin.site()) {
                            (
                                ExpressionNode::Transition {
                                    reason: DynamicTransitionReason::Required { .. },
                                    ..
                                },
                                DynamicOperationSite::KeyframeTransition { .. },
                            ) => true,
                            (
                                ExpressionNode::Scale { .. },
                                DynamicOperationSite::ControllerSize {
                                    role: DynamicControllerSizeRole::FamilyScale,
                                },
                            ) => true,
                            _ => false,
                        };
                        if !matches {
                            return Err(IntentError(
                                "Dynamic operation origin does not match its node".into(),
                            ));
                        }
                        origin.validate()?;
                        result
                            .handles
                            .push(DynamicOperationHandle::new(origin.clone(), None));
                    }
                    if let ExpressionNode::Numeric(program) = &view {
                        for (index, origin) in program.operations.iter() {
                            let matches = matches!(
                                (program.nodes.get(*index as usize), origin.site()),
                                (
                                    Some(AngleNumericNode::Transition { .. }),
                                    DynamicOperationSite::KeyframeTransition { .. }
                                ) | (
                                    Some(AngleNumericNode::ScaleFrom { .. }),
                                    DynamicOperationSite::ControllerSize {
                                        role: DynamicControllerSizeRole::AngleNumericScaleFrom,
                                    }
                                )
                            );
                            if !matches {
                                return Err(IntentError(
                                    "numeric Angle operation origin does not match its node".into(),
                                ));
                            }
                            origin.validate()?;
                            result
                                .handles
                                .push(DynamicOperationHandle::new(origin.clone(), Some(*index)));
                        }
                    }
                }
            }
            let attributed = &result.handles[first..];
            let keyframe = attributed.iter().any(|handle| {
                matches!(
                    handle.site(),
                    DynamicOperationSite::KeyframeTransition { .. }
                )
            });
            let size = attributed
                .iter()
                .any(|handle| matches!(handle.site(), DynamicOperationSite::ControllerSize { .. }));
            match view {
                ExpressionNode::Transition {
                    reason: DynamicTransitionReason::Required { .. },
                    ..
                } if !keyframe => result.unattributed += 1,
                ExpressionNode::Scale { .. } if !size => result.unattributed += 1,
                ExpressionNode::Numeric(program)
                    if !keyframe
                        && program
                            .nodes
                            .iter()
                            .any(|node| matches!(node, AngleNumericNode::Transition { .. })) =>
                {
                    result.unattributed += 1
                }
                _ => {}
            }
        }
        Ok(result)
    }
}
