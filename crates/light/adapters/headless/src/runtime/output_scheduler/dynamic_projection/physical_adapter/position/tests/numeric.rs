//! Actual Live numeric Angle Current retains arithmetic until each physical destination.
//! Reference endpoint solves use the same captured inputs and accepted mechanical anchors;
//! configured/controller Size, phase and Random envelopes are still logical shared state.
use super::programs::{
    commanded_angles, destination_dynamic_rig, position_definition, program,
    start_position_dynamic_sized, verify_live_native,
};
use super::*;
use crate::runtime::dynamic_source_origins::DynamicRuntimeSourceCheckpoint;
use light_dynamics::{
    DynamicDefinition, DynamicFamilyRepresentation, DynamicLaneBody, DynamicRandomGroup,
    DynamicRandomRange, DynamicRuntimeSample, DynamicValue, DynamicValueSource,
    KeyframeConfiguration, MaxMinConfiguration, MiddleAmplitudeConfiguration, PeriodicFunction,
    ProgrammingLaneConfiguration, PwmShape,
};

const LITERAL: f64 = 80.;
const AMPLITUDE: f64 = 25.;
const WAVE_SIZE: f64 = 1.5;
const CONTROLLER_SIZE: f64 = 1.25;

fn current_axes(
    rig: &Rig,
    base: &AttributeValue,
    previous: Option<&PositionContinuity>,
) -> Vec<(FixtureId, [f64; 2])> {
    let reference = rig.resolve_with(&[(rig.root, base.clone())], previous, &[]);
    rig.verify(&reference);
    reference.results[0]
        .achieved
        .outcomes
        .iter()
        .map(|outcome| {
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(outcome.result.angular_error_degrees.unwrap() < 0.04);
            (outcome.destination, outcome.result.achieved.unwrap())
        })
        .collect()
}

fn live_frame(
    rig: &Rig,
    lane: &PhysicalAdapterLane<PositionAdapter>,
    runtime: &mut DynamicRuntime,
    origins: &mut DynamicSourceOrigins,
    scratch: &mut HybridFrameScratch,
) -> (PreparedOutputFrame, PublishedPhysicalFrame<PositionAdapter>) {
    let capture = rig.capture();
    let output = prepare_live(rig, &capture, &capture, lane, runtime, origins, scratch).unwrap();
    assert!(
        output.requirements.is_empty(),
        "numeric Live requirements: {:#?}; sampled: {:#?}; completed destinations: {:#?}",
        output
            .requirements
            .iter()
            .map(|requirement| (
                requirement.target,
                requirement.owner,
                requirement_debug(&requirement.reason)
            ))
            .collect::<Vec<_>>(),
        output.sampled.samples,
        output
            .results
            .iter()
            .map(|row| &row.achieved.destinations)
            .collect::<Vec<_>>()
    );
    assert_eq!(output.results.len(), 1);
    verify_live_native(rig, &capture, lane, &output);
    (capture, output)
}

pub(super) fn requirement_debug(
    reason: &crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyRequirementReason,
) -> String {
    use crate::runtime::output_scheduler::dynamic_projection::{
        family_inputs::CapturedFamilyRequirement,
        programming_projection::hybrid::HybridFamilyRequirementReason as Reason,
    };
    match reason {
        Reason::Input(CapturedFamilyRequirement::Dynamic(input)) => {
            format!("Input Dynamic: {:?}; rank: {:?}", input.reason, input.rank)
        }
        Reason::Input(CapturedFamilyRequirement::Fixed { .. }) => "Input Fixed".into(),
        Reason::Current {
            address,
            requirement,
        } => {
            format!("Current {address:?}: {requirement:?}")
        }
        Reason::Composition(requirement) => format!("Composition: {requirement:?}"),
        Reason::LegacyOwnerOverlap => "LegacyOwnerOverlap".into(),
        Reason::ScalarBaselineChanged => "ScalarBaselineChanged".into(),
    }
}

fn pan_sample<'a>(
    output: &'a PublishedPhysicalFrame<PositionAdapter>,
    definition: &DynamicDefinition,
) -> &'a DynamicRuntimeSample {
    output
        .sampled
        .samples
        .iter()
        .find(|sample| sample.lane_id == definition.lanes[0].id)
        .unwrap()
}

