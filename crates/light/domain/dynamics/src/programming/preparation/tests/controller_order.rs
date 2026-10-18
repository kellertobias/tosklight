//! Reusing only the exact identity permutation never reuses sampled values or validation.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
thread_local! { static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) }; }
pub(crate) fn record(hit: bool) {
    COUNTS.with(|count| {
        let (hits, misses) = count.get();
        count.set((hits + usize::from(hit), misses + usize::from(!hit)));
    });
}
fn counts() -> (usize, usize) {
    COUNTS.with(|count| count.replace((0, 0)))
}
fn inputs() -> Vec<DynamicRuntimeSample> {
    (0..24)
        .rev()
        .map(|i| {
            let mut s = sample(100 + i % 3, color(ColorComponent::Red, 0.2));
            s.instance_id = Uuid::from_u128(1 + i / 12);
            s.controller_id = Uuid::from_u128(10 + i / 6);
            s.target = FixtureId(Uuid::from_u128(1000 + i / 3));
            s
        })
        .collect()
}
fn order(
    input: &[DynamicRuntimeSample],
    scratch: &mut DynamicFamilyPreparationScratch,
    reuse: bool,
) {
    controller::sort_controllers(input, scratch, &|keys| keys.sort_unstable(), reuse);
}
#[test]
fn controller_order_identity_hits_and_every_key_edit_misses() {
    let input = inputs();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    counts();
    order(&input, &mut scratch, true);
    order(&input, &mut scratch, true);
    assert_eq!(counts(), (1, 1));
    let expected = (scratch.order.clone(), scratch.controllers.clone());
    let mut fresh = input.clone();
    fresh[4].priority += 1;
    fresh[4].activation_mix = 0.5;
    fresh[4].expression = color(ColorComponent::Red, 0.8);
    order(&fresh, &mut scratch, true);
    assert_eq!(counts(), (1, 0));
    assert_eq!(
        (scratch.order.clone(), scratch.controllers.clone()),
        expected
    );
    for field in 0..6 {
        let mut changed = input.clone();
        match field {
            0 => changed[4].instance_id = Uuid::from_u128(999),
            1 => changed[4].controller_id = Uuid::from_u128(999),
            2 => changed[4].target = FixtureId(Uuid::from_u128(999)),
            3 => changed[4].lane_id = Uuid::from_u128(999),
            4 => changed.swap(4, 5),
            _ => {
                changed.pop();
            }
        }
        order(&input, &mut scratch, true);
        counts();
        order(&changed, &mut scratch, true);
        assert_eq!(counts(), (0, 1));
        let mut oracle = DynamicFamilyPreparationScratch::default();
        order(&changed, &mut oracle, false);
        assert_eq!(scratch.order, oracle.order);
        assert_eq!(scratch.controllers, oracle.controllers);
    }
}
#[test]
fn controller_order_custom_then_canonical_and_partial_sort_panic_cannot_reuse() {
    let input = inputs();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let calls = Cell::new(0);
    for _ in 0..2 {
        controller::sort_controllers(
            &input,
            &mut scratch,
            &|keys| {
                calls.set(calls.get() + 1);
                keys.sort_unstable_by(|a, b| b.cmp(a));
            },
            false,
        );
        assert!(scratch.sort_keys.is_empty());
    }
    assert_eq!(calls.get(), 2);
    counts();
    order(&input, &mut scratch, true);
    assert_eq!(counts(), (0, 1));
    let canonical = scratch.order.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| controller::sort_controllers(
            &input,
            &mut scratch,
            &|keys| {
                keys.reverse();
                panic!("injected sorter failure");
            },
            false
        )))
        .is_err()
    );
    assert!(scratch.order.is_empty());
    assert!(scratch.controllers.is_empty());
    counts();
    order(&input, &mut scratch, true);
    assert_eq!(counts(), (0, 1));
    assert_eq!(scratch.order, canonical);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut changed = input.clone();
            changed[0].lane_id = Uuid::from_u128(999);
            controller::sort_controllers(
                &changed,
                &mut scratch,
                &|keys| {
                    keys.sort_unstable();
                    panic!("canonical failure after sorting");
                },
                true,
            );
        }))
        .is_err()
    );
    counts();
    order(&input, &mut scratch, true);
    assert_eq!(counts(), (0, 1));
    assert_eq!(scratch.order, canonical);
}
struct Workers {
    chunks: usize,
    reuse: bool,
    calls: Cell<usize>,
}
impl PreparationWorkers for Workers {
    fn chunks(&self) -> usize {
        self.chunks
    }
    fn reuses_controller_order(&self) -> bool {
        self.reuse
    }
    fn sort_controller_keys(&self, keys: &mut [ControllerSortKey]) {
        self.calls.set(self.calls.get() + 1);
        keys.sort_unstable();
    }
    fn run(
        &mut self,
        prepare: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        Ok((0..self.chunks)
            .map(|chunk| prepare(chunk, &Sources::default()))
            .collect())
    }
    fn run_completion(
        &mut self,
        _: &(dyn Fn(usize, &dyn PreparationSources) -> crate::CompletedChunk + Sync),
    ) -> Result<Vec<crate::CompletedChunk>, TransitionError> {
        unreachable!()
    }
    fn append_logs(&mut self, _: usize, from: (usize, usize), to: (usize, usize)) {
        assert_eq!(from, (0, 0));
        assert_eq!(to, (0, 0));
    }
}
fn snapshot(view: PreparedDynamicFamilySamples<'_>) -> String {
    let groups = view
        .families
        .iter()
        .map(|group| {
            let base = match group.owner {
                ProgrammingOwner::Color => semantic(ColorIntent::default()),
                _ => angles(0.0, 0.0),
            };
            let mut scratch = RetainedFamilyCompositionScratch::default();
            let value = compose_retained_dynamic_family_traced(
                group.owner,
                &base,
                &group.samples,
                &FamilyCompositionContext {
                    edit: FamilyEditContext {
                        color_model: Some(&VirtualColorAuthoringV1),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                &Frame,
                &mut scratch,
            );
            format!(
                "{:?}/{:?}/{:?}/{}",
                group.target,
                group.owner,
                value,
                format!(
                    "{:?}",
                    value.as_ref().ok().and_then(|value| {
                        let fields = ProgrammingFieldScope::for_value(group.owner, value).ok()?;
                        let trace = scratch.family_trace();
                        Some(trace.query_fields_with_base(trace.root()?, &fields))
                    })
                )
            )
        })
        .collect::<Vec<_>>();
    format!("{groups:?}/{:?}/{:?}", view.legacy, view.requirements)
}
#[test]
fn controller_order_public_preparation_reuses_layout_but_not_values_errors_or_requirements() {
    for chunks in [1, 2, 4] {
        let mut reused = DynamicFamilyPreparationScratch::default();
        let mut oracle = DynamicFamilyPreparationScratch::default();
        let mut workers = Workers {
            chunks,
            reuse: true,
            calls: Cell::new(0),
        };
        let mut generic = Workers {
            chunks,
            reuse: false,
            calls: Cell::new(0),
        };
        for frame in 0..6 {
            let mut input = inputs();
            for s in &mut input {
                s.expression = color(ColorComponent::Red, 0.1 + frame as f32 * 0.1);
                s.priority += frame;
                s.activation_mix = 0.9 - frame as f32 * 0.01;
            }
            // Current must still refresh requirements despite a stable identity permutation.
            if frame == 2 || frame == 3 {
                input[0].expression = current(ProgrammingComponent::Tilt);
            }
            let actual = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut reused,
                &mut workers,
            )
            .map(snapshot);
            let expected = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut oracle,
                &mut generic,
            )
            .map(snapshot);
            assert_eq!(actual, expected);
            assert!(actual.is_ok());
        }
        assert_eq!(workers.calls.get(), 1);
        assert_eq!(generic.calls.get(), 6);
        // Invalid fresh data uses the retained layout and still clears all output/cache state.
        for fault in 0..3 {
            let mut input = inputs();
            if fault == 1 {
                input[1].lane_id = input[0].lane_id;
            } else if fault == 0 {
                input[20].activation_mix = f32::NAN;
            } else {
                input[20].expression = color(ColorComponent::Red, f32::NAN);
            }
            let a = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut reused,
                &mut workers,
            )
            .map(snapshot);
            let b = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut oracle,
                &mut generic,
            )
            .map(snapshot);
            assert_eq!(a, b);
            assert!(a.is_err());
            assert!(reused.families.is_empty());
            assert!(reused.cache.is_empty());
            let input = inputs();
            let a = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut reused,
                &mut workers,
            )
            .map(snapshot);
            let b = prepare_dynamic_family_samples_in_parallel(
                &input,
                &[],
                &Sources::default(),
                None,
                &mut oracle,
                &mut generic,
            )
            .map(snapshot);
            assert_eq!(a, b);
        }
    }
}
#[test]
fn controller_order_clear_empty_and_serial_after_custom_preserve_public_behavior() {
    let input = inputs();
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(&input, &Sources::default(), None, &mut scratch).unwrap();
    scratch.clear();
    assert!(!scratch.controllers.is_empty()); // Existing clear deliberately retains this storage.
    let mut workers = Workers {
        chunks: 2,
        reuse: true,
        calls: Cell::new(0),
    };
    let empty = prepare_dynamic_family_samples_in_parallel(
        &[],
        &[],
        &Sources::default(),
        None,
        &mut scratch,
        &mut workers,
    )
    .unwrap();
    assert!(empty.families.is_empty());
    assert!(scratch.controllers.is_empty());
    let mut generic = Workers {
        chunks: 2,
        reuse: false,
        calls: Cell::new(0),
    };
    prepare_dynamic_family_samples_in_parallel(
        &input,
        &[],
        &Sources::default(),
        None,
        &mut scratch,
        &mut generic,
    )
    .unwrap();
    assert!(scratch.sort_keys.is_empty());
    counts();
    prepare_dynamic_family_samples(&input, &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(counts(), (0, 1));
    counts();
    prepare_dynamic_family_samples(&input, &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(counts(), (1, 0));
}

#[test]
fn controller_order_serial_reuses_only_identity_while_current_is_read_again() {
    let input = [
        sample(10, angle(ProgrammingComponent::Pan, 20.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
    ];
    let mut reused = DynamicFamilyPreparationScratch::default();
    let mut oracle = DynamicFamilyPreparationScratch::default();
    counts();
    for tilt in [Some(10.0), None, Some(30.0)] {
        let actual_sources = Sources {
            pan: Some(20.0),
            tilt,
            reads: Cell::new(0),
        };
        let expected_sources = Sources {
            pan: Some(20.0),
            tilt,
            reads: Cell::new(0),
        };
        oracle.sort_keys.clear();
        let actual = prepare_dynamic_family_samples(&input, &actual_sources, None, &mut reused)
            .map(snapshot);
        let expected = prepare_dynamic_family_samples(&input, &expected_sources, None, &mut oracle)
            .map(snapshot);
        assert_eq!(actual, expected);
        assert!(actual.is_ok());
        assert!(actual_sources.reads.get() > 0);
        assert_eq!(actual_sources.reads.get(), expected_sources.reads.get());
    }
    assert_eq!(counts(), (2, 4));
}
