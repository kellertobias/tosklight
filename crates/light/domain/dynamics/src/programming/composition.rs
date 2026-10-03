//! Composition after lane sampling, before fixture fitting. Retained component expressions
//! evaluate against their eligible stack underlay. Whole expressions requiring live geometry
//! stay in their frame resolver; this layer never reads fitted channels or invents Current.
use super::{
    CompiledComponentExpression, CompiledComponentExpressionSet, CompiledDynamicValueAddress,
    ComponentExpressionObserver, ComponentExpressionStep, DynamicFamilyRepresentation,
    DynamicSampleExpression, DynamicSemanticColorBasis, DynamicValue, DynamicValueAddress,
    address::ensure, extract_compatible_dynamic_value,
};
use light_core::{AttributeValue, programming::*};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

mod adoption;
mod endpoint_output;
mod native_fixed;
mod position_segment;
mod retained_family;
mod segment;
mod trace;
mod whole_activation;
use adoption::{adopt, requirement};
pub use endpoint_output::{FamilyEndpointOutputContext, FamilyEndpointOutputControl};
pub use retained_family::*;
use segment::{compose_segment, flush, mark_covered_components};
pub use trace::*;

/// Fixed rows use their actual position in the captured input collections. This is frame-local
/// ordering identity, never authored provenance or a fabricated Dynamic instance/lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FamilyFixedSampleSource {
    Programmer,
    ExtraProgrammer,
    Cue,
}

/// Exact common-rank ties follow scalar collection order: Dynamics, Programmer, extra
/// Programmer, then Cue. Dynamic ties retain their existing instance/controller/lane order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FamilySampleIdentity {
    Dynamic {
        instance_id: Uuid,
        controller_id: Uuid,
        lane_id: Uuid,
    },
    Fixed {
        source: FamilyFixedSampleSource,
        row_index: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FamilyDynamicSampleIdentity {
    pub instance_id: Uuid,
    pub controller_id: Uuid,
    pub lane_id: Uuid,
}

/// Existing priority/recency order, including Cue precision within a millisecond. Fixed row
/// attribution remains the optional source occurrence carried by the trace, not this rank.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FamilySampleRank {
    pub priority: i16,
    pub changed_at_millis: u64,
    pub changed_at_submillis_nanos: u32,
    pub stable_order: u128,
    pub identity: FamilySampleIdentity,
}

impl FamilySampleRank {
    pub fn dynamic_identity(self) -> Option<FamilyDynamicSampleIdentity> {
        match self.identity {
            FamilySampleIdentity::Dynamic {
                instance_id,
                controller_id,
                lane_id,
            } => Some(FamilyDynamicSampleIdentity {
                instance_id,
                controller_id,
                lane_id,
            }),
            FamilySampleIdentity::Fixed { .. } => None,
        }
    }

    fn validate(self) -> Result<(), IntentError> {
        ensure(
            self.changed_at_submillis_nanos < 1_000_000,
            "family rank submillisecond nanoseconds are out of range",
        )
    }

    fn with_dynamic_lane(self, lane_id: Uuid) -> Result<Self, IntentError> {
        let dynamic = self.dynamic_identity().ok_or_else(|| {
            IntentError("retained Dynamic cohort requires a Dynamic source identity".into())
        })?;
        Ok(Self {
            identity: FamilySampleIdentity::Dynamic {
                instance_id: dynamic.instance_id,
                controller_id: dynamic.controller_id,
                lane_id,
            },
            ..self
        })
    }
}

/// A sampled component still owns only that component during arbitration. Only the composed
/// result becomes a whole-family engine contribution. Native addresses pin the original model.
#[derive(Clone)]
pub struct FamilySample {
    address: Arc<CompiledDynamicValueAddress>,
    body: FamilySampleBody,
    /// Only projections of the same immutable authored tree may share one lane rank. Their
    /// component key is a private fragment tie-breaker, never a fabricated persisted lane ID.
    projection: Option<Arc<DynamicSampleExpression>>,
    fix_at: bool,
    // Internal raw endpoint cohort: its enclosing expression applies output control once.
    endpoint_output_exempt: bool,
    /// Original runtime leaves for a synthetic owner bundle, or the captured provenance of
    /// one materialized authored leaf. None means the single sampled address/rank is an
    /// unattributed authored source; generated Current must be marked explicitly.
    trace_sources: Option<Arc<[trace::FamilyTraceLeaf]>>,
    trace_node: Option<FamilyTraceNodeId>,
    pub rank: FamilySampleRank,
    pub activation_mix: f32,
}

