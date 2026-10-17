use super::*;

fn assert_joint_requirement(requirement: &DynamicFamilyPreparationRequirement) {
    assert_eq!(requirement.owner, ProgrammingOwner::Position);
    assert_eq!(
        requirement.reason,
        DynamicFamilyPreparationRequirementReason::Transition(
            TransitionRequirement::LiveJointAngles,
        ),
    );
}

#[test]
fn missing_current_preserves_other_owners_controllers_targets_and_one_scalar_fragment() {
    let mut independent_position = sample(30, whole(ProgrammingOwner::Position, angles(7.0, 8.0)));
    independent_position.controller_id = Uuid::from_u128(9);
    let mut independent_focus = sample(
        40,
        whole(ProgrammingOwner::Focus, AttributeValue::Normalized(0.3)),
    );
    independent_focus.target = FixtureId(Uuid::from_u128(99));
    let inputs = [
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                Some(color(ColorComponent::Uv, 0.8)),
                0.25,
            ),
        ),
        sample(
            11,
            resume(
                Some(current(ProgrammingComponent::Tilt)),
                Some(E::LegacyScalar {
                    attribute: AttributeKey::intensity(),
                    value: 0.6,
                    occurrence: None,
                    dependency_occurrence: None,
                }),
                0.25,
            ),
        ),
        independent_position,
        independent_focus,
    ];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let sources = Sources::default();
    let prepared = prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
    assert_eq!(prepared.requirements.len(), 1);
    assert_joint_requirement(&prepared.requirements[0]);
    assert_eq!(prepared.requirements[0].rank, rank(&inputs[1]));
    assert_eq!(prepared.requirements[0].target, inputs[0].target);
    assert_eq!(prepared.families.len(), 3);
    let position = prepared
        .families
        .iter()
        .find(|group| group.owner == ProgrammingOwner::Position)
        .unwrap();
    assert_eq!(position.samples.len(), 1);
    assert_eq!(
        compose(position, &angles(0.0, 0.0)).unwrap(),
        angles(7.0, 8.0)
    );
    assert_eq!(prepared.legacy.len(), 1);
    let mut legacy = Vec::new();
    assert!(prepared.legacy[0].expression.visit_legacy_contributions(
        |attribute, value, influence| {
            legacy.push((attribute.clone(), value, influence));
        }
    ));
    assert_eq!(legacy, vec![(AttributeKey::intensity(), 0.6, 0.25)]);
    assert_eq!(sources.reads.get(), 1);
}

#[test]
fn missing_current_is_not_release_to_an_otherwise_complete_target_endpoint() {
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0, 2.0, 3.0],
    )));
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for progress in [0.0, 0.5, 1.0] {
        let inputs = [
            sample(
                10,
                resume(
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    Some(whole(ProgrammingOwner::Position, target.clone())),
                    progress,
                ),
            ),
            sample(
                11,
                resume(Some(current(ProgrammingComponent::Tilt)), None, progress),
            ),
        ];
        let sources = Sources::default();
        let prepared =
            prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
        if progress < 1.0 {
            assert!(
                prepared.families.is_empty(),
                "no replacement for the captured static family"
            );
            assert_eq!(prepared.requirements.len(), 1);
            assert_joint_requirement(&prepared.requirements[0]);
            assert_eq!(sources.reads.get(), 1);
        } else {
            assert!(prepared.requirements.is_empty());
            assert_eq!(prepared.families.len(), 1);
            assert_eq!(compose(&prepared.families[0], &target).unwrap(), target);
            assert_eq!(sources.reads.get(), 0);
        }
    }
}

#[test]
fn compiler_requirement_keeps_its_reason_and_independent_owner() {
    // A nested whole Current needs a live joint materialization which the strict whole
    // compiler cannot provide. Splitting still preserves the independent Color endpoint.
    let address = Arc::new(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: None,
    });
    let unresolved = E::Scale {
        address: address.clone(),
        base: DynamicValue::Family(angles(0.0, 0.0)),
        value: Arc::new(E::AngleCurrent { address }),
        factor: 0.5,
        baseline_occurrence: None,
    };
    let input = sample(
        10,
        resume(Some(unresolved), Some(color(ColorComponent::Uv, 0.8)), 0.5),
    );
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&[input.clone()], &Sources::default(), None, &mut scratch)
            .unwrap();
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].owner, ProgrammingOwner::Color);
    assert_eq!(
        prepared.requirements,
        &[DynamicFamilyPreparationRequirement {
            target: input.target,
            owner: ProgrammingOwner::Position,
            rank: rank(&input),
            reason: DynamicFamilyPreparationRequirementReason::Transition(
                TransitionRequirement::CompatibleOwners
            ),
        }]
    );
}

