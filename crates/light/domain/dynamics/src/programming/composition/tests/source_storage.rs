//! Frozen allocating source/trace oracle across provenance storage and retained transfers.
use super::*;
use crate::{
    DynamicSourceDependency, DynamicSourceOccurrenceId, FamilyExpressionOperation,
    WholeFamilyExpressionFrameResolver,
};
use light_core::programming::ProgrammingTraceField;

fn occurrence(id: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(id)).unwrap()
}
fn old_provenance(
    mut sample: FamilySample,
    occurrence: Option<DynamicSourceOccurrenceId>,
    dependency: Option<&DynamicSourceDependency>,
) -> FamilySample {
    if occurrence.is_none() && dependency.is_none() {
        return sample;
    }
    let authored = FamilyTraceSource {
        rank: sample.rank,
        footprint: sample.footprint(),
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
    sample.trace_sources = Some(leaves.into());
    sample
}
fn old_sources(sample: &FamilySample) -> Vec<trace::FamilyTraceLeaf> {
    sample
        .trace_sources
        .as_deref()
        .map(|s| s.to_vec())
        .unwrap_or_else(|| {
            vec![trace::FamilyTraceLeaf::Source(FamilyTraceSource {
                rank: sample.rank,
                footprint: sample.footprint(),
                role: FamilyTraceRole::Authored,
                occurrence: None,
            })]
        })
}
fn old_trace(sample: &FamilySample, arena: &mut FamilyTraceArena) -> FamilyTraceNodeId {
    if let Some(node) = sample.trace_node {
        return node;
    }
    let mut leaves = old_sources(sample)
        .into_iter()
        .map(|leaf| arena.leaf(leaf))
        .collect::<Vec<_>>();
    let appearance = if leaves.len() == 1 {
        leaves.pop().unwrap()
    } else {
        arena.bundle(leaves)
    };
    arena.control(sample.rank, sample.footprint(), appearance, None)
}
fn scopes() -> Vec<ProgrammingFieldScope> {
    use ProgrammingTraceField as F;
    let fields = [
        F::ColorXyz,
        F::ColorRecipeRed,
        F::ColorRecipeGreen,
        F::ColorRecipeBlue,
        F::ColorRecipeAmber,
        F::WhiteBlend,
        F::Temperature,
        F::Duv,
        F::Uv,
        F::RelativeOutput,
        F::Allocation,
        F::ColorWheels,
        F::ColorWheel(1),
        F::Pan,
        F::Tilt,
        F::TargetReference,
        F::TargetX,
        F::TargetY,
        F::TargetZ,
        F::Focus,
        F::Zoom,
        F::ZoomConvention,
        F::NativeColorChannel(Uuid::from_u128(99)),
        F::NativeColorIdentity,
        F::NativePrediction,
    ];
    let mut scopes = fields
        .iter()
        .map(|field| ProgrammingFieldScope::new([*field]))
        .collect::<Vec<_>>();
    scopes.push(ProgrammingFieldScope::new(fields));
    scopes
}
fn compare_trace(
    a: &FamilyTraceArena,
    x: FamilyTraceNodeId,
    b: &FamilyTraceArena,
    y: FamilyTraceNodeId,
) {
    assert_eq!(
        x, y,
        "leaf, bundle and control node order must remain exact"
    );
    for scope in scopes() {
        assert_eq!(
            a.query_fields_with_base(x, &scope),
            b.query_fields_with_base(y, &scope)
        );
        assert_eq!(
            a.control_sources_for_fields(x, &scope),
            b.control_sources_for_fields(y, &scope)
        );
    }
}
fn compare(new: &FamilySample, old: &FamilySample) {
    assert_eq!(
        format!("{:?}", new.trace_sources),
        format!("{:?}", old.trace_sources)
    );
    assert_eq!(
        format!("{:?}", new.trace_sources().collect::<Vec<_>>()),
        format!("{:?}", old_sources(old))
    );
    let mut a = FamilyTraceArena::default();
    let mut b = FamilyTraceArena::default();
    let x = sample_trace(new, &mut a);
    let y = old_trace(old, &mut b);
    compare_trace(&a, x, &b, y);
}
fn cases() -> Vec<(FamilySample, FamilySample)> {
    let base = color(ColorComponent::Red, 0.4, 1);
    let known = DynamicSourceDependency::compatible(Some(occurrence(2)), base.address().address());
    let unknown = DynamicSourceDependency::unknown(Some(occurrence(3)));
    let mut cases = Vec::new();
    for dependency in [None, Some(&known), Some(&unknown)] {
        for authored in [None, Some(occurrence(1))] {
            cases.push((
                base.clone().with_leaf_provenance(authored, dependency),
                old_provenance(base.clone(), authored, dependency),
            ));
        }
    }
    let empty = base.clone().with_trace_sources(Arc::from([]));
    cases.push((empty.clone(), empty));
    let sources = [
        FamilyTraceSource {
            rank: base.rank,
            footprint: base.footprint(),
            role: FamilyTraceRole::Authored,
            occurrence: Some(occurrence(4)),
        },
        FamilyTraceSource {
            rank: color(ColorComponent::Green, 0.5, 2).rank,
            footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Color(
                ColorComponent::Green,
            )),
            role: FamilyTraceRole::CalculationDependency,
            occurrence: Some(occurrence(5)),
        },
    ];
    let multi = base.with_trace_sources(Arc::from(sources));
    cases.push((multi.clone(), multi));
    cases
}
#[test]
fn all_source_storage_shapes_keep_exact_leaf_order_and_full_field_transfers() {
    for (new, old) in cases() {
        compare(&new, &old);
        compare(&new.clone(), &old.clone());
        let mut a = FamilyTraceArena::default();
        let mut b = FamilyTraceArena::default();
        let x = sample_trace(&new, &mut a);
        let y = old_trace(&old, &mut b);
        let mut reused_new = new.clone();
        let mut reused_old = old.clone();
        reused_new.trace_node = Some(x);
        reused_old.trace_node = Some(y);
        assert_eq!(sample_trace(&reused_new, &mut a), x);
        assert_eq!(old_trace(&reused_old, &mut b), y);
        compare_trace(&a, x, &b, y);
    }
    let empty = color(ColorComponent::Red, 0.4, 1).with_trace_sources(Arc::from([]));
    assert_eq!(
        empty.trace_sources().count(),
        0,
        "explicit absence cannot invent authored fallback"
    );
    assert_eq!(
        color(ColorComponent::Red, 0.4, 1).trace_sources().count(),
        1
    );
}
#[test]
fn captured_rank_and_component_survive_rank_edits_and_address_rebinding() {
    for (mut new, mut old) in cases() {
        let rank = color(ColorComponent::Blue, 0.7, 7).rank;
        new.rank = rank;
        old.rank = rank;
        compare(&new, &old); // A public rank assignment must not rewrite captured provenance.
        let next = color(ColorComponent::Blue, 0.7, 8).rank;
        new.set_rank(next);
        old.set_rank(next);
        compare(&new, &old);
        let address = color(ColorComponent::Green, 0.4, 1).address;
        new.address = address.clone();
        old.address = address;
        compare(&new, &old); // The control footprint changes; captured source footprints do not.
    }
    let mut sample =
        color(ColorComponent::Red, 0.4, 1).with_leaf_provenance(Some(occurrence(1)), None);
    let mut old = old_provenance(
        color(ColorComponent::Red, 0.4, 1),
        Some(occurrence(1)),
        None,
    );
    let rank = color(ColorComponent::Red, 0.4, 9).rank;
    sample.set_rank(rank);
    old.set_rank(rank);
    compare(&sample, &old); // Matching captured old ranks still follow the explicit setter.
}
struct NoFrame;
impl WholeFamilyExpressionFrameResolver for NoFrame {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("literal samples must not query a frame resolver")
    }
}
#[test]
fn synthetic_angle_bundles_and_retained_position_keep_original_axis_provenance() {
    let pan = angle(ProgrammingComponent::Pan, 30., 1);
    let mut tilt = angle(ProgrammingComponent::Tilt, 20., 2);
    tilt.rank = pan.rank.with_dynamic_lane(Uuid::from_u128(500)).unwrap();
    let known = DynamicSourceDependency::compatible(Some(occurrence(4)), tilt.address().address());
    let new = vec![
        pan.clone().with_leaf_provenance(Some(occurrence(1)), None),
        tilt.clone()
            .with_leaf_provenance(Some(occurrence(2)), Some(&known)),
    ];
    let old = vec![
        old_provenance(pan, Some(occurrence(1)), None),
        old_provenance(tilt, Some(occurrence(2)), Some(&known)),
    ];
    let mut a = Vec::new();
    let mut b = Vec::new();
    bundle_angles(
        &new,
        &mut a,
        &mut ComponentCompositionScratch::default(),
        true,
    )
    .unwrap();
    bundle_angles(
        &old,
        &mut b,
        &mut ComponentCompositionScratch::default(),
        true,
    )
    .unwrap();
    assert_eq!(a.len(), 1);
    compare(&a[0], &b[0]);
    let context = FamilyCompositionContext::default();
    let mut x = RetainedFamilyCompositionScratch::default();
    let mut y = RetainedFamilyCompositionScratch::default();
    let input = |samples: &[FamilySample]| {
        samples
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>()
    };
    let left = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &angles(0., 0.),
        &input(&new),
        &context,
        &NoFrame,
        &mut x,
    )
    .unwrap();
    let right = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &angles(0., 0.),
        &input(&old),
        &context,
        &NoFrame,
        &mut y,
    )
    .unwrap();
    assert_eq!(left, right);
    compare_trace(
        x.family_trace(),
        x.family_trace().root().unwrap(),
        y.family_trace(),
        y.family_trace().root().unwrap(),
    );
}