#[derive(Clone)]
enum FamilySampleBody {
    Materialized(DynamicValue),
    ComponentExpression(Arc<CompiledComponentExpression>),
}

impl FamilySample {
    /// Source boundary. Verify complete Direct ownership with the pinned original model once,
    /// before retaining this sample. Composition cannot accept an unverified whole recipe.
    pub fn new(
        address: Arc<CompiledDynamicValueAddress>,
        value: DynamicValue,
        rank: FamilySampleRank,
        activation_mix: f32,
    ) -> Result<Self, IntentError> {
        rank.validate()?;
        address.validate_source_value(&value)?;
        ensure(
            activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
            "Dynamic activation influence must be between zero and one",
        )?;
        Ok(Self {
            address,
            body: FamilySampleBody::Materialized(value),
            projection: None,
            fix_at: false,
            endpoint_output_exempt: false,
            trace_sources: None,
            trace_node: None,
            rank,
            activation_mix,
        })
    }

    pub fn address(&self) -> &CompiledDynamicValueAddress {
        &self.address
    }
    pub fn materialized_value(&self) -> Option<&DynamicValue> {
        match &self.body {
            FamilySampleBody::Materialized(value) => Some(value),
            FamilySampleBody::ComponentExpression(_) => None,
        }
    }

    /// Keep one exact component footprint until its actual stack underlay is available.
    /// A completed release has no participation, including in representation selection.
    pub fn retained_component(
        expression: Arc<CompiledComponentExpression>,
        rank: FamilySampleRank,
        activation_mix: f32,
    ) -> Result<Self, IntentError> {
        rank.validate()?;
        ensure(
            activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
            "Dynamic activation influence must be between zero and one",
        )?;
        Ok(Self {
            address: expression.address().clone(),
            body: FamilySampleBody::ComponentExpression(expression),
            projection: None,
            fix_at: false,
            endpoint_output_exempt: false,
            trace_sources: None,
            trace_node: None,
            rank,
            activation_mix,
        })
    }

    /// Split a compatible multi-component tree without promoting any fragment to whole-owner
    /// ownership. Original rank/activation are shared; exact released fragments are absent.
    pub fn retained_components(
        expressions: &CompiledComponentExpressionSet,
        rank: FamilySampleRank,
        activation_mix: f32,
    ) -> Result<Vec<Self>, IntentError> {
        ensure(
            activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
            "Dynamic activation influence must be between zero and one",
        )?;
        expressions
            .components()
            .iter()
            .map(|expression| {
                let mut sample =
                    Self::retained_component(expression.clone(), rank, activation_mix)?;
                sample.projection = Some(expressions.expression().clone());
                Ok(sample)
            })
            .collect()
    }

    fn order_key(&self) -> (FamilySampleRank, Option<ProgrammingComponent>) {
        (
            self.rank,
            self.projection
                .as_ref()
                .and(self.address.address().component),
        )
    }

    fn endpoint_control(
        &self,
        context: &FamilyCompositionContext<'_>,
    ) -> FamilyEndpointOutputControl {
        self.captured_endpoint_control(|rank| endpoint_output::control(rank, context))
    }

    /// Apply this sample's actual endpoint-gate policy to an immutable captured control.
    /// Fixed masks and already-controlled derived endpoints bypass the callback entirely.
    pub fn captured_endpoint_control(
        &self,
        control: impl FnOnce(FamilySampleRank) -> FamilyEndpointOutputControl,
    ) -> FamilyEndpointOutputControl {
        if self.fix_at || self.endpoint_output_exempt {
            FamilyEndpointOutputControl::Unchanged
        } else {
            control(self.rank)
        }
    }

    fn participates(&self) -> bool {
        self.activation_mix > 0.0
            && match &self.body {
                FamilySampleBody::Materialized(_) => true,
                FamilySampleBody::ComponentExpression(expression) => expression.participates(),
            }
    }

