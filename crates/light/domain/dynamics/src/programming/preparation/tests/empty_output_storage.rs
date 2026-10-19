use super::*;

#[test]
fn retained_groups_rewrite_their_storage_in_place() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let sources = Sources::default();
    let mut input = sample(1, color(ColorComponent::Red, 0.2));
    prepare_dynamic_family_samples(&[input.clone()], &sources, None, &mut scratch).unwrap();
    let pointer = scratch.families[0].samples.as_ptr();
    input.expression = color(ColorComponent::Red, 0.8);
    input.priority += 1;
    let fresh = prepare_dynamic_family_samples(
        &[input.clone()],
        &sources,
        None,
        &mut DynamicFamilyPreparationScratch::default(),
    )
    .map(preparation_snapshot)
    .unwrap();
    let retained = prepare_dynamic_family_samples(&[input], &sources, None, &mut scratch)
        .map(preparation_snapshot)
        .unwrap();
    assert_eq!(retained, fresh);
    assert_eq!(
        scratch.families[0].samples.as_ptr(),
        pointer,
        "an unchanged layout keeps its group storage (TL-842 C4)"
    );
}

fn drain(scratch: &mut DynamicFamilyPreparationScratch) -> Vec<FamilyCompositionSample> {
    let mut moved = Vec::new();
    for group in scratch.prepared_for_drain().families {
        moved.append(&mut group.samples);
    }
    moved
}

fn inputs(frame: u128) -> Vec<DynamicRuntimeSample> {
    let mut inputs = Vec::new();
    for target in (0..if frame == 1 { 3 } else { 5 }).rev() {
        for (lane, component) in [
            ColorComponent::Red,
            ColorComponent::Green,
            ColorComponent::Blue,
            ColorComponent::Uv,
        ]
        .into_iter()
        .enumerate()
        {
            let mut input = sample(
                10 + lane as u128,
                color(component, 0.1 + frame as f32 * 0.2),
            );
            input.target = FixtureId(Uuid::from_u128(100 + target));
            input.priority += frame as i16;
            input.activation_mix = 1.0 - frame as f32 * 0.1;
            inputs.push(input);
        }
    }
    let mut pan = sample(30, angle(ProgrammingComponent::Pan, 60.));
    pan.controller_id = Uuid::from_u128(90);
    let mut tilt = sample(31, current(ProgrammingComponent::Tilt));
    tilt.controller_id = pan.controller_id;
    inputs.extend([
        pan,
        tilt,
        sample(
            40,
            E::LegacyScalar {
                attribute: AttributeKey::intensity(),
                value: 0.6,
                occurrence: None,
                dependency_occurrence: None,
            },
        ),
    ]);
    inputs
}

#[test]
fn drained_and_borrowed_outputs_match_values_traces_source_calls_and_chunk_replay() {
    for chunks in [0, 1, 2, 4] {
        for stop in [false, true] {
            let mut reused = DynamicFamilyPreparationScratch::default();
            let mut borrowed = DynamicFamilyPreparationScratch::default();
            for frame in 0..4 {
                let input = inputs(frame);
                let requirement = DynamicFamilyPreparationRequirement {
                    target: input[0].target,
                    owner: ProgrammingOwner::Color,
                    rank: rank(&input[0]),
                    reason: DynamicFamilyPreparationRequirementReason::Transition(
                        TransitionRequirement::CompatibleOwners,
                    ),
                };
                let expected =
                    prepare_observed(&input, &[requirement.clone()], &mut borrowed, chunks, stop);
                let actual = prepare_observed(&input, &[requirement], &mut reused, chunks, stop);
                assert_eq!(actual, expected);
                assert!(actual.0.is_ok());
                assert!(actual.1 > 0, "the oracle must exercise a Current read");
                assert!(!drain(&mut reused).is_empty());
                // The reference retains all samples: no retired group is eligible for reuse.
                assert!(borrowed.families.iter().all(|g| !g.samples.is_empty()));
                assert_eq!(reused.cache.len(), borrowed.cache.len());
            }
        }
    }
}

#[test]
fn unwritten_groups_and_samples_are_dropped_when_the_frame_finishes() {
    let input = sample(1, color(ColorComponent::Red, 0.2));
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
    let stale = FamilySample::new(
        address,
        DynamicValue::Family(AttributeValue::Normalized(0.4)),
        rank(&input),
        1.,
    )
    .unwrap();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(&[input.clone()], &Sources::default(), None, &mut scratch)
        .unwrap();
    // Last frame's sample beyond what this frame writes, and a group this frame does not write.
    scratch.families[0].samples.push(stale.clone().into());
    scratch.families.push(DynamicFamilySampleGroup {
        target: FixtureId(Uuid::from_u128(77)),
        owner: ProgrammingOwner::Focus,
        samples: vec![stale.into()],
    });
    let prepared =
        prepare_dynamic_family_samples(&[input], &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].samples.len(), 1);
    assert!(
        weak.upgrade().is_none(),
        "unwritten samples must not outlive the frame that dropped them"
    );
    assert_eq!(scratch.family_indices.len(), 1);
}

#[test]
fn failed_preparation_double_clear_and_retry_match_borrowed_reference() {
    for chunks in [0, 1, 2, 4] {
        for late in [false, true] {
            let mut reused = DynamicFamilyPreparationScratch::default();
            let mut borrowed = DynamicFamilyPreparationScratch::default();
            let valid = inputs(0);
            assert_eq!(
                prepare_observed(&valid, &[], &mut reused, chunks, false),
                prepare_observed(&valid, &[], &mut borrowed, chunks, false)
            );
            drop(drain(&mut reused));
            let mut invalid = valid.clone();
            // A separate late controller permits earlier output before the failure.
            let index = if late { invalid.len() - 1 } else { 0 };
            invalid[index].controller_id = Uuid::from_u128(if late { 999 } else { 1 });
            invalid[index].activation_mix = f32::NAN;
            let result = prepare_observed(&invalid, &[], &mut reused, chunks, false);
            assert_eq!(
                result,
                prepare_observed(&invalid, &[], &mut borrowed, chunks, false)
            );
            assert!(result.0.is_err());
            for scratch in [&reused, &borrowed] {
                assert!(scratch.families.is_empty() && scratch.legacy.is_empty());
                assert!(scratch.cache.is_empty() && scratch.requirements.is_empty());
            }
            let retired = reused.take_retired();
            let reference_retired = borrowed.take_retired();
            assert_eq!(retired.1.legacy.len(), reference_retired.1.legacy.len());
            assert_eq!(
                prepare_observed(&inputs(2), &[], &mut reused, chunks, false),
                prepare_observed(&inputs(2), &[], &mut borrowed, chunks, false)
            );
            drop(drain(&mut reused));
            reused.clear();
            borrowed.clear();
            assert!(reused.families.is_empty() && reused.family_indices.is_empty());
            assert_eq!(
                prepare_observed(&inputs(3), &[], &mut reused, chunks, false),
                prepare_observed(&inputs(3), &[], &mut borrowed, chunks, false)
            );
        }
    }
}