fn assert_pairs(
    output: &PublishedPhysicalFrame<PositionAdapter>,
    base: &AttributeValue,
    expected: &[(FixtureId, [f64; 2])],
    label: &str,
) {
    let row = &output.results[0];
    assert_eq!(program(row).base, *base);
    assert_eq!(
        program(row).samples.len(),
        1,
        "complete numeric Angle pair competes once"
    );
    assert_eq!(row.achieved.destinations.len(), 2);
    for destination in &row.achieved.destinations {
        let actual = commanded_angles(&destination.value);
        let expected = expected
            .iter()
            .find(|(id, _)| *id == destination.destination)
            .unwrap()
            .1;
        for axis in 0..2 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 0.06,
                "{label} {:?} axis{axis}: {} != {}",
                destination.destination,
                actual[axis],
                expected[axis]
            );
        }
        let outcome = row
            .achieved
            .outcomes
            .iter()
            .find(|outcome| outcome.destination == destination.destination)
            .unwrap();
        assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
        assert!(
            outcome
                .result
                .achieved
                .unwrap()
                .iter()
                .zip(actual)
                .all(|(a, b)| (a - b).abs() < 0.023)
        );
    }
}

#[test]
fn actual_live_numeric_current_keyframes_preserve_direction_and_independent_config_controller_size()
{
    for current_first in [true, false] {
        for (config_size, controller_size) in [(1., 1.), (2., 1.), (1., 2.), (2., 2.)] {
            let (rig, _) = destination_dynamic_rig();
            let base = target(TargetReference::Origin, [4., 8., 1.]);
            let address = DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(ProgrammingComponent::Pan),
            };
            let mut definition = position_definition(
                address,
                [
                    DynamicValue::Scalar(LITERAL as f32),
                    DynamicValue::Scalar(LITERAL as f32),
                ],
            );
            let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
                unreachable!()
            };
            let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
                unreachable!()
            };
            config.size = config_size;
            config.points[usize::from(!current_first)].source = DynamicValueSource::Current;
            let mut runtime =
                start_position_dynamic_sized(&rig, &base, &definition, None, controller_size);
            let lane = PhysicalAdapterLane::live(PositionAdapter::default());
            let mut origins = DynamicSourceOrigins::default();
            let mut scratch = HybridFrameScratch::default();
            let (_, first) = live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
            let initial_source = pan_sample(&first, &definition);
            let source_identity = (
                initial_source.instance_id,
                initial_source.controller_id,
                initial_source.activated_at_millis,
            );
            let accepted = lane
                .continuity(rig.root, ProgrammingOwner::Position)
                .unwrap();
            let current = current_axes(&rig, &base, Some(&accepted));
            // DynamicOn records its clock before the first +25ms capture. The reference
            // capture adds another 25ms; +50ms plus the final capture totals 125ms since
            // activation, or one-quarter of the first 0→0.5 keyframe segment.
            rig.clock.advance_millis(50);
            let (capture, output) =
                live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
            assert_eq!(
                capture.sampled_at().timestamp_millis() as u64
                    - runtime.snapshot().instances[0].started_at_millis,
                125,
                "reference captures must not alter the intended oscillator test phase"
            );
            let sample = pan_sample(&output, &definition);
            assert_eq!(
                (
                    sample.instance_id,
                    sample.controller_id,
                    sample.activated_at_millis
                ),
                source_identity
            );
            let expected = current
                .iter()
                .map(|&(destination, axes)| {
                    let (left, right) = if current_first {
                        (axes[0], LITERAL)
                    } else {
                        (LITERAL, axes[0])
                    };
                    let interpolated = left + 0.25 * (right - left);
                    // Configuration Size uses the original first keyframe as pivot. Controller Size
                    // independently scales the configured result around fresh destination Current.
                    let configured = left + f64::from(config_size) * (interpolated - left);
                    let pan = axes[0] + f64::from(controller_size) * (configured - axes[0]);
                    (destination, [pan, axes[1]])
                })
                .collect::<Vec<_>>();
            assert_pairs(
                &output,
                &base,
                &expected,
                &format!(
                    "Current-first={current_first} config={config_size} controller={controller_size}"
                ),
            );
            assert!(
                (current[0].1[0] - current[1].1[0]).abs() > 1.,
                "copies must provide differing physical Current"
            );
            assert_eq!(rig.engine.snapshot().dynamics[0], definition);
            assert_eq!(
                runtime.snapshot().instances.len(),
                1,
                "copy evaluation does not create logical oscillator instances"
            );
        }
    }
}

