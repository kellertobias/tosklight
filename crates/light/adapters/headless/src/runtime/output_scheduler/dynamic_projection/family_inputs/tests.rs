use super::*;
use crate::runtime::output_scheduler::dynamic_projection::fixed_masks::{
    CapturedFixedMaskRows, FixedMaskCompilationScratch, compile_captured_fixed_masks,
};
use light_core::{
    AttributeValue, NativeColorIdentity, NativeColorValue, PhysicalDataQuality, programming::*,
};
use light_dynamics::*;
use std::sync::Arc;
use uuid::Uuid;

fn target(id: u128) -> FixtureId {
    FixtureId(Uuid::from_u128(id))
}
fn rank() -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 0,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(3),
        },
    }
}
fn sample(owner: ProgrammingOwner, value: AttributeValue) -> FamilyCompositionSample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(owner, &value).unwrap(),
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Family(value),
        rank(),
        1.0,
    )
    .unwrap()
    .into()
}
fn row(target: FixtureId, mask: ProgrammingFamilyFixAt) -> (Uuid, i16, DynamicAddressValue) {
    (
        Uuid::from_u128(99),
        10,
        DynamicAddressValue {
            fixture_id: target,
            attribute: mask.address.owner().key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask,
                timing: Default::default(),
            },
            changed_at_millis: 200,
            programmer_order: 1,
        },
    )
}
fn input(rows: &[(Uuid, i16, DynamicAddressValue)]) -> CapturedFixedMaskRows<'_> {
    CapturedFixedMaskRows {
        now: chrono::DateTime::from_timestamp_millis(1_000).unwrap(),
        programmer_values: rows,
        programmer_rows: None,
        extra_programmer_values: &[],
        cue_values: &[],
    }
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
        panic!("same-representation values need no frame conversion")
    }
}
fn compose(group: &CapturedFamilyInput, base: &AttributeValue) -> AttributeValue {
    compose_retained_dynamic_family(
        group.group.owner,
        base,
        &group.group.samples,
        &FamilyCompositionContext::default(),
        &NoFrame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap()
}

fn assert_draining_matches_borrowed(
    dynamic: &PreparedDynamicFamilySamples<'_>,
    fixed: &[PreparedFixedMask],
    frozen: &rustc_hash::FxHashSet<(FixtureId, ProgrammingOwner)>,
) {
    let mut moving = dynamic
        .families
        .iter()
        .map(|group| DynamicFamilySampleGroup {
            target: group.target,
            owner: group.owner,
            samples: group.samples.clone(),
        })
        .collect::<Vec<_>>();
    let mut reference = CapturedFamilyInputScratch::default();
    let mut actual = CapturedFamilyInputScratch::default();
    assemble_captured_family_inputs(dynamic, fixed, &mut reference, None);
    assemble_captured_family_inputs_draining(
        DrainingPreparedDynamicFamilySamples {
            families: &mut moving,
            requirements: dynamic.requirements,
        },
        fixed,
        &mut actual,
        None,
    );
    assert!(moving.iter().all(|group| group.samples.is_empty()));
    reference.hold_frozen(frozen);
    actual.hold_frozen(frozen);
    let static_targets = [
        (target(10), ProgrammingOwner::Position),
        (target(901), ProgrammingOwner::Focus),
    ];
    let (expected, expected_static) = reference.with_static_targets(&static_targets);
    let (observed, observed_static) = actual.with_static_targets(&static_targets);
    assert_eq!(expected_static, observed_static);
    assert_eq!(expected.len(), observed.len());
    for (expected, observed) in expected.iter().zip(observed) {
        assert_eq!(
            (expected.group.target, expected.group.owner),
            (observed.group.target, observed.group.owner)
        );
        assert_eq!(expected.requirements, observed.requirements);
        assert_eq!(expected.group.samples.len(), observed.group.samples.len());
        for (expected, observed) in expected.group.samples.iter().zip(&observed.group.samples) {
            match (expected, observed) {
                (FamilyCompositionSample::Known(a), FamilyCompositionSample::Known(b)) => {
                    assert_eq!(a.address().address(), b.address().address());
                    assert_eq!(a.materialized_value(), b.materialized_value());
                    assert_eq!(
                        (a.rank, a.activation_mix, a.is_fix_at()),
                        (b.rank, b.activation_mix, b.is_fix_at())
                    );
                }
                (
                    FamilyCompositionSample::WholeExpression {
                        expression: a,
                        rank: ar,
                        activation_mix: am,
                    },
                    FamilyCompositionSample::WholeExpression {
                        expression: b,
                        rank: br,
                        activation_mix: bm,
                    },
                ) => {
                    assert!(Arc::ptr_eq(a, b));
                    assert_eq!((ar, am), (br, bm));
                }
                (
                    FamilyCompositionSample::CoupledExpression {
                        expression: a,
                        rank: ar,
                        activation_mix: am,
                    },
                    FamilyCompositionSample::CoupledExpression {
                        expression: b,
                        rank: br,
                        activation_mix: bm,
                    },
                ) => {
                    assert!(Arc::ptr_eq(a, b));
                    assert_eq!((ar, am), (br, bm));
                }
                _ => panic!("sample variant changed during consuming handoff"),
            }
        }
        let base = match expected.group.owner {
            ProgrammingOwner::Position => {
                AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 10.0)))
            }
            ProgrammingOwner::Color => {
                AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                    intent: ColorIntent::default(),
                }))
            }
            _ => AttributeValue::Normalized(0.2),
        };
        let mut expected_trace = RetainedFamilyCompositionScratch::default();
        let mut actual_trace = RetainedFamilyCompositionScratch::default();
        let compose = |entry: &CapturedFamilyInput,
                       scratch: &mut RetainedFamilyCompositionScratch| {
            compose_retained_dynamic_family_traced(
                entry.group.owner,
                &base,
                &entry.group.samples,
                &FamilyCompositionContext::default(),
                &NoFrame,
                scratch,
            )
            .map_err(|error| format!("{error:?}"))
        };
        assert_eq!(
            compose(expected, &mut expected_trace),
            compose(observed, &mut actual_trace)
        );
        for component in [
            ProgrammingComponent::Pan,
            ProgrammingComponent::Tilt,
            ProgrammingComponent::Focus,
            ProgrammingComponent::Color(ColorComponent::Red),
            ProgrammingComponent::Color(ColorComponent::Uv),
        ] {
            let sources = |scratch: &RetainedFamilyCompositionScratch| {
                let trace = scratch.family_trace();
                trace
                    .root()
                    .and_then(|root| trace.sources_for_component(root, component))
            };
            assert_eq!(sources(&expected_trace), sources(&actual_trace));
        }
    }
}