    fn validate_value(&self) -> Result<(), IntentError> {
        self.rank.validate()?;
        match &self.body {
            FamilySampleBody::Materialized(value) => self.address.validate_value(value),
            // Immutable compiled nodes have already validated addresses, domains and sources.
            FamilySampleBody::ComponentExpression(_) => Ok(()),
        }
    }

    fn component_needs_underlay(&self) -> bool {
        self.activation_mix < 1.0
            || match &self.body {
                FamilySampleBody::Materialized(_) => false,
                FamilySampleBody::ComponentExpression(expression) => expression.needs_underlay(),
            }
    }

    fn component_value(
        &self,
        underlay: Option<&DynamicValue>,
    ) -> Result<DynamicValue, TransitionError> {
        match &self.body {
            FamilySampleBody::Materialized(value) => Ok(value.clone()),
            FamilySampleBody::ComponentExpression(expression) => {
                expression.evaluate(underlay)?.ok_or_else(|| {
                    IntentError("inactive component expression entered composition".into()).into()
                })
            }
        }
    }

    pub(super) fn into_fix_at(mut self) -> Self {
        self.fix_at = true;
        self
    }

    pub fn is_fix_at(&self) -> bool {
        self.fix_at
    }

    /// Preserve original lane and Current roles when a runtime adapter materializes a pair.
    /// These tokens are observational only; they never change ranking or the composed value.
    pub fn with_trace_sources(mut self, sources: Arc<[FamilyTraceSource]>) -> Self {
        self.trace_sources = Some(
            sources
                .iter()
                .copied()
                .map(trace::FamilyTraceLeaf::Source)
                .collect(),
        );
        self
    }

    fn footprint(&self) -> FamilyTraceFootprint {
        self.address
            .address()
            .component
            .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component)
    }

    /// Retain a materialized authored leaf's captured occurrence and Current dependency, as
    /// the observed expression compilers do for interior leaves. The authored and Current
    /// roles stay distinct; Current keeps its own field transfer. Absent evidence stays
    /// absent: an unattributed leaf keeps the Unknown authored fallback without allocating.
    pub(crate) fn with_leaf_provenance(
        mut self,
        occurrence: Option<crate::DynamicSourceOccurrenceId>,
        dependency: Option<&crate::DynamicSourceDependency>,
    ) -> Self {
        if occurrence.is_none() && dependency.is_none() {
            return self;
        }
        let authored = FamilyTraceSource {
            rank: self.rank,
            footprint: self.footprint(),
            role: FamilyTraceRole::Authored,
            occurrence,
        };
        let mut leaves = vec![trace::FamilyTraceLeaf::Source(authored)];
        if let Some(dependency) = dependency {
            leaves.push(trace::FamilyTraceLeaf::Current(
                FamilyTraceSource {
                    role: FamilyTraceRole::CalculationDependency,
                    occurrence: dependency.occurrence,
                    ..authored
                },
                dependency.clone(),
            ));
        }
        self.trace_sources = Some(leaves.into());
        self
    }

    /// Reuse a cached compiled sample under this frame's arbitration rank. Leaf provenance
    /// recorded at the sample's own previous rank follows it; identities, roles and transfers
    /// are unchanged. An unchanged rank, the steady state, neither copies nor allocates.
    pub(crate) fn set_rank(&mut self, rank: FamilySampleRank) {
        let previous = std::mem::replace(&mut self.rank, rank);
        if previous == rank {
            return;
        }
        if let Some(sources) = &mut self.trace_sources {
            *sources = sources
                .iter()
                .cloned()
                .map(|mut leaf| {
                    let (trace::FamilyTraceLeaf::Source(source)
                    | trace::FamilyTraceLeaf::Current(source, _)) = &mut leaf;
                    if source.rank == previous {
                        source.rank = rank;
                    }
                    leaf
                })
                .collect();
        }
    }

    fn trace_sources(&self) -> impl Iterator<Item = trace::FamilyTraceLeaf> + '_ {
        self.trace_sources
            .as_deref()
            .map(|sources| sources.to_vec())
            .unwrap_or_else(|| {
                vec![trace::FamilyTraceLeaf::Source(FamilyTraceSource {
                    rank: self.rank,
                    footprint: self.footprint(),
                    role: FamilyTraceRole::Authored,
                    occurrence: None,
                })]
            })
            .into_iter()
    }

    /// Sampled component lanes retain their compiled domain; no source prediction happens
    /// here. A whole-family replacement goes through `new` at its source boundary.
    pub fn set_component_value(&mut self, value: DynamicValue) -> Result<(), IntentError> {
        ensure(
            self.address.address().component.is_some(),
            "whole-family sample requires source validation",
        )?;
        self.address.validate_value(&value)?;
        self.body = FamilySampleBody::Materialized(value);
        Ok(())
    }
}

