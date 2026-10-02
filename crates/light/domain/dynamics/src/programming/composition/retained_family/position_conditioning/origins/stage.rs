//! Original lexical composition-use locators. Prepared indices are converted through explicit
//! compiler/conditioning maps; neither operands nor replay order can construct authority.
use super::super::super::position_program::{PositionMaskStage, PositionStageOperandContinuation};
use super::super::super::{
    PositionCompletionStage, PreparedPositionStageKind, PreparedPositionStageRoute,
    PreparedPositionStageUse,
};
use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionStageKind {
    ComponentAdoption { component: ProgrammingComponent },
    WholeAdoption,
    WholeSegmentTransition,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionStageOperand {
    AdoptionInput,
    TransitionFrom,
    TransitionTo,
}
impl PositionStageOperand {
    pub(in crate::programming::composition) fn segment(
        self,
    ) -> position_segment::PositionSegmentOperand {
        match self {
            Self::AdoptionInput => position_segment::PositionSegmentOperand::AdoptionInput,
            Self::TransitionFrom => position_segment::PositionSegmentOperand::TransitionFrom,
            Self::TransitionTo => position_segment::PositionSegmentOperand::TransitionTo,
        }
    }
}
#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct OriginalSource {
    pub(super) slot: usize,
    pub(super) member: Option<PositionSourceNode>,
}
#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) enum OriginalUse {
    RootFinalSegment,
    UnderlayFor(Vec<OriginalSource>),
    SourceCohort {
        consumer: OriginalSource,
        node: PositionSourceNode,
    },
}
/// Segment stages are public through `kind`. Envelope stages share the exact route
/// identity but are public only through `PositionEnvelopeLocator`, so a segment-only
/// consumer can never receive a locator outside `PositionStageKind`. Mask stages are
/// likewise public only through `PositionMaskLocator`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum LocatedStage {
    Segment(PositionStageKind),
    Envelope(PositionCompletionStage),
    Mask(PositionMaskStage),
}
/// Only a bound original continuation can issue this locator. It can be replayed against
/// another branch of that registry, retaining the same lexical source/member operation.
#[derive(Clone)]
pub struct PositionStageLocator {
    captured: Arc<CapturedProgram>,
    uses: Vec<OriginalUse>,
    trigger: Vec<OriginalSource>,
    kind: LocatedStage,
}
impl PartialEq for PositionStageLocator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.captured, &other.captured)
            && self.uses == other.uses
            && self.trigger == other.trigger
            && self.kind == other.kind
    }
}
impl Eq for PositionStageLocator {}
impl Hash for PositionStageLocator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.captured).hash(state);
        self.uses.hash(state);
        self.trigger.hash(state);
        self.kind.hash(state);
    }
}
impl std::fmt::Debug for PositionStageLocator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PositionStageLocator")
            .field("capture_id", &self.capture_id())
            .field("kind", &self.kind)
            .field("lexical_depth", &self.uses.len())
            .finish_non_exhaustive()
    }
}
impl PositionStageLocator {
    /// Check the exact original trigger and lexical consumers in the changed branch.
    /// This is structural presence, never a numerical preview or a new evaluation.
    pub(super) fn active_in_branch(
        &self,
        branch: &PositionProgramBranch,
    ) -> Result<bool, TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &branch.captured),
            "Position enclosing stage belongs to another registry",
        )?;
        let sources = branch.conditioned_with_origins()?;
        let present = |source: &OriginalSource| {
            sources.iter().any(|bound| {
                bound.original_index == source.slot
                    && source.member.as_ref().is_none_or(|member| {
                        bound
                            .origins
                            .locations
                            .values()
                            .any(|location| *location == member.node)
                    })
            })
        };
        if !self.trigger.iter().all(|source| present(source)) {
            return Ok(false);
        }
        for entry in &self.uses {
            match entry {
                OriginalUse::RootFinalSegment => {}
                OriginalUse::UnderlayFor(consumers)
                    if !consumers.iter().all(|source| present(source)) =>
                {
                    return Ok(false);
                }
                OriginalUse::SourceCohort { consumer, node } => {
                    if !present(consumer) || !branch.original_node_active(node)? {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }
        Ok(true)
    }
    /// A local member prefix/operation is narrower than the outer atomic cohort value.
    /// Outer cohort family stages have outer-source triggers and do not satisfy this test.
    pub(super) fn inside_cohort_members(&self, cohort: &PositionSourceNode) -> bool {
        matches!(self.uses.iter().rev().find(|entry| matches!(entry, OriginalUse::SourceCohort { .. })), Some(OriginalUse::SourceCohort { node, .. }) if node == cohort)
            && !self.trigger.is_empty()
            && self.trigger.iter().all(|source| source.member.as_ref().is_some_and(|member| member.source_index == cohort.source_index && matches!((member.node, cohort.node), (NodeLocation::Member(forest, _), NodeLocation::Forest(other)) if forest == other)))
    }
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
    pub fn kind(&self) -> PositionStageKind {
        match self.kind {
            LocatedStage::Segment(kind) => kind,
            LocatedStage::Envelope(_) => {
                unreachable!("envelope locators are public only as PositionEnvelopeLocator")
            }
            LocatedStage::Mask(_) => {
                unreachable!("mask locators are public only as PositionMaskLocator")
            }
        }
    }
    /// Internal discriminator; segment-only public APIs filter envelope locators out.
    pub(crate) fn envelope_stage(&self) -> Option<PositionCompletionStage> {
        match self.kind {
            LocatedStage::Segment(_) | LocatedStage::Mask(_) => None,
            LocatedStage::Envelope(stage) => Some(stage),
        }
    }
    /// Internal discriminator; segment-only public APIs filter mask locators out.
    pub(crate) fn mask_stage(&self) -> Option<PositionMaskStage> {
        match self.kind {
            LocatedStage::Mask(stage) => Some(stage),
            LocatedStage::Segment(_) | LocatedStage::Envelope(_) => None,
        }
    }
    fn supports(&self, operand: PositionStageOperand) -> bool {
        matches!(
            (self.kind, operand),
            (
                LocatedStage::Segment(
                    PositionStageKind::ComponentAdoption { .. } | PositionStageKind::WholeAdoption
                ),
                PositionStageOperand::AdoptionInput
            ) | (
                LocatedStage::Segment(PositionStageKind::WholeSegmentTransition)
                    | LocatedStage::Envelope(_),
                PositionStageOperand::TransitionFrom | PositionStageOperand::TransitionTo
            ) | (
                LocatedStage::Mask(PositionMaskStage::Adoption),
                PositionStageOperand::AdoptionInput
            ) | (
                LocatedStage::Mask(PositionMaskStage::Transition),
                PositionStageOperand::TransitionFrom | PositionStageOperand::TransitionTo
            )
        )
    }
}
impl PositionProgramBranch {
    /// Check original registry and operand-role authority before lending owned scratch.
    /// Reachability in this branch is resolved by replay; a removed stage becomes Inactive.
    pub fn validate_stage_operand(
        &self,
        locator: &PositionStageLocator,
        operand: PositionStageOperand,
    ) -> Result<(), TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &locator.captured),
            "Position stage belongs to another captured registry",
        )?;
        ensure(
            locator.supports(operand),
            "Position operand does not match the original stage",
        )?;
        Ok(())
    }
    /// Replay the existing compositor under this changed branch and stop before the exact
    /// original operation. Missing/covered operations are Inactive, never guessed replacements.
    pub fn begin_stage_operand(
        &self,
        locator: &PositionStageLocator,
        operand: PositionStageOperand,
        context: &FamilyCompositionContext<'_>,
        scratch: RetainedFamilyCompositionScratch,
        tracing: bool,
    ) -> Result<PositionStageOperandContinuation, TransitionError> {
        self.validate_stage_operand(locator, operand)?;
        Ok(PositionStageOperandContinuation::new(
            self.begin_composition(context, scratch, tracing)?,
            locator.clone(),
            operand,
        ))
    }
}
impl PositionOriginBinding {
    fn stage_sources(
        &self,
        origins: &[PreparedSourceOrigin],
        scope: Option<(usize, usize)>,
    ) -> Result<Option<Vec<OriginalSource>>, TransitionError> {
        let mut sources = Vec::new();
        if origins.is_empty() {
            return Ok(None);
        }
        for origin in origins {
            let (slot, member) = if let Some((slot, forest)) = scope {
                // Inside this exact original cohort, a child's caller slot is the conditioned
                // member offset. Map it explicitly; release compaction cannot change authority.
                if origin.member.is_some() {
                    return Ok(None);
                }
                let source = self.source(slot)?;
                let Some(member) =
                    self.handle(source, NodeLocation::Member(forest, origin.original_index))?
                else {
                    return Ok(None);
                };
                (slot, Some(member))
            } else {
                let source = self.source(origin.original_index)?;
                let member = if let Some(member) = origin.member {
                    let FamilyCompositionSample::CoupledExpression { expression, .. } =
                        &source.sample
                    else {
                        return Ok(None);
                    };
                    let Some(NodeLocation::Forest(forest)) =
                        Self::coupled_location(expression, expression.trace_root_node())
                    else {
                        return Ok(None);
                    };
                    let Some(member) = self.handle(source, NodeLocation::Member(forest, member))?
                    else {
                        return Ok(None);
                    };
                    Some(member)
                } else {
                    None
                };
                (origin.original_index, member)
            };
            let source = OriginalSource { slot, member };
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        // Synthetic assembly requires a separate explicit original assembly identity. Matching
        // two contributors' values/addresses is not permission to invent one source operation.
        Ok((sources.len() == 1).then_some(sources))
    }
    pub(super) fn lexical_route(
        &self,
        raw_uses: &[PreparedPositionStageUse],
        trigger_origins: &[PreparedSourceOrigin],
    ) -> Result<
        Option<(
            Vec<OriginalUse>,
            Vec<OriginalSource>,
            Option<(usize, usize)>,
        )>,
        TransitionError,
    > {
        let mut uses = Vec::new();
        let mut scope = None;
        for entry in raw_uses {
            match entry {
                PreparedPositionStageUse::UnsupportedCoupledEndpoint => return Ok(None),
                PreparedPositionStageUse::RootFinalSegment => {
                    ensure(
                        uses.is_empty(),
                        "Position stage has an invalid root use path",
                    )?;
                    uses.push(OriginalUse::RootFinalSegment);
                }
                PreparedPositionStageUse::UnderlayFor { consumer_origins } => {
                    let Some(consumer) = self.stage_sources(consumer_origins, scope)? else {
                        return Ok(None);
                    };
                    uses.push(OriginalUse::UnderlayFor(consumer));
                }
                PreparedPositionStageUse::SourceCohort {
                    consumer_origins,
                    expression,
                    endpoint_nodes,
                } => {
                    // A nested source list currently contains Whole/materialized members, not
                    // another coupled source. Do not invent a deeper path if that contract changes.
                    if scope.is_some() || endpoint_nodes.len() != 1 {
                        return Ok(None);
                    }
                    let Some(mut consumer) = self.stage_sources(consumer_origins, None)? else {
                        return Ok(None);
                    };
                    let consumer = consumer.remove(0);
                    if consumer.member.is_some() {
                        return Ok(None);
                    }
                    let Some(NodeLocation::Forest(forest)) =
                        Self::coupled_location(expression, endpoint_nodes[0])
                    else {
                        return Ok(None);
                    };
                    let source = self.source(consumer.slot)?;
                    let Some(node) = self.handle(source, NodeLocation::Forest(forest))? else {
                        return Ok(None);
                    };
                    if !matches!(
                        nodes::describe(&self.captured, node.source_index, node.node)?,
                        NodeDescription::SourceCohort { .. }
                    ) {
                        return Ok(None);
                    }
                    scope = Some((consumer.slot, forest));
                    uses.push(OriginalUse::SourceCohort { consumer, node });
                }
            }
        }
        ensure(
            matches!(uses.first(), Some(OriginalUse::RootFinalSegment)),
            "Position stage lacks original root use",
        )?;
        let Some(trigger) = self.stage_sources(trigger_origins, scope)? else {
            return Ok(None);
        };
        Ok(Some((uses, trigger, scope)))
    }
    pub(crate) fn stage_locator(
        &self,
        route: &PreparedPositionStageRoute,
    ) -> Result<Option<PositionStageLocator>, TransitionError> {
        let Some((uses, trigger, _)) = self.lexical_route(&route.uses, &route.trigger_origins)?
        else {
            return Ok(None);
        };
        let kind = match route.kind {
            PreparedPositionStageKind::Adoption {
                component: Some(component),
            } => LocatedStage::Segment(PositionStageKind::ComponentAdoption { component }),
            PreparedPositionStageKind::Adoption { component: None } => {
                LocatedStage::Segment(PositionStageKind::WholeAdoption)
            }
            PreparedPositionStageKind::WholeSegmentTransition => {
                LocatedStage::Segment(PositionStageKind::WholeSegmentTransition)
            }
            PreparedPositionStageKind::Envelope(stage) => LocatedStage::Envelope(stage),
        };
        Ok(Some(PositionStageLocator {
            captured: self.captured.clone(),
            uses,
            trigger,
            kind,
        }))
    }
    /// A partial whole mask applies only at the root lexical level, over the composed and
    /// adopted prefix of every lower root source. Its authority is the mask's own original
    /// slot in this registry; neither a lower (descendant) source's route nor the mask's
    /// rank, values or progress can name it.
    pub(crate) fn mask_locator(
        &self,
        origins: &[PreparedSourceOrigin],
        stage: PositionMaskStage,
    ) -> Result<Option<PositionStageLocator>, TransitionError> {
        let [origin] = origins else {
            return Ok(None);
        };
        if origin.member.is_some() {
            return Ok(None);
        }
        let source = self.source(origin.original_index)?;
        ensure(
            self.captured
                .sources
                .get(source.original_index)
                .and_then(FamilyCompositionSample::whole_mask)
                .is_some_and(|mask| mask.activation_mix < 1.0),
            "Position mask stage has no original partial whole mask",
        )?;
        Ok(Some(PositionStageLocator {
            captured: self.captured.clone(),
            uses: vec![OriginalUse::RootFinalSegment],
            trigger: vec![OriginalSource {
                slot: source.original_index,
                member: None,
            }],
            kind: LocatedStage::Mask(stage),
        }))
    }
}
