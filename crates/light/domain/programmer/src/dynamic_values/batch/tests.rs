use super::super::{mutation_changes, same_track};
use super::*;
use crate::ProgrammerRegistry;
use light_core::{SessionId, programming::ProgrammingComponent};
use light_dynamics::{
    ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicPhaseSpreadMode, DynamicReference, DynamicRunMode,
    DynamicSpeed, DynamicTargetBinding, DynamicValueTiming, PhaseDistribution, PhaseOrdering,
    Rational,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn reference() -> DynamicReference {
    let definition = DynamicDefinition {
        id: Uuid::from_u128(0xd1),
        pool_number: 1,
        revision: 1,
        name: "batch".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    DynamicReference {
        dynamic_id: Some(definition.id),
        last_known_pool_number: 1,
        embedded_fallback: DynamicDefinitionSnapshot {
            definition: Arc::new(definition),
        },
    }
}

fn dynamic_on(link: Uuid, lane: Uuid, reference: &DynamicReference) -> DynamicSemanticValue {
    DynamicSemanticValue::DynamicOn {
        instance_link: link,
        dynamic: reference.clone(),
        lane_id: lane,
        overrides: DynamicInstanceOverrides {
            size: 1.0,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: DynamicValueTiming::default(),
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) % bound
    }
}

fn random_mutation(
    rng: &mut Rng,
    fixtures: &[FixtureId],
    reference: &DynamicReference,
) -> DynamicProgrammerValueMutation {
    let fixture_id = fixtures[rng.below(fixtures.len() as u64) as usize];
    let attribute = AttributeKey(["color", "position"][rng.below(2) as usize].into());
    let link = Uuid::from_u128(10 + u128::from(rng.below(2)));
    let lane = Uuid::from_u128(1 + u128::from(rng.below(2)));
    let value = match rng.below(8) {
        0 | 1 => dynamic_on(link, lane, reference),
        2 => DynamicSemanticValue::DynamicOff {
            instance_link: link,
            timing: DynamicValueTiming::default(),
        },
        3 => DynamicSemanticValue::Release,
        4 => DynamicSemanticValue::ProgrammingRelease {
            component: [None, Some(ProgrammingComponent::Pan)][rng.below(2) as usize],
        },
        5 => DynamicSemanticValue::FixAt {
            value: [0.25, 0.5][rng.below(2) as usize],
            timing: DynamicValueTiming::default(),
        },
        _ => {
            return DynamicProgrammerValueMutation::Release {
                fixture_id,
                attribute,
                instance_link: [None, Some(link)][rng.below(2) as usize],
            };
        }
    };
    DynamicProgrammerValueMutation::Set {
        fixture_id,
        attribute,
        value,
    }
}

/// The row-by-row path, with a counter in place of the registry's order and clock.
fn row_by_row(
    values: &mut Vec<DynamicAddressValue>,
    mutation: &DynamicProgrammerValueMutation,
    order: &mut u64,
) {
    match mutation {
        DynamicProgrammerValueMutation::Set {
            fixture_id,
            attribute,
            value,
        } => {
            values.retain(|stored| {
                !value.replaces_address(
                    *fixture_id,
                    attribute,
                    stored.value.track_key(),
                    stored.fixture_id,
                    &stored.attribute,
                )
            });
            *order += 1;
            values.push(row(*fixture_id, attribute, value, *order));
        }
        DynamicProgrammerValueMutation::Release {
            fixture_id,
            attribute,
            instance_link,
        } => values.retain(|stored| !same_track(stored, *fixture_id, attribute, *instance_link)),
    }
}

fn row(
    fixture_id: FixtureId,
    attribute: &AttributeKey,
    value: &DynamicSemanticValue,
    order: u64,
) -> DynamicAddressValue {
    DynamicAddressValue {
        fixture_id,
        attribute: attribute.clone(),
        value: value.clone(),
        programmer_order: order,
        changed_at_millis: order,
    }
}

#[test]
fn indexed_gesture_leaves_the_rows_and_change_answer_of_the_row_by_row_path() {
    let reference = reference();
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    for case in 0..400 {
        let fixtures = (0..1 + rng.below(4))
            .map(|_| FixtureId::new())
            .collect::<Vec<_>>();
        let mut order = 0;
        let mut stored = Vec::new();
        for _ in 0..rng.below(40) {
            let mutation = random_mutation(&mut rng, &fixtures, &reference);
            row_by_row(&mut stored, &mutation, &mut order);
        }
        let gesture = (0..1 + rng.below(30))
            .map(|_| random_mutation(&mut rng, &fixtures, &reference))
            .collect::<Vec<_>>();
        let index = DynamicValueIndex::new(&stored);
        for mutation in &gesture {
            assert_eq!(
                index.changes(mutation),
                mutation_changes(&stored, mutation),
                "case {case}: {mutation:?}"
            );
        }
        let mut expected = stored.clone();
        let mut expected_order = order;
        // TL-641: the whole-gesture helper the registry prepares outside its write lock.
        let mut stamped = order;
        let applied = apply_indexed(&stored, &gesture, |fixture_id, attribute, value| {
            stamped += 1;
            row(fixture_id, attribute, value, stamped)
        });
        let changes = gesture
            .iter()
            .any(|mutation| mutation_changes(&stored, mutation));
        let mut index = DynamicValueIndex::new(&stored);
        for mutation in &gesture {
            row_by_row(&mut expected, mutation, &mut expected_order);
            index.remove(mutation);
            if let DynamicProgrammerValueMutation::Set {
                fixture_id,
                attribute,
                value,
            } = mutation
            {
                order += 1;
                index.push(row(*fixture_id, attribute, value, order));
            }
        }
        let values = index.into_values();
        assert_eq!(
            applied,
            changes.then(|| values.clone()),
            "case {case}: whole-gesture helper"
        );
        assert_eq!(values, expected, "case {case}");
    }
}

/// Starting a Dynamic on 4,000 targets compared every mutation with every stored row under
/// the Programmer lock (seconds). The gesture must stay near-linear in the target count.
#[test]
fn large_gestures_apply_in_time_proportional_to_their_targets() {
    let reference = reference();
    let link = Uuid::from_u128(7);
    for targets in [1_000_usize, 4_000] {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session);
        let fixtures = (0..targets).map(|_| FixtureId::new()).collect::<Vec<_>>();
        let gesture = |value: &dyn Fn(usize) -> DynamicSemanticValue| {
            fixtures
                .iter()
                .flat_map(|fixture| {
                    (0..2).map(move |lane| DynamicProgrammerValueMutation::Set {
                        fixture_id: *fixture,
                        attribute: AttributeKey("position".into()),
                        value: value(lane),
                    })
                })
                .collect::<Vec<_>>()
        };
        let start =
            gesture(&|lane| dynamic_on(link, Uuid::from_u128(lane as u128 + 1), &reference));
        let restart =
            gesture(&|lane| dynamic_on(link, Uuid::from_u128(lane as u128 + 1), &reference));
        let started = Instant::now();
        assert!(registry.apply_dynamic_values(session, &start, None));
        // Reapplying the same lanes changes nothing.
        assert!(!registry.apply_dynamic_values(session, &restart, None));
        let off = [DynamicProgrammerValueMutation::Set {
            fixture_id: fixtures[0],
            attribute: AttributeKey("position".into()),
            value: DynamicSemanticValue::DynamicOff {
                instance_link: link,
                timing: DynamicValueTiming::default(),
            },
        }];
        assert!(registry.apply_dynamic_values(session, &off, None));
        let elapsed = started.elapsed();
        let rows = registry.state.read().as_ref().unwrap().dynamic_values.len();
        assert_eq!(rows, 1, "the Off replaces every lane of its instance");
        let bound = Duration::from_micros(25) * targets as u32;
        assert!(
            elapsed < bound,
            "{targets} targets took {elapsed:?}, bound {bound:?}"
        );
    }
}