fn sample_trace(sample: &FamilySample, trace: &mut FamilyTraceArena) -> FamilyTraceNodeId {
    if let Some(node) = sample.trace_node {
        return node;
    }
    let mut leaves = sample
        .trace_sources()
        .map(|source| trace.leaf(source))
        .collect::<Vec<_>>();
    let appearance = if leaves.len() == 1 {
        leaves.pop().expect("one source")
    } else {
        trace.bundle(leaves)
    };
    trace.control(
        sample.rank,
        sample
            .address
            .address()
            .component
            .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component),
        appearance,
        None,
    )
}

fn component_value_and_trace(
    sample: &FamilySample,
    underlay: Option<&DynamicValue>,
    trace: Option<&mut FamilyTraceArena>,
    underlay_trace: Option<FamilyTraceNodeId>,
) -> Result<(DynamicValue, Option<FamilyTraceNodeId>), TransitionError> {
    let Some(trace) = trace else {
        return sample.component_value(underlay).map(|value| (value, None));
    };
    let FamilySampleBody::ComponentExpression(expression) = &sample.body else {
        return sample.component_value(underlay).map(|value| {
            let node = sample_trace(sample, trace);
            (value, Some(node))
        });
    };
    struct Observer<'a> {
        trace: &'a mut FamilyTraceArena,
        nodes: Vec<Option<FamilyTraceNodeId>>,
        rank: FamilySampleRank,
        footprint: FamilyTraceFootprint,
        underlay: Option<FamilyTraceNodeId>,
    }
    impl ComponentExpressionObserver for Observer<'_> {
        fn evaluated(
            &mut self,
            node: usize,
            step: ComponentExpressionStep<'_>,
            _: &DynamicValue,
        ) -> Result<(), TransitionError> {
            if self.nodes.len() <= node {
                self.nodes.resize(node + 1, None);
            }
            let id = match step {
                ComponentExpressionStep::Underlay => self.underlay.ok_or(
                    TransitionError::Requires(TransitionRequirement::MaterializedEndpoints),
                )?,
                ComponentExpressionStep::Authored {
                    occurrence,
                    dependency_occurrence,
                    ..
                } => {
                    let authored = self.trace.source(FamilyTraceSource {
                        rank: self.rank,
                        footprint: self.footprint,
                        role: FamilyTraceRole::Authored,
                        occurrence,
                    });
                    if let Some(dependency) = dependency_occurrence {
                        let dependency = self.trace.current_source(
                            FamilyTraceSource {
                                rank: self.rank,
                                footprint: self.footprint,
                                role: FamilyTraceRole::CalculationDependency,
                                occurrence: dependency.occurrence,
                            },
                            dependency,
                        );
                        self.trace.bundle(vec![authored, dependency])
                    } else {
                        authored
                    }
                }
                ComponentExpressionStep::Transition { from, to, .. } => self.trace.blend(
                    self.nodes[from].expect("compiled outgoing component"),
                    self.nodes[to].expect("compiled incoming component"),
                ),
            };
            self.nodes[node] = Some(id);
            Ok(())
        }
    }
    let mut observer = Observer {
        trace,
        nodes: Vec::new(),
        rank: sample.rank,
        footprint: FamilyTraceFootprint::Component(
            sample
                .address
                .address()
                .component
                .expect("component expression"),
        ),
        underlay: underlay_trace,
    };
    let value = expression
        .evaluate_observed(underlay, Some(&mut observer))?
        .ok_or_else(|| IntentError("inactive component expression entered composition".into()))?;
    let root = observer.nodes[expression.trace_root_node()].expect("evaluated component root");
    Ok((value, Some(root)))
}

