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
    let groups = assemble_captured_family_inputs(&dynamic, fixed, &mut scratch);
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
    let groups = assemble_captured_family_inputs(&dynamic, fixed, &mut scratch);
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
            &mut scratch
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
            &mut CapturedFamilyInputScratch::default()
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
    scratch.clear();
    let (groups, added) = scratch.with_static_targets(&[(active, ProgrammingOwner::Position)]);
    assert_eq!(groups.len(), 1);
    assert_eq!(added, vec![(active, ProgrammingOwner::Position)]);
}
