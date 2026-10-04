//! Runtime samples become owner groups before composition. This bridge resolves only explicit
//! Angle Current partners; eligible underlays and physical frame requirements remain deferred.
use super::expression::{ExpressionNode, ExpressionNodeRef};
use super::*;
use crate::{DynamicNativeModelResolver, DynamicRuntimeSample};
use light_core::{AttributeValue, FixtureId, programming::*};
use rustc_hash::FxHashMap as HashMap;
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;

pub struct DynamicFamilySampleGroup {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    pub samples: Vec<FamilyCompositionSample>,
}

/// Borrowed until the next preparation. Legacy fragments retain their own Resume influence
/// and controller activation, and must pass through the existing scalar projection separately.
pub struct PreparedDynamicFamilySamples<'a> {
    pub families: &'a [DynamicFamilySampleGroup],
    pub legacy: &'a [DynamicRuntimeSample],
    /// Passive candidate/cohort gaps. These sources did not enter composition; another valid
    /// source can still participate. With no remaining sources, retain the captured static
    /// family. The publisher applies coverage/visibility filtering before showing notices.
    pub requirements: &'a [DynamicFamilyPreparationRequirement],
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicFamilyPreparationRequirement {
    pub target: FixtureId,
    pub owner: ProgrammingOwner,
    /// A lane/owner candidate, or the highest original lane of a correlated Position cohort.
    /// A Position requirement never permits emitting the remaining incomplete Angle pair.
    pub rank: FamilySampleRank,
    pub reason: DynamicFamilyPreparationRequirementReason,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DynamicFamilyPreparationRequirementReason {
    Transition(TransitionRequirement),
    NativeColorModelUnavailable(crate::NativeColorModelUnavailable),
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct CacheKey {
    instance: Uuid,
    controller: Uuid,
    target: FixtureId,
    lane: Uuid,
    owner: ProgrammingOwner,
}

#[derive(Clone)]
struct CompiledSample {
    // Own the original expression, never a raw pointer key with an unrelated lifetime.
    expression: Arc<DynamicSampleExpression>,
    samples: Vec<FamilyCompositionSample>,
}

/// Capacity is reused, with at most one compiled expression per currently present lane/owner.
/// Changed values/progress compile again. Native expressions deliberately re-resolve the exact
/// original source on each preparation: this API has no native-model generation token. Position
/// Current is resolved before cache comparison on every call, including paused controllers.
/// This is a correctness bridge, not a no-allocation or no-compilation frame-path guarantee.
#[derive(Default)]
pub struct DynamicFamilyPreparationScratch {
    order: Vec<usize>,
    /// One range of `order` per controller (instance, controller, target).
    controllers: Vec<std::ops::Range<usize>>,
    /// Each controller's chunk in a parallel preparation.
    chunk_of_controller: Vec<usize>,
    sort_keys: Vec<ControllerSortKey>,
    controller: Vec<DynamicRuntimeSample>,
    position: Vec<DynamicRuntimeSample>,
    families: Vec<DynamicFamilySampleGroup>,
    family_buffers: Vec<Vec<FamilyCompositionSample>>,
    family_indices: HashMap<(FixtureId, ProgrammingOwner), usize>,
    legacy: Vec<DynamicRuntimeSample>,
    requirements: Vec<DynamicFamilyPreparationRequirement>,
    sampling_requirements: Vec<DynamicFamilyPreparationRequirement>,
    cache: HashMap<CacheKey, CompiledSample>,
    /// Last frame's compiled samples this preparation did not reuse (TL-639 round 6), for the
    /// caller to free off the frame's thread ([`Self::take_retired`]).
    retired: HashMap<CacheKey, CompiledSample>,
}

/// Compiled samples a preparation no longer needs; dropping them only frees memory.
pub struct RetiredPreparation(#[allow(dead_code)] HashMap<CacheKey, CompiledSample>);

impl DynamicFamilyPreparationScratch {
    /// Take the compiled samples the last preparation retired, to drop them elsewhere. Left
    /// in place, they are dropped by the next preparation.
    pub fn take_retired(&mut self) -> RetiredPreparation {
        RetiredPreparation(std::mem::take(&mut self.retired))
    }

    /// The last preparation's result.
    pub fn prepared(&self) -> PreparedDynamicFamilySamples<'_> {
        PreparedDynamicFamilySamples {
            families: &self.families,
            legacy: &self.legacy,
            requirements: &self.requirements,
        }
    }

    /// Drop retained compiled sources at a show/dependency boundary.
    pub fn clear(&mut self) {
        self.clear_output();
        self.cache.clear();
        self.controller.clear();
        self.position.clear();
        self.order.clear();
        self.sort_keys.clear();
    }

    fn clear_output(&mut self) {
        for mut group in self.families.drain(..) {
            group.samples.clear();
            self.family_buffers.push(group.samples);
        }
        self.family_indices.clear();
        self.legacy.clear();
        self.requirements.clear();
        self.sampling_requirements.clear();
    }
}

impl<O: PreparationOutput> Preparer<'_, O> {
    fn prepare_sample(
        &mut self,
        sample: DynamicRuntimeSample,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), TransitionError> {
        for (owner, expression) in split_owners(Arc::new(sample.expression.clone()))? {
            self.prepare_part(&sample, owner, expression, native_models)?;
        }
        Ok(())
    }

    fn prepare_part(
        &mut self,
        sample: &DynamicRuntimeSample,
        owner: Option<ProgrammingOwner>,
        expression: Arc<DynamicSampleExpression>,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), TransitionError> {
        if let Some(owner) = owner {
            if self.sampling_required(sample, owner) {
                return Ok(());
            }
            self.prepare_owner_sample(sample, owner, expression, native_models)
        } else {
            let mut legacy = sample.clone();
            legacy.expression = expression.as_ref().clone();
            // A retained hot edit can now address several old scalar attributes.
            legacy.address = None;
            self.out.legacy(legacy);
            Ok(())
        }
    }

    fn prepare_position_controller(
        &mut self,
        sources: &dyn DynamicValueSourceResolver,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), TransitionError> {
        self.position.clear();
        if let Some(first) = self.controller.first() {
            for sample in self.controller.iter() {
                address::ensure(
                    sample.priority == first.priority
                        && sample.activated_at_millis == first.activated_at_millis
                        && sample.activation_mix == first.activation_mix,
                    "Position branches require one controller, target, rank and activation influence",
                )?;
            }
        }
        for index in 0..self.controller.len() {
            let sample = self.controller[index].clone();
            for (owner, expression) in split_owners(Arc::new(sample.expression.clone()))? {
                if owner == Some(ProgrammingOwner::Position) {
                    if self.sampling_required(&sample, ProgrammingOwner::Position) {
                        continue;
                    }
                    let mut position = sample.clone();
                    position.expression = expression.as_ref().clone();
                    position.address = None;
                    self.position.push(position);
                } else {
                    // Split independent owners and legacy before resolving the correlated
                    // Position forest. An unavailable Angle partner cannot erase them.
                    self.prepare_part(&sample, owner, expression, native_models)?;
                }
            }
        }
        if self.position.is_empty() {
            return Ok(());
        }
        let checked = CheckedCurrentSources::new(sources);
        let sources = RequiredPositionSources {
            sources: &checked,
            missing: Cell::new(false),
        };
        let result = bundle_position_component_forest(&self.position, &sources);
        let representative = self
            .position
            .last()
            .map(|sample| (sample.target, rank(sample)));
        let result = match (result, checked.take_error()) {
            (Err(error @ TransitionError::Invalid(_)), _)
            | (_, Some(error @ TransitionError::Invalid(_))) => return Err(error),
            (_, Some(error)) => Err(error),
            (result, None) => result,
        };
        let requirement = match result {
            Ok(bundle) => {
                debug_assert!(bundle.remainder.is_empty(), "Position-only forest");
                if sources.missing.get() || bundle.position.is_none() {
                    // Missing Current is not a Release endpoint. Withhold this controller's
                    // entire Position cohort, including its otherwise complete branches.
                    Some(TransitionRequirement::LiveJointAngles)
                } else {
                    let (target, _) = representative.expect("Position source");
                    self.out.append(
                        target,
                        ProgrammingOwner::Position,
                        vec![bundle.position.unwrap()],
                    );
                    None
                }
            }
            Err(TransitionError::Requires(requirement)) => Some(requirement),
            Err(error) => return Err(error),
        };
        if let (Some(requirement), Some((target, rank))) = (requirement, representative) {
            self.out.require(DynamicFamilyPreparationRequirement {
                target,
                owner: ProgrammingOwner::Position,
                rank,
                reason: DynamicFamilyPreparationRequirementReason::Transition(requirement),
            });
        }
        self.position.clear();
        Ok(())
    }

    fn sampling_required(&self, sample: &DynamicRuntimeSample, owner: ProgrammingOwner) -> bool {
        self.sampling_requirements.iter().any(|required| {
            let Some(identity) = required.rank.dynamic_identity() else {
                return false;
            };
            required.target == sample.target
                && required.owner == owner
                && identity.instance_id == sample.instance_id
                && identity.controller_id == sample.controller_id
                && (owner == ProgrammingOwner::Position || identity.lane_id == sample.lane_id)
        })
    }

    fn require(
        &mut self,
        sample: &DynamicRuntimeSample,
        owner: ProgrammingOwner,
        requirement: TransitionRequirement,
    ) {
        self.out.require(DynamicFamilyPreparationRequirement {
            target: sample.target,
            owner,
            rank: rank(sample),
            reason: DynamicFamilyPreparationRequirementReason::Transition(requirement),
        });
    }

    fn prepare_owner_sample(
        &mut self,
        sample: &DynamicRuntimeSample,
        owner: ProgrammingOwner,
        expression: Arc<DynamicSampleExpression>,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), TransitionError> {
        let rank = rank(sample);
        let key = CacheKey {
            instance: sample.instance_id,
            controller: sample.controller_id,
            target: sample.target,
            lane: sample.lane_id,
            owner,
        };
        let shape = match classify(&expression) {
            Ok(shape) => shape,
            Err(TransitionError::Requires(requirement)) => {
                self.require(sample, owner, requirement);
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        // Current/Size can introduce an original Direct source which was not a cold lane
        // dependency. Capture its capability here; expected absence affects this candidate,
        // never every other family/fixture. The strict compiler still validates available
        // exact domains, using these same captured model Arcs without another lookup.
        let captured_models = if shape.native {
            let captured = PreparedNativeSources::capture(&expression, native_models)?;
            if !captured.unavailable.is_empty() {
                for unavailable in captured.unavailable {
                    self.out.require(DynamicFamilyPreparationRequirement {
                        target: sample.target,
                        owner,
                        rank,
                        reason:
                            DynamicFamilyPreparationRequirementReason::NativeColorModelUnavailable(
                                unavailable,
                            ),
                    });
                }
                return Ok(());
            }
            Some(captured)
        } else {
            None
        };
        let native_models = captured_models
            .as_ref()
            .map(|models| models as &dyn DynamicNativeModelResolver);
        let compiled = if !shape.native
            && let Some(cached) = self.previous.take(&key)
            && cached.expression == expression
        {
            cached
        } else {
            let samples = match compile(
                expression.clone(),
                owner,
                shape,
                native_models,
                rank,
                sample.activation_mix,
            ) {
                Ok(samples) => samples,
                Err(TransitionError::Requires(requirement)) => {
                    self.require(sample, owner, requirement);
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            CompiledSample {
                expression: expression.clone(),
                samples,
            }
        };
        let mut samples = compiled.samples.clone();
        for candidate in &mut samples {
            match candidate {
                FamilyCompositionSample::Known(candidate) => {
                    candidate.set_rank(rank);
                    candidate.activation_mix = sample.activation_mix;
                }
                FamilyCompositionSample::WholeExpression {
                    rank: value,
                    activation_mix,
                    ..
                }
                | FamilyCompositionSample::CoupledExpression {
                    rank: value,
                    activation_mix,
                    ..
                } => {
                    *value = rank;
                    *activation_mix = sample.activation_mix;
                }
            }
        }
        self.out.append(sample.target, owner, samples);
        if !shape.native {
            match &mut self.keep {
                Keep::Cache(cache) => {
                    cache.insert(key, compiled);
                }
                Keep::Log(log) => log.push((key, compiled)),
            }
        }
        Ok(())
    }
}

/// Observe only reads actually requested by the forest. Exact inactive branches must neither
/// perform Current reads nor create requirements. Values and dependency proof stay paired.
struct RequiredPositionSources<'a> {
    sources: &'a dyn DynamicValueSourceResolver,
    missing: Cell<bool>,
}

impl DynamicValueSourceResolver for RequiredPositionSources<'_> {
    fn try_position_current_family(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.sources.try_position_current_family(target, address)
    }
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        let value = self.sources.current(target, address);
        if value.is_none() {
            self.missing.set(true);
        }
        value
    }

    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        self.sources.current_dependency(target, address)
    }

    fn current_family_occurrence(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<DynamicSourceOccurrenceId> {
        self.sources.current_family_occurrence(target, address)
    }

    fn current_family_base(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> Option<AttributeValue> {
        self.sources.current_family_base(target, address)
    }

    fn authored_occurrence(
        &self,
        instance_id: Uuid,
        controller_id: Uuid,
        target: FixtureId,
        lane_id: Uuid,
    ) -> Option<DynamicSourceOccurrenceId> {
        self.sources
            .authored_occurrence(instance_id, controller_id, target, lane_id)
    }

    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance_id: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.sources.preset(source, instance_id, target)
    }
}

/// Tie order is controller UUID, followed by the rank's original instance/controller/lane IDs.
/// It is independent of input iteration and uses no newly generated identities.
fn rank(sample: &DynamicRuntimeSample) -> FamilySampleRank {
    FamilySampleRank {
        priority: sample.priority,
        changed_at_millis: sample.activated_at_millis,
        changed_at_submillis_nanos: 0,
        stable_order: sample.controller_id.as_u128(),
        identity: crate::FamilySampleIdentity::Dynamic {
            instance_id: sample.instance_id,
            controller_id: sample.controller_id,
            lane_id: sample.lane_id,
        },
    }
}

pub fn prepare_dynamic_family_samples<'a>(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &'a mut DynamicFamilyPreparationScratch,
) -> Result<PreparedDynamicFamilySamples<'a>, TransitionError> {
    prepare_dynamic_family_samples_with_requirements(samples, &[], sources, native_models, scratch)
}

