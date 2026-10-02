//! Exact original graph-node and lexical-use binding. Arithmetic, values and graph ordinals
//! alone confer no authority; the conditioned compiler maps every supported original node.
use super::super::super::{
    GraphOperationKind, PreparedPositionGraphExpression, PreparedPositionGraphRoute,
};
use super::stage::{OriginalSource, OriginalUse};
use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionGraphOperationKind {
    Required,
    Size,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PositionGraphOperationOperand {
    RequiredOutgoing,
    RequiredIncoming,
    SizeBaseline,
    SizeValue,
}
impl PositionGraphOperationOperand {
    pub(in crate::programming::composition) fn internal(
        self,
    ) -> super::super::super::GraphOperationOperand {
        use super::super::super::GraphOperationOperand as O;
        match self {
            Self::RequiredOutgoing => O::RequiredOutgoing,
            Self::RequiredIncoming => O::RequiredIncoming,
            Self::SizeBaseline => O::SizeBaseline,
            Self::SizeValue => O::SizeValue,
        }
    }
    fn supports(self, kind: PositionGraphOperationKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::RequiredOutgoing | Self::RequiredIncoming,
                PositionGraphOperationKind::Required
            ) | (
                Self::SizeBaseline | Self::SizeValue,
                PositionGraphOperationKind::Size
            )
        )
    }
}
/// Owner-local identity of one original Required/Size operation at one exact lexical use,
/// issued by its pending request or actual reached-operation report. This is not cross-owner
/// producer or physical-frame authority.
#[derive(Clone)]
pub struct PositionGraphOperationLocator {
    captured: Arc<CapturedProgram>,
    uses: Vec<OriginalUse>,
    trigger: Vec<OriginalSource>,
    node: PositionSourceNode,
    kind: PositionGraphOperationKind,
}
impl PartialEq for PositionGraphOperationLocator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.captured, &other.captured)
            && self.uses == other.uses
            && self.trigger == other.trigger
            && self.node == other.node
            && self.kind == other.kind
    }
}
impl Eq for PositionGraphOperationLocator {}
impl Hash for PositionGraphOperationLocator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.captured).hash(state);
        self.uses.hash(state);
        self.trigger.hash(state);
        self.node.hash(state);
        self.kind.hash(state);
    }
}
impl std::fmt::Debug for PositionGraphOperationLocator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PositionGraphOperationLocator")
            .field("capture_id", &self.capture_id())
            .field("kind", &self.kind)
            .field("lexical_depth", &self.uses.len())
            .finish_non_exhaustive()
    }
}
impl PositionGraphOperationLocator {
    pub fn capture_id(&self) -> Uuid {
        self.captured.capture_id
    }
    pub fn kind(&self) -> PositionGraphOperationKind {
        self.kind
    }
    pub fn operation_node(&self) -> &PositionSourceNode {
        &self.node
    }
    pub fn source_node(&self) -> &PositionSourceNode {
        &self.node
    }
}
impl PositionProgramBranch {
    /// Validate identity and operation role before lending owned scratch. Reachability after
    /// branch conditioning is established by actual prefix replay, not this check.
    pub fn validate_graph_operation_operand(
        &self,
        locator: &PositionGraphOperationLocator,
        operand: PositionGraphOperationOperand,
    ) -> Result<(), TransitionError> {
        ensure(
            Arc::ptr_eq(&self.captured, &locator.captured),
            "Position graph operation belongs to another captured registry",
        )?;
        ensure(
            operand.supports(locator.kind),
            "Position graph operand has another operation role",
        )?;
        Ok(())
    }
}
impl PositionOriginBinding {
    /// Registry identity only; reachability is established by the actual bound replay.
    pub(crate) fn owns_graph_locator(&self, locator: &PositionGraphOperationLocator) -> bool {
        Arc::ptr_eq(&self.captured, &locator.captured)
    }
    pub(super) fn graph_identity(
        &self,
        route: &PreparedPositionGraphRoute,
    ) -> Result<Option<(Vec<OriginalUse>, Vec<OriginalSource>, PositionSourceNode)>, TransitionError>
    {
        let Some((uses, trigger, scope)) =
            self.lexical_route(&route.uses, &route.trigger_origins)?
        else {
            return Ok(None);
        };
        let [origin] = route.trigger_origins.as_slice() else {
            return Ok(None);
        };
        let (source, location) = match &route.expression {
            PreparedPositionGraphExpression::Whole(expression) => {
                let Some(node) = expression.retained_node_origin(route.node) else {
                    return Ok(None);
                };
                if let Some((slot, forest)) = scope {
                    if origin.member.is_some() {
                        return Ok(None);
                    }
                    (
                        self.source(slot)?,
                        NodeLocation::MemberTape(forest, origin.original_index, node),
                    )
                } else {
                    let Some(location) = self.prepared_whole_location(*origin, node)? else {
                        return Ok(None);
                    };
                    (self.source(origin.original_index)?, location)
                }
            }
            PreparedPositionGraphExpression::Coupled(expression) => {
                if scope.is_some() || origin.member.is_some() {
                    return Ok(None);
                }
                let Some(location) = Self::coupled_location(expression, route.node) else {
                    return Ok(None);
                };
                (self.source(origin.original_index)?, location)
            }
        };
        let Some(node) = self.handle(source, location)? else {
            return Ok(None);
        };
        Ok(Some((uses, trigger, node)))
    }
    pub(crate) fn graph_locator(
        &self,
        route: &PreparedPositionGraphRoute,
    ) -> Result<Option<PositionGraphOperationLocator>, TransitionError> {
        let Some((uses, trigger, node)) = self.graph_identity(route)? else {
            return Ok(None);
        };
        let kind = match (
            route.kind,
            nodes::describe(&self.captured, node.source_index, node.node)?,
        ) {
            (
                GraphOperationKind::Required,
                NodeDescription::Transition {
                    reason: DynamicTransitionReason::Required { .. },
                    ..
                },
            ) => PositionGraphOperationKind::Required,
            (GraphOperationKind::Size, NodeDescription::Scale { .. }) => {
                PositionGraphOperationKind::Size
            }
            _ => return Ok(None),
        };
        Ok(Some(PositionGraphOperationLocator {
            captured: self.captured.clone(),
            uses,
            trigger,
            node,
            kind,
        }))
    }
}
