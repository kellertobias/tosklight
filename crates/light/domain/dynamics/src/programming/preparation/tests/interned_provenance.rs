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