#[derive(Default)]
struct ComponentCompositionScratch {
    edits: Vec<ComponentEdit>,
    components: Vec<(ProgrammingComponent, DynamicValue)>,
    angle_order: Vec<usize>,
    angle_address: Option<Arc<CompiledDynamicValueAddress>>,
    covered_components: HashSet<ProgrammingComponent>,
    covered_native_functions: HashMap<Uuid, Uuid>,
    covered: Vec<bool>,
    pending_native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
}

pub type FamilyCompositionScratch = RetainedFamilyCompositionScratch;

/// Coherent adoption supplied by the caller when the pre-Dynamic base uses another frame or
/// native source. It must be a complete materialized family in the requested representation.
/// No destination fixture is used to reinterpret a stored Direct source.
#[derive(Default)]
pub struct FamilyCompositionContext<'a> {
    pub edit: FamilyEditContext<'a>,
    pub adopted_base: Option<&'a AttributeValue>,
    /// Resolve an exact intermediate underlay in the caller's pinned frame. The fixed edit
    /// context/adopted_base above applies only to the original pre-Dynamic base.
    /// Changing a function within the same native source must preserve all other channels.
    pub resolve_adoption: Option<&'a FamilyAdoptionResolver<'a>>,
    /// Transient captured controller controls; never stored in retained sample history.
    pub endpoint_output: Option<FamilyEndpointOutputContext<'a>>,
}

pub type FamilyAdoptionResolver<'a> =
    dyn Fn(&AttributeValue, &DynamicValueAddress) -> Result<AttributeValue, TransitionError> + 'a;

/// Compose compatible sampled lanes into one complete owner. Reuse `scratch` across frames.
/// Static/Current is the `base`, not a mask that stops effects. Component FixAT participates
/// in its component's rank/mix stack. A whole FixAT owns every component at its rank, so lower
/// orthogonal lanes cannot bypass it. Higher samples then compose over that complete underlay.
pub fn compose_dynamic_family(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilySample],
    context: &FamilyCompositionContext<'_>,
    scratch: &mut FamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    retained_family::compose_known_dynamic_family(owner, base, samples, context, scratch)
}

/// A complete Angle Dynamic competes as one contribution. Explicit Current partner samples
/// are resolved upstream from the immutable static frame. Missing sources suppress that whole
/// bundle; they must never borrow an axis from another instance or controller. FixAT keeps its
/// explicit component mask and therefore does not participate in Dynamic bundle completion.
fn bundle_angles(
    samples: &[FamilySample],
    bundled: &mut Vec<FamilySample>,
    scratch: &mut ComponentCompositionScratch,
    tracing: bool,
) -> Result<(), TransitionError> {
    bundle_angles_with_membership(samples, bundled, scratch, tracing, None)
}