/// A failed numeric Current candidate is omitted at its original owner/rank. Position remains
/// a complete controller/target cohort; independent legacy and other owners still participate.
/// Original payload/evidence validation precedes suppression, including inactive branches.
pub fn prepare_dynamic_family_samples_with_requirements<'a>(
    samples: &[DynamicRuntimeSample],
    sampling_requirements: &[DynamicFamilyPreparationRequirement],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    scratch: &'a mut DynamicFamilyPreparationScratch,
) -> Result<PreparedDynamicFamilySamples<'a>, TransitionError> {
    scratch.clear_output();
    scratch
        .sampling_requirements
        .extend_from_slice(sampling_requirements);
    scratch
        .requirements
        .extend_from_slice(sampling_requirements);
    let mut previous = std::mem::take(&mut scratch.cache);
    let result = prepare(samples, sources, native_models, &mut previous, scratch);
    scratch.retired = previous;
    if let Err(error) = result {
        scratch.clear();
        return Err(error);
    }
    scratch.families.sort_unstable_by_key(family_order);
    Ok(PreparedDynamicFamilySamples {
        families: &scratch.families,
        legacy: &scratch.legacy,
        requirements: &scratch.requirements,
    })
}

/// A completed hot edit has its selected endpoint's mask. Validate the original history first,
/// then remove inactive endpoints before deciding whether a controller needs Angle pairing.
/// Interior Resume nodes retain their original occurrence and progress.
fn prune_exact_branches(
    expression: Arc<DynamicSampleExpression>,
) -> Result<Option<Arc<DynamicSampleExpression>>, IntentError> {
    let needs_pruning = ExpressionNodeRef::new(&expression)
        .postorder(false)?
        .into_iter()
        .any(|node| {
            matches!(
                node.node(),
                Ok(ExpressionNode::Transition {
                    progress: 0.0 | 1.0,
                    ..
                } | ExpressionNode::Scale {
                    factor: 0.0 | 1.0,
                    ..
                })
            )
        });
    if !needs_pruning {
        return Ok(Some(expression));
    }
    let mut tape = RetainedExpressionTape::from_roots(&[expression.clone()])?;
    let original_root = tape.roots[0];
    let count = tape.nodes.len();
    let mut mapped = Vec::<Option<RetainedNodeId>>::with_capacity(count);
    for index in 0..count {
        let original = RetainedNodeId(index as u32);
        let replacement = match tape.nodes[index].clone() {
            RetainedExpressionNode::Transition {
                from,
                to,
                progress,
                reason,
            } => {
                let next_from = from.and_then(|id| mapped[id.0 as usize]);
                let next_to = to.and_then(|id| mapped[id.0 as usize]);
                if progress == 0.0 || progress == 1.0 {
                    mapped.push(if progress == 0.0 { next_from } else { next_to });
                    continue;
                }
                if next_from.is_none() && next_to.is_none() {
                    mapped.push(None);
                    continue;
                }
                (from != next_from || to != next_to).then_some(RetainedExpressionNode::Transition {
                    from: next_from,
                    to: next_to,
                    progress,
                    reason,
                })
            }
            RetainedExpressionNode::Scale {
                address,
                base,
                value,
                factor,
                baseline_occurrence,
            } => {
                if factor == 1.0 {
                    mapped.push(mapped[value.0 as usize]);
                    continue;
                }
                if factor == 0.0 {
                    let DynamicValue::Family(family) = &base else {
                        return Err(IntentError(
                            "Dynamic Size requires a whole-family baseline".into(),
                        ));
                    };
                    Some(RetainedExpressionNode::Programming {
                        address: DynamicValueAddress::whole_family(address.owner(), family)?,
                        value: base,
                        occurrence: None,
                        dependency_occurrence: Some(crate::DynamicSourceDependency::identity(
                            baseline_occurrence,
                        )),
                    })
                } else if let Some(next) = mapped[value.0 as usize] {
                    (next != value).then_some(RetainedExpressionNode::Scale {
                        address,
                        base,
                        value: next,
                        factor,
                        baseline_occurrence,
                    })
                } else {
                    // A released child is this Size's eligible underlay. Its declared whole
                    // address remains necessary to evaluate the retained baseline operation.
                    None
                }
            }
            _ => None,
        };
        let id = if let Some(node) = replacement {
            let id = RetainedNodeId(
                u32::try_from(tape.nodes.len())
                    .map_err(|_| IntentError("projected sample is too large".into()))?,
            );
            tape.nodes.push(node);
            id
        } else {
            original
        };
        mapped.push(Some(id));
    }
    let Some(root) = mapped[original_root.0 as usize] else {
        return Ok(None);
    };
    if root == original_root {
        return Ok(Some(expression));
    }
    tape.roots = vec![root];
    tape.compact_reachable()?;
    let root = tape.roots[0];
    Ok(Some(Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    })))
}