#[test]
fn fixed_pan_masks_one_axis_of_the_complete_dynamic_pair_and_focus_merges_in_its_own_group() {
    let angles = |pan, tilt| AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)));
    let dynamic_groups = [
        DynamicFamilySampleGroup {
            target: target(10),
            owner: ProgrammingOwner::Focus,
            samples: vec![sample(
                ProgrammingOwner::Focus,
                AttributeValue::Normalized(0.8),
            )],
        },
        DynamicFamilySampleGroup {
            target: target(10),
            owner: ProgrammingOwner::Position,
            samples: vec![sample(ProgrammingOwner::Position, angles(90.0, 45.0))],
        },
    ];
    let dynamic = PreparedDynamicFamilySamples {
        families: &dynamic_groups,
        legacy: &[],
        requirements: &[],
    };
    let rows = [
        row(
            target(10),
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::Pan),
                angles(-30.0, -70.0),
            )
            .unwrap(),
        ),
        row(
            target(10),
            ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Focus,
                None,
                AttributeValue::Normalized(0.4),
            )
            .unwrap(),
        ),
    ];
    let mut compilation = FixedMaskCompilationScratch::default();
    let fixed = compile_captured_fixed_masks(&input(&rows), None, None, &mut compilation).unwrap();
    let mut scratch = CapturedFamilyInputScratch::default();
    assert_draining_matches_borrowed(&dynamic, fixed, &Default::default());
    let groups = assemble_captured_family_inputs(&dynamic, fixed, &mut scratch, None);
    assert_eq!(groups.len(), 2);
    for group in groups {
        assert!(group.requirements.is_empty());
        assert_eq!(group.group.samples.len(), 2);
        match group.group.owner {
            ProgrammingOwner::Position => {
                assert_eq!(compose(group, &angles(0.0, 10.0)), angles(-30.0, 45.0))
            }
            ProgrammingOwner::Focus => assert_eq!(
                compose(group, &AttributeValue::Normalized(0.2)),
                AttributeValue::Normalized(0.4)
            ),
            _ => unreachable!(),
        }
    }
}

