//! TL-578: a materialized typed leaf keeps its captured authored occurrence and Current
//! calculation dependency through preparation and traced composition.
use super::*;

const AUTHORED: u128 = 101;
const CURRENT: u128 = 202;

/// Materialized leaves never query Current, Presets or physical frames again.
struct NoSources;
impl DynamicValueSourceResolver for NoSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("materialized leaf must not query Current again")
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        panic!("materialized leaf must not query Presets")
    }
}

fn id(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}

fn component_address() -> Arc<DynamicValueAddress> {
    Arc::new(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: Some(ProgrammingComponent::Focus),
    })
}

fn focus_leaf(
    whole_family: bool,
    occurrence: Option<DynamicSourceOccurrenceId>,
    dependency: Option<DynamicSourceDependency>,
) -> E {
    let (address, value) = if whole_family {
        let value = AttributeValue::Normalized(0.6);
        (
            Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &value).unwrap()),
            DynamicValue::Family(value),
        )
    } else {
        (component_address(), DynamicValue::Scalar(0.6))
    };
    E::Programming {
        address,
        value,
        occurrence,
        dependency_occurrence: dependency,
    }
}

fn identified(whole_family: bool) -> E {
    focus_leaf(
        whole_family,
        Some(id(AUTHORED)),
        Some(DynamicSourceDependency::identity(Some(id(CURRENT)))),
    )
}

fn retained(leaf: E) -> E {
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(leaf)]).unwrap();
    E::Retained {
        root: tape.roots()[0],
        tape: Arc::new(tape),
    }
}

/// An exactly completed Resume whose outgoing branch is pruned before compilation.
fn completed_resume(leaf: E) -> E {
    resume(
        Some(focus_leaf(false, Some(id(303)), None)),
        Some(leaf),
        1.0,
    )
}

fn focus_fields() -> ProgrammingFieldScope {
    ProgrammingFieldScope::from_component(ProgrammingComponent::Focus)
}

struct Traced {
    value: AttributeValue,
    rank: FamilySampleRank,
    footprint: FamilyTraceFootprint,
    component: Option<Vec<FamilyTraceSource>>,
    detailed: Option<Vec<FamilyTraceContribution>>,
}

fn prepare_and_trace(expression: E, scratch: &mut DynamicFamilyPreparationScratch) -> Traced {
    prepare_and_trace_sample(sample(20, expression), scratch)
}

fn prepare_and_trace_sample(
    sample: DynamicRuntimeSample,
    scratch: &mut DynamicFamilyPreparationScratch,
) -> Traced {
    let prepared = prepare_dynamic_family_samples(&[sample], &NoSources, None, scratch).unwrap();
    assert!(prepared.requirements.is_empty());
    assert!(prepared.legacy.is_empty());
    assert_eq!(prepared.families.len(), 1);
    let group = &prepared.families[0];
    assert_eq!(group.owner, ProgrammingOwner::Focus);
    assert_eq!(group.samples.len(), 1);
    let FamilyCompositionSample::Known(known) = &group.samples[0] else {
        panic!("a materialized leaf remains a Known sample")
    };
    assert!(known.materialized_value().is_some());
    let rank = known.rank;
    let footprint = known
        .address()
        .address()
        .component
        .map_or(FamilyTraceFootprint::Whole, FamilyTraceFootprint::Component);
    let mut composition = RetainedFamilyCompositionScratch::default();
    let value = compose_retained_dynamic_family_traced(
        group.owner,
        &AttributeValue::Normalized(0.1),
        &group.samples,
        &FamilyCompositionContext::default(),
        &Frame,
        &mut composition,
    )
    .unwrap();
    let trace = composition.family_trace();
    let root = trace.root().unwrap();
    Traced {
        value,
        rank,
        footprint,
        component: trace.sources_for_component(root, ProgrammingComponent::Focus),
        detailed: trace.query_fields(root, &focus_fields()),
    }
}

fn source(
    traced: &Traced,
    role: FamilyTraceRole,
    occurrence: Option<DynamicSourceOccurrenceId>,
) -> FamilyTraceSource {
    FamilyTraceSource {
        rank: traced.rank,
        footprint: traced.footprint,
        role,
        occurrence,
    }
}

fn assert_identified(traced: &Traced) {
    assert_eq!(traced.value, AttributeValue::Normalized(0.6));
    let authored = source(traced, FamilyTraceRole::Authored, Some(id(AUTHORED)));
    let dependency = source(
        traced,
        FamilyTraceRole::CalculationDependency,
        Some(id(CURRENT)),
    );
    assert_eq!(
        traced.component.as_deref(),
        Some(&[authored, dependency][..])
    );
    assert_eq!(
        traced.detailed.as_deref(),
        Some(
            &[
                FamilyTraceContribution {
                    source: authored,
                    fields: focus_fields(),
                },
                // Identity transfer: Current's own Focus field was read unchanged.
                FamilyTraceContribution {
                    source: dependency,
                    fields: focus_fields(),
                },
            ][..]
        )
    );
    // Current stays a calculation input; it is never a newly authored Release target.
    assert!(
        traced
            .component
            .iter()
            .flatten()
            .all(|source| source.role != FamilyTraceRole::Authored
                || source.occurrence != Some(id(CURRENT)))
    );
}

