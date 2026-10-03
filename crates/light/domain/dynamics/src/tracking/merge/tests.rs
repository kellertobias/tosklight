use super::*;
use crate::tracking::tests::{off, on, reference};
use crate::{DynamicReference, DynamicValueTiming};
use light_core::{AttributeValue, programming::ProgrammingComponent};
use std::time::{Duration, Instant};

/// Small deterministic generator (xorshift64*), so failures reproduce.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

fn random_row(
    rng: &mut Rng,
    fixtures: &[FixtureId],
    links: &[Uuid],
    reference: &DynamicReference,
) -> DynamicAddressValue {
    let fixture = fixtures[rng.below(fixtures.len() as u64) as usize];
    let link = links[rng.below(links.len() as u64) as usize];
    // Few distinct stamps, a share of legacy zero orders, so ties and mixed comparisons occur.
    let order = if rng.below(4) == 0 { 0 } else { rng.below(12) };
    let changed = rng.below(12);
    let lane = Uuid::from_u128(u128::from(rng.below(3)) + 1);
    let mut row = match rng.below(7) {
        0 | 1 => on(fixture, link, lane, order, reference),
        2 => off(fixture, link, order),
        _ => DynamicAddressValue {
            fixture_id: fixture,
            attribute: AttributeKey("color".into()),
            value: match rng.below(4) {
                0 => DynamicSemanticValue::Release,
                1 => DynamicSemanticValue::ProgrammingRelease {
                    component: [None, Some(ProgrammingComponent::Pan)][rng.below(2) as usize],
                },
                2 => DynamicSemanticValue::FixAt {
                    value: 0.5,
                    timing: DynamicValueTiming::default(),
                },
                _ => DynamicSemanticValue::Static {
                    value: AttributeValue::Normalized(0.25),
                    timing: DynamicValueTiming::default(),
                },
            },
            programmer_order: order,
            changed_at_millis: changed,
        },
    };
    if rng.below(2) == 0 {
        row.attribute = AttributeKey(["color", "position"][rng.below(2) as usize].into());
    }
    row.changed_at_millis = changed;
    row
}

fn identities(rows: &[&DynamicAddressValue]) -> Vec<*const DynamicAddressValue> {
    rows.iter().map(|row| *row as *const _).collect()
}

#[test]
fn indexed_fold_keeps_exactly_the_rows_the_pairwise_relation_keeps() {
    let reference = reference();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..600 {
        let fixtures = (0..1 + rng.below(4))
            .map(|_| FixtureId::new())
            .collect::<Vec<_>>();
        let links = (0..1 + rng.below(3))
            .map(|index| Uuid::from_u128(100 + index as u128))
            .collect::<Vec<_>>();
        let count = 1 + rng.below(if case % 3 == 0 { 120 } else { 40 }) as usize;
        let rows = (0..count)
            .map(|_| random_row(&mut rng, &fixtures, &links, &reference))
            .collect::<Vec<_>>();
        let borrowed = rows.iter().collect::<Vec<_>>();
        let expected = pairwise(borrowed.clone());
        let built = BucketIndex::build(&borrowed);
        let indexed = borrowed
            .iter()
            .enumerate()
            .filter(|(position, row)| !built.dropped(*position, row))
            .map(|(_, row)| *row)
            .collect::<Vec<_>>();
        assert_eq!(
            identities(&indexed),
            identities(&expected),
            "case {case}: {count} rows"
        );
        assert_eq!(
            identities(&fold(borrowed)),
            identities(&expected),
            "case {case}"
        );
    }
}

/// Starting a Dynamic on thousands of targets used to compare every pair of authored rows on
/// the first frame (more than ten seconds at 4,000 targets). The fold must stay near-linear:
/// for a start (every lane of every target survives) and when one instance-wide Off
/// conflicts with every row of its instance.
#[test]
fn fold_time_grows_near_linearly_with_target_count() {
    let reference = reference();
    let rows = |targets: usize, with_off: bool| {
        let link = Uuid::from_u128(7);
        (0..targets)
            .flat_map(|index| {
                let fixture = FixtureId::new();
                let order = 1 + index as u64 * 3;
                let mut rows = vec![
                    on(fixture, link, Uuid::from_u128(1), order, &reference),
                    on(fixture, link, Uuid::from_u128(2), order + 1, &reference),
                ];
                rows.extend(with_off.then(|| off(fixture, link, order + 2)));
                rows
            })
            .collect::<Vec<_>>()
    };
    let timed = |rows: &[DynamicAddressValue]| {
        let started = Instant::now();
        let kept = fold(rows.iter().collect());
        (started.elapsed(), kept.len())
    };
    timed(&rows(64, true));
    for targets in [1_000, 4_000] {
        // Generous for a loaded host: 25 µs per target (the fold takes under 1 µs). The pairwise
        // relation took about 250 µs per target at 4,000.
        let bound = Duration::from_micros(25) * targets as u32;
        let (elapsed, kept) = timed(&rows(targets, false));
        assert_eq!(kept, targets * 2, "a start keeps every lane");
        assert!(
            elapsed < bound,
            "start of {targets}: {elapsed:?}, bound {bound:?}"
        );
        let with_off = rows(targets, true);
        let (elapsed, kept) = timed(&with_off);
        // Only the newest Off survives: every other row conflicts with a later Off.
        assert_eq!(kept, 1);
        assert!(
            elapsed < bound,
            "Off over {targets}: {elapsed:?}, bound {bound:?}"
        );
    }
}
