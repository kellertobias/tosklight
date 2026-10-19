//! Borrowed lowering retains whole-controller validation before any publication.
use super::super::controller::{Keep, PreparationOutput, Preparer, Previous};
use super::*;

struct Witness<'a> {
    original: &'a Arc<DynamicValueAddress>,
    counts: Vec<usize>,
}
impl PreparationOutput for Witness<'_> {
    fn append_one(&mut self, _: FixtureId, _: ProgrammingOwner, _: FamilyCompositionSample) {
        self.counts.push(Arc::strong_count(self.original));
    }
    fn append(&mut self, _: FixtureId, _: ProgrammingOwner, _: Vec<FamilyCompositionSample>) {
        panic!("plain leaf must append one")
    }
    fn legacy(&mut self, _: DynamicRuntimeSample) {
        panic!("not legacy")
    }
    fn require(&mut self, _: DynamicFamilyPreparationRequirement) {
        panic!("not unavailable")
    }
}
fn witness(input: &[DynamicRuntimeSample], force_old: bool) -> (Result<(), String>, Vec<usize>) {
    let E::Programming { address, .. } = &input[0].expression else {
        panic!()
    };
    let mut output = Witness {
        original: address,
        counts: Vec::new(),
    };
    let (mut controller, mut position, mut previous, mut cache) = (
        Vec::new(),
        Vec::new(),
        HashMap::default(),
        HashMap::default(),
    );
    let indices = (0..input.len()).collect::<Vec<_>>();
    let mut run = || {
        Preparer {
            controller: &mut controller,
            position: &mut position,
            sampling_requirements: &[],
            previous: Previous::Owned(&mut previous),
            keep: Keep::Cache(&mut cache),
            provenance: crate::programming::composition::LeafProvenance::Fresh,
            out: &mut output,
        }
        .prepare_controller(input, &indices, &Sources::default(), None)
        .map_err(|e| format!("{e:?}"))
    };
    let result = if force_old { old(run) } else { run() };
    (result, output.counts)
}
#[test]
fn borrowed_plain_controller_does_not_hold_a_temporary_source_address_copy() {
    let input = [sample(10, color(ColorComponent::Red, 0.4))];
    let E::Programming { address, .. } = &input[0].expression else {
        panic!()
    };
    let before = Arc::strong_count(address);
    assert_eq!(
        witness(&input, false),
        (Ok(()), vec![before]),
        "append_one must borrow the original source, without its temporary controller clone"
    );
    assert_eq!(
        witness(&input, true),
        (Ok(()), vec![before + 1]),
        "the forced old oracle must still exercise the copy"
    );
    let mut late_invalid = vec![
        input[0].clone(),
        sample(11, color(ColorComponent::Green, 0.5)),
    ];
    late_invalid[1].activation_mix = f32::NAN;
    for force_old in [false, true] {
        let (error, writes) = witness(&late_invalid, force_old);
        assert!(error.is_err());
        assert!(
            writes.is_empty(),
            "all controller validation precedes append_one"
        );
    }
}

thread_local! { static FORCE_OLD: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn forced_old() -> bool {
    FORCE_OLD.with(Cell::get)
}
fn old<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCE_OLD.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(FORCE_OLD.with(|flag| flag.replace(true)));
    run()
}