/// TL-652: an Angle Dynamic over a Target keeps that Target as its live base. Changing the base
/// Target while the Dynamic runs moves the Current the lane works about on the next frame; the
/// Target is never baked into a one-time Angle pose.
#[test]
fn actual_live_angle_current_follows_a_changed_base_target_while_the_dynamic_runs() {
    let (rig, _) = destination_dynamic_rig();
    let base = target(TargetReference::Origin, [4., 8., 1.]);
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let mut definition = position_definition(
        address,
        [
            DynamicValue::Scalar(LITERAL as f32),
            DynamicValue::Scalar(LITERAL as f32),
        ],
    );
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
        unreachable!()
    };
    config.points[0].source = DynamicValueSource::Current;
    let mut runtime = start_position_dynamic_sized(&rig, &base, &definition, None, 1.);
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    // Pan runs from Current (0) to the literal (0.5) over a 1000 ms cycle; Tilt follows Current.
    let expected = |current: &[(FixtureId, [f64; 2])], fraction: f64| {
        current
            .iter()
            .map(|&(destination, axes)| {
                (
                    destination,
                    [axes[0] + fraction * (LITERAL - axes[0]), axes[1]],
                )
            })
            .collect::<Vec<_>>()
    };
    live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    let accepted = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    let before = current_axes(&rig, &base, Some(&accepted));
    rig.clock.advance_millis(50);
    let (capture, output) = live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    let started = runtime.snapshot().instances[0].started_at_millis;
    assert_eq!(
        capture.sampled_at().timestamp_millis() as u64 - started,
        125
    );
    assert_pairs(&output, &base, &expected(&before, 0.25), "original Target");

    // The operator moves the base Target (an X offset edit) while the Dynamic keeps running.
    let moved = target(TargetReference::Origin, [1., 8., 1.]);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        moved.clone(),
    );
    let accepted = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    let after = current_axes(&rig, &moved, Some(&accepted));
    rig.clock.advance_millis(75);
    let (capture, output) = live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
    assert_eq!(
        capture.sampled_at().timestamp_millis() as u64 - started,
        250
    );
    assert_pairs(&output, &moved, &expected(&after, 0.5), "moved Target");
    for ((_, old), (_, new)) in before.iter().zip(&after) {
        assert!(
            (old[0] - new[0]).abs() > 1. || (old[1] - new[1]).abs() > 1.,
            "the moved Target must solve to a different Current: {old:?} vs {new:?}"
        );
    }
    assert_eq!(
        runtime.snapshot().instances.len(),
        1,
        "the base change keeps the running instance and its phase"
    );
}

#[derive(Clone, Copy, Debug)]
enum Wave {
    MiddleAmplitude,
    MaxMin,
    Random,
}