#[test]
fn recovering_current_reuses_buffers_and_does_not_reuse_failed_position_or_requirements() {
    let inputs = [
        sample(10, angle(ProgrammingComponent::Pan, 90.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
        sample(12, color(ColorComponent::Uv, 0.8)),
    ];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch).unwrap();
    let color_cache = scratch.cache.values().next().unwrap().expression.clone();
    let position_capacity = scratch.position.capacity();
    let requirement_capacity = scratch.requirements.capacity();
    for tilt in [Some(30.0), None, Some(60.0)] {
        let sources = Sources {
            tilt,
            ..Default::default()
        };
        let prepared =
            prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
        if let Some(tilt) = tilt {
            assert!(prepared.requirements.is_empty());
            let position = prepared
                .families
                .iter()
                .find(|group| group.owner == ProgrammingOwner::Position)
                .unwrap();
            assert_eq!(
                compose(position, &angles(0.0, 0.0)).unwrap(),
                angles(90.0, tilt)
            );
        } else {
            assert_eq!(prepared.families.len(), 1);
            assert_eq!(prepared.requirements.len(), 1);
        }
        assert_eq!(sources.reads.get(), 1);
        assert!(Arc::ptr_eq(
            &color_cache,
            &scratch.cache.values().next().unwrap().expression
        ));
        assert_eq!(scratch.position.capacity(), position_capacity);
        assert_eq!(scratch.requirements.capacity(), requirement_capacity);
    }
    let empty =
        prepare_dynamic_family_samples(&[], &Sources::default(), None, &mut scratch).unwrap();
    assert!(empty.families.is_empty() && empty.legacy.is_empty() && empty.requirements.is_empty());
    assert!(scratch.cache.is_empty());
}

#[test]
fn malformed_current_and_inactive_authored_evidence_remain_fatal() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let inputs = [
        sample(10, angle(ProgrammingComponent::Pan, 90.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
        sample(12, color(ColorComponent::Uv, 0.8)),
    ];
    assert!(matches!(
        prepare_dynamic_family_samples(
            &inputs,
            &Sources {
                tilt: Some(f32::NAN),
                ..Default::default()
            },
            None,
            &mut scratch
        ),
        Err(TransitionError::Invalid(_))
    ));
    assert!(
        scratch.families.is_empty() && scratch.requirements.is_empty() && scratch.cache.is_empty()
    );

    let mut invalid = angle(ProgrammingComponent::Pan, 90.0);
    let E::Programming {
        dependency_occurrence,
        ..
    } = &mut invalid
    else {
        unreachable!()
    };
    *dependency_occurrence = Some(DynamicSourceDependency::mapped(
        None,
        ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::new([ProgrammingTraceField::Focus]),
            remap: Arc::from([]),
        },
    ));
    let inputs = [
        sample(
            10,
            resume(Some(invalid), Some(color(ColorComponent::Uv, 0.8)), 1.0),
        ),
        sample(11, current(ProgrammingComponent::Tilt)),
    ];
    assert!(matches!(
        prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch),
        Err(TransitionError::Invalid(_))
    ));
    assert!(
        scratch.families.is_empty() && scratch.requirements.is_empty() && scratch.cache.is_empty()
    );
}

#[test]
fn unavailable_native_candidate_does_not_remove_another_color_candidate() {
    let model = models();
    let native = sample(
        10,
        E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: model.model.source.clone(),
                },
                component: Some(ProgrammingComponent::NativeColor(model.model.binding)),
            }),
            value: DynamicValue::Native(10),
            occurrence: None,
            dependency_occurrence: None,
        },
    );
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared = prepare_dynamic_family_samples(
        &[native, sample(11, color(ColorComponent::Uv, 0.8))],
        &Sources::default(),
        None,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].samples.len(), 1);
    assert_eq!(prepared.requirements.len(), 1);
    assert!(matches!(
        prepared.requirements[0].reason,
        DynamicFamilyPreparationRequirementReason::NativeColorModelUnavailable(_)
    ));
}
