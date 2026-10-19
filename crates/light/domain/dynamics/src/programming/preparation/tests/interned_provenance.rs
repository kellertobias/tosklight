//! TL-842 C2: a plain Color leaf's provenance is shared with the next frame only while every
//! input of its leaves (rank, footprint, occurrence and dependency) is unchanged.
use super::*;

fn id(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}

fn attributed(value: f32, occurrence: u128, dependency: Option<u128>) -> E {
    let mut leaf = color(ColorComponent::Red, value);
    let E::Programming {
        occurrence: authored,
        dependency_occurrence,
        ..
    } = &mut leaf
    else {
        unreachable!()
    };
    *authored = Some(id(occurrence));
    *dependency_occurrence =
        dependency.map(|current| DynamicSourceDependency::identity(Some(id(current))));
    leaf
}

type Provenance = (
    *const (),
    Vec<(FamilyTraceSource, Option<DynamicSourceDependency>)>,
);

/// The single prepared Color sample's provenance, serially or over four replayed chunks.
fn prepare(
    samples: &[DynamicRuntimeSample],
    scratch: &mut DynamicFamilyPreparationScratch,
    parallel: bool,
) -> Option<Provenance> {
    let prepared = if parallel {
        prepare_dynamic_family_samples_in_parallel(
            samples,
            &[],
            &Sources::default(),
            None,
            scratch,
            &mut FourChunks,
        )
    } else {
        prepare_dynamic_family_samples(samples, &Sources::default(), None, scratch)
    }
    .unwrap();
    let group = prepared.families.first()?;
    assert_eq!(group.samples.len(), 1);
    let FamilyCompositionSample::Known(known) = &group.samples[0] else {
        panic!("a plain Color leaf stays a Known sample")
    };
    known.leaf_provenance_for_test()
}

fn leaves(
    provenance: &Provenance,
) -> Vec<(FamilyTraceRole, Option<DynamicSourceOccurrenceId>, i16)> {
    provenance
        .1
        .iter()
        .map(|(source, _)| (source.role, source.occurrence, source.rank.priority))
        .collect()
}

#[test]
fn unchanged_leaf_inputs_share_provenance_and_any_change_rebuilds_it() {
    for parallel in [false, true] {
        let mut scratch = DynamicFamilyPreparationScratch::default();
        let lane = |value, priority, dependency| {
            let mut sample = sample(10, attributed(value, 7, dependency));
            sample.priority = priority;
            sample
        };
        let first = prepare(&[lane(0.2, 10, Some(8))], &mut scratch, parallel).unwrap();
        // A changed value alone keeps every leaf input: the provenance is shared.
        let second = prepare(&[lane(0.6, 10, Some(8))], &mut scratch, parallel).unwrap();
        assert_eq!(
            second.0, first.0,
            "parallel={parallel}: unchanged inputs share storage"
        );
        assert_eq!(second.1, first.1);
        assert_eq!(
            leaves(&second),
            [
                (FamilyTraceRole::Authored, Some(id(7)), 10),
                (FamilyTraceRole::CalculationDependency, Some(id(8)), 10),
            ]
        );
        // A new rank must not keep the old rank's leaves.
        let ranked = prepare(&[lane(0.6, 20, Some(8))], &mut scratch, parallel).unwrap();
        assert_ne!(
            ranked.0, second.0,
            "parallel={parallel}: rank change rebuilds"
        );
        assert_eq!(
            leaves(&ranked),
            [
                (FamilyTraceRole::Authored, Some(id(7)), 20),
                (FamilyTraceRole::CalculationDependency, Some(id(8)), 20),
            ]
        );
        // Neither may a new or removed Current dependency.
        let moved = prepare(&[lane(0.6, 20, Some(9))], &mut scratch, parallel).unwrap();
        assert_ne!(
            moved.0, ranked.0,
            "parallel={parallel}: dependency change rebuilds"
        );
        assert_eq!(moved.1[1].0.occurrence, Some(id(9)));
        assert_eq!(
            moved.1[1].1,
            Some(DynamicSourceDependency::identity(Some(id(9))))
        );
        let removed = prepare(&[lane(0.6, 20, None)], &mut scratch, parallel).unwrap();
        assert_eq!(
            leaves(&removed),
            [(FamilyTraceRole::Authored, Some(id(7)), 20)]
        );
        // A frame without the lane keeps nothing for later frames.
        assert!(prepare(&[], &mut scratch, parallel).is_none());
        assert_eq!(scratch.provenance.len(), (1, 0), "parallel={parallel}");
        assert!(prepare(&[], &mut scratch, parallel).is_none());
        assert_eq!(scratch.provenance.len(), (0, 0), "parallel={parallel}");
        // An unattributed leaf allocates nothing and records nothing.
        let mut plain = sample(10, color(ColorComponent::Red, 0.4));
        plain.priority = 20;
        assert!(prepare(&[plain], &mut scratch, parallel).is_none());
        assert_eq!(scratch.provenance.len(), (0, 0), "parallel={parallel}");
    }
}