#[derive(Clone, Copy, Default)]
struct Shape {
    native: bool,
    components: bool,
    whole: bool,
}

fn classify(expression: &DynamicSampleExpression) -> Result<Shape, TransitionError> {
    let mut shape = Shape::default();
    if let DynamicSampleExpression::Programming { address, value, .. } = expression {
        // TL-639: the leaf's own node is the whole postorder.
        shape.native = matches!(
            address.representation,
            DynamicFamilyRepresentation::DirectColor { .. }
        ) || matches!(value, DynamicValue::Family(light_core::AttributeValue::ColorProgram(program))
                if matches!(program.as_ref(), ColorProgram::Direct { .. }));
        shape.components = address.component.is_some();
        shape.whole = address.component.is_none();
        return Ok(shape);
    }
    // Classification sees original typed leaves, including inactive branches. The chosen
    // compiler performs its own exact-endpoint pruning; errors are never caught and retried.
    for node in ExpressionNodeRef::new(expression).postorder(false)? {
        match node.node()? {
            ExpressionNode::Programming(address, value, ..)
            | ExpressionNode::Scale {
                address,
                base: value,
                ..
            } => {
                shape.native |= matches!(
                    address.representation,
                    DynamicFamilyRepresentation::DirectColor { .. }
                ) || matches!(value, DynamicValue::Family(light_core::AttributeValue::ColorProgram(program))
                        if matches!(program.as_ref(), ColorProgram::Direct { .. }));
                shape.components |= address.component.is_some();
                shape.whole |= address.component.is_none();
            }
            ExpressionNode::Transition { .. } => {}
            _ => {
                return Err(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ));
            }
        }
    }
    Ok(shape)
}