/// Record input membership when each output is emitted, including synthetic Angle pairs.
/// Membership names the caller's input indices, never the representative output rank.
fn bundle_angles_with_membership(
    samples: &[FamilySample],
    bundled: &mut Vec<FamilySample>,
    scratch: &mut ComponentCompositionScratch,
    tracing: bool,
    mut membership: Option<&mut Vec<Vec<usize>>>,
) -> Result<(), TransitionError> {
    bundled.clear();
    if let Some(membership) = membership.as_mut() {
        membership.clear();
    }
    scratch.angle_order.clear();
    for (index, sample) in samples.iter().enumerate() {
        ensure(
            sample.address.address().owner() == ProgrammingOwner::Position,
            "composition contains a different owner",
        )?;
        ensure(
            sample.activation_mix.is_finite() && (0.0..=1.0).contains(&sample.activation_mix),
            "Dynamic activation influence must be between zero and one",
        )?;
        sample.validate_value()?;
        if !sample.participates() {
            continue;
        }
        if !sample.fix_at
            && sample.address.address().representation == DynamicFamilyRepresentation::Angles
            && sample.address.address().component.is_some()
        {
            ensure(
                sample.rank.dynamic_identity().is_some(),
                "Angle bundle requires a Dynamic source identity",
            )?;
            scratch.angle_order.push(index);
        } else {
            bundled.push(sample.clone());
            if let Some(membership) = membership.as_mut() {
                membership.push(vec![index]);
            }
        }
    }
    scratch.angle_order.sort_unstable_by_key(|&i| {
        samples[i]
            .rank
            .dynamic_identity()
            .expect("validated Dynamic Angle source")
    });
    let mut start = 0;
    while start < scratch.angle_order.len() {
        let first = &samples[scratch.angle_order[start]];
        let mut end = start + 1;
        while end < scratch.angle_order.len() {
            let next = &samples[scratch.angle_order[end]];
            let next_identity = next
                .rank
                .dynamic_identity()
                .expect("validated Dynamic Angle source");
            let first_identity = first
                .rank
                .dynamic_identity()
                .expect("validated Dynamic Angle source");
            if (next_identity.instance_id, next_identity.controller_id)
                != (first_identity.instance_id, first_identity.controller_id)
            {
                break;
            }
            end += 1;
        }
        let mut axes = [None, None];
        for &i in &scratch.angle_order[start..end] {
            let sample = &samples[i];
            ensure(
                sample.rank.priority == first.rank.priority
                    && sample.rank.changed_at_millis == first.rank.changed_at_millis
                    && sample.rank.changed_at_submillis_nanos
                        == first.rank.changed_at_submillis_nanos
                    && sample.rank.stable_order == first.rank.stable_order
                    && sample.activation_mix == first.activation_mix,
                "an Angle bundle must share its controller rank and activation influence",
            )?;
            let axis = match sample.address.address().component {
                Some(ProgrammingComponent::Pan) => 0,
                Some(ProgrammingComponent::Tilt) => 1,
                _ => unreachable!("validated Angle component"),
            };
            let Some(DynamicValue::Scalar(value)) = sample.materialized_value() else {
                unreachable!("validated Angle value")
            };
            ensure(
                axes[axis].replace(*value).is_none(),
                "duplicate axis in one Angle bundle",
            )?;
        }
        if let [Some(pan), Some(tilt)] = axes {
            let origins = tracing.then(|| {
                scratch.angle_order[start..end]
                    .iter()
                    .flat_map(|&index| samples[index].trace_sources())
                    .collect::<Vec<_>>()
            });
            let address = if let Some(address) = &scratch.angle_address {
                address.clone()
            } else {
                let address = Arc::new(CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Angles,
                        component: None,
                    },
                    None,
                )?);
                scratch.angle_address = Some(address.clone());
                address
            };
            let mut bundled_sample = FamilySample::new(
                address,
                DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
                    pan, tilt,
                )))),
                samples[scratch.angle_order[end - 1]].rank,
                first.activation_mix,
            )?;
            if let Some(origins) = origins {
                bundled_sample.trace_sources = Some(origins.into());
            }
            bundled.push(bundled_sample);
            if let Some(membership) = membership.as_mut() {
                membership.push(scratch.angle_order[start..end].to_vec());
            }
        }
        start = end;
    }
    Ok(())
}

fn validate_source_order(a: &FamilySample, b: &FamilySample) -> Result<(), IntentError> {
    ensure(
        a.rank != b.rank
            || matches!((&a.projection, &b.projection), (Some(a_origin), Some(b_origin))
                if Arc::ptr_eq(a_origin, b_origin)
                    && a.address.address().component != b.address.address().component),
        "Dynamic composition requires distinct stable source/lane identities or verified component fragments",
    )
}