fn wave_definition(wave: Wave) -> DynamicDefinition {
    let mut definition = position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(0.), DynamicValue::Scalar(0.)],
    );
    // The existing paused_lane_hot_edit_retains_original_owner regression uses JoinSyncNow:
    // synchronized Pause retains authored arithmetic, while StartNow only freezes phase and
    // still reads pool edits. Use the shared two-beat transport for the same 1000ms cycle.
    definition.default_activation = light_dynamics::ActivationPolicy::JoinSyncNow;
    definition.speed = light_dynamics::DynamicSpeed::SpeedGroup {
        group: light_dynamics::SpeedGroup::A,
        beats_per_cycle: light_dynamics::Rational {
            numerator: 2,
            denominator: 1,
        },
    };
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        unreachable!()
    };
    let literal = || DynamicValueSource::Value {
        value: DynamicValue::Scalar(LITERAL as f32),
    };
    body.configuration = match wave {
        Wave::MiddleAmplitude => {
            ProgrammingLaneConfiguration::MiddleAmplitude(MiddleAmplitudeConfiguration {
                middle: DynamicValueSource::Current,
                amplitude: DynamicValue::Scalar(AMPLITUDE as f32),
                function: PeriodicFunction::LinearUp,
                size: WAVE_SIZE as f32,
                pwm: PwmShape::default(),
                invert_waveform: false,
            })
        }
        Wave::MaxMin => ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Current,
            maximum: literal(),
            function: PeriodicFunction::LinearUp,
            size: WAVE_SIZE as f32,
            pwm: PwmShape::default(),
        }),
        Wave::Random => ProgrammingLaneConfiguration::Random,
    };
    if matches!(wave, Wave::Random) {
        let group = Uuid::new_v4();
        definition.lanes[0].random_group_id = Some(group);
        definition.random_groups = vec![DynamicRandomGroup {
            id: group,
            seed: 591,
            range: DynamicRandomRange::Programming {
                low: DynamicValueSource::Current,
                high: literal(),
            },
            decision_interval_millis: 50,
            start_probability: 1.,
            mean_duration_millis: 1000,
            duration_spread_millis: 0,
            attack_ratio: 0.8,
            decay_ratio: 0.,
        }];
    }
    definition
}

fn waveform_factor(
    output: &PublishedPhysicalFrame<PositionAdapter>,
    current: &[(FixtureId, [f64; 2])],
    wave: Wave,
) -> f64 {
    let mut factors = Vec::new();
    for destination in &output.results[0].achieved.destinations {
        let actual = commanded_angles(&destination.value);
        let baseline = current
            .iter()
            .find(|(id, _)| *id == destination.destination)
            .unwrap()
            .1;
        assert!(
            (actual[1] - baseline[1]).abs() < 0.06,
            "numeric waveform cannot author the Tilt Current partner"
        );
        let factor = match wave {
            Wave::MiddleAmplitude => {
                (actual[0] - baseline[0]) / (AMPLITUDE * WAVE_SIZE * CONTROLLER_SIZE)
            }
            Wave::MaxMin | Wave::Random => {
                (actual[0] - baseline[0]) / ((LITERAL - baseline[0]) * CONTROLLER_SIZE)
            }
        };
        factors.push(factor);
    }
    assert!(
        (factors[0] - factors[1]).abs() < 0.004,
        "{wave:?} samples one shared phase/envelope, not another draw per copy: {factors:?}"
    );
    factors[0]
}