fn direct_mask() -> ProgrammingFamilyFixAt {
    ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Color,
        None,
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: NativeColorIdentity {
                    profile_id: Uuid::from_u128(1),
                    profile_revision: 2,
                    profile_digest: "source".into(),
                    mode_id: Uuid::from_u128(2),
                    head_id: Uuid::from_u128(3),
                    path_id: Uuid::from_u128(4),
                    model_revision: 1,
                    native_layout_signature: "rgbuv".into(),
                },
                channels: vec![NativeColorValue {
                    channel_id: Uuid::from_u128(5),
                    function_id: Uuid::from_u128(6),
                    raw: u32::MAX - 1,
                }],
                spreads: vec![],
            },
            portable: PortableColorEstimate {
                model_revision: 1,
                visible: None,
                uv: Some(PortableUv {
                    amount: 0.7,
                    quality: PhysicalDataQuality::Estimated,
                }),
                quality: PhysicalDataQuality::Unknown,
                limitations: vec!["unknown visible color".into()],
            },
        })),
    )
    .unwrap()
}

#[test]
fn unavailable_fixed_and_position_inputs_keep_complete_evidence_even_without_ready_samples() {
    let mask = direct_mask();
    let rows = [row(target(20), mask.clone())];
    let requirement = DynamicFamilyPreparationRequirement {
        target: target(10),
        owner: ProgrammingOwner::Position,
        rank: rank(),
        reason: DynamicFamilyPreparationRequirementReason::Transition(
            TransitionRequirement::LiveJointAngles,
        ),
    };
    let requirements = [requirement.clone()];
    let dynamic = PreparedDynamicFamilySamples {
        families: &[],
        legacy: &[],
        requirements: &requirements,
    };
    let mut compilation = FixedMaskCompilationScratch::default();
    let fixed = compile_captured_fixed_masks(&input(&rows), None, None, &mut compilation).unwrap();
    let mut scratch = CapturedFamilyInputScratch::default();
    assert_draining_matches_borrowed(&dynamic, fixed, &Default::default());
    let groups = assemble_captured_family_inputs(&dynamic, fixed, &mut scratch, None);
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|group| group.group.samples.is_empty()));
    assert_eq!(groups[0].group.target, target(10));
    assert!(
        matches!(&groups[0].requirements[..], [CapturedFamilyRequirement::Dynamic(actual)] if actual == &requirement)
    );
    match &groups[1].requirements[..] {
        [
            CapturedFamilyRequirement::Fixed {
                mask: actual,
                rank,
                occurrence,
                activation_mix,
                reason,
            },
        ] => {
            assert_eq!(
                actual, &mask,
                "raw precision and independent UV survive the unavailable model"
            );
            assert_eq!(*rank, fixed[0].rank);
            assert_eq!(*occurrence, None);
            assert_eq!(*activation_mix, 1.0);
            assert!(matches!(
                reason,
                FixedMaskRequirement::NativeColorModelUnavailable(_)
            ));
        }
        _ => panic!("unavailable Fixed must remain in the family"),
    }
    // A later empty selection/capture cannot inherit any previous candidate or warning.
    assert!(
        assemble_captured_family_inputs(
            &PreparedDynamicFamilySamples {
                families: &[],
                legacy: &[],
                requirements: &[],
            },
            &[],
            &mut scratch,
            None,
        )
        .is_empty()
    );
}

#[test]
fn delayed_fixed_input_neither_masks_nor_reports_unavailable_before_its_activation() {
    let mut row = row(target(20), direct_mask());
    let DynamicSemanticValue::ProgrammingFixAt { timing, .. } = &mut row.2.value else {
        unreachable!()
    };
    timing.delay_millis = Some(2_000);
    let rows = [row];
    let mut compilation = FixedMaskCompilationScratch::default();
    let fixed = compile_captured_fixed_masks(&input(&rows), None, None, &mut compilation).unwrap();
    assert!(!fixed[0].participates());
    assert!(
        assemble_captured_family_inputs(
            &PreparedDynamicFamilySamples {
                families: &[],
                legacy: &[],
                requirements: &[],
            },
            fixed,
            &mut CapturedFamilyInputScratch::default(),
            None,
        )
        .is_empty()
    );
}

