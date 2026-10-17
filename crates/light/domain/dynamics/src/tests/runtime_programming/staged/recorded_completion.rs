//! Applying recorded values needs a compiled lane only when a transition must be installed.
use super::*;
use crate::{CompletedChunk, PreparationSources, PreparationWorkers, PreparedChunk};
use light_core::programming::TransitionError;

struct Captured;
impl DynamicValueSourceResolver for Captured {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(DynamicValue::Scalar(0.3))
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
impl PreparationSources for Captured {
    fn finish_controller(&self) -> Option<(usize, usize)> {
        Some((0, 0))
    }
}
struct Workers;
impl PreparationWorkers for Workers {
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
        Ok(std::thread::scope(|scope| {
            (0..4)
                .map(|chunk| scope.spawn(move || run(chunk, &Captured)))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect()
        }))
    }
    fn append_logs(&mut self, _: usize, _: (usize, usize), _: (usize, usize)) {}
}
fn setup(definition: &DynamicDefinition) -> DynamicRuntime {
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
    runtime.start(request).unwrap();
    runtime
}
fn frame(
    runtime: &mut DynamicRuntime,
    at: u64,
    recorded: bool,
    fail: bool,
) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    runtime.with_output_frame_transaction(&mut transaction, |runtime| {
        runtime.sample_all_programming_staged(
            at,
            10,
            &transports(at),
            &Sources { current: 0.2 },
            &Captured,
            None,
            &mut scratch,
            |_, deferred| {
                let completed = if recorded {
                    deferred.complete_in_parallel(&Captured, &mut Workers)?
                } else {
                    deferred.complete(&Captured)?
                };
                let samples = completed.samples().to_vec();
                if fail {
                    return Err(DynamicRuntimeError::InvalidSample(
                        "after recorded completion".into(),
                    ));
                }
                Ok((completed, samples))
            },
        )
    })
}
fn compare(lane: DynamicLane, expected_first_writes: usize) {
    let definition = definition(lane);
    let mut serial = setup(&definition);
    let mut recorded = setup(&definition);
    for (index, at) in [211, 212, 213].into_iter().enumerate() {
        let expected = frame(&mut serial, at, false, false).unwrap();
        DynamicRuntime::take_completion_lane_lookups();
        let actual = frame(&mut recorded, at, true, false).unwrap();
        let lookups = DynamicRuntime::take_completion_lane_lookups();
        assert_eq!(
            actual, expected,
            "full samples, rank, activation and provenance"
        );
        assert_eq!(
            recorded.snapshot(),
            serial.snapshot(),
            "last samples and runtime state"
        );
        assert_eq!(
            lookups,
            (0, if index == 0 { expected_first_writes } else { 0 }),
            "recorded results never look up a lane unless installing a newly compiled transition"
        );
    }
    let before = recorded.snapshot();
    assert!(frame(&mut recorded, 512, true, true).is_err());
    assert_eq!(
        recorded.snapshot(),
        before,
        "failed application restores the journal"
    );
    let expected = frame(&mut serial, 512, false, false).unwrap();
    assert_eq!(
        frame(&mut recorded, 512, true, false).unwrap(),
        expected,
        "retry after failed completion matches ordinary evaluation"
    );
    assert_eq!(recorded.snapshot(), serial.snapshot());
}
#[test]
fn recorded_maxmin_skips_compiled_lane_lookup_and_matches_serial() {
    compare(
        ramp(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            },
            value(DynamicValue::Scalar(0.1)),
            value(DynamicValue::Scalar(0.9)),
        ),
        0,
    );
}
#[test]
fn recorded_keyframe_miss_installs_once_then_hits_match_serial() {
    let mut lane = focus_lane();
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        unreachable!()
    };
    body.configuration = ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
        points: vec![
            DynamicKeyframe {
                position: 0.0,
                source: value(DynamicValue::Scalar(0.1)),
                interpolation: ScalarInterpolation::Linear,
            },
            DynamicKeyframe {
                position: 0.5,
                source: value(DynamicValue::Scalar(0.9)),
                interpolation: ScalarInterpolation::Linear,
            },
        ],
        size: 1.0,
    });
    compare(lane, 4);
}
