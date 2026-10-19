//! Exact map-path oracle for adjacent writes; no compiled value or group identity is cached.
use super::super::controller::{FamilyOutput, PreparationOutput};
use super::*;

thread_local! {
    static FORCE_MAP: Cell<bool> = const { Cell::new(false) };
    static HITS: Cell<usize> = const { Cell::new(0) };
}
pub(crate) fn forced_map() -> bool {
    FORCE_MAP.with(Cell::get)
}
pub(crate) fn hit() {
    HITS.with(|count| count.set(count.get() + 1));
}
fn observed<T>(force: bool, run: impl FnOnce() -> T) -> (T, usize) {
    struct Reset(bool, usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            FORCE_MAP.with(|v| v.set(self.0));
            HITS.with(|v| v.set(self.1));
        }
    }
    let _reset = Reset(
        FORCE_MAP.with(|v| v.replace(force)),
        HITS.with(|v| v.replace(0)),
    );
    let result = run();
    (result, HITS.with(Cell::get))
}
fn payload(owner: ProgrammingOwner, id: u128) -> FamilyCompositionSample {
    let expression = match owner {
        ProgrammingOwner::Color => color(ColorComponent::Red, id as f32 / 100.),
        ProgrammingOwner::Position => whole(owner, angles(id as f32, 30.)),
        _ => unreachable!(),
    };
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(
        &[sample(id, expression)],
        &Sources::default(),
        None,
        &mut scratch,
    )
    .unwrap();
    scratch.families[0].samples[0].clone()
}
fn view(families: &[DynamicFamilySampleGroup]) -> String {
    preparation_snapshot(PreparedDynamicFamilySamples {
        families,
        legacy: &[],
        requirements: &[],
    })
}
#[test]
fn adjacent_hits_preserve_first_map_position_and_interleaved_whole_vector_appends() {
    let a = FixtureId(Uuid::from_u128(1));
    let b = FixtureId(Uuid::from_u128(2));
    let color = ProgrammingOwner::Color;
    let position = ProgrammingOwner::Position;
    let values = [
        payload(color, 10),
        payload(color, 20),
        payload(position, 30),
    ];
    let run = |force| {
        observed(force, || {
            let (mut families, mut indices, mut cursors, mut created, mut legacy, mut requirements) = (
                Vec::new(),
                HashMap::default(),
                Vec::new(),
                false,
                Vec::new(),
                Vec::new(),
            );
            let mut output = FamilyOutput {
                families: &mut families,
                family_indices: &mut indices,
                cursors: &mut cursors,
                created: &mut created,
                last: None,
                legacy: &mut legacy,
                requirements: &mut requirements,
            };
            output.append_one(a, color, values[0].clone());
            output.append_one(a, color, values[1].clone()); // adjacent hit
            output.append_one(a, position, values[2].clone()); // same target, different owner
            output.append_one(a, color, values[0].clone()); // earlier group, by map
            output.append(b, color, vec![values[1].clone()]); // Vec creates last group
            output.append_one(b, color, values[0].clone()); // adjacent hit after Vec
            output.append(a, position, vec![]); // no group or last-key change
            output.append(a, color, vec![values[1].clone()]); // existing nonlast
            output.append_one(b, color, values[1].clone()); // last written was (a, color)
            assert_eq!(indices[&(a, color)], 0);
            assert_eq!(indices[&(a, position)], 1);
            assert_eq!(indices[&(b, color)], 2);
            assert_eq!(
                families.iter().map(|g| g.samples.len()).collect::<Vec<_>>(),
                [4, 1, 3]
            );
            view(&families)
        })
    };
    let (expected, misses) = run(true);
    let (actual, hits) = run(false);
    assert_eq!(actual, expected);
    assert_eq!(misses, 0);
    assert_eq!(
        hits, 2,
        "adjacent writes to the last written group skip the map"
    );
}
fn inputs(frame: u128) -> Vec<DynamicRuntimeSample> {
    let mut input = Vec::new();
    for target in 0..3 {
        for (lane, component) in [
            ColorComponent::Red,
            ColorComponent::Green,
            ColorComponent::Blue,
        ]
        .into_iter()
        .enumerate()
        {
            let mut s = sample(
                10 + lane as u128,
                color(component, frame as f32 * 0.1 + 0.2),
            );
            s.target = FixtureId(Uuid::from_u128(100 + target));
            s.activation_mix = 0.7;
            input.push(s);
        }
    }
    let mut pan = sample(40, angle(ProgrammingComponent::Pan, 60.));
    pan.controller_id = Uuid::from_u128(90);
    let mut tilt = sample(41, current(ProgrammingComponent::Tilt));
    tilt.controller_id = pan.controller_id;
    input.extend([pan, tilt]);
    input
}
#[test]
fn public_serial_and_ordered_worker_replay_match_forced_map_with_requirements_and_fallback() {
    for chunks in [0, 1, 2, 4] {
        for stop in [false, true] {
            let mut old = DynamicFamilyPreparationScratch::default();
            let mut new = DynamicFamilyPreparationScratch::default();
            for frame in 0..3 {
                let input = inputs(frame);
                let requirements = if frame == 1 {
                    vec![DynamicFamilyPreparationRequirement {
                        target: input[0].target,
                        owner: ProgrammingOwner::Color,
                        rank: rank(&input[0]),
                        reason: DynamicFamilyPreparationRequirementReason::Transition(
                            TransitionRequirement::CompatibleOwners,
                        ),
                    }]
                } else {
                    vec![]
                };
                let (expected, _) = observed(true, || {
                    prepare_observed(&input, &requirements, &mut old, chunks, stop)
                });
                let (actual, hits) = observed(false, || {
                    prepare_observed(&input, &requirements, &mut new, chunks, stop)
                });
                assert_eq!(actual, expected);
                assert!(actual.0.is_ok());
                assert!(actual.1 > 0, "Current must still resolve freshly");
                assert!(
                    hits >= 4,
                    "multiple real controller outputs must use the shortcut"
                );
                assert_eq!(old.cache.len(), new.cache.len());
                if frame != 1 {
                    for group in old.prepared_for_drain().families {
                        group.samples.clear();
                    }
                    for group in new.prepared_for_drain().families {
                        group.samples.clear();
                    }
                }
            }
        }
    }
}
#[test]
fn fresh_current_missing_values_late_validation_and_recovery_match_original_map() {
    let run = |force| {
        let mut scratch = DynamicFamilyPreparationScratch::default();
        let mut history = Vec::new();
        for tilt in [Some(10.), None, Some(50.)] {
            let mut input = inputs(0);
            let sources = Sources {
                tilt,
                ..Default::default()
            };
            let (result, hits) = observed(force, || {
                prepare_dynamic_family_samples(&input, &sources, None, &mut scratch)
                    .map(preparation_snapshot)
                    .map_err(|e| format!("{e:?}"))
            });
            assert_eq!(sources.reads.get(), 1);
            if tilt.is_some() {
                assert!(result.is_ok());
                if !force {
                    assert!(hits > 0);
                }
            }
            history.push((result, sources.reads.get(), scratch.cache.len()));
            input[2].activation_mix = f32::NAN;
            let result = observed(force, || {
                prepare_dynamic_family_samples(&input, &sources, None, &mut scratch)
                    .map(preparation_snapshot)
                    .map_err(|e| format!("{e:?}"))
            })
            .0;
            assert!(result.is_err());
            history.push((result, sources.reads.get(), scratch.cache.len()));
            input[2].activation_mix = 0.7;
            input[1].lane_id = input[0].lane_id;
            let result = observed(force, || {
                prepare_dynamic_family_samples(&input, &sources, None, &mut scratch)
                    .map(preparation_snapshot)
                    .map_err(|e| format!("{e:?}"))
            })
            .0;
            assert!(result.is_err());
            history.push((result, sources.reads.get(), scratch.cache.len()));
        }
        history
    };
    assert_eq!(run(false), run(true));
}
