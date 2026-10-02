use super::*;
use crate::ProgrammingFamilyFixAt;

fn fixed(source: FamilyFixedSampleSource, row_index: usize) -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 1,
        identity: FamilySampleIdentity::Fixed { source, row_index },
    }
}

fn focus(value: f32, rank: FamilySampleRank) -> FamilySample {
    ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Focus,
        None,
        AttributeValue::Normalized(value),
    )
    .unwrap()
    .compile(None, &FamilyEditContext::default(), rank, 1.0)
    .unwrap()
}

#[test]
fn cue_submillis_precedes_stable_order_in_actual_fixed_composition() {
    let mut older = fixed(FamilyFixedSampleSource::Cue, 1);
    older.changed_at_submillis_nanos = 100;
    older.stable_order = u128::MAX;
    let mut newer = fixed(FamilyFixedSampleSource::Cue, 0);
    newer.changed_at_submillis_nanos = 200;
    newer.stable_order = 0;
    let mut samples = [focus(0.2, older), focus(0.8, newer)];
    for _ in 0..2 {
        assert_eq!(
            compose(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(0.4),
                &samples
            )
            .unwrap(),
            AttributeValue::Normalized(0.8)
        );
        samples.reverse();
    }
    newer.changed_at_submillis_nanos = 1_000_000;
    assert!(
        ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Focus,
            None,
            AttributeValue::Normalized(0.8)
        )
        .unwrap()
        .compile(None, &FamilyEditContext::default(), newer, 1.0)
        .is_err()
    );
}

#[test]
fn fixed_capture_rows_order_without_authorship_and_do_not_form_dynamic_angle_pairs() {
    let identities = [
        fixed(FamilyFixedSampleSource::Programmer, 0),
        fixed(FamilyFixedSampleSource::Programmer, 1),
        fixed(FamilyFixedSampleSource::ExtraProgrammer, 0),
        fixed(FamilyFixedSampleSource::Cue, 0),
        fixed(FamilyFixedSampleSource::Cue, 1),
    ];
    assert!(identities.windows(2).all(|pair| pair[0] < pair[1]));
    let mut samples = identities
        .into_iter()
        .enumerate()
        .map(|(index, rank)| focus(index as f32 / 5.0, rank))
        .collect::<Vec<_>>();
    assert!(
        samples
            .iter()
            .all(|sample| sample.rank.dynamic_identity().is_none())
    );
    // No occurrence metadata is needed to apply or order a valid captured mask.
    assert!(samples.iter().flat_map(FamilySample::trace_sources).all(
        |leaf| matches!(leaf, FamilyTraceLeaf::Source(source) if source.occurrence.is_none())
    ));
    for _ in 0..2 {
        assert_eq!(
            compose(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(0.9),
                &samples
            )
            .unwrap(),
            AttributeValue::Normalized(0.8)
        );
        samples.reverse();
    }
    let pan = ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Position,
        Some(ProgrammingComponent::Pan),
        angles(90.0, 40.0),
    )
    .unwrap()
    .compile(
        None,
        &FamilyEditContext::default(),
        fixed(FamilyFixedSampleSource::Programmer, 0),
        1.0,
    )
    .unwrap();
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(10.0, 20.0), &[pan]).unwrap(),
        angles(90.0, 20.0)
    );
}

#[test]
fn dynamic_rank_suffix_preserves_instance_controller_lane_order() {
    let mut ranks = Vec::new();
    for (priority, millis, order, instance, controller, lane) in [
        (11, 0, 0, 0, 0, 0),
        (10, 101, 0, 0, 0, 0),
        (10, 100, 2, 0, 0, 0),
        (10, 100, 1, 2, 0, 0),
        (10, 100, 1, 1, 2, 0),
        (10, 100, 1, 1, 1, 2),
        (10, 100, 1, 1, 1, 1),
    ] {
        ranks.push(FamilySampleRank {
            priority,
            changed_at_millis: millis,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(instance),
                controller_id: Uuid::from_u128(controller),
                lane_id: Uuid::from_u128(lane),
            },
        });
    }
    let mut legacy = ranks.clone();
    legacy.sort_by_key(|rank| {
        let source = rank.dynamic_identity().unwrap();
        (
            rank.priority,
            rank.changed_at_millis,
            rank.stable_order,
            source.instance_id,
            source.controller_id,
            source.lane_id,
        )
    });
    ranks.sort();
    assert_eq!(ranks, legacy);
    assert!(
        ranks[0] < fixed(FamilyFixedSampleSource::Programmer, 0),
        "exact common-key ties preserve the scalar Dynamic-before-fixed order"
    );
}