/// TL-842: a Color lane rewritten in place keeps its own provenance when every leaf input is
/// unchanged, without a cache lookup per lane per frame; a changed input still rebuilds it.
#[test]
fn unchanged_refreshed_leaves_keep_provenance_without_a_lookup() {
    use crate::programming::composition::INTERNS;
    let interns = || INTERNS.with(|count| count.get());
    let refreshes = || super::super::plain_scalar::tests::REFRESHES.with(|count| count.get());
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let lane = |value, priority, dependency| {
        let mut sample = sample(10, attributed(value, 7, dependency));
        sample.priority = priority;
        sample
    };
    let first = prepare(&[lane(0.2, 10, Some(8))], &mut scratch, false).unwrap();
    let (looked_up, refreshed) = (interns(), refreshes());
    let second = prepare(&[lane(0.6, 10, Some(8))], &mut scratch, false).unwrap();
    assert_eq!(refreshes() - refreshed, 1, "the lane is rewritten in place");
    assert_eq!(
        interns() - looked_up,
        0,
        "unchanged provenance needs no lookup"
    );
    assert_eq!(second, first, "the same leaves, in the same storage");
    // Any changed input still goes through the cache and rebuilds.
    let looked_up = interns();
    let ranked = prepare(&[lane(0.6, 20, Some(8))], &mut scratch, false).unwrap();
    assert_eq!(interns() - looked_up, 1);
    assert_ne!(ranked.0, second.0);
    assert_eq!(ranked.1[0].0.rank.priority, 20);
    let moved = prepare(&[lane(0.6, 20, Some(9))], &mut scratch, false).unwrap();
    assert_ne!(moved.0, ranked.0);
    assert_eq!(
        moved.1[1].1,
        Some(DynamicSourceDependency::identity(Some(id(9))))
    );
    let removed = prepare(&[lane(0.6, 20, None)], &mut scratch, false).unwrap();
    assert_eq!(
        removed.1.len(),
        1,
        "a removed dependency drops its Current leaf"
    );
    let added = prepare(&[lane(0.6, 20, Some(9))], &mut scratch, false).unwrap();
    assert_eq!(
        added.1.len(),
        2,
        "an added dependency adds its Current leaf"
    );
    let mut reattributed = sample(10, attributed(0.6, 5, Some(9)));
    reattributed.priority = 20;
    let reattributed = prepare(&[reattributed], &mut scratch, false).unwrap();
    assert_eq!(reattributed.1[0].0.occurrence, Some(id(5)));
    // The same Current occurrence with a different transfer is a different dependency.
    let mut unknown = sample(10, attributed(0.6, 5, Some(9)));
    unknown.priority = 20;
    let E::Programming {
        dependency_occurrence,
        ..
    } = &mut unknown.expression
    else {
        unreachable!()
    };
    *dependency_occurrence = Some(DynamicSourceDependency::unknown(Some(id(9))));
    let transferred = prepare(&[unknown], &mut scratch, false).unwrap();
    assert_eq!(
        transferred.1[1].1,
        Some(DynamicSourceDependency::unknown(Some(id(9))))
    );
    // Without a dependency, a new rank still rebuilds the single authored leaf.
    let plain = |priority| {
        let mut sample = sample(10, attributed(0.6, 5, None));
        sample.priority = priority;
        sample
    };
    prepare(&[plain(20)], &mut scratch, false).unwrap();
    let reranked = prepare(&[plain(30)], &mut scratch, false).unwrap();
    assert_eq!(reranked.1[0].0.rank.priority, 30);
}
