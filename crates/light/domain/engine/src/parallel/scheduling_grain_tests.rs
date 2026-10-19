use super::*;
use light_dynamics::InstanceWorkers;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicUsize, Ordering},
};

fn uneven_value(index: usize) -> usize {
    // Deliberately different costs; no timing assumption or required execution order.
    let rounds = if index.is_multiple_of(7) { 10_000 } else { 3 };
    let mut value = index;
    for _ in 0..rounds {
        value = std::hint::black_box(value).wrapping_mul(31).wrapping_add(7);
    }
    value
}

#[test]
fn grain_policy_is_bounded_for_empty_small_and_overflowing_budgets() {
    assert_eq!(scheduling_min_len(20, 2), 5);
    assert_eq!(scheduling_min_len(8, 2), 2);
    for items in [0, 1, 2, 3, 7, 8, 19, 20, usize::MAX] {
        for workers in [0, 1, 2, 4, usize::MAX / 2, usize::MAX] {
            let grain = scheduling_min_len(items, workers);
            assert!((1..=items.max(1)).contains(&grain));
            if workers > items {
                assert_eq!(grain, 1);
            }
        }
    }
    assert_eq!(scheduling_min_len(usize::MAX, usize::MAX), 1);
    assert_eq!(scheduling_min_len(usize::MAX, 0), usize::MAX);
}

#[test]
fn ordered_callbacks_stay_exactly_once_and_results_stable_under_uneven_work() {
    for workers in [1, 2, 4] {
        let pool = OutputPool::new(workers);
        for chunks in [0, 1, 3, 8, 19, 37] {
            let seen = (0..chunks).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
            let mut scratch = vec![0usize; workers];
            let actual = run_ordered(pool.as_ref(), &mut scratch, chunks, |uses, chunk| {
                *uses += 1;
                assert_eq!(seen[chunk].fetch_add(1, Ordering::Relaxed), 0);
                (chunk, uneven_value(chunk))
            });
            assert_eq!(
                actual,
                (0..chunks)
                    .map(|i| (i, uneven_value(i)))
                    .collect::<Vec<_>>()
            );
            assert_eq!(scratch.iter().sum::<usize>(), chunks);
            assert!(seen.iter().all(|count| count.load(Ordering::Relaxed) == 1));
        }
    }
}

#[test]
fn indexed_callbacks_stay_exactly_once_with_uneven_work_and_small_inputs() {
    for workers in [2, 4] {
        let pool = OutputPool::new(workers).unwrap();
        for count in [0, 1, 3, 8, 20, 37] {
            let seen = (0..count).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
            let output = (0..count).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
            pool.run_indexed(count, &|index| {
                assert_eq!(seen[index].fetch_add(1, Ordering::Relaxed), 0);
                output[index].store(uneven_value(index), Ordering::Relaxed);
            });
            assert!(seen.iter().all(|count| count.load(Ordering::Relaxed) == 1));
            assert_eq!(
                output
                    .iter()
                    .map(|v| v.load(Ordering::Relaxed))
                    .collect::<Vec<_>>(),
                (0..count).map(uneven_value).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn panic_propagates_and_same_pool_and_scratch_recover_for_both_apis() {
    for workers in [2, 4] {
        let pool = OutputPool::new(workers).unwrap();
        let mut scratch = vec![0usize; workers];
        let failed = catch_unwind(AssertUnwindSafe(|| {
            run_ordered(Some(&pool), &mut scratch, 8, |uses, index| {
                *uses += 1;
                assert_ne!(index, 3, "ordered callback failure");
                index
            })
        }));
        assert!(failed.is_err());
        scratch.fill(0);
        assert_eq!(
            run_ordered(Some(&pool), &mut scratch, 8, |uses, index| {
                *uses += 1;
                index
            }),
            (0..8).collect::<Vec<_>>()
        );
        assert_eq!(scratch.iter().sum::<usize>(), 8);
        let failed = catch_unwind(AssertUnwindSafe(|| {
            pool.run_indexed(20, &|index| {
                assert_ne!(index, 7, "indexed callback failure")
            });
        }));
        assert!(failed.is_err());
        let seen = (0..20).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
        pool.run_indexed(20, &|index| {
            seen[index].fetch_add(1, Ordering::Relaxed);
        });
        assert!(seen.iter().all(|count| count.load(Ordering::Relaxed) == 1));
    }
}
