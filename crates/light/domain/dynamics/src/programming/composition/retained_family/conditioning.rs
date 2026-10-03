//! Branch-local views of the original source stack. Indices, source ranks and projected
//! fragment identities remain stable; no destination fixture can supply a native model.
//! This reference path recompiles changed trees and is not a hot-frame allocation guarantee.
use super::*;
use crate::{DynamicNativeModelResolver, DynamicTransitionReason};
use light_core::NativeColorIdentity;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BranchDecision {
    pub instance_id: Uuid,
    pub controller_id: Uuid,
    pub occurrence_id: Uuid,
    pub incoming: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct BranchState {
    pub decisions: Vec<BranchDecision>,
    pub overrides: Vec<(usize, Option<Arc<DynamicSampleExpression>>)>,
}

impl BranchState {
    pub fn choose(
        &mut self,
        source_rank: FamilySampleRank,
        occurrence_id: Uuid,
        incoming: bool,
    ) -> Result<(), TransitionError> {
        ensure(!occurrence_id.is_nil(), "branch occurrence must not be nil")?;
        let identity = source_rank.dynamic_identity().ok_or_else(|| {
            IntentError("Resume branch requires a Dynamic source identity".into())
        })?;
        if let Some(existing) = self.decisions.iter().find(|decision| {
            decision.instance_id == identity.instance_id
                && decision.controller_id == identity.controller_id
                && decision.occurrence_id == occurrence_id
        }) {
            ensure(
                existing.incoming == incoming,
                "conflicting choices for one retained Resume occurrence",
            )?;
        } else {
            self.decisions.push(BranchDecision {
                instance_id: identity.instance_id,
                controller_id: identity.controller_id,
                occurrence_id,
                incoming,
            });
        }
        Ok(())
    }

    /// Nested queries replace an earlier override at the same original source index.
    pub fn replace(&mut self, index: usize, node: Option<Arc<DynamicSampleExpression>>) {
        if let Some((_, current)) = self.overrides.iter_mut().find(|(key, _)| *key == index) {
            *current = node;
        } else {
            self.overrides.push((index, node));
        }
    }

    fn validate(&self, len: usize) -> Result<(), TransitionError> {
        let mut checked = Self::default();
        for decision in &self.decisions {
            let rank = FamilySampleRank {
                priority: 0,
                changed_at_millis: 0,
                changed_at_submillis_nanos: 0,
                stable_order: 0,
                identity: crate::FamilySampleIdentity::Dynamic {
                    instance_id: decision.instance_id,
                    controller_id: decision.controller_id,
                    lane_id: Uuid::nil(),
                },
            };
            checked.choose(rank, decision.occurrence_id, decision.incoming)?;
        }
        for (offset, (index, node)) in self.overrides.iter().enumerate() {
            ensure(
                *index < len,
                "branch override source index is out of bounds",
            )?;
            ensure(
                !self.overrides[..offset].iter().any(|(key, _)| key == index),
                "branch source has duplicate overrides",
            )?;
            if let Some(node) = node {
                node.validate()?;
            }
        }
        Ok(())
    }

    fn choice(&self, rank: FamilySampleRank, occurrence_id: Uuid) -> Option<bool> {
        let identity = rank.dynamic_identity()?;
        self.decisions
            .iter()
            .find(|decision| {
                decision.instance_id == identity.instance_id
                    && decision.controller_id == identity.controller_id
                    && decision.occurrence_id == occurrence_id
            })
            .map(|decision| decision.incoming)
    }
}

/// Entries never move: a released/absent source becomes `None` at its original index.
pub(super) fn condition_sources(
    samples: &[FamilyCompositionSample],
    state: &BranchState,
    _base: &AttributeValue,
) -> Result<Vec<Option<FamilyCompositionSample>>, TransitionError> {
    state.validate(samples.len())?;
    validate_resume_progress(samples)?;
    let models = OriginalModels(samples);
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let replacement = state.overrides.iter().find(|(key, _)| *key == index);
            let root = match replacement {
                Some((_, None)) => return Ok(None),
                Some((_, Some(root))) => root.clone(),
                None => match sample {
                    FamilyCompositionSample::Known(known) => match &known.body {
                        FamilySampleBody::Materialized(_) => return Ok(Some(sample.clone())),
                        FamilySampleBody::ComponentExpression(expression) => {
                            Arc::new(expression.expression().clone())
                        }
                    },
                    FamilyCompositionSample::WholeExpression { expression, .. } => {
                        Arc::new(expression.expression().clone())
                    }
                    FamilyCompositionSample::CoupledExpression { expression, .. } => expression
                        .expression()
                        .cloned()
                        .ok_or(TransitionError::Requires(
                            TransitionRequirement::CompatibleOwners,
                        ))?,
                },
            };
            let Some(conditioned) = condition_node(&root, sample.rank(), state)? else {
                return Ok(None);
            };
            // Reuse the cold compiled source when neither its root nor a branch changed.
            if replacement.is_none() && Arc::ptr_eq(&root, &conditioned) {
                return Ok(Some(sample.clone()));
            }
            reconstruct(sample, conditioned, &models).map(Some)
        })
        .collect()
}

