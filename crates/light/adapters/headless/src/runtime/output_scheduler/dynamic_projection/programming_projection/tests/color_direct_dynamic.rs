//! TL-600: a running whole Direct Color Dynamic (Keyframes, and a typed Random lane) of a
//! source foreign to this fixture, taken over by a whole FixAT fade into another foreign Direct
//! recipe, held at full (the Dynamic's output is suppressed by the higher whole FixAT) and
//! released. A paired reference rig runs the same Dynamic uninterrupted at the same clock.
//!
//! - Every frame has one Color owner (asserted by the shared rig).
//! - The fade's from endpoint is the Dynamic's CURRENT recipe this frame, forward-evaluated by
//!   its original model (not a fitted channel set and not a stale recorded estimate).
//! - The Dynamic keeps its instance identity, clock/phase and Random stream through the
//!   takeover; after Release it shows exactly what the uninterrupted reference shows.
//! - The stored Dynamic definition and the FixAT request remain the saved recipes.
use super::super::super::physical_adapter::color::profiles::{patched, rgbal, rgbw};
use super::super::super::physical_adapter::color::tests::direct::{direct, identity};
use super::super::super::physical_adapter::color::{DirectEstimateOrigin, DirectReplayOutcome};
use super::super::super::physical_adapter::*;
use super::color_direct_transition::{Rig, is_semantic, semantic_value};
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Lane {
    Keyframes,
    Random,
}

pub(super) fn direct_dynamic(
    lane: Lane,
    low: &AttributeValue,
    high: &AttributeValue,
) -> DynamicDefinition {
    let mut definition = pan_definition();
    definition.name = "Foreign Direct Color".into();
    definition.lanes.truncate(1);
    let AttributeValue::ColorProgram(program) = low else {
        unreachable!()
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        unreachable!()
    };
    let value = |value: &AttributeValue| DynamicValueSource::Value {
        value: DynamicValue::Family(value.clone()),
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: recipe.source.clone(),
            },
            component: None,
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [(0., low), (0.5, high)]
                .map(|(position, key)| DynamicKeyframe {
                    position,
                    source: value(key),
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .to_vec(),
            size: 1.,
        }),
    });
    if matches!(lane, Lane::Random) {
        // Typed Random needs a numeric component: one native Blue control of the same source
        // rides on the whole recipe with its own instance-owned Random stream.
        let blue = &recipe.channels[2];
        let group = Uuid::from_u128(600);
        let mut random = definition.lanes[0].clone();
        random.id = Uuid::from_u128(601);
        random.random_group_id = Some(group);
        random.body = DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: recipe.source.clone(),
                },
                component: Some(ProgrammingComponent::NativeColor(
                    light_core::NativeColorBinding {
                        channel_id: blue.channel_id,
                        function_id: blue.function_id,
                    },
                )),
            },
            configuration: ProgrammingLaneConfiguration::Random,
        });
        definition.lanes.push(random);
        let native = |raw| DynamicValueSource::Value {
            value: DynamicValue::Native(raw),
        };
        definition.random_groups = vec![DynamicRandomGroup {
            id: group,
            seed: 600,
            range: DynamicRandomRange::Programming {
                low: native(0),
                high: native(255),
            },
            decision_interval_millis: 100,
            start_probability: 0.6,
            mean_duration_millis: 400,
            duration_spread_millis: 200,
            attack_ratio: 0.5,
            decay_ratio: 0.5,
        }];
    }
    definition
}

impl Rig {
    fn start_dynamic(&mut self, definition: &DynamicDefinition, instance_link: Uuid) {
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(&self.profile, self.target, 1)].into(),
                native_color_sources: self.catalogue.clone(),
                dynamics: vec![definition.clone()].into(),
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        self.runtime
            .install_definitions([definition.clone()])
            .unwrap();
        let mutations = definition
            .lanes
            .iter()
            .map(
                |lane| light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: self.target,
                    attribute: ProgrammingOwner::Color.key(),
                    value: dynamic_on(definition, lane.id, instance_link),
                },
            )
            .collect::<Vec<_>>();
        assert!(
            self.programmers
                .apply_dynamic_values(self.session, &mutations, None)
        );
    }

    /// Clock, phase and Random stream state of every running instance.
    pub(super) fn clocks(&self) -> Vec<String> {
        self.runtime
            .snapshot()
            .instances
            .iter()
            .map(|instance| {
                format!(
                    "{} {} {} {:?} {:?}",
                    instance.started_at_millis,
                    instance.paused_elapsed_millis,
                    instance.completed,
                    instance.phase_by_lane_target,
                    instance.random_streams,
                )
            })
            .collect()
    }
}

/// One lane of `definition` switched on under `instance_link` (Programmer or Cue row).
pub(super) fn dynamic_on(
    definition: &DynamicDefinition,
    lane_id: Uuid,
    instance_link: Uuid,
) -> DynamicSemanticValue {
    DynamicSemanticValue::DynamicOn {
        instance_link,
        lane_id,
        dynamic: DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        },
        overrides: DynamicInstanceOverrides {
            size: 1.,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.,
        },
        timing: Default::default(),
    }
}

/// The portable appearance of a Direct value, forward-evaluated by its ORIGINAL model now.
pub(super) fn forward(rig: &Rig, value: &AttributeValue) -> AttributeValue {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("Color program")
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        panic!("Direct Dynamic output")
    };
    let estimate = rig
        .catalogue
        .resolve(&recipe.source)
        .unwrap()
        .predict(recipe)
        .unwrap();
    semantic_value(&semantic_color_adoption(&estimate, None).unwrap().intent)
}

