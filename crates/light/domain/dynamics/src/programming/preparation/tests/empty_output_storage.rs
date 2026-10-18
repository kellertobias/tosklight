use super::*;

#[test]
fn drained_append_one_reuses_storage_before_retirement_is_freed() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let sources = Sources::default();
    let mut input = sample(1, color(ColorComponent::Red, 0.2));
    prepare_dynamic_family_samples(&[input.clone()], &sources, None, &mut scratch).unwrap();
    let pointer = scratch.families[0].samples.as_ptr();
    let capacity = scratch.families[0].samples.capacity();
    assert!(capacity > 0);
    let mut moved = Vec::new();
    moved.append(&mut scratch.prepared_for_drain().families[0].samples);
    input.expression = color(ColorComponent::Red, 0.8);
    input.priority += 1;
    prepare_dynamic_family_samples(&[input], &sources, None, &mut scratch).unwrap();
    assert_eq!(
        scratch.families[0].samples.as_ptr(),
        pointer,
        "reuse the drained buffer while retirement still owns every non-reused buffer"
    );
    assert_eq!(scratch.families[0].samples.capacity(), capacity);
    assert_eq!(moved.len(), 1);
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
fn reuse_skips_nonempty_output_and_preserves_legacy_retirement_and_append_vec_move() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(
        &inputs(0),
        &Sources {
            tilt: Some(40.),
            ..Default::default()
        },
        None,
        &mut scratch,
    )
    .unwrap();
    let empty_pointer = scratch.families[0].samples.as_ptr();
    scratch.families[0].samples.clear();
    let retained = scratch.families[1].samples.as_ptr();
    let retained_len = scratch.families[1].samples.len();
    let mut incoming = vec![scratch.families[1].samples[0].clone()];
    incoming.reserve(10);
    let incoming_pointer = incoming.as_ptr();
    let leaf = incoming[0].clone();
    // The search must skip a live payload before reaching the empty storage.
    scratch.families.swap(0, 1);
    scratch.clear_output();
    let mut out = FamilyOutput {
        families: &mut scratch.families,
        family_indices: &mut scratch.family_indices,
        legacy: &mut scratch.legacy,
        requirements: &mut scratch.requirements,
        drained_storage: scratch.retired_output.families.iter_mut(),
    };
    let target = FixtureId(Uuid::from_u128(999));
    out.append(target, ProgrammingOwner::Color, incoming);
    out.append_one(
        FixtureId(Uuid::from_u128(998)),
        ProgrammingOwner::Color,
        leaf,
    );
    assert_eq!(scratch.families[0].samples.as_ptr(), incoming_pointer);
    assert_eq!(scratch.families[1].samples.as_ptr(), empty_pointer);
    assert_eq!(
        scratch.retired_output.families[0].samples.as_ptr(),
        retained
    );
    assert_eq!(
        scratch.retired_output.families[0].samples.len(),
        retained_len
    );
    assert_eq!(scratch.retired_output.legacy.len(), 1);
    let retired = scratch.take_retired();
    assert_eq!(retired.1.families[0].samples.len(), retained_len);
    assert_eq!(retired.1.legacy.len(), 1);
}

#[test]
fn nonempty_payload_lives_until_retirement_drop_but_empty_buffer_is_reused() {
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
    let retained = FamilySample::new(
        address,
        DynamicValue::Family(AttributeValue::Normalized(0.4)),
        rank(&input),
        1.,
    )
    .unwrap();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(&[input.clone()], &Sources::default(), None, &mut scratch)
        .unwrap();
    let pointer = scratch.families[0].samples.as_ptr();
    drop(drain(&mut scratch));
    scratch.families.push(DynamicFamilySampleGroup {
        target: input.target,
        owner: ProgrammingOwner::Focus,
        samples: vec![retained.into()],
    });
    prepare_dynamic_family_samples(&[input], &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(scratch.families[0].samples.as_ptr(), pointer);
    assert!(weak.upgrade().is_some());
    let retirement = scratch.take_retired();
    assert!(weak.upgrade().is_some());
    drop(retirement);
    assert!(
        weak.upgrade().is_none(),
        "nonempty samples retain exactly the existing retirement lifetime"
    );
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
            assert_eq!(retired.1.families.len(), reference_retired.1.families.len());
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