/// One Resume occurrence describes one simultaneous source-membership change. Validate before
/// conditioning removes those nodes, using the actual active projected trees. Exact endpoints
/// and zero Size make their unused history unreachable and must not revive it for validation.
pub(super) fn validate_resume_progress(
    samples: &[FamilyCompositionSample],
) -> Result<(), TransitionError> {
    let mut occurrences = HashMap::new();
    for sample in samples {
        let root = match sample {
            FamilyCompositionSample::Known(known) if known.participates() => match &known.body {
                FamilySampleBody::ComponentExpression(expression) => expression.expression(),
                FamilySampleBody::Materialized(_) => continue,
            },
            FamilyCompositionSample::WholeExpression {
                expression,
                activation_mix,
                ..
            } if *activation_mix > 0.0 && expression.participates() => expression.expression(),
            FamilyCompositionSample::CoupledExpression {
                expression,
                activation_mix,
                ..
            } if *activation_mix > 0.0 && !expression.roles().is_empty() => expression
                .expression()
                .map(AsRef::as_ref)
                .ok_or(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ))?,
            _ => continue,
        };
        let rank = sample.rank();
        use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
        for node in ExpressionNodeRef::new(root).postorder(true)? {
            if let ExpressionNode::Transition {
                progress, reason, ..
            } = node.node()?
            {
                ensure(
                    progress.is_finite() && (0.0..=1.0).contains(&progress),
                    "retained transition progress must be finite and between zero and one",
                )?;
                if let DynamicTransitionReason::Resume { occurrence_id } = reason {
                    let identity = rank.dynamic_identity().ok_or_else(|| {
                        IntentError("Resume branch requires a Dynamic source identity".into())
                    })?;
                    let key = (identity.instance_id, identity.controller_id, occurrence_id);
                    if let Some(existing) = occurrences.insert(key, progress) {
                        ensure(
                            existing == progress,
                            "one scoped Resume occurrence has different progress",
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn condition_node(
    node: &Arc<DynamicSampleExpression>,
    rank: FamilySampleRank,
    state: &BranchState,
) -> Result<Option<Arc<DynamicSampleExpression>>, TransitionError> {
    use crate::RetainedExpressionTape;
    let tape = Arc::new(RetainedExpressionTape::from_roots(&[node.clone()])?);
    let result = condition_tape_node(&tape, tape.roots[0], rank, state)?;
    Ok(if result.changed {
        result.expression
    } else {
        Some(node.clone())
    })
}

pub(super) struct ConditionedTapeNode {
    pub expression: Option<Arc<DynamicSampleExpression>>,
    /// Imported output tape node -> exact input tape node. Synthetic release/baseline
    /// helpers have no retained-operation origin.
    pub origins: Vec<Option<crate::RetainedNodeId>>,
    pub changed: bool,
}

pub(super) fn condition_tape_node(
    source: &Arc<crate::RetainedExpressionTape>,
    original_root: crate::RetainedNodeId,
    rank: FamilySampleRank,
    state: &BranchState,
) -> Result<ConditionedTapeNode, TransitionError> {
    condition_tape_node_with_local(source, original_root, rank, state, &[])
}
pub(super) fn condition_tape_node_with_local(
    source: &Arc<crate::RetainedExpressionTape>,
    original_root: crate::RetainedNodeId,
    rank: FamilySampleRank,
    state: &BranchState,
    local: &[(
        crate::RetainedNodeId,
        super::position_conditioning::PositionLocalEndpoint,
    )],
) -> Result<ConditionedTapeNode, TransitionError> {
    use crate::RetainedNodeId;
    source.validate()?;
    validate_local_endpoints(source, local)?;
    ensure(
        source.node(original_root).is_some(),
        "conditioned Position source root is absent",
    )?;
    let tape = source.as_ref().clone();
    let origins = (0..tape.nodes.len())
        .map(|id| Some(RetainedNodeId(id as u32)))
        .collect::<Vec<_>>();
    let count = tape.nodes.len();
    let mapped = Vec::<Option<RetainedNodeId>>::with_capacity(count);
    let mut conditioning = TapeConditioning {
        tape,
        origins,
        mapped,
        local,
        rank,
        state,
    };
    for index in 0..count {
        let result = conditioning.condition(index)?;
        conditioning.mapped.push(result);
    }
    let TapeConditioning {
        tape,
        origins,
        mapped,
        ..
    } = conditioning;
    let Some(root) = mapped[original_root.0 as usize] else {
        return Ok(ConditionedTapeNode {
            expression: None,
            origins: Vec::new(),
            changed: true,
        });
    };
    import_conditioned(tape, &origins, root, original_root)
}

fn validate_local_endpoints(
    source: &crate::RetainedExpressionTape,
    local: &[(
        crate::RetainedNodeId,
        super::position_conditioning::PositionLocalEndpoint,
    )],
) -> Result<(), TransitionError> {
    use super::position_conditioning::PositionLocalEndpoint as E;
    use crate::RetainedExpressionNode as N;
    for &(node, endpoint) in local {
        ensure(
            matches!(
                (source.node(node), endpoint),
                (
                    Some(N::Transition {
                        reason: DynamicTransitionReason::Required { .. },
                        ..
                    }),
                    E::RequiredOutgoing | E::RequiredIncoming
                ) | (Some(N::Scale { .. }), E::ScaleBaseline | E::ScaleValue)
            ),
            "local Position tape endpoint has no matching original operation",
        )?;
    }
    Ok(())
}

/// The working copy of one source tape while its nodes are conditioned in input order.
struct TapeConditioning<'a> {
    tape: crate::RetainedExpressionTape,
    origins: Vec<Option<crate::RetainedNodeId>>,
    mapped: Vec<Option<crate::RetainedNodeId>>,
    local: &'a [(
        crate::RetainedNodeId,
        super::position_conditioning::PositionLocalEndpoint,
    )],
    rank: FamilySampleRank,
    state: &'a BranchState,
}

impl TapeConditioning<'_> {
    fn append(
        &mut self,
        node: crate::RetainedExpressionNode,
        origin: Option<crate::RetainedNodeId>,
    ) -> Result<crate::RetainedNodeId, IntentError> {
        let id = crate::RetainedNodeId(
            u32::try_from(self.tape.nodes.len())
                .map_err(|_| IntentError("conditioned expression has too many nodes".into()))?,
        );
        self.tape.nodes.push(node);
        self.origins.push(origin);
        Ok(id)
    }

    fn local_endpoint(
        &self,
        original: crate::RetainedNodeId,
    ) -> Option<super::position_conditioning::PositionLocalEndpoint> {
        self.local
            .iter()
            .find(|(node, _)| *node == original)
            .map(|(_, endpoint)| *endpoint)
    }

    fn condition(
        &mut self,
        index: usize,
    ) -> Result<Option<crate::RetainedNodeId>, TransitionError> {
        use crate::{RetainedExpressionNode as N, RetainedNodeId};
        let original = RetainedNodeId(index as u32);
        match self.tape.nodes[index].clone() {
            N::Transition {
                from,
                to,
                progress,
                reason,
            } => self.condition_transition(original, from, to, progress, reason),
            N::Scale {
                address,
                base,
                value,
                factor,
                baseline_occurrence,
            } => self.condition_scale(original, address, base, value, factor, baseline_occurrence),
            _ => Ok(Some(original)),
        }
    }

    fn condition_transition(
        &mut self,
        original: crate::RetainedNodeId,
        from: Option<crate::RetainedNodeId>,
        to: Option<crate::RetainedNodeId>,
        progress: f32,
        reason: DynamicTransitionReason,
    ) -> Result<Option<crate::RetainedNodeId>, TransitionError> {
        use super::position_conditioning::PositionLocalEndpoint as E;
        use crate::RetainedExpressionNode as N;
        let local_endpoint = self.local_endpoint(original);
        let selected = match local_endpoint {
            Some(E::RequiredOutgoing) => Some(false),
            Some(E::RequiredIncoming) => Some(true),
            _ => match reason {
                DynamicTransitionReason::Resume { occurrence_id } => {
                    self.state.choice(self.rank, occurrence_id)
                }
                _ => None,
            },
        }
        .or(if progress == 0.0 {
            Some(false)
        } else if progress == 1.0 {
            Some(true)
        } else {
            None
        });
        let next_from = from.and_then(|id| self.mapped[id.0 as usize]);
        let next_to = to.and_then(|id| self.mapped[id.0 as usize]);
        Ok(if let Some(incoming) = selected {
            if incoming { next_to } else { next_from }
        } else if next_from.is_none() && next_to.is_none() {
            None
        } else if from == next_from && to == next_to {
            Some(original)
        } else {
            Some(self.append(
                N::Transition {
                    from: next_from,
                    to: next_to,
                    progress,
                    reason,
                },
                Some(original),
            )?)
        })
    }

    fn condition_scale(
        &mut self,
        original: crate::RetainedNodeId,
        address: DynamicValueAddress,
        base: DynamicValue,
        value: crate::RetainedNodeId,
        factor: f32,
        baseline_occurrence: Option<crate::DynamicSourceOccurrenceId>,
    ) -> Result<Option<crate::RetainedNodeId>, TransitionError> {
        use super::position_conditioning::PositionLocalEndpoint as E;
        use crate::RetainedExpressionNode as N;
        let local_endpoint = self.local_endpoint(original);
        Ok(
            if factor == 0.0 || local_endpoint == Some(E::ScaleBaseline) {
                let baseline_address = match &base {
                    DynamicValue::Family(family) => {
                        DynamicValueAddress::whole_family(address.owner(), family)?
                    }
                    _ if local_endpoint == Some(E::ScaleBaseline) => address.clone(),
                    _ => return Err(IntentError("invalid whole Size baseline".into()).into()),
                };
                Some(self.append(
                    N::Programming {
                        address: baseline_address,
                        value: base,
                        occurrence: None,
                        dependency_occurrence: Some(crate::DynamicSourceDependency::identity(
                            baseline_occurrence,
                        )),
                    },
                    None,
                )?)
            } else if factor == 1.0 || local_endpoint == Some(E::ScaleValue) {
                self.mapped[value.0 as usize]
            } else {
                let next = if let Some(next) = self.mapped[value.0 as usize] {
                    next
                } else {
                    // Keep an exact released child with the original child's declared
                    // address. Its unreachable endpoint is never evaluated or model-resolved.
                    self.append(
                        N::Transition {
                            from: Some(value),
                            to: None,
                            progress: 1.0,
                            reason: DynamicTransitionReason::Required {
                                requirement: TransitionRequirement::MaterializedEndpoints,
                            },
                        },
                        None,
                    )?
                };
                if next == value {
                    Some(original)
                } else {
                    Some(self.append(
                        N::Scale {
                            address,
                            base,
                            value: next,
                            factor,
                            baseline_occurrence,
                        },
                        Some(original),
                    )?)
                }
            },
        )
    }
}

/// Normalize the conditioned tape and map each imported node back to its exact input node.
fn import_conditioned(
    tape: crate::RetainedExpressionTape,
    origins: &[Option<crate::RetainedNodeId>],
    root: crate::RetainedNodeId,
    original_root: crate::RetainedNodeId,
) -> Result<ConditionedTapeNode, TransitionError> {
    use crate::RetainedExpressionTape;
    let changed = root != original_root;
    let expression = Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    });
    let (normalized, keys) =
        RetainedExpressionTape::from_roots_with_source_keys(std::slice::from_ref(&expression))?;
    let DynamicSampleExpression::Retained { tape: raw, .. } = expression.as_ref() else {
        unreachable!()
    };
    let raw_identity = Arc::as_ptr(raw) as usize;
    let origins = keys
        .into_iter()
        .map(|(identity, node)| {
            if identity != raw_identity {
                return Err(IntentError(
                    "conditioned tape import has foreign source identity".into(),
                ));
            }
            node.and_then(|node| origins.get(node.0 as usize).copied().flatten())
                .map(Some)
                .map_or(Ok(None), Ok)
        })
        .collect::<Result<Vec<_>, IntentError>>()?;
    let root = normalized.roots[0];
    Ok(ConditionedTapeNode {
        expression: Some(Arc::new(DynamicSampleExpression::Retained {
            tape: Arc::new(normalized),
            root,
        })),
        origins,
        changed,
    })
}