pub(super) fn portable(value: &AttributeValue) -> AttributeValue {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("Color program")
    };
    let ColorProgram::Direct { portable, .. } = program.as_ref() else {
        panic!("Direct")
    };
    semantic_value(&semantic_color_adoption(portable, None).unwrap().intent)
}

pub(super) fn assert_forward_fallback(result: &PhysicalHeadResult<ColorAdapter>, label: &str) {
    let status = result.quality.direct.as_ref().expect(label);
    assert_eq!(status.origin, DirectEstimateOrigin::Forward, "{label}");
    assert!(
        matches!(status.replay, DirectReplayOutcome::Fallback { .. }),
        "{label}: foreign recipe falls back after forward evaluation"
    );
}

fn running_direct_dynamic_survives_foreign_takeover_and_release(lane_kind: Lane) {
    let source = rgbal();
    let destination = rgbw();
    let base = semantic_value(&ColorIntent::default());
    let mut rig = Rig::new(|_, _| base.clone(), &[&source, &destination]);
    let low = direct(&rig.catalogue, &source, &[65535, 0, 0, 0, 0]);
    let high = direct(&rig.catalogue, &source, &[0, 255, 0, 0, 0]);
    let definition = direct_dynamic(lane_kind, &low, &high);
    rig.start_dynamic(&definition, Uuid::from_u128(6001));
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    // Running: the foreign recipe is forward-evaluated by its original before fallback.
    for _ in 0..3 {
        let running = rig.frame(130, &lane).expect("Dynamic owner");
        assert_forward_fallback(&running, "running");
    }
    let instances = rig.runtime.instance_ids();
    assert_eq!(instances.len(), 1);
    // The uninterrupted reference is replayed later from an exact fork of THIS runtime (Random
    // draws are keyed by the instance identity) over the same captured times.
    let started = light_core::ApplicationClock::now(rig.clock.as_ref());
    let fork = rig.runtime.fork_for_cold_install();
    let fork_origins = rig.origins.clone();

    // Whole FixAT takeover fade into a different foreign source, full hold, Release.
    let to = direct(&rig.catalogue, &destination, &[0, 0, 255, 0]);
    rig.fade_to(to.clone(), 1_000);
    let mut steps = Vec::new();
    for index in 0..3 {
        let fading = rig.typed(250, &lane);
        assert!(rig.requirements.is_empty(), "fade {index}: not passive");
        assert!(
            is_semantic(&fading.value),
            "fade {index}: portable appearance"
        );
        steps.push((250, Some(fading.value)));
    }
    // Completion and hold at full: the higher whole FixAT suppresses the Dynamic's output
    // while the instance keeps running underneath.
    for index in 0..3 {
        assert_eq!(
            rig.typed(250, &lane).value,
            to,
            "full FixAT {index}: saved recipe"
        );
        steps.push((250, None));
    }
    assert_eq!(
        rig.runtime.instance_ids(),
        instances,
        "same oscillator instance"
    );
    rig.release();
    for _ in 0..4 {
        let released = rig.typed(170, &lane);
        assert_forward_fallback(&released, "released");
        steps.push((170, Some(released.value)));
    }
    assert_eq!(rig.runtime.instance_ids(), instances);
    let clocks = rig.clocks();
    // Saved requests: the running definition still stores the authored recipes.
    let stored = rig
        .runtime
        .instance_definition(instances[0])
        .unwrap()
        .clone();
    assert_eq!(stored.lanes, definition.lanes);
    assert_eq!(stored.random_groups, definition.random_groups);

    // Uninterrupted reference over the same times (the FixAT is released, so absent).
    rig.clock.set(started);
    rig.runtime = fork;
    rig.origins = fork_origins;
    let reference_lane = PhysicalAdapterLane::live(ColorAdapter::default());
    for (index, (advance, observed)) in steps.into_iter().enumerate() {
        let reference = rig.typed(advance, &reference_lane);
        assert_eq!(rig.runtime.instance_ids(), instances, "reference {index}");
        assert_forward_fallback(&reference, "reference");
        assert_eq!(
            portable(&reference.value),
            forward(&rig, &reference.value),
            "reference {index}: the composed recipe carries its current forward estimate"
        );
        let Some(observed) = observed else { continue };
        if index < 3 {
            // From = the Dynamic's CURRENT recipe this frame, by its original model.
            let progress = 0.25 * (index + 1) as f32;
            let expected = interpolate_programming_value(
                &forward(&rig, &reference.value),
                &forward(&rig, &to),
                progress,
            )
            .unwrap();
            assert_eq!(observed, expected, "fade {index}");
        } else {
            assert_eq!(
                observed, reference.value,
                "released {index}: same clock/phase/Random"
            );
        }
    }
    assert_eq!(
        rig.clocks(),
        clocks,
        "clock/phase/Random advanced identically"
    );
    assert!(rig.catalogue.resolve(&identity(&source)).is_ok());
}

#[test]
fn running_direct_keyframes_keep_identity_and_phase_through_foreign_takeover_and_release() {
    running_direct_dynamic_survives_foreign_takeover_and_release(Lane::Keyframes);
}

#[test]
fn running_direct_random_keeps_its_stream_through_foreign_takeover_and_release() {
    running_direct_dynamic_survives_foreign_takeover_and_release(Lane::Random);
}
