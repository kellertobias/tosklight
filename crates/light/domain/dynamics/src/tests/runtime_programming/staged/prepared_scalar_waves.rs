//! Cold literal kernels equal the generic arithmetic through edits and live controller Size.
use super::*;
use crate::{CompletedChunk, PreparationSources, PreparationWorkers, PreparedChunk};
use light_core::programming::{ColorComponent, PositionIntent, TransitionError};
use light_core::{
    NativeColorBinding, NativeColorIdentity,
    programming::{
        IntentError, NativeColorComponentDescriptor, NativeColorEditModel, NativeColorRecipe,
        PortableColorEstimate,
    },
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Captured {
    admitted: AtomicUsize,
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
        Some(DynamicValue::Scalar(0.35))
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
                .map(|chunk| {
                    scope.spawn(move || {
                        DynamicRuntime::take_scalar_wave_samples();
                        let output = run(chunk, sources);
                        sources.admitted.fetch_add(
                            DynamicRuntime::take_scalar_wave_samples(),
                            Ordering::Relaxed,
                        );
                        output
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect()
        }))
    }
    fn append_logs(&mut self, _: usize, _: (usize, usize), _: (usize, usize)) {}
}

#[derive(Clone)]
struct Model {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: 255,
            continuous: true,
        })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        panic!("component waveform does not predict a whole native recipe")
    }
}
impl DynamicNativeModelResolver for Model {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        assert_eq!(source, &self.source);
        Ok(Arc::new(self.clone()))
    }
}
fn model() -> Model {
    Model {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(201),
            profile_revision: 1,
            profile_digest: "original".into(),
            mode_id: Uuid::from_u128(202),
            head_id: Uuid::from_u128(203),
            path_id: Uuid::from_u128(204),
            model_revision: 1,
            native_layout_signature: "original-layout".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::from_u128(205),
            function_id: Uuid::from_u128(206),
        },
    }
}
fn literal_lane() -> DynamicLane {
    let mut lane = ramp(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Red)),
        },
        value(DynamicValue::Scalar(0.1)),
        value(DynamicValue::Scalar(0.9)),
    );
    lane.id = Uuid::from_u128(10);
    lane
}
fn setup(definition: &DynamicDefinition) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::with_native_color_models(1, Arc::new(model()));
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
    source: &Captured,
    parallel: bool,
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
            source,
            None,
            &mut scratch,
            |_, deferred| {
                let completed = if parallel {
                    deferred.complete_in_parallel(source, &mut Workers(source))?
                } else {
                    deferred.complete(source)?
                };
                let samples = completed.samples().to_vec();
                if fail {
                    return Err(DynamicRuntimeError::InvalidSample(
                        "scalar kernel rollback".into(),
                    ));
                }
                Ok((completed, samples))
            },
        )
    })
}
#[test]
fn sealed_scalar_waves_match_generic_sources_and_samples_through_hot_edits_and_live_size() {
    let mut definition = definition(literal_lane());
    let mut reference = setup(&definition);
    let mut optimized = setup(&definition);
    let source = Captured::default();
    for step in 0..7 {
        if step > 0 {
            definition.revision += 1;
            let mut next = literal_lane();
            if step == 1 || step == 2 {
                let DynamicLaneBody::Programming(body) = &mut next.body else {
                    unreachable!()
                };
                let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
                    unreachable!()
                };
                config.minimum = if step == 1 {
                    DynamicValueSource::Current
                } else {
                    DynamicValueSource::Preset {
                        preset_id: "literal-edit".into(),
                        address: body.address.clone(),
                        last_valid_by_target: vec![],
                        retained: None,
                    }
                };
            }
            if step == 3 {
                let native = model();
                next = ramp(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::DirectColor {
                            source: native.source,
                        },
                        component: Some(ProgrammingComponent::NativeColor(native.binding)),
                    },
                    value(DynamicValue::Native(10)),
                    value(DynamicValue::Native(220)),
                );
                next.id = Uuid::from_u128(10);
            }
            definition.lanes = vec![next];
            for runtime in [&mut reference, &mut optimized] {
                runtime.install_definitions([definition.clone()]).unwrap();
            }
        }
        for runtime in [&mut reference, &mut optimized] {
            runtime
                .update_controller(
                    Uuid::from_u128(101),
                    Some(if step == 5 { 0.4 } else { 1.0 }),
                    None,
                    None,
                )
                .unwrap();
        }
        let at = 211 + step * 37;
        source.take_calls();
        let expected = DynamicRuntime::with_generic_scalar_waves(|| {
            frame(&mut reference, at, &source, false, false)
        })
        .unwrap();
        let calls = source.take_calls();
        source.admitted.store(0, Ordering::Relaxed);
        let actual = frame(&mut optimized, at, &source, true, false).unwrap();
        assert_eq!(
            actual, expected,
            "step {step}: complete value, occurrence, rank and activation"
        );
        assert_eq!(
            source.take_calls(),
            calls,
            "step {step}: source reads remain generic for Current/Preset/live Size"
        );
        assert_eq!(optimized.snapshot(), reference.snapshot());
        assert_eq!(
            source.admitted.load(Ordering::Relaxed),
            if [0, 4, 5, 6].contains(&step) { 4 } else { 0 },
            "step {step}: sealed literal admission and native/Current/Preset fallback"
        );
    }
    let before = optimized.snapshot();
    assert!(frame(&mut optimized, 600, &source, true, true).is_err());
    assert_eq!(optimized.snapshot(), before);
    let expected = DynamicRuntime::with_generic_scalar_waves(|| {
        frame(&mut reference, 600, &source, false, false)
    })
    .unwrap();
    assert_eq!(
        frame(&mut optimized, 600, &source, true, false).unwrap(),
        expected
    );
    assert_eq!(optimized.snapshot(), reference.snapshot());
}