fn selected(component: ColorComponent, value: f32) -> E {
    let mut expression = color(component, value);
    if component == ColorComponent::WhiteBlend {
        let E::Programming { address, .. } = &mut expression else {
            unreachable!()
        };
        Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Recipe,
        };
    }
    expression
}
fn inputs(frame: u128) -> Vec<DynamicRuntimeSample> {
    let components = [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
        ColorComponent::Amber,
        ColorComponent::WhiteBlend,
        ColorComponent::Uv,
    ];
    (0..8)
        .flat_map(|target| {
            components
                .iter()
                .take(if target < 3 {
                    3
                } else if target < 6 {
                    4
                } else {
                    6
                })
                .enumerate()
                .map(move |(lane, component)| {
                    let mut s = sample(
                        10 + lane as u128,
                        selected(*component, 0.1 + frame as f32 * 0.2),
                    );
                    s.target = FixtureId(Uuid::from_u128(100 + target));
                    s.priority += frame as i16 + lane as i16;
                    s.activated_at_millis += frame as u64;
                    s.activation_mix = 1.0 - frame as f32 * 0.2;
                    if let E::Programming {
                        address,
                        occurrence,
                        dependency_occurrence,
                        ..
                    } = &mut s.expression
                    {
                        *occurrence = Some(
                            DynamicSourceOccurrenceId::new(Uuid::from_u128(200 + frame)).unwrap(),
                        );
                        *dependency_occurrence = Some(DynamicSourceDependency::compatible(
                            Some(
                                DynamicSourceOccurrenceId::new(Uuid::from_u128(300 + frame))
                                    .unwrap(),
                            ),
                            address,
                        ));
                    }
                    s
                })
        })
        .collect()
}
fn requirement(input: &DynamicRuntimeSample) -> DynamicFamilyPreparationRequirement {
    DynamicFamilyPreparationRequirement {
        target: input.target,
        owner: ProgrammingOwner::Color,
        rank: rank(input),
        reason: DynamicFamilyPreparationRequirementReason::Transition(
            TransitionRequirement::CompatibleOwners,
        ),
    }
}
#[test]
fn borrowed_plain_controller_matches_owned_samples_traces_requirements_and_worker_replay() {
    for chunks in [0, 1, 2, 4] {
        for stop in [false, true] {
            let (mut fast, mut reference) = (
                DynamicFamilyPreparationScratch::default(),
                DynamicFamilyPreparationScratch::default(),
            );
            for frame in 0..3 {
                let input = inputs(frame);
                let requirements = [requirement(&input[4])];
                let expected =
                    old(|| prepare_observed(&input, &requirements, &mut reference, chunks, stop));
                let actual = prepare_observed(&input, &requirements, &mut fast, chunks, stop);
                assert_eq!(actual, expected);
                assert!(actual.0.is_ok());
                assert_eq!(actual.1, 0);
                assert!(fast.cache.is_empty());
            }
        }
    }
}
#[test]
fn borrowed_plain_controller_validates_before_lowering_and_retries_after_error() {
    for chunks in [0, 1, 2, 4] {
        let (mut fast, mut reference) = (
            DynamicFamilyPreparationScratch::default(),
            DynamicFamilyPreparationScratch::default(),
        );
        for fault in 0..7 {
            let mut input = inputs(0);
            input[0].expression = selected(ColorComponent::Red, f32::NAN);
            match fault {
                0 => input[1].lane_id = input[0].lane_id,
                1 => input[1].activation_mix = f32::NAN,
                2 => {
                    if let E::Programming {
                        dependency_occurrence,
                        ..
                    } = &mut input[1].expression
                    {
                        *dependency_occurrence = Some(DynamicSourceDependency::mapped(
                            None,
                            ProgrammingFieldTransfer {
                                identity: ProgrammingFieldScope::from_component(
                                    ProgrammingComponent::Focus,
                                ),
                                remap: Arc::from([]),
                            },
                        ));
                    }
                }
                3 => {}
                4 => input[0].instance_id = Uuid::nil(),
                _ => {
                    input.truncate(3);
                    for sample in &mut input {
                        sample.instance_id = Uuid::nil();
                    }
                    input[0].expression = selected(ColorComponent::Red, 0.4);
                    if fault == 5 {
                        input[1].activation_mix = f32::NAN;
                    } else if let E::Programming {
                        dependency_occurrence,
                        ..
                    } = &mut input[1].expression
                    {
                        *dependency_occurrence = Some(DynamicSourceDependency::mapped(
                            None,
                            ProgrammingFieldTransfer {
                                identity: ProgrammingFieldScope::from_component(
                                    ProgrammingComponent::Focus,
                                ),
                                remap: Arc::from([]),
                            },
                        ));
                    }
                }
            }
            let expected = old(|| prepare_observed(&input, &[], &mut reference, chunks, false));
            let actual = prepare_observed(&input, &[], &mut fast, chunks, false);
            assert_eq!(actual, expected);
            assert!(actual.0.is_err());
            assert!(fast.families.is_empty() && fast.controller.is_empty());
            let valid = inputs(2);
            assert_eq!(
                prepare_observed(&valid, &[], &mut fast, chunks, false),
                old(|| prepare_observed(&valid, &[], &mut reference, chunks, false))
            );
        }
        let mut invalid = inputs(0);
        invalid[0].activation_mix = f32::NAN;
        let req = [requirement(&invalid[0])];
        assert_eq!(
            prepare_observed(&invalid, &req, &mut fast, chunks, false),
            old(|| prepare_observed(&invalid, &req, &mut reference, chunks, false))
        );
        assert!(
            prepare_observed(&invalid, &req, &mut fast, chunks, false)
                .0
                .is_err()
        );
    }
}
#[test]
fn borrowed_plain_controller_mixed_wrapped_position_and_native_fall_back_unchanged() {
    let leaf = selected(ColorComponent::Red, 0.4);
    let model = models();
    let native = E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: model.model.source.clone(),
            },
            component: Some(ProgrammingComponent::NativeColor(model.model.binding)),
        }),
        value: DynamicValue::Native(12),
        occurrence: None,
        dependency_occurrence: None,
    };
    for expression in [
        E::Operation {
            origin: None,
            value: Arc::new(leaf.clone()),
        },
        resume(
            Some(leaf.clone()),
            Some(selected(ColorComponent::Blue, 0.7)),
            0.5,
        ),
        angle(ProgrammingComponent::Pan, 30.),
        current(ProgrammingComponent::Tilt),
        native,
        E::LegacyScalar {
            attribute: AttributeKey::intensity(),
            value: 0.5,
            occurrence: None,
            dependency_occurrence: None,
        },
    ] {
        let input = [sample(10, leaf.clone()), sample(11, expression)];
        for chunks in [0, 1, 2, 4] {
            let (mut fast, mut reference) = (
                DynamicFamilyPreparationScratch::default(),
                DynamicFamilyPreparationScratch::default(),
            );
            assert_eq!(
                prepare_observed(&input, &[], &mut fast, chunks, false),
                old(|| prepare_observed(&input, &[], &mut reference, chunks, false))
            );
        }
    }
    assert!(std::panic::catch_unwind(|| old(|| panic!("forced old restoration"))).is_err());
    assert!(!forced_old());
}
