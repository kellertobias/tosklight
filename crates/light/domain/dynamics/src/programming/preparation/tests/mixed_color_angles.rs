//! Mixed controllers reuse the existing Color scalar slots without touching Angle pairing.
use super::*;
use std::cell::RefCell;

fn general<T>(work: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            MIXED_PLAIN_GENERAL_ONLY.with(|v| v.set(self.0));
        }
    }
    let _reset = Reset(MIXED_PLAIN_GENERAL_ONLY.with(|v| v.replace(true)));
    work()
}
fn mixed(value: f32) -> Vec<DynamicRuntimeSample> {
    let mut result = vec![
        sample(10, angle(ProgrammingComponent::Pan, 30.)),
        sample(11, angle(ProgrammingComponent::Tilt, 20.)),
    ];
    for (index, component) in [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
        ColorComponent::WhiteBlend,
    ]
    .into_iter()
    .enumerate()
    {
        let mut expression = color(component, value);
        let E::Programming {
            address,
            occurrence,
            dependency_occurrence,
            ..
        } = &mut expression
        else {
            unreachable!()
        };
        Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Recipe,
        };
        *occurrence =
            Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(100 + index as u128)).unwrap());
        *dependency_occurrence = Some(DynamicSourceDependency::identity(Some(
            DynamicSourceOccurrenceId::new(Uuid::from_u128(200 + index as u128)).unwrap(),
        )));
        result.push(sample(20 + index as u128, expression));
    }
    result.push(sample(
        30,
        E::LegacyScalar {
            attribute: AttributeKey::intensity(),
            value: 0.7,
            occurrence: None,
            dependency_occurrence: None,
        },
    ));
    result
}
struct Reads {
    inner: Sources,
    events: RefCell<Vec<String>>,
}
impl Reads {
    fn new(available: bool) -> Self {
        Self {
            inner: Sources {
                pan: available.then_some(70.),
                tilt: available.then_some(80.),
                ..Default::default()
            },
            events: RefCell::new(Vec::new()),
        }
    }
}
impl DynamicValueSourceResolver for Reads {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.events
            .borrow_mut()
            .push(format!("current:{target:?}:{address:?}"));
        self.inner.current(target, address)
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        panic!("prepared mixed leaves must not query Presets")
    }
    fn current_dependency(
        &self,
        target: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        self.events
            .borrow_mut()
            .push(format!("dependency:{target:?}:{address:?}"));
        self.inner.current_dependency(target, address)
    }
}
#[derive(Debug, PartialEq)]
struct Observed {
    groups: Vec<(
        FixtureId,
        ProgrammingOwner,
        Vec<String>,
        String,
        Vec<String>,
    )>,
    legacy: Vec<DynamicRuntimeSample>,
    requirements: Vec<DynamicFamilyPreparationRequirement>,
}
fn observed(prepared: PreparedDynamicFamilySamples<'_>) -> Observed {
    let groups = prepared
        .families
        .iter()
        .map(|group| {
            let metadata = group
                .samples
                .iter()
                .map(|sample| match sample {
                    FamilyCompositionSample::Known(leaf) => format!(
                        "known/{:?}/{:?}/{:?}/{:?}",
                        leaf.address().address(),
                        leaf.materialized_value(),
                        leaf.rank,
                        leaf.activation_mix
                    ),
                    FamilyCompositionSample::CoupledExpression {
                        rank,
                        activation_mix,
                        ..
                    } => format!("coupled/{rank:?}/{activation_mix:?}"),
                    FamilyCompositionSample::WholeExpression {
                        rank,
                        activation_mix,
                        ..
                    } => format!("whole/{rank:?}/{activation_mix:?}"),
                })
                .collect();
            let base = if group.owner == ProgrammingOwner::Position {
                angles(0., 0.)
            } else {
                semantic(ColorIntent::default())
            };
            let mut composition = RetainedFamilyCompositionScratch::default();
            let result = compose_retained_dynamic_family_traced(
                group.owner,
                &base,
                &group.samples,
                &FamilyCompositionContext {
                    edit: FamilyEditContext {
                        color_model: Some(&VirtualColorAuthoringV1),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                &Frame,
                &mut composition,
            );
            let arena = composition.family_trace();
            let components = if group.owner == ProgrammingOwner::Position {
                vec![ProgrammingComponent::Pan, ProgrammingComponent::Tilt]
            } else {
                [
                    ColorComponent::Red,
                    ColorComponent::Green,
                    ColorComponent::Blue,
                    ColorComponent::WhiteBlend,
                    ColorComponent::Hue,
                    ColorComponent::Amber,
                    ColorComponent::Uv,
                ]
                .into_iter()
                .map(ProgrammingComponent::Color)
                .collect()
            };
            let trace = components
                .into_iter()
                .map(|component| {
                    format!(
                        "{component:?}/{:?}",
                        arena.root().and_then(|root| arena
                            .query_fields(root, &ProgrammingFieldScope::from_component(component)))
                    )
                })
                .collect();
            (
                group.target,
                group.owner,
                metadata,
                format!("{result:?}"),
                trace,
            )
        })
        .collect();
    Observed {
        groups,
        legacy: prepared.legacy.to_vec(),
        requirements: prepared.requirements.to_vec(),
    }
}
fn prepare(
    samples: &[DynamicRuntimeSample],
    requirements: &[DynamicFamilyPreparationRequirement],
    sources: &Reads,
    scratch: &mut DynamicFamilyPreparationScratch,
) -> Result<Observed, String> {
    prepare_dynamic_family_samples_with_requirements(samples, requirements, sources, None, scratch)
        .map(observed)
        .map_err(|e| format!("{e:?}"))
}
fn compare(
    samples: &[DynamicRuntimeSample],
    requirements: &[DynamicFamilyPreparationRequirement],
    available: bool,
    a: &mut DynamicFamilyPreparationScratch,
    b: &mut DynamicFamilyPreparationScratch,
) -> Result<Observed, String> {
    let x = Reads::new(available);
    let y = Reads::new(available);
    let fast = prepare(samples, requirements, &x, a);
    let slow = general(|| prepare(samples, requirements, &y, b));
    assert_eq!(fast, slow);
    assert_eq!(*x.events.borrow(), *y.events.borrow());
    fast
}
#[test]
fn mixed_angle_four_color_leaves_use_slots_with_fresh_values_ranks_and_full_provenance() {
    let mut fast = DynamicFamilyPreparationScratch::default();
    let mut old = DynamicFamilyPreparationScratch::default();
    let mut first = None;
    for (frame, value) in [0.2, 0.8, 0.8, 0.4].into_iter().enumerate() {
        let mut samples = mixed(value);
        for sample in &mut samples {
            sample.priority += frame as i16;
            sample.activation_mix = 1. - frame as f32 * 0.15;
        }
        MIXED_PLAIN_HITS.with(|v| v.set(0));
        let output = compare(&samples, &[], true, &mut fast, &mut old).unwrap();
        assert_eq!(
            MIXED_PLAIN_HITS.with(Cell::get),
            4,
            "fail-first: all four Color leaves must use the mixed-controller slot path"
        );
        assert_eq!(output.groups.len(), 2);
        assert_eq!(output.legacy.len(), 1);
        assert!(output.requirements.is_empty());
        assert!(fast.cache.is_empty());
        assert_eq!(
            old.cache.len(),
            4,
            "the frozen general route retains one expression per Color lane"
        );
        let color = fast
            .families
            .iter()
            .find(|group| group.owner == ProgrammingOwner::Color)
            .unwrap();
        let FamilyCompositionSample::Known(leaf) = &color.samples[0] else {
            panic!("known Color")
        };
        let address = leaf.address() as *const _;
        if let Some(previous) = first {
            assert_eq!(address, previous);
        } else {
            first = Some(address);
        }
        assert_eq!(
            leaf.materialized_value(),
            Some(&DynamicValue::Scalar(value))
        );
    }
}
#[test]
fn mixed_current_sampling_requirements_and_wrapped_or_unsupported_leaves_keep_general_fallback() {
    let plain = mixed(0.4);
    let mut cases = vec![plain.clone()];
    let mut current = plain.clone();
    current[1].expression = super::current(ProgrammingComponent::Tilt);
    cases.push(current);
    let mut wrapper = plain.clone();
    wrapper[2].expression = E::Operation {
        origin: None,
        value: Arc::new(wrapper[2].expression.clone()),
    };
    cases.push(wrapper);
    let mut hue = plain.clone();
    hue[2].expression = color(ColorComponent::Hue, 0.3);
    cases.push(hue);
    let mut transition = plain.clone();
    transition[2].expression = resume(
        Some(color(ColorComponent::Red, 0.1)),
        Some(color(ColorComponent::Red, 0.9)),
        0.4,
    );
    cases.push(transition);
    for samples in &cases {
        for available in [false, true] {
            for required in [
                None,
                Some(ProgrammingOwner::Color),
                Some(ProgrammingOwner::Position),
            ] {
                let requirements = required
                    .map(|owner| {
                        vec![DynamicFamilyPreparationRequirement {
                            target: samples[2].target,
                            owner,
                            rank: rank(if owner == ProgrammingOwner::Color {
                                &samples[2]
                            } else {
                                &samples[0]
                            }),
                            reason: DynamicFamilyPreparationRequirementReason::Transition(
                                TransitionRequirement::LiveJointAngles,
                            ),
                        }]
                    })
                    .unwrap_or_default();
                let output = compare(
                    samples,
                    &requirements,
                    available,
                    &mut DynamicFamilyPreparationScratch::default(),
                    &mut DynamicFamilyPreparationScratch::default(),
                )
                .unwrap();
                assert!(output.legacy.len() == 1);
            }
        }
    }
}
#[test]
fn mixed_hot_edits_and_failures_preserve_error_precedence_clear_state_and_recover() {
    let plain = mixed(0.3);
    let mut fast = DynamicFamilyPreparationScratch::default();
    let mut old = DynamicFamilyPreparationScratch::default();
    let baseline = compare(&plain, &[], true, &mut fast, &mut old).unwrap();
    for edit in 0..5 {
        let mut invalid = plain.clone();
        match edit {
            0 => invalid[3].lane_id = invalid[2].lane_id,
            1 => invalid[2].activation_mix = 1.5,
            2 => {
                let E::Programming { value, .. } = &mut invalid[2].expression else {
                    unreachable!()
                };
                *value = DynamicValue::Scalar(f32::NAN);
            }
            3 => invalid[3].priority += 1,
            _ => {
                let E::Programming { address, .. } = &mut invalid[2].expression else {
                    unreachable!()
                };
                Arc::make_mut(address).representation = DynamicFamilyRepresentation::Angles;
            }
        }
        assert!(compare(&invalid, &[], true, &mut fast, &mut old).is_err());
        for scratch in [&fast, &old] {
            assert!(
                scratch.cache.is_empty()
                    && scratch.families.is_empty()
                    && scratch.legacy.is_empty()
                    && scratch.requirements.is_empty()
            );
        }
        assert_eq!(
            compare(&plain, &[], true, &mut fast, &mut old).unwrap(),
            baseline
        );
    }
    let mut wrapped = plain.clone();
    wrapped[2].expression = E::Operation {
        origin: None,
        value: Arc::new(wrapped[2].expression.clone()),
    };
    compare(&wrapped, &[], true, &mut fast, &mut old).unwrap();
    assert_eq!(
        compare(&plain, &[], true, &mut fast, &mut old).unwrap(),
        baseline
    );
}
#[test]
fn mixed_fixed_slots_keep_parallel_chunk_replay_identical_to_general_and_serial() {
    let mut samples = Vec::new();
    for index in 0..12 {
        let mut controller = mixed(0.2 + index as f32 * 0.01);
        for sample in &mut controller {
            sample.target = FixtureId(Uuid::from_u128(300 + index));
        }
        samples.extend(controller);
    }
    let expected = general(|| {
        prepare(
            &samples,
            &[],
            &Reads::new(true),
            &mut DynamicFamilyPreparationScratch::default(),
        )
    })
    .unwrap();
    let sources = Sources::default();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared = prepare_dynamic_family_samples_in_parallel(
        &samples,
        &[],
        &sources,
        None,
        &mut scratch,
        &mut FourChunks,
    )
    .unwrap();
    assert_eq!(observed(prepared), expected);
    assert!(scratch.cache.is_empty());
}

#[test]
fn all_six_fixed_slots_keep_current_requirement_and_recovery_provenance() {
    let mut fast = DynamicFamilyPreparationScratch::default();
    let mut old = DynamicFamilyPreparationScratch::default();
    let mut samples = mixed(0.4);
    samples[1].expression = super::current(ProgrammingComponent::Tilt);
    for (index, component) in [ColorComponent::Amber, ColorComponent::Uv]
        .into_iter()
        .enumerate()
    {
        let mut expression = color(component, 0.6);
        let E::Programming {
            occurrence,
            dependency_occurrence,
            ..
        } = &mut expression
        else {
            unreachable!()
        };
        *occurrence =
            Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(600 + index as u128)).unwrap());
        *dependency_occurrence = Some(DynamicSourceDependency::identity(Some(
            DynamicSourceOccurrenceId::new(Uuid::from_u128(700 + index as u128)).unwrap(),
        )));
        samples.push(sample(24 + index as u128, expression));
    }
    for available in [true, false, true] {
        MIXED_PLAIN_HITS.with(|v| v.set(0));
        let output = compare(&samples, &[], available, &mut fast, &mut old).unwrap();
        assert_eq!(MIXED_PLAIN_HITS.with(Cell::get), 6);
        assert_eq!(output.requirements.is_empty(), available);
        assert!(fast.cache.is_empty());
        assert_eq!(old.cache.len(), 6);
    }
}