pub(super) fn reconstruct(
    sample: &FamilyCompositionSample,
    root: Arc<DynamicSampleExpression>,
    models: &OriginalModels<'_>,
) -> Result<FamilyCompositionSample, TransitionError> {
    let (rank, activation_mix, owner) = match sample {
        FamilyCompositionSample::Known(known) => {
            // A FixAT mask cannot become a retained transition or change its footprint.
            if known.fix_at {
                let root = root.shallow()?;
                let DynamicSampleExpression::Programming { address, value, .. } = &root else {
                    return Err(IntentError(
                        "cannot replace a FixAT mask with a retained tree".into(),
                    )
                    .into());
                };
                ensure(
                    address.as_ref() == known.address.address(),
                    "conditioned FixAT mask changed its address",
                )?;
                known.address.validate_source_value(value)?;
                let mut conditioned = known.clone();
                conditioned.body = FamilySampleBody::Materialized(value.clone());
                return Ok(FamilyCompositionSample::Known(conditioned));
            }
            if known.address.address().component.is_some() {
                let expression = Arc::new(CompiledComponentExpression::new(
                    root,
                    known.address.clone(),
                )?);
                let mut conditioned = known.clone();
                conditioned.body = FamilySampleBody::ComponentExpression(expression);
                return Ok(FamilyCompositionSample::Known(conditioned));
            }
            (
                known.rank,
                known.activation_mix,
                known.address.address().owner(),
            )
        }
        FamilyCompositionSample::WholeExpression {
            expression,
            rank,
            activation_mix,
        } => {
            if only_whole(&root) {
                return Ok(FamilyCompositionSample::WholeExpression {
                    expression: Arc::new(expression.recompile_conditioned(root, models)?),
                    rank: *rank,
                    activation_mix: *activation_mix,
                });
            }
            (*rank, *activation_mix, expression.owner())
        }
        FamilyCompositionSample::CoupledExpression {
            expression,
            rank,
            activation_mix,
        } => (*rank, *activation_mix, expression.owner()),
    };
    let expression = Arc::new(CompiledCoupledExpression::new(root, Some(models))?);
    ensure(
        expression.owner() == owner,
        "conditioned source changed its programming owner",
    )?;
    Ok(FamilyCompositionSample::CoupledExpression {
        expression,
        rank,
        activation_mix,
    })
}