#[test]
fn identified_component_leaf_keeps_authored_and_current_roles_on_every_materialized_path() {
    for (path, expression) in [
        ("direct leaf", identified(false)),
        ("retained tape leaf", retained(identified(false))),
        (
            "exactly pruned Resume endpoint",
            completed_resume(identified(false)),
        ),
        (
            "pruned endpoint inside a retained tape",
            retained(completed_resume(identified(false))),
        ),
    ] {
        let traced = prepare_and_trace(expression, &mut DynamicFamilyPreparationScratch::default());
        assert_eq!(
            traced.footprint,
            FamilyTraceFootprint::Component(ProgrammingComponent::Focus),
            "{path}"
        );
        assert_identified(&traced);
    }
}

#[test]
fn identified_whole_family_leaf_keeps_authored_and_current_roles_on_every_materialized_path() {
    for (path, expression) in [
        ("direct leaf", identified(true)),
        ("retained tape leaf", retained(identified(true))),
        (
            "exactly pruned Resume endpoint",
            completed_resume(identified(true)),
        ),
    ] {
        let traced = prepare_and_trace(expression, &mut DynamicFamilyPreparationScratch::default());
        assert_eq!(traced.footprint, FamilyTraceFootprint::Whole, "{path}");
        assert_identified(&traced);
    }
}

#[test]
fn leaf_dependency_evidence_keeps_absent_unknown_and_known_empty_distinct() {
    for whole_family in [false, true] {
        let mut scratch = DynamicFamilyPreparationScratch::default();
        // No dependency record: only the authored source contributes.
        let traced = prepare_and_trace(
            retained(focus_leaf(whole_family, Some(id(AUTHORED)), None)),
            &mut scratch,
        );
        assert_eq!(traced.value, AttributeValue::Normalized(0.6));
        let authored = source(&traced, FamilyTraceRole::Authored, Some(id(AUTHORED)));
        assert_eq!(traced.component.as_deref(), Some(&[authored][..]));

        // A used Current input of unknown identity keeps its role without borrowing an ID.
        let traced = prepare_and_trace(
            completed_resume(focus_leaf(
                whole_family,
                Some(id(AUTHORED)),
                Some(DynamicSourceDependency::identity(None)),
            )),
            &mut scratch,
        );
        assert_eq!(traced.value, AttributeValue::Normalized(0.6));
        assert_eq!(
            traced.component.as_deref(),
            Some(
                &[
                    source(&traced, FamilyTraceRole::Authored, Some(id(AUTHORED))),
                    source(&traced, FamilyTraceRole::CalculationDependency, None),
                ][..]
            )
        );

        // Unknown transfer evidence is not a known empty contributor set.
        let traced = prepare_and_trace(
            focus_leaf(
                whole_family,
                Some(id(AUTHORED)),
                Some(DynamicSourceDependency::unknown(Some(id(CURRENT)))),
            ),
            &mut scratch,
        );
        assert_eq!(traced.value, AttributeValue::Normalized(0.6));
        assert_eq!(traced.component, None);
        assert_eq!(traced.detailed, None);

        // A known transfer which reads no Current field proves no Current contribution.
        let traced = prepare_and_trace(
            focus_leaf(
                whole_family,
                Some(id(AUTHORED)),
                Some(DynamicSourceDependency::mapped(
                    Some(id(CURRENT)),
                    ProgrammingFieldTransfer {
                        identity: ProgrammingFieldScope::empty(),
                        remap: Arc::from([]),
                    },
                )),
            ),
            &mut scratch,
        );
        assert_eq!(traced.value, AttributeValue::Normalized(0.6));
        let authored = source(&traced, FamilyTraceRole::Authored, Some(id(AUTHORED)));
        assert_eq!(traced.component.as_deref(), Some(&[authored][..]));

        // Unattributed legacy history remains Unknown, never a borrowed identity.
        let traced = prepare_and_trace(focus_leaf(whole_family, None, None), &mut scratch);
        assert_eq!(
            traced.component.as_deref(),
            Some(&[source(&traced, FamilyTraceRole::Authored, None)][..])
        );
    }
}

#[test]
fn cached_leaf_provenance_follows_the_current_frame_rank() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let expression = retained(identified(false));
    let first = prepare_and_trace(expression.clone(), &mut scratch);
    assert_identified(&first);
    // The same immutable expression reuses its compiled sample under a newer arbitration rank.
    let mut later = sample(20, expression);
    later.priority = 30;
    later.activated_at_millis = 900;
    let second = prepare_and_trace_sample(later, &mut scratch);
    assert_ne!(first.rank, second.rank);
    assert_identified(&second);
}