#[test]
fn actual_live_numeric_waveforms_share_envelope_and_paused_checkpoint_survives_hot_edit() {
    for wave in [Wave::MiddleAmplitude, Wave::MaxMin, Wave::Random] {
        let (rig, _) = destination_dynamic_rig();
        let base = target(TargetReference::Origin, [4., 8., 1.]);
        let unanchored = current_axes(&rig, &base, None);
        // The reference capture advanced 25 ms. Start on the next 1000 ms transport cycle
        // so the shared 120 BPM/two-beat clock and controller-relative reference agree.
        rig.clock.advance_millis(975);
        let definition = wave_definition(wave);
        let mut runtime =
            start_position_dynamic_sized(&rig, &base, &definition, None, CONTROLLER_SIZE as f32);
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let mut origins = DynamicSourceOrigins::default();
        let mut scratch = HybridFrameScratch::default();
        live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
        let accepted = lane
            .continuity(rig.root, ProgrammingOwner::Position)
            .unwrap();
        let current = current_axes(&rig, &base, Some(&accepted));
        rig.clock.advance_millis(50);
        let (capture, output) = live_frame(&rig, &lane, &mut runtime, &mut origins, &mut scratch);
        assert_eq!(
            capture.sampled_at().timestamp_millis() as u64
                - runtime.snapshot().instances[0].started_at_millis,
            125,
            "reference captures must not alter the intended oscillator test phase"
        );
        let factor = waveform_factor(&output, &current, wave);
        match wave {
            Wave::MiddleAmplitude => assert!(
                (factor + 0.75).abs() < 0.004,
                "125ms middle/amplitude arithmetic retains size > 1: {factor}; {:?}",
                pan_sample(&output, &definition).expression
            ),
            Wave::MaxMin => assert!(
                (factor + 0.0625).abs() < 0.004,
                "MaxMin Size amplifies outside its endpoints: {factor}; {:?}",
                pan_sample(&output, &definition).expression
            ),
            Wave::Random => {
                assert!(
                    factor > 0. && factor < 1.,
                    "test observes a nontrivial Random attack envelope: {factor}"
                );
                assert_eq!(
                    runtime.snapshot().instances[0].random_streams.len(),
                    1,
                    "one Random stream belongs to the logical target, not each physical copy"
                );
            }
        }
        let held = pan_sample(&output, &definition).expression.clone();
        let instance = pan_sample(&output, &definition).instance_id;
        let controller = pan_sample(&output, &definition).controller_id;
        let started = runtime.snapshot().instances[0].started_at_millis;
        // The actual captured producer reconciles global Pause from the Engine each tick.
        // Set the authoritative transport as well as the runtime snapshot being persisted.
        rig.engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        runtime.set_global_paused(true, capture.sampled_at().timestamp_millis() as u64);
        let checkpoint =
            DynamicRuntimeSourceCheckpoint::capture(runtime.snapshot(), &origins).unwrap();
        let saved = serde_json::to_value(checkpoint).unwrap();
        let decoded: DynamicRuntimeSourceCheckpoint = serde_json::from_value(saved).unwrap();
        let (snapshot, mut restored_origins) = decoded.restore().unwrap();
        let mut restored =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        restored.restore_snapshot(snapshot).unwrap();

        // Editing the active pool's numeric configuration cannot replace already held numeric
        // source arithmetic; it is adopted when transport resumes, not during paused evaluation.
        let mut edited = definition.clone();
        edited.revision += 1;
        let DynamicLaneBody::Programming(body) = &mut edited.lanes[0].body else {
            unreachable!()
        };
        body.configuration = ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0., 0.5]
                .map(|position| light_dynamics::DynamicKeyframe {
                    position,
                    source: DynamicValueSource::Value {
                        value: DynamicValue::Scalar(999.),
                    },
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .to_vec(),
            size: 1.,
        });
        let engine_snapshot = rig.engine.snapshot();
        rig.engine
            .replace_snapshot(EngineSnapshot {
                dynamics: vec![edited.clone()].into(),
                revision: engine_snapshot.revision + 1,
                ..engine_snapshot.as_ref().clone()
            })
            .unwrap();
        runtime.install_definitions([edited.clone()]).unwrap();
        restored.install_definitions([edited.clone()]).unwrap();
        rig.clock.advance_millis(500);
        // Both comparison lanes intentionally start without accepted physical anchors. This
        // isolates checkpointed arithmetic from legitimate differences in mechanical continuity.
        let paused_lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let (_, paused) = live_frame(
            &rig,
            &paused_lane,
            &mut runtime,
            &mut origins,
            &mut HybridFrameScratch::default(),
        );
        let restored_lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let (_, replayed) = live_frame(
            &rig,
            &restored_lane,
            &mut restored,
            &mut restored_origins,
            &mut HybridFrameScratch::default(),
        );
        for output in [&paused, &replayed] {
            let sample = pan_sample(output, &definition);
            assert_eq!(
                (sample.instance_id, sample.controller_id),
                (instance, controller)
            );
            assert_eq!(
                sample.expression, held,
                "{wave:?}: paused source arithmetic survives pool edit and JSON checkpoint"
            );
            assert!((waveform_factor(output, &unanchored, wave) - factor).abs() < 0.004);
            assert_eq!(program(&output.results[0]).base, base);
        }
        assert_eq!(runtime.snapshot().instances[0].started_at_millis, started);
        assert_eq!(restored.snapshot().instances[0].started_at_millis, started);
        assert_eq!(
            paused.results[0]
                .writes
                .iter()
                .map(|write| (write.slot, write.raw))
                .collect::<Vec<_>>(),
            replayed.results[0]
                .writes
                .iter()
                .map(|write| (write.slot, write.raw))
                .collect::<Vec<_>>(),
            "same paused descriptor and frame inputs produce the same destination-native output after reload"
        );
        assert_eq!(rig.engine.snapshot().dynamics[0], edited);
    }
}
