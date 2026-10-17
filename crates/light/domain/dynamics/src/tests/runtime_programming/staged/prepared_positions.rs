//! The prepared time prefix is shared by lane, but phase, Current and source proof stay fresh.
use super::*;
use crate::{CompletedChunk, PreparationSources, PreparationWorkers, PreparedChunk};
use light_core::programming::{ColorComponent, PositionIntent, TransitionError};
use std::sync::Mutex;

#[derive(Default)]
struct Captured {
    calls: Mutex<Vec<(u128, &'static str)>>,
}
impl Captured {
    fn take_calls(&self) -> Vec<(u128, &'static str)> {
        let mut calls = std::mem::take(&mut *self.calls.lock().unwrap());
        calls.sort();
        calls
    }
}
impl DynamicValueSourceResolver for Captured {
    fn current(&self, target: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.calls
            .lock()
            .unwrap()
            .push((target.0.as_u128(), "current"));
        Some(DynamicValue::Scalar(0.3))
    }
    fn try_position_current_family(
        &self,
        target: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.calls
            .lock()
            .unwrap()
            .push((target.0.as_u128(), "position"));
        Ok(Some(AttributeValue::Position(Arc::new(
            PositionIntent::angles(10., 20.),
        ))))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.calls
            .lock()
            .unwrap()
            .push((target.0.as_u128(), "preset"));
        None
    }
    fn authored_occurrence(
        &self,
        _: Uuid,
        _: Uuid,
        target: FixtureId,
        lane: Uuid,
    ) -> Option<DynamicSourceOccurrenceId> {
        Some(
            DynamicSourceOccurrenceId::new(Uuid::from_u128(
                1000 + target.0.as_u128() + lane.as_u128(),
            ))
            .unwrap(),
        )
    }
}
impl PreparationSources for Captured {
    fn source_free_completion(&self) -> bool {
        true
    }
    fn finish_controller(&self) -> Option<(usize, usize)> {
        Some((0, 0))
    }
}
struct Workers<'a>(&'a Captured);
impl PreparationWorkers for Workers<'_> {
    fn chunks(&self) -> usize {
        4
    }
    fn run(
        &mut self,
        _: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        unreachable!("completion only")
    }
    fn run_completion(
        &mut self,
        run: &(dyn Fn(usize, &dyn PreparationSources) -> CompletedChunk + Sync),
    ) -> Result<Vec<CompletedChunk>, TransitionError> {
        let sources = self.0;
        Ok(std::thread::scope(|scope| {
            (0..4)
                .map(|chunk| scope.spawn(move || run(chunk, sources)))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect()
        }))
    }
    fn append_logs(&mut self, _: usize, _: (usize, usize), _: (usize, usize)) {}
}
fn lanes() -> Vec<DynamicLane> {
    let addresses = [
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Red)),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        pan_address(),
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Tilt),
        },
    ];
    addresses
        .into_iter()
        .enumerate()
        .map(|(index, address)| {
            let mut lane = ramp(
                address,
                if index == 0 {
                    value(DynamicValue::Scalar(0.1))
                } else {
                    DynamicValueSource::Current
                },
                value(DynamicValue::Scalar(0.9)),
            );
            lane.id = Uuid::from_u128(10 + index as u128);
            lane.speed_multiplier = Rational {
                numerator: 3 + index as u32,
                denominator: 2,
            };
            lane
        })
        .collect()
}
fn setup(definition: &DynamicDefinition) -> (DynamicRuntime, Uuid) {
    let mut runtime = DynamicRuntime::default();
    runtime.derive_instance_ids_from(Uuid::from_u128(99));
    runtime.install_definitions([definition.clone()]).unwrap();
    let mut request = start_request(
        definition.id,
        controller(101, 7, false),
        FixtureId(Uuid::from_u128(1)),
        0,
        false,
    );
    request.target_scope.ordered_targets = [1, 17, 33, 49]
        .map(|id| FixtureId(Uuid::from_u128(id)))
        .to_vec();
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 400;
    let instance = runtime.start(request).unwrap();
    (runtime, instance)
}
fn frame(
    runtime: &mut DynamicRuntime,
    at: u64,
    bpm: f64,
    sources: &Captured,
    parallel: bool,
    fail: bool,
) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    let mut speed = transports(at);
    for group in &mut speed {
        group.effective_bpm = bpm;
    }
    runtime.with_output_frame_transaction(&mut transaction, |runtime| {
        runtime.sample_all_programming_staged(
            at,
            10,
            &speed,
            &Sources { current: 0.2 },
            sources,
            None,
            &mut scratch,
            |_, deferred| {
                let completed = if parallel {
                    deferred.complete_in_parallel(sources, &mut Workers(sources))?
                } else {
                    deferred.complete(sources)?
                };
                let samples = completed.samples().to_vec();
                if fail {
                    return Err(DynamicRuntimeError::InvalidSample(
                        "prepared position rollback".into(),
                    ));
                }
                Ok((completed, samples))
            },
        )
    })
}
#[test]
fn prepared_positions_are_per_lane_per_frame_and_match_serial_sources_across_clock_edits() {
    let mut definition = definition(focus_lane());
    definition.lanes = lanes();
    let (mut serial, instance) = setup(&definition);
    let (mut parallel, _) = setup(&definition);
    let sources = Captured::default();
    for step in 0..8 {
        let at = 211 + step * 37;
        if step == 1 || step == 2 {
            definition.revision += 1;
            for lane in &mut definition.lanes {
                lane.width = if step == 1 { 0.3 } else { 1.0 };
                lane.speed_multiplier = Rational {
                    numerator: 7,
                    denominator: 3,
                };
            }
            definition.speed = DynamicSpeed::SpeedGroup {
                group: SpeedGroup::A,
                beats_per_cycle: Rational {
                    numerator: 3,
                    denominator: 2,
                },
            };
            for runtime in [&mut serial, &mut parallel] {
                runtime.install_definitions([definition.clone()]).unwrap();
            }
        }
        for runtime in [&mut serial, &mut parallel] {
            runtime
                .update_controller(
                    Uuid::from_u128(101),
                    Some(if step == 3 { 0.4 } else { 1.0 }),
                    None,
                    Some(-31.0 * step as f32),
                )
                .unwrap();
            if step == 4 || step == 5 {
                runtime
                    .set_controller_paused_with_resume(
                        instance,
                        Uuid::from_u128(101),
                        step == 4,
                        at,
                        Some(ActivationPolicy::JoinSyncNow),
                    )
                    .unwrap();
            }
        }
        let bpm = 60.0 + step as f64 * 7.0;
        sources.take_calls();
        let expected = frame(&mut serial, at, bpm, &sources, false, false).unwrap();
        let expected_calls = sources.take_calls();
        DynamicRuntime::take_position_prefixes();
        let actual = frame(&mut parallel, at, bpm, &sources, true, false).unwrap();
        assert_eq!(
            DynamicRuntime::take_position_prefixes(),
            definition.lanes.len(),
            "one fresh prefix per compiled lane, not per target or evaluation attempt"
        );
        assert_eq!(
            actual, expected,
            "frame {step}: complete expressions, values, provenance and order"
        );
        assert_eq!(
            sources.take_calls(),
            expected_calls,
            "frame {step}: Current and adoption source calls"
        );
        assert_eq!(parallel.snapshot(), serial.snapshot());
    }
    let before = parallel.snapshot();
    assert!(frame(&mut parallel, 600, 117., &sources, true, true).is_err());
    assert_eq!(parallel.snapshot(), before);
    let expected = frame(&mut serial, 600, 117., &sources, false, false).unwrap();
    assert_eq!(
        frame(&mut parallel, 600, 117., &sources, true, false).unwrap(),
        expected
    );
    assert_eq!(parallel.snapshot(), serial.snapshot());
}