/// Activate one whole-family sample over its eligible underlay. The underlay is first adopted
/// into the sample's own representation and interpolated natively. Only when that adoption is
/// specifically `Requires(ColorAppearance)` for a whole Color sample, and the caller supplied
/// its pinned frame resolver, is the original from/to Transition sampled by that resolver.
fn apply_whole(
    underlay: AttributeValue,
    sample: &FamilySample,
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
    frame: Option<&dyn crate::WholeFamilyExpressionFrameResolver>,
    traced: bool,
) -> Result<whole_activation::WholeActivation, TransitionError> {
    let Some(DynamicValue::Family(target)) = sample.materialized_value() else {
        return Err(IntentError("whole-family sample has a component value".into()).into());
    };
    if sample.activation_mix == 1.0 {
        return Ok(whole_activation::WholeActivation::native(target.clone()));
    }
    let original = frame
        .filter(|_| whole_activation::routes_color_appearance(sample.address.address(), &underlay))
        .map(|frame| (frame, underlay.clone()));
    let from = match adopt(underlay, sample.address.address(), context, original_base) {
        Ok(from) => from,
        Err(TransitionError::Requires(TransitionRequirement::ColorAppearance))
            if original.is_some() =>
        {
            let (frame, underlay) = original.expect("checked frame route");
            return whole_activation::appearance_transition(
                &underlay, target, sample, frame, traced,
            );
        }
        Err(error) => return Err(error),
    };
    CompiledProgrammingTransition::new(from, target.clone(), sample.address.native_model())?
        .sample(sample.activation_mix)
        .map(whole_activation::WholeActivation::native)
}

fn orthogonal(address: &DynamicValueAddress) -> bool {
    address
        .component
        .is_some_and(|component| component.descriptor().role == ComponentRole::ColorOrthogonal)
}

fn target_reference(sample: &FamilySample) -> Option<TargetReference> {
    if let Some(DynamicValue::Family(AttributeValue::Position(position))) =
        sample.materialized_value()
        && let PositionIntent::Target { reference, .. } = position.as_ref()
    {
        return Some(*reference);
    }
    match sample.address.address().representation {
        DynamicFamilyRepresentation::Target { reference } => reference,
        _ => None,
    }
}

fn compatible(sample: &FamilySample, winner: &FamilySample) -> bool {
    use DynamicFamilyRepresentation as R;
    if let (
        Some(ProgrammingComponent::NativeColor(candidate)),
        Some(ProgrammingComponent::NativeColor(selected)),
    ) = (
        sample.address.address().component,
        winner.address.address().component,
    ) && candidate.channel_id == selected.channel_id
        && candidate.function_id != selected.function_id
        && !(native_fixed::is_native_fixed(winner) && winner.activation_mix < 1.0)
    {
        return false;
    }
    match (
        &sample.address.address().representation,
        &winner.address.address().representation,
    ) {
        (R::SemanticColor { basis: a }, R::SemanticColor { basis: b }) => {
            orthogonal(sample.address.address())
                || a == b
                || *a == DynamicSemanticColorBasis::Retain
                || *b == DynamicSemanticColorBasis::Retain
        }
        (R::Target { .. }, R::Target { .. }) => {
            target_reference(sample) == target_reference(winner)
        }
        (candidate, winner) => candidate == winner,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod captured_endpoint_control_tests {
    use super::*;

    #[test]
    fn captured_gate_keeps_fixed_and_derived_exemptions_lazy() {
        let mut sample = FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: crate::DynamicFamilyRepresentation::Angles,
                        component: Some(ProgrammingComponent::Pan),
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Scalar(10.),
            FamilySampleRank {
                priority: 10,
                changed_at_millis: 1,
                changed_at_submillis_nanos: 0,
                stable_order: 1,
                identity: crate::FamilySampleIdentity::Dynamic {
                    instance_id: Uuid::new_v4(),
                    controller_id: Uuid::new_v4(),
                    lane_id: Uuid::new_v4(),
                },
            },
            0.5,
        )
        .unwrap();
        assert!(matches!(
            sample.captured_endpoint_control(|rank| {
                assert_eq!(rank, sample.rank);
                FamilyEndpointOutputControl::Suppressed
            }),
            FamilyEndpointOutputControl::Suppressed
        ));
        sample.fix_at = true;
        assert!(matches!(
            sample.captured_endpoint_control(|_| panic!("Fixed gate is exempt")),
            FamilyEndpointOutputControl::Unchanged
        ));
        sample.fix_at = false;
        sample.endpoint_output_exempt = true;
        assert!(matches!(
            sample.captured_endpoint_control(|_| panic!("Derived gate was already applied")),
            FamilyEndpointOutputControl::Unchanged
        ));
    }
}