#[test]
fn adding_static_programs_preserves_existing_dynamic_groups_and_deduplicates_targets() {
    let mut scratch = CapturedFamilyInputScratch::default();
    let active = target(401);
    let static_peer = target(402);
    // An existing zero-sample group represents an unavailable Dynamic candidate, not a
    // static fallback. It must remain in the ordinary skip/requirement path.
    scratch.group(active, ProgrammingOwner::Position);
    let (groups, added) = scratch.with_static_targets(&[
        (active, ProgrammingOwner::Position),
        (static_peer, ProgrammingOwner::Position),
        (static_peer, ProgrammingOwner::Position),
    ]);
    assert_eq!(groups.len(), 2);
    assert_eq!(added, vec![(static_peer, ProgrammingOwner::Position)]);
    assert_eq!(groups[0].group.target, active);
    assert!(groups.iter().all(|entry| entry.group.samples.is_empty()));
    scratch.clear(None);
    let (groups, added) = scratch.with_static_targets(&[(active, ProgrammingOwner::Position)]);
    assert_eq!(groups.len(), 1);
    assert_eq!(added, vec![(active, ProgrammingOwner::Position)]);
}

#[test]
fn consuming_handoff_does_not_add_a_compiled_address_reference() {
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress::whole_family(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(0.4),
            )
            .unwrap(),
            None,
        )
        .unwrap(),
    );
    let mut dynamic = [DynamicFamilySampleGroup {
        target: target(10),
        owner: ProgrammingOwner::Focus,
        samples: vec![
            FamilySample::new(
                address.clone(),
                DynamicValue::Family(AttributeValue::Normalized(0.4)),
                rank(),
                1.0,
            )
            .unwrap()
            .into(),
        ],
    }];
    dynamic[0].samples.reserve(1); // room for the appended Fixed sample below
    let storage = dynamic[0].samples.as_ptr();
    let capacity = dynamic[0].samples.capacity();
    let before = Arc::strong_count(&address);
    let mut scratch = CapturedFamilyInputScratch::default();
    let groups = assemble_captured_family_inputs_draining(
        DrainingPreparedDynamicFamilySamples {
            families: &mut dynamic,
            requirements: &[],
        },
        &[],
        &mut scratch,
        None,
    );
    assert_eq!(groups[0].group.samples.len(), 1);
    assert_eq!(
        Arc::strong_count(&address),
        before,
        "handoff must move samples, not clone their immutable compiled address"
    );
    // TL-842 C4: the prepared vector is borrowed whole, then returned without the samples the
    // frame appended (a Fixed mask), keeping its storage for the next preparation.
    assert!(dynamic[0].samples.is_empty());
    assert_eq!(scratch.groups[0].group.samples.as_ptr(), storage);
    let appended = scratch.groups[0].group.samples[0].clone();
    scratch.groups[0].group.samples.push(appended);
    scratch.return_lent(&mut dynamic);
    assert_eq!(dynamic[0].samples.as_ptr(), storage);
    assert_eq!(dynamic[0].samples.capacity(), capacity);
    assert_eq!(dynamic[0].samples.len(), 1);
    assert!(scratch.groups[0].group.samples.is_empty());
    assert_eq!(Arc::strong_count(&address), before);
    // A second return, or one to a changed layout, changes nothing.
    scratch.return_lent(&mut dynamic);
    assert_eq!(dynamic[0].samples.len(), 1);
}

/// A loan that is never returned (a failed frame) leaves the prepared group empty and frees
/// the borrowed samples with the next assembly.
#[test]
fn unreturned_loan_is_dropped_with_the_next_assembly() {
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress::whole_family(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(0.4),
            )
            .unwrap(),
            None,
        )
        .unwrap(),
    );
    let weak = Arc::downgrade(&address);
    let mut dynamic = vec![DynamicFamilySampleGroup {
        target: target(10),
        owner: ProgrammingOwner::Focus,
        samples: vec![
            FamilySample::new(
                address,
                DynamicValue::Family(AttributeValue::Normalized(0.4)),
                rank(),
                1.0,
            )
            .unwrap()
            .into(),
        ],
    }];
    let mut scratch = CapturedFamilyInputScratch::default();
    assemble_captured_family_inputs_draining(
        DrainingPreparedDynamicFamilySamples {
            families: &mut dynamic,
            requirements: &[],
        },
        &[],
        &mut scratch,
        None,
    );
    assert!(dynamic[0].samples.is_empty());
    let mut next: Vec<DynamicFamilySampleGroup> = Vec::new();
    assemble_captured_family_inputs_draining(
        DrainingPreparedDynamicFamilySamples {
            families: &mut next,
            requirements: &[],
        },
        &[],
        &mut scratch,
        None,
    );
    assert!(weak.upgrade().is_none(), "the unreturned samples are freed");
    assert!(scratch.groups.is_empty());
    scratch.return_lent(&mut dynamic);
    assert!(dynamic[0].samples.is_empty(), "nothing stale returns later");
}