#[derive(Default)]
struct PreparedNativeSources {
    available: Vec<Arc<dyn NativeColorEditModel + Send + Sync>>,
    unavailable: Vec<crate::NativeColorModelUnavailable>,
}

impl PreparedNativeSources {
    fn capture(
        expression: &DynamicSampleExpression,
        provider: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<Self, IntentError> {
        let mut result = Self::default();
        for node in ExpressionNodeRef::new(expression).postorder(true)? {
            let (address, value) = match node.node()? {
                ExpressionNode::Programming(address, value, ..)
                | ExpressionNode::Scale {
                    address,
                    base: value,
                    ..
                } => (address, value),
                _ => continue,
            };
            if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation {
                result.capture_source(source, provider)?;
            }
            if let DynamicValue::Family(AttributeValue::ColorProgram(program)) = value
                && let ColorProgram::Direct { recipe, .. } = program.as_ref()
            {
                result.capture_source(&recipe.source, provider)?;
            }
        }
        Ok(result)
    }

    fn capture_source(
        &mut self,
        source: &light_core::NativeColorIdentity,
        provider: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), IntentError> {
        if self.available.iter().any(|model| model.source() == source)
            || self
                .unavailable
                .iter()
                .any(|reason| &reason.source == source)
        {
            return Ok(());
        }
        source.validate()?;
        let capability = match provider {
            Some(provider) => provider.resolve_capability(source)?,
            None => {
                crate::NativeColorModelCapability::Unavailable(crate::NativeColorModelUnavailable {
                    source: source.clone(),
                    reason: crate::NativeColorUnavailableReason::MissingResolver,
                    detail: "Original native Color model is not available".into(),
                })
            }
        };
        match capability {
            crate::NativeColorModelCapability::Available(model) => {
                address::ensure(
                    model.source() == source,
                    "native model resolver returned a different original source",
                )?;
                self.available.push(model);
            }
            crate::NativeColorModelCapability::Unavailable(reason) => {
                address::ensure(
                    &reason.source == source,
                    "native capability refers to a different original source",
                )?;
                self.unavailable.push(reason);
            }
        }
        Ok(())
    }
}

impl DynamicNativeModelResolver for PreparedNativeSources {
    fn resolve(
        &self,
        source: &light_core::NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.available
            .iter()
            .find(|model| model.source() == source)
            .cloned()
            .ok_or_else(|| {
                IntentError(
                    "expression requested a native source outside its captured dependencies".into(),
                )
            })
    }
}

fn compile(
    expression: Arc<DynamicSampleExpression>,
    owner: ProgrammingOwner,
    shape: Shape,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    rank: FamilySampleRank,
    activation_mix: f32,
) -> Result<Vec<FamilyCompositionSample>, TransitionError> {
    // A materialized leaf keeps its captured authored occurrence and Current dependency;
    // the numeric sample alone would reduce both to an unattributed authored source.
    if let Ok(ExpressionNode::Programming(address, value, occurrence, dependency)) =
        ExpressionNodeRef::new(&expression).node()
    {
        let model =
            if let DynamicFamilyRepresentation::DirectColor { source } = &address.representation {
                Some(
                    native_models
                        .ok_or(TransitionError::Requires(
                            TransitionRequirement::NativeColorModel,
                        ))?
                        .resolve(source)?,
                )
            } else {
                None
            };
        let address = Arc::new(CompiledDynamicValueAddress::new(address.clone(), model)?);
        return Ok(vec![
            FamilySample::new(address, value.clone(), rank, activation_mix)?
                .with_leaf_provenance(occurrence, dependency)
                .into(),
        ]);
    }
    if !shape.components {
        let expression = Arc::new(CompiledProgrammingFamilyExpression::new(
            expression,
            owner,
            None,
            native_models,
        )?);
        return Ok(if expression.participates() {
            vec![FamilyCompositionSample::WholeExpression {
                expression,
                rank,
                activation_mix,
            }]
        } else {
            vec![]
        });
    }
    if !shape.whole && components_compatible(&expression)? {
        let expressions = CompiledComponentExpressionSet::new(expression, native_models)?;
        return Ok(
            FamilySample::retained_components(&expressions, rank, activation_mix)?
                .into_iter()
                .map(Into::into)
                .collect(),
        );
    }
    let expression = Arc::new(CompiledCoupledExpression::new(expression, native_models)?);
    Ok(vec![FamilyCompositionSample::CoupledExpression {
        expression,
        rank,
        activation_mix,
    }])
}

/// Match the component-set compiler's compatibility contract before choosing it. In particular,
/// two functions of one native channel and two Target frames require coupled endpoint cohorts.
fn components_compatible(expression: &DynamicSampleExpression) -> Result<bool, IntentError> {
    let mut addresses: Vec<&DynamicValueAddress> = Vec::new();
    for node in ExpressionNodeRef::new(expression).postorder(true)? {
        let ExpressionNode::Programming(address, ..) = node.node()? else {
            continue;
        };
        for prior in &addresses {
            let compatible = match (&prior.representation, &address.representation) {
                (
                    DynamicFamilyRepresentation::SemanticColor { basis: a },
                    DynamicFamilyRepresentation::SemanticColor { basis: b },
                ) => {
                    a == b
                        || *a == DynamicSemanticColorBasis::Retain
                        || *b == DynamicSemanticColorBasis::Retain
                }
                (a, b) => a == b,
            };
            if !compatible || (prior.component == address.component && *prior != address) {
                return Ok(false);
            }
            if let (
                Some(ProgrammingComponent::NativeColor(a)),
                Some(ProgrammingComponent::NativeColor(b)),
            ) = (prior.component, address.component)
                && a.channel_id == b.channel_id
                && a.function_id != b.function_id
            {
                return Ok(false);
            }
        }
        addresses.push(address);
    }
    Ok(true)
}

/// Preserve one root per owner plus legacy, replacing other-owner branches with absence.
/// This is a flat projection of the original history, never a numerical blend or borrowed value.
/// TL-639: an unwrapped Programming or legacy scalar leaf, whose only node is itself.
/// A single node without children (TL-639 round 4 adds the Angle leaves): it has no exact branch
/// to prune.
fn is_plain_leaf(expression: &DynamicSampleExpression) -> bool {
    matches!(
        expression,
        DynamicSampleExpression::Programming { .. }
            | DynamicSampleExpression::LegacyScalar { .. }
            | DynamicSampleExpression::AngleNumeric { .. }
            | DynamicSampleExpression::AngleCurrent { .. }
    )
}

fn split_owners(
    expression: Arc<DynamicSampleExpression>,
) -> Result<Vec<(Option<ProgrammingOwner>, Arc<DynamicSampleExpression>)>, TransitionError> {
    // A plain leaf has exactly one owner (legacy: none), so it is its own projection.
    match expression.as_ref() {
        DynamicSampleExpression::Programming { address, .. } => {
            let owner = address.owner();
            return Ok(vec![(Some(owner), expression)]);
        }
        DynamicSampleExpression::LegacyScalar { .. } => return Ok(vec![(None, expression)]),
        DynamicSampleExpression::AngleNumeric { .. }
        | DynamicSampleExpression::AngleCurrent { .. } => {
            return Ok(vec![(Some(ProgrammingOwner::Position), expression)]);
        }
        _ => {}
    }
    let mut owners = Vec::new();
    for node in ExpressionNodeRef::new(&expression).postorder(false)? {
        let owner = match node.node()? {
            ExpressionNode::Programming(address, ..) | ExpressionNode::Scale { address, .. } => {
                Some(address.owner())
            }
            ExpressionNode::Legacy(..) => None,
            ExpressionNode::Current(_) | ExpressionNode::Numeric(_) => {
                Some(ProgrammingOwner::Position)
            }
            ExpressionNode::Transition { .. } => continue,
        };
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
    if owners.len() == 1 {
        return Ok(vec![(owners[0], expression)]);
    }
    let source = RetainedExpressionTape::from_roots(&[expression])?;
    let mut result = Vec::new();
    for owner in owners {
        let mut tape = RetainedExpressionTape::empty();
        let mut mapped = Vec::<Option<RetainedNodeId>>::with_capacity(source.nodes.len());
        for node in &source.nodes {
            let projected = match node {
                RetainedExpressionNode::Programming { address, .. }
                | RetainedExpressionNode::Scale { address, .. }
                    if Some(address.owner()) != owner =>
                {
                    None
                }
                RetainedExpressionNode::LegacyScalar { .. } if owner.is_some() => None,
                RetainedExpressionNode::AngleCurrent { .. }
                | RetainedExpressionNode::AngleNumeric { .. }
                    if owner != Some(ProgrammingOwner::Position) =>
                {
                    None
                }
                RetainedExpressionNode::Transition {
                    from,
                    to,
                    progress,
                    reason,
                } => {
                    let from = from.and_then(|id| mapped[id.0 as usize]);
                    let to = to.and_then(|id| mapped[id.0 as usize]);
                    (from.is_some() || to.is_some()).then_some(RetainedExpressionNode::Transition {
                        from,
                        to,
                        progress: *progress,
                        reason: *reason,
                    })
                }
                RetainedExpressionNode::Scale {
                    address,
                    base,
                    value,
                    factor,
                    baseline_occurrence,
                } => Some(RetainedExpressionNode::Scale {
                    address: address.clone(),
                    base: base.clone(),
                    value: mapped[value.0 as usize]
                        .ok_or_else(|| IntentError("projected Size lost its owner".into()))?,
                    factor: *factor,
                    baseline_occurrence: *baseline_occurrence,
                }),
                node => Some(node.clone()),
            };
            let id = if let Some(node) = projected {
                let id = RetainedNodeId(
                    u32::try_from(tape.nodes.len())
                        .map_err(|_| IntentError("projected sample is too large".into()))?,
                );
                tape.nodes.push(node);
                Some(id)
            } else {
                None
            };
            mapped.push(id);
        }
        if let Some(root) = mapped[source.roots[0].0 as usize] {
            tape.roots.push(root);
            tape.inherit_operations(&source, &mapped)?;
            tape.validate()?;
            result.push((
                owner,
                Arc::new(DynamicSampleExpression::Retained {
                    tape: Arc::new(tape),
                    root,
                }),
            ));
        }
    }
    Ok(result)
}

mod controller;
use controller::*;
mod parallel;
pub use parallel::{
    ControllerSortKey, PreparationSources, PreparationWorkers, PreparedChunk, TARGET_SHARDS,
    family_order, prepare_dynamic_family_samples_in_parallel, shard_chunk, target_shard,
};

#[cfg(test)]
mod tests;
