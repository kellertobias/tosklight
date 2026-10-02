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
    use super::position_conditioning::PositionLocalEndpoint as E;
    use crate::{RetainedExpressionNode as N, RetainedExpressionTape, RetainedNodeId};
    source.validate()?;
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
    ensure(
        source.node(original_root).is_some(),
        "conditioned Position source root is absent",
    )?;
    let mut tape = source.as_ref().clone();
    let mut origins = (0..tape.nodes.len())
        .map(|id| Some(RetainedNodeId(id as u32)))
        .collect::<Vec<_>>();
    let count = tape.nodes.len();
    let mut mapped = Vec::<Option<RetainedNodeId>>::with_capacity(count);
    let append = |tape: &mut RetainedExpressionTape,
                  origins: &mut Vec<Option<RetainedNodeId>>,
                  node,
                  origin|
     -> Result<RetainedNodeId, IntentError> {
        let id = RetainedNodeId(
            u32::try_from(tape.nodes.len())
                .map_err(|_| IntentError("conditioned expression has too many nodes".into()))?,
        );
        tape.nodes.push(node);
        origins.push(origin);
        Ok(id)
    };
    for index in 0..count {
        let original = RetainedNodeId(index as u32);
        let result = match tape.nodes[index].clone() {
            N::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let local_endpoint = local
                    .iter()
                    .find(|(node, _)| *node == original)
                    .map(|(_, endpoint)| *endpoint);
                let selected = match local_endpoint {
                    Some(E::RequiredOutgoing) => Some(false),
                    Some(E::RequiredIncoming) => Some(true),
                    _ => match reason {
                        DynamicTransitionReason::Resume { occurrence_id } => {
                            state.choice(rank, occurrence_id)
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
                let next_from = from.and_then(|id| mapped[id.0 as usize]);
                let next_to = to.and_then(|id| mapped[id.0 as usize]);
                if let Some(incoming) = selected {
                    if incoming { next_to } else { next_from }
                } else if next_from.is_none() && next_to.is_none() {
                    None
                } else if from == next_from && to == next_to {
                    Some(original)
                } else {
                    Some(append(
                        &mut tape,
                        &mut origins,
                        N::Transition {
                            from: next_from,
                            to: next_to,
                            progress,
                            reason,
                        },
                        Some(original),
                    )?)
                }
            }
            N::Scale {
                address,
                base,
                value,
                factor,
                baseline_occurrence,
            } => {
                let local_endpoint = local
                    .iter()
                    .find(|(node, _)| *node == original)
                    .map(|(_, endpoint)| *endpoint);
                if factor == 0.0 || local_endpoint == Some(E::ScaleBaseline) {
                    let baseline_address = match &base {
                        DynamicValue::Family(family) => {
                            DynamicValueAddress::whole_family(address.owner(), family)?
                        }
                        _ if local_endpoint == Some(E::ScaleBaseline) => address.clone(),
                        _ => return Err(IntentError("invalid whole Size baseline".into()).into()),
                    };
                    Some(append(
                        &mut tape,
                        &mut origins,
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
                    mapped[value.0 as usize]
                } else {
                    let next = if let Some(next) = mapped[value.0 as usize] {
                        next
                    } else {
                        // Keep an exact released child with the original child's declared
                        // address. Its unreachable endpoint is never evaluated or model-resolved.
                        append(
                            &mut tape,
                            &mut origins,
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
                        Some(append(
                            &mut tape,
                            &mut origins,
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
                }
            }
            _ => Some(original),
        };
        mapped.push(result);
    }
    let Some(root) = mapped[original_root.0 as usize] else {
        return Ok(ConditionedTapeNode {
            expression: None,
            origins: Vec::new(),
            changed: true,
        });
    };
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
mod tests {
    use super::*;
    use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};

    fn rank(controller: u128, lane: u128) -> FamilySampleRank {
        FamilySampleRank {
            priority: 3,
            changed_at_millis: 17,
            changed_at_submillis_nanos: 0,
            stable_order: lane,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(1),
                controller_id: Uuid::from_u128(controller),
                lane_id: Uuid::from_u128(lane),
            },
        }
    }

    fn color(component: ColorComponent, value: f32) -> Arc<DynamicSampleExpression> {
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::SemanticColor {
                    basis: DynamicSemanticColorBasis::Retain,
                },
                component: Some(ProgrammingComponent::Color(component)),
            }),
            value: DynamicValue::Scalar(value),
            occurrence: None,
            dependency_occurrence: None,
        })
    }

    fn focus(value: f32) -> Arc<DynamicSampleExpression> {
        let value = AttributeValue::Normalized(value);
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &value).unwrap(),
            ),
            value: DynamicValue::Family(value),
            occurrence: None,
            dependency_occurrence: None,
        })
    }

    fn resume(
        from: Option<Arc<DynamicSampleExpression>>,
        to: Option<Arc<DynamicSampleExpression>>,
        occurrence_id: Uuid,
    ) -> Arc<DynamicSampleExpression> {
        Arc::new(DynamicSampleExpression::Transition {
            from,
            to,
            progress: 0.4,
            reason: DynamicTransitionReason::Resume { occurrence_id },
        })
    }

    fn progress(
        mut node: Arc<DynamicSampleExpression>,
        value: f32,
    ) -> Arc<DynamicSampleExpression> {
        let DynamicSampleExpression::Transition { progress, .. } = Arc::make_mut(&mut node) else {
            panic!()
        };
        *progress = value;
        node
    }

    fn semantic_leaf(uv: f32) -> Arc<DynamicSampleExpression> {
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                uv: UvIntent { amount: uv },
                ..Default::default()
            },
        }));
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
            ),
            value: DynamicValue::Family(value),
            occurrence: None,
            dependency_occurrence: None,
        })
    }

    fn color_whole(
        root: Arc<DynamicSampleExpression>,
        source_rank: FamilySampleRank,
    ) -> FamilyCompositionSample {
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Color, None, None)
                    .unwrap(),
            ),
            rank: source_rank,
            activation_mix: 1.0,
        }
    }

    fn whole(
        root: Arc<DynamicSampleExpression>,
        rank: FamilySampleRank,
    ) -> FamilyCompositionSample {
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Focus, None, None)
                    .unwrap(),
            ),
            rank,
            activation_mix: 0.7,
        }
    }

    #[test]
    fn choices_are_idempotent_and_conflicts_do_not_change_state() {
        let occurrence = Uuid::from_u128(40);
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        state.choose(rank(2, 4), occurrence, true).unwrap();
        assert_eq!(state.decisions.len(), 1);
        assert!(state.choose(rank(2, 5), occurrence, false).is_err());
        assert!(state.decisions[0].incoming);
        state.choose(rank(9, 3), occurrence, false).unwrap();
        assert_eq!(state.decisions.len(), 2);
        state.replace(0, Some(focus(0.2)));
        state.replace(0, None);
        assert_eq!(state.overrides.len(), 1);
        assert!(state.overrides[0].1.is_none());
    }

    #[test]
    fn synchronized_projected_fragments_keep_masks_identity_and_original_indices() {
        let occurrence = Uuid::from_u128(40);
        let root = resume(
            Some(color(ColorComponent::Uv, 0.8)),
            Some(color(ColorComponent::WhiteBlend, 0.2)),
            occurrence,
        );
        let set = CompiledComponentExpressionSet::new(root.clone(), None).unwrap();
        let samples = FamilySample::retained_components(&set, rank(2, 3), 0.7)
            .unwrap()
            .into_iter()
            .map(FamilyCompositionSample::Known)
            .collect::<Vec<_>>();
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let result = condition_sources(&samples, &state, &AttributeValue::Normalized(0.0)).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].is_none());
        let FamilyCompositionSample::Known(white) = result[1].as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(white.rank, rank(2, 3));
        assert_eq!(white.activation_mix, 0.7);
        assert_eq!(
            white.address.address().component,
            Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend))
        );
        assert!(Arc::ptr_eq(white.projection.as_ref().unwrap(), &root));
        let FamilySampleBody::ComponentExpression(compiled) = &white.body else {
            panic!()
        };
        assert_eq!(
            compiled.evaluate(None).unwrap(),
            Some(DynamicValue::Scalar(0.2))
        );
    }

    #[test]
    fn choices_match_instance_controller_and_occurrence_but_keep_required_progress() {
        let occurrence = Uuid::from_u128(40);
        let root = resume(Some(focus(0.2)), Some(focus(0.8)), occurrence);
        let mut other_instance = rank(2, 5);
        let FamilySampleIdentity::Dynamic { instance_id, .. } = &mut other_instance.identity else {
            unreachable!()
        };
        *instance_id = Uuid::from_u128(99);
        let required = Arc::new(DynamicSampleExpression::Transition {
            from: Some(focus(0.3)),
            to: Some(focus(0.7)),
            progress: 0.25,
            reason: DynamicTransitionReason::Required {
                requirement: TransitionRequirement::MaterializedEndpoints,
            },
        });
        let samples = [
            whole(root.clone(), rank(2, 3)),
            whole(root.clone(), rank(9, 4)),
            whole(root, other_instance),
            whole(required.clone(), rank(2, 6)),
        ];
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let result = condition_sources(&samples, &state, &AttributeValue::Normalized(0.1)).unwrap();
        let FamilyCompositionSample::WholeExpression {
            expression,
            rank: actual_rank,
            activation_mix,
        } = result[0].as_ref().unwrap()
        else {
            panic!()
        };
        assert_eq!(expression.expression(), focus(0.8).as_ref());
        assert_eq!(*actual_rank, rank(2, 3));
        assert_eq!(*activation_mix, 0.7);
        for index in 1..=3 {
            let FamilyCompositionSample::WholeExpression {
                expression: old, ..
            } = &samples[index]
            else {
                panic!()
            };
            let FamilyCompositionSample::WholeExpression {
                expression: new, ..
            } = result[index].as_ref().unwrap()
            else {
                panic!()
            };
            assert!(Arc::ptr_eq(old, new));
        }
    }

    #[test]
    fn override_precedes_decisions_and_whole_to_component_stays_narrow() {
        let value = AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [0.0; 3],
        )));
        let root = Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
            ),
            value: DynamicValue::Family(value.clone()),
            occurrence: None,
            dependency_occurrence: None,
        });
        let samples = [FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                CompiledProgrammingFamilyExpression::new(
                    root,
                    ProgrammingOwner::Position,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: rank(2, 3),
            activation_mix: 0.7,
        }];
        let address = Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        });
        let point_x = Arc::new(DynamicSampleExpression::Programming {
            address: address.clone(),
            value: DynamicValue::Scalar(2.0),
            occurrence: None,
            dependency_occurrence: None,
        });
        let occurrence = Uuid::from_u128(40);
        let mut state = BranchState::default();
        state.replace(0, Some(resume(None, Some(point_x), occurrence)));
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let result = condition_sources(&samples, &state, &value).unwrap();
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            result[0].as_ref().unwrap()
        else {
            panic!()
        };
        let crate::CoupledExpressionFootprint::Exact {
            address: actual,
            value,
        } = expression.footprint(crate::CoupledExpressionRole::Base)
        else {
            panic!()
        };
        assert_eq!(actual.address(), address.as_ref());
        assert_eq!(*value, DynamicValue::Scalar(2.0));
    }

    struct NoFrame;
    impl WholeFamilyExpressionFrameResolver for NoFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            Err(TransitionError::Requires(requirement))
        }
    }

    #[test]
    fn released_scale_child_uses_underlay_and_keeps_real_size_baseline() {
        let occurrence = Uuid::from_u128(40);
        let baseline = AttributeValue::Normalized(0.2);
        let root = Arc::new(DynamicSampleExpression::Scale {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &baseline).unwrap(),
            ),
            base: DynamicValue::Family(baseline),
            value: resume(Some(focus(0.4)), None, occurrence),
            factor: 2.0,
            baseline_occurrence: None,
        });
        let samples = [whole(root, rank(2, 3))];
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let underlay = AttributeValue::Normalized(0.5);
        let result = condition_sources(&samples, &state, &underlay).unwrap();
        let FamilyCompositionSample::WholeExpression { expression, .. } =
            result[0].as_ref().unwrap()
        else {
            panic!()
        };
        let AttributeValue::Normalized(value) = expression.evaluate(&underlay, &NoFrame).unwrap()
        else {
            panic!()
        };
        assert!((value - 0.8).abs() < 0.00001);
    }

    #[test]
    fn scoped_progress_mismatch_is_rejected_before_a_choice_removes_resume_nodes() {
        let occurrence = Uuid::from_u128(40);
        let whole = color_whole(
            resume(
                Some(semantic_leaf(0.2)),
                Some(semantic_leaf(0.8)),
                occurrence,
            ),
            rank(2, 3),
        );
        let projected = CompiledComponentExpressionSet::new(
            progress(
                resume(
                    Some(color(ColorComponent::Uv, 0.2)),
                    Some(color(ColorComponent::WhiteBlend, 0.8)),
                    occurrence,
                ),
                0.7,
            ),
            None,
        )
        .unwrap();
        let mut samples = vec![whole.clone()];
        samples.extend(
            FamilySample::retained_components(&projected, rank(2, 4), 1.0)
                .unwrap()
                .into_iter()
                .map(FamilyCompositionSample::Known),
        );
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        assert!(matches!(
            condition_sources(&samples, &state, &AttributeValue::Normalized(0.0)),
            Err(TransitionError::Invalid(IntentError(message))) if message.contains("different progress")
        ));

        let coupled = FamilyCompositionSample::CoupledExpression {
            expression: Arc::new(
                CompiledCoupledExpression::new(
                    progress(
                        resume(
                            Some(semantic_leaf(0.5)),
                            Some(color(ColorComponent::Uv, 0.8)),
                            occurrence,
                        ),
                        0.7,
                    ),
                    None,
                )
                .unwrap(),
            ),
            rank: rank(2, 5),
            activation_mix: 1.0,
        };
        assert!(validate_resume_progress(&[whole, coupled]).is_err());
    }

    #[test]
    fn resume_progress_is_independent_between_instances_controllers_and_occurrences() {
        let occurrence = Uuid::from_u128(40);
        for different_scope in 0..3 {
            let mut other_rank = rank(2, 4);
            let mut other_occurrence = occurrence;
            let FamilySampleIdentity::Dynamic {
                instance_id,
                controller_id,
                ..
            } = &mut other_rank.identity
            else {
                unreachable!()
            };
            match different_scope {
                0 => *instance_id = Uuid::from_u128(90),
                1 => *controller_id = Uuid::from_u128(91),
                _ => other_occurrence = Uuid::from_u128(92),
            }
            let samples = [
                whole(
                    resume(Some(focus(0.2)), Some(focus(0.8)), occurrence),
                    rank(2, 3),
                ),
                whole(
                    progress(
                        resume(Some(focus(0.3)), Some(focus(0.7)), other_occurrence),
                        0.7,
                    ),
                    other_rank,
                ),
            ];
            validate_resume_progress(&samples).unwrap();
        }
    }

    #[test]
    fn inactive_history_does_not_create_a_resume_progress_conflict() {
        let occurrence = Uuid::from_u128(40);
        let unused = progress(resume(Some(focus(0.3)), Some(focus(0.7)), occurrence), 0.9);
        let exact = Arc::new(DynamicSampleExpression::Transition {
            from: Some(focus(0.2)),
            to: Some(unused.clone()),
            progress: 0.0,
            reason: DynamicTransitionReason::Required {
                requirement: TransitionRequirement::MaterializedEndpoints,
            },
        });
        let zero_size = Arc::new(DynamicSampleExpression::Scale {
            address: Arc::new(
                DynamicValueAddress::whole_family(
                    ProgrammingOwner::Focus,
                    &AttributeValue::Normalized(0.2),
                )
                .unwrap(),
            ),
            base: DynamicValue::Family(AttributeValue::Normalized(0.2)),
            value: unused.clone(),
            factor: 0.0,
            baseline_occurrence: None,
        });
        let mut inactive = whole(unused, rank(2, 6));
        let FamilyCompositionSample::WholeExpression { activation_mix, .. } = &mut inactive else {
            panic!()
        };
        *activation_mix = 0.0;
        let samples = [
            whole(
                resume(Some(focus(0.2)), Some(focus(0.8)), occurrence),
                rank(2, 3),
            ),
            whole(exact, rank(2, 4)),
            whole(zero_size, rank(2, 5)),
            inactive,
        ];
        condition_sources(
            &samples,
            &BranchState::default(),
            &AttributeValue::Normalized(0.0),
        )
        .unwrap();
    }

    struct NativeModel(NativeColorIdentity);
    impl NativeColorEditModel for NativeModel {
        fn source(&self) -> &NativeColorIdentity {
            &self.0
        }
        fn descriptor(&self, _: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
            None
        }
        fn predict(
            &self,
            recipe: &NativeColorRecipe,
        ) -> Result<PortableColorEstimate, IntentError> {
            ensure(recipe.source == self.0, "different original source")?;
            Ok(PortableColorEstimate {
                model_revision: self.0.model_revision,
                visible: None,
                uv: None,
                quality: PhysicalDataQuality::Unknown,
                limitations: vec![],
            })
        }
    }
    struct NativeModels(Arc<NativeModel>);
    impl DynamicNativeModelResolver for NativeModels {
        fn resolve(
            &self,
            source: &NativeColorIdentity,
        ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
            ensure(source == &self.0.0, "original source unavailable")?;
            Ok(self.0.clone())
        }
    }
    fn native_base() -> (NativeModels, AttributeValue) {
        let source = NativeColorIdentity {
            profile_id: Uuid::from_u128(51),
            profile_revision: 1,
            profile_digest: "pinned".into(),
            mode_id: Uuid::from_u128(52),
            head_id: Uuid::from_u128(53),
            path_id: Uuid::from_u128(54),
            model_revision: 1,
            native_layout_signature: "one-u32".into(),
        };
        let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: source.clone(),
                channels: vec![NativeColorValue {
                    channel_id: Uuid::from_u128(55),
                    function_id: Uuid::from_u128(56),
                    raw: u32::MAX - 1,
                }],
                spreads: vec![],
            },
            portable: PortableColorEstimate {
                model_revision: 1,
                visible: None,
                uv: None,
                quality: PhysicalDataQuality::Unknown,
                limitations: vec![],
            },
        }));
        (NativeModels(Arc::new(NativeModel(source))), value)
    }
    fn nested_semantic_release(occurrence: Uuid) -> Arc<DynamicSampleExpression> {
        Arc::new(DynamicSampleExpression::Transition {
            from: Some(resume(
                Some(semantic_leaf(0.2)),
                Some(semantic_leaf(0.8)),
                occurrence,
            )),
            to: None,
            progress: 0.5,
            reason: DynamicTransitionReason::Required {
                requirement: TransitionRequirement::MaterializedEndpoints,
            },
        })
    }

    #[test]
    fn conditioned_covered_whole_does_not_require_the_generic_direct_bases_model() {
        let occurrence = Uuid::from_u128(40);
        let (models, base) = native_base();
        let lower = color_whole(nested_semantic_release(occurrence), rank(2, 3));
        let cover = FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::SemanticColor {
                            basis: DynamicSemanticColorBasis::Recipe,
                        },
                        component: Some(ProgrammingComponent::Color(ColorComponent::Red)),
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Scalar(0.2),
            rank(3, 4),
            1.0,
        )
        .unwrap();
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let result = condition_sources(&[lower, cover.into()], &state, &base).unwrap();
        let FamilyCompositionSample::WholeExpression { expression, .. } =
            result[0].as_ref().unwrap()
        else {
            panic!()
        };
        assert!(expression.needs_underlay());
        assert!(
            expression
                .resolve_original_native_model(models.0.source())
                .is_err(),
            "a generic base must not introduce a new source-model requirement"
        );
    }

    #[test]
    fn conditioned_whole_keeps_models_already_pinned_for_its_actual_underlay() {
        let occurrence = Uuid::from_u128(40);
        let (models, base) = native_base();
        let original = Arc::new(
            CompiledProgrammingFamilyExpression::new(
                nested_semantic_release(occurrence),
                ProgrammingOwner::Color,
                Some(&base),
                Some(&models),
            )
            .unwrap(),
        );
        let pinned = original
            .resolve_original_native_model(models.0.source())
            .unwrap();
        let samples = [FamilyCompositionSample::WholeExpression {
            expression: original,
            rank: rank(2, 3),
            activation_mix: 1.0,
        }];
        let mut state = BranchState::default();
        state.choose(rank(2, 3), occurrence, true).unwrap();
        let result = condition_sources(&samples, &state, &base).unwrap();
        let FamilyCompositionSample::WholeExpression { expression, .. } =
            result[0].as_ref().unwrap()
        else {
            panic!()
        };
        let retained = expression
            .resolve_original_native_model(models.0.source())
            .unwrap();
        assert!(Arc::ptr_eq(&pinned, &retained));
    }
}