#[test]
fn consuming_handoff_matches_clone_oracle_for_changing_color_duplicate_groups_and_freeze() {
    let mut reference = CapturedFamilyInputScratch::default();
    let mut actual = CapturedFamilyInputScratch::default();
    for value in [0.1, 0.8, 0.3] {
        let mut known = match sample(ProgrammingOwner::Focus, AttributeValue::Normalized(value)) {
            FamilyCompositionSample::Known(sample) => sample,
            _ => unreachable!(),
        };
        known.activation_mix = 0.75;
        known.rank.changed_at_submillis_nanos = 42;
        let source = FamilyTraceSource {
            rank: known.rank,
            footprint: FamilyTraceFootprint::Whole,
            role: FamilyTraceRole::CalculationDependency,
            occurrence: None,
        };
        let dynamic = [
            DynamicFamilySampleGroup {
                target: target(10),
                owner: ProgrammingOwner::Focus,
                samples: vec![known.with_trace_sources(Arc::from([source])).into()],
            },
            DynamicFamilySampleGroup {
                target: target(10),
                owner: ProgrammingOwner::Focus,
                samples: vec![sample(
                    ProgrammingOwner::Focus,
                    AttributeValue::Normalized(value / 2.0),
                )],
            },
            DynamicFamilySampleGroup {
                target: target(20),
                owner: ProgrammingOwner::Color,
                samples: vec![sample(
                    ProgrammingOwner::Color,
                    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                        intent: ColorIntent {
                            relative_output: value,
                            ..Default::default()
                        },
                    })),
                )],
            },
        ];
        let borrowed = PreparedDynamicFamilySamples {
            families: &dynamic,
            legacy: &[],
            requirements: &[],
        };
        assert_draining_matches_borrowed(&borrowed, &[], &Default::default());
        assert_draining_matches_borrowed(
            &borrowed,
            &[],
            &[(target(10), ProgrammingOwner::Focus)]
                .into_iter()
                .collect(),
        );
        let mut moving = dynamic;
        assemble_captured_family_inputs(
            &PreparedDynamicFamilySamples {
                families: &moving,
                legacy: &[],
                requirements: &[],
            },
            &[],
            &mut reference,
            None,
        );
        assemble_captured_family_inputs_draining(
            DrainingPreparedDynamicFamilySamples {
                families: &mut moving,
                requirements: &[],
            },
            &[],
            &mut actual,
            None,
        );
        assert_eq!(
            compose(&reference.groups[0], &AttributeValue::Normalized(0.2)),
            compose(&actual.groups[0], &AttributeValue::Normalized(0.2))
        );
    }
    // The next empty frame releases the same candidate inventory instead of retaining old data.
    assemble_captured_family_inputs_draining(
        DrainingPreparedDynamicFamilySamples {
            families: &mut [],
            requirements: &[],
        },
        &[],
        &mut actual,
        None,
    );
    assert!(actual.groups.is_empty() && actual.spare.is_empty());
}

#[test]
fn consuming_handoff_keeps_retained_resume_expression_and_underlay_provenance() {
    let endpoint = AttributeValue::Normalized(0.9);
    for progress in [0.0, 0.35, 1.0] {
        let expression = Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(DynamicSampleExpression::Transition {
                    from: None,
                    to: Some(Arc::new(DynamicSampleExpression::Programming {
                        address: Arc::new(
                            DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &endpoint)
                                .unwrap(),
                        ),
                        value: DynamicValue::Family(endpoint.clone()),
                        occurrence: None,
                        dependency_occurrence: None,
                    })),
                    progress,
                    reason: DynamicTransitionReason::Resume {
                        occurrence_id: Uuid::from_u128(123),
                    },
                }),
                ProgrammingOwner::Focus,
                None,
                None,
            )
            .unwrap(),
        );
        let dynamic = [DynamicFamilySampleGroup {
            target: target(10),
            owner: ProgrammingOwner::Focus,
            samples: vec![
                sample(ProgrammingOwner::Focus, AttributeValue::Normalized(0.4)),
                FamilyCompositionSample::WholeExpression {
                    expression,
                    rank: FamilySampleRank {
                        stable_order: 1,
                        ..rank()
                    },
                    activation_mix: 0.75,
                },
            ],
        }];
        assert_draining_matches_borrowed(
            &PreparedDynamicFamilySamples {
                families: &dynamic,
                legacy: &[],
                requirements: &[],
            },
            &[],
            &Default::default(),
        );
    }
}