fn only_whole(node: &DynamicSampleExpression) -> bool {
    use crate::programming::expression::{ExpressionNode, ExpressionNodeRef};
    ExpressionNodeRef::new(node)
        .postorder(false)
        .is_ok_and(|nodes| {
            nodes.into_iter().all(|node| match node.node() {
                Ok(
                    ExpressionNode::Programming(address, ..)
                    | ExpressionNode::Scale { address, .. },
                ) => address.component.is_none(),
                Ok(ExpressionNode::Transition { .. }) => true,
                _ => false,
            })
        })
}

pub(super) struct OriginalModels<'a>(pub(super) &'a [FamilyCompositionSample]);

impl DynamicNativeModelResolver for OriginalModels<'_> {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        for sample in self.0 {
            let model = match sample {
                FamilyCompositionSample::Known(known) => known.address.native_model(),
                FamilyCompositionSample::WholeExpression { expression, .. } => {
                    expression.resolve_original_native_model(source).ok()
                }
                FamilyCompositionSample::CoupledExpression { expression, .. } => {
                    expression.resolve_original_native_model(source).ok()
                }
            };
            if let Some(model) = model
                && model.source() == source
            {
                return Ok(model);
            }
        }
        Err(IntentError(
            "conditioned source has no pinned original native model".into(),
        ))
    }
}

#[cfg(test)]
mod tests;
