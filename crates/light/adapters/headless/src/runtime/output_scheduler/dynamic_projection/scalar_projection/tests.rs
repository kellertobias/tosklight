use super::*;
use crate::runtime::dynamic_source_origins::DynamicSourceOrigins;
use family_inputs::{CapturedFamilyInputScratch, assemble_captured_family_inputs};
use fixed_masks::{
    CapturedFixedMaskRows, FixedMaskCompilationScratch, compile_captured_fixed_masks,
};
use light_core::{ProgrammerEditStamp, ProgrammerId, programming::*};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;

type Row = (Uuid, i16, DynamicAddressValue);
const BASE: AttributeValue = AttributeValue::Normalized(0.2);

struct Sources;
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.2)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicTickSource for Sources {
    fn value(&self, _: FixtureId, _: &AttributeKey) -> Option<&AttributeValue> {
        Some(&BASE)
    }
}

fn target() -> FixtureId {
    FixtureId(Uuid::from_u128(31))
}

fn row(attribute: &str, value: f32, fade: u64) -> Row {
    (
        Uuid::from_u128(77),
        100,
        DynamicAddressValue {
            fixture_id: target(),
            attribute: AttributeKey(attribute.into()),
            value: DynamicSemanticValue::FixAt {
                value,
                timing: DynamicValueTiming {
                    fade_millis: Some(fade),
                    delay_millis: None,
                },
            },
            changed_at_millis: 1_000,
            programmer_order: 7,
        },
    )
}

fn with_inputs<T>(
    programmer: Vec<Row>,
    extra: Vec<Row>,
    cues: Vec<light_playback::ActiveCueDynamicValue>,
    run: impl FnOnce(&CapturedDynamicInputs<'_>) -> T,
) -> T {
    let engine = Engine::new(ProgrammerRegistry::default());
    let capture = engine.prepare_output_frame(Default::default());
    let snapshot = capture.snapshot();
    let addresser = capture.frame_addresser();
    let sidecars = programmer
        .iter()
        .map(|row| {
            let programmer_id = ProgrammerId(row.0);
            light_engine::CapturedDynamicProgrammerRow {
                programmer_id,
                lane: light_engine::CapturedDynamicProgrammerLane::Live,
                source: light_engine::ContributionSourceId::programmer(programmer_id),
                stamp: Some(ProgrammerEditStamp {
                    changed_at: chrono::DateTime::from_timestamp_millis(
                        row.2.changed_at_millis as i64,
                    )
                    .unwrap(),
                    programmer_order: row.2.programmer_order,
                }),
                changed_at_millis: row.2.changed_at_millis,
                programmer_order: row.2.programmer_order,
            }
        })
        .collect::<Vec<_>>();
    let programmer = Arc::new(programmer);
    let speeds = [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: 1_500,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5];
    run(&CapturedDynamicInputs {
        now: chrono::DateTime::from_timestamp_millis(1_500).unwrap(),
        speed_transports: &speeds,
        rate: 40,
        snapshot: &snapshot,
        programmer_values: &programmer,
        programmer_rows: Some(&sidecars),
        cue_values: &cues,
        dynamic_playbacks: &[],
        playback_paused: false,
        addresser: &addresser,
        extra_programmer_values: &extra,
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    })
}

fn controls<'a>(
    playbacks: &'a HashMap<Uuid, DynamicPlaybackControl>,
    cues: &'a HashMap<Uuid, CueDynamicOutputControl>,
) -> CapturedDynamicOutputControls<'a> {
    CapturedDynamicOutputControls { playbacks, cues }
}

struct NoConversion;
impl WholeFamilyExpressionFrameResolver for NoConversion {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("Focus needs no frame conversion")
    }
}

#[test]
fn partial_focus_fixed_enters_family_assembly_once_and_keeps_exact_evidence() {
    with_inputs(vec![row("focus", 1., 1_000)], vec![], vec![], |inputs| {
        let mut origins = DynamicSourceOrigins::default();
        origins
            .reconcile_captured_programming_fixed_sources(
                inputs.programmer_values,
                inputs.programmer_rows,
                inputs.cue_values,
            )
            .unwrap();
        let mut compilation = FixedMaskCompilationScratch::default();
        let fixed = compile_captured_fixed_masks(
            &CapturedFixedMaskRows::from_inputs(inputs),
            Some(&origins),
            None,
            &mut compilation,
        )
        .unwrap();
        assert_eq!(fixed.len(), 1);
        assert_eq!(fixed[0].authored_activation_mix, 0.5);
        assert!(fixed[0].occurrence.is_some());
        let rank = fixed[0].rank;
        let occurrence = fixed[0].occurrence;
        let original = fixed[0].original.clone();
        let playbacks = HashMap::new();
        let cues = HashMap::new();
        let scalar = project_hybrid_scalar_samples(
            inputs,
            &[],
            controls(&playbacks, &cues),
            &Sources,
            fixed,
            &mut HybridScalarProjectionScratch::default(),
        );
        assert!(scalar.is_empty());
        let legacy =
            project_captured_dynamic_samples(inputs, &[], controls(&playbacks, &cues), &Sources);
        let legacy_value = &legacy[0].samples()[0].value().value;
        assert!(
            (legacy_value.normalized().unwrap() - 0.6).abs() < 1e-6,
            "contract 0 retains its existing scalar fade"
        );
        let mut assembly = CapturedFamilyInputScratch::default();
        let prepared = PreparedDynamicFamilySamples {
            families: &[],
            legacy: &[],
            requirements: &[],
        };
        let groups = assemble_captured_family_inputs(&prepared, fixed, &mut assembly);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].group.samples.len(), 1);
        let compose = |base| {
            compose_retained_dynamic_family(
                ProgrammingOwner::Focus,
                base,
                &groups[0].group.samples,
                &FamilyCompositionContext::default(),
                &NoConversion,
                &mut RetainedFamilyCompositionScratch::default(),
            )
            .unwrap()
            .normalized()
            .unwrap()
        };
        assert!((compose(&BASE) - 0.6).abs() < 1e-6);
        assert!(
            (compose(legacy_value) - 0.8).abs() < 1e-6,
            "feeding the already blended scalar result would incorrectly apply activation twice"
        );
        assert_eq!(fixed[0].rank, rank);
        assert_eq!(fixed[0].occurrence, occurrence);
        assert_eq!(fixed[0].original, original);
    });
}

fn cue(attribute: &str, value: f32, enabled: bool) -> light_playback::ActiveCueDynamicValue {
    let source = light_playback::SequenceMasterSource {
        playback_number: Some(1),
        playback_identity: Some(light_playback::PlaybackIdentity::physical(1).unwrap()),
        cue_list_id: light_core::CueListId(Uuid::from_u128(42)),
        temporary: false,
    };
    light_playback::ActiveCueDynamicValue {
        source,
        source_key: light_playback::CueDynamicSourceKey::Normal { source },
        output_enabled: enabled,
        sequence_master: 1.,
        snap_sequence_master: 1.,
        playback_number: source.playback_number,
        cue_list_id: source.cue_list_id,
        authored_cue_id: Uuid::from_u128(43),
        current_cue_id: Uuid::from_u128(44),
        priority: 150,
        changed_at: chrono::DateTime::from_timestamp(1, 875_321).unwrap(),
        changed_at_millis: 1_000,
        transition_ordinal: 19,
        fixture_id: target(),
        attribute: AttributeKey(attribute.into()),
        value: DynamicSemanticValue::FixAt {
            value,
            timing: Default::default(),
        },
    }
}

fn legacy(attribute: &str, value: f32, order: u128) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(100 + order),
        controller_id: Uuid::from_u128(200 + order),
        lane_id: Uuid::from_u128(300 + order),
        target: target(),
        priority: 50,
        activated_at_millis: 500,
        activation_mix: 1.,
        address: None,
        expression: DynamicSampleExpression::LegacyScalar {
            attribute: AttributeKey(attribute.into()),
            value,
            occurrence: None,
            dependency_occurrence: None,
        },
    }
}

#[test]
fn exclusions_use_exact_captured_row_indices_and_keep_legacy_point_focus_and_nonfamily_fixed() {
    let mut delayed = row("focus", 1., 1_000);
    let DynamicSemanticValue::FixAt { timing, .. } = &mut delayed.2.value else {
        unreachable!()
    };
    timing.delay_millis = Some(1_000);
    let programmer = vec![delayed, row("intensity", 0.3, 0)];
    let extra = vec![row("point.position.z", 0.4, 0), row("focus", 0.9, 0)];
    let cues = vec![cue("focus", 0.8, false), cue("shutter", 0.45, true)];
    let samples = [legacy("point.position.x", 0.7, 1), legacy("focus", 0.65, 2)];
    let original_samples = samples.clone();
    with_inputs(programmer, extra, cues, |inputs| {
        let mut compilation = FixedMaskCompilationScratch::default();
        let fixed = compile_captured_fixed_masks(
            &CapturedFixedMaskRows::from_inputs(inputs),
            None,
            None,
            &mut compilation,
        )
        .unwrap();
        assert_eq!(fixed.len(), 3);
        assert_eq!(fixed[0].authored_activation_mix, 0.);
        assert!(!fixed[2].output_enabled);
        let original = fixed
            .iter()
            .map(|row| (row.rank, row.stamp, row.original.clone()))
            .collect::<Vec<_>>();
        let playbacks = HashMap::new();
        let cues = HashMap::new();
        let mut scratch = HybridScalarProjectionScratch::default();
        let result = project_hybrid_scalar_samples(
            inputs,
            &samples,
            controls(&playbacks, &cues),
            &Sources,
            fixed,
            &mut scratch,
        );
        assert_eq!(
            scratch.excluded,
            FxHashSet::from_iter([
                (FamilyFixedSampleSource::Programmer, 0),
                (FamilyFixedSampleSource::ExtraProgrammer, 1),
                (FamilyFixedSampleSource::Cue, 0),
            ]),
            "delayed and disabled masks are excluded too, without collapsing the three row namespaces"
        );
        let values = result
            .iter()
            .flat_map(|batch| batch.samples())
            .map(|sample| (sample.value().attribute.0.as_ref(), sample.value()))
            .collect::<HashMap<_, _>>();
        assert_eq!(values.len(), 5);
        for (key, expected) in [
            ("point.position.x", 0.7),
            ("point.position.z", 0.4),
            ("focus", 0.65),
            ("intensity", 0.3),
            ("shutter", 0.45),
        ] {
            assert_eq!(
                values[key].value,
                AttributeValue::Normalized(expected),
                "{key}"
            );
        }
        assert_eq!(values["point.position.x"].priority, samples[0].priority);
        assert_eq!(
            values["point.position.x"].programmer_order,
            samples[0].controller_id.as_u128() as u64
        );
        assert_eq!(values["intensity"].merge_mode, MergeMode::Htp);
        assert_eq!(values["focus"].merge_mode, MergeMode::Ltp);
        assert_eq!(
            values["shutter"].changed_at, inputs.cue_values[1].changed_at,
            "submillisecond Cue rank must survive the inclusion filter"
        );
        assert_eq!(
            values["shutter"].programmer_order,
            inputs.cue_values[1].transition_ordinal
        );
        assert_eq!(
            fixed
                .iter()
                .map(|row| (row.rank, row.stamp, row.original.clone()))
                .collect::<Vec<_>>(),
            original
        );
        assert_eq!(
            samples, original_samples,
            "scalar projection only borrows the retained samples"
        );
        // The next capture may reuse the same array index for an ordinary scalar row.
        with_inputs(vec![row("intensity", 0.55, 0)], vec![], vec![], |next| {
            let mut compilation = FixedMaskCompilationScratch::default();
            let fixed = compile_captured_fixed_masks(
                &CapturedFixedMaskRows::from_inputs(next),
                None,
                None,
                &mut compilation,
            )
            .unwrap();
            assert!(fixed.is_empty());
            let result = project_hybrid_scalar_samples(
                next,
                &[],
                controls(&playbacks, &cues),
                &Sources,
                fixed,
                &mut scratch,
            );
            assert!(scratch.excluded.is_empty());
            assert_eq!(
                result[0].samples()[0].value().value,
                AttributeValue::Normalized(0.55)
            );
        });
    });
}

#[test]
fn unavailable_direct_fixed_remains_in_family_requirements_without_scalar_fallback() {
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: light_core::NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 2,
                profile_digest: "original".into(),
                mode_id: Uuid::from_u128(2),
                head_id: Uuid::from_u128(3),
                path_id: Uuid::from_u128(4),
                model_revision: 1,
                native_layout_signature: "rgbuv".into(),
            },
            channels: vec![light_core::NativeColorValue {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: light_core::PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }));
    let mut stored = row("color", 0., 0);
    stored.2.value = DynamicSemanticValue::Static {
        value: value.clone(),
        timing: Default::default(),
    };
    with_inputs(vec![stored], vec![], vec![], |inputs| {
        let mut compilation = FixedMaskCompilationScratch::default();
        let fixed = compile_captured_fixed_masks(
            &CapturedFixedMaskRows::from_inputs(inputs),
            None,
            None,
            &mut compilation,
        )
        .unwrap();
        assert!(matches!(
            fixed[0].state,
            fixed_masks::CompiledFixedMaskState::Requires(_)
        ));
        let playbacks = HashMap::new();
        let cues = HashMap::new();
        let mut scratch = HybridScalarProjectionScratch::default();
        assert!(
            project_hybrid_scalar_samples(
                inputs,
                &[],
                controls(&playbacks, &cues),
                &Sources,
                fixed,
                &mut scratch
            )
            .is_empty()
        );
        let legacy =
            project_captured_dynamic_samples(inputs, &[], controls(&playbacks, &cues), &Sources);
        assert_eq!(
            legacy[0].samples()[0].value().value,
            value,
            "contract 0 remains on its original route"
        );
        let prepared = PreparedDynamicFamilySamples {
            families: &[],
            legacy: &[],
            requirements: &[],
        };
        let mut assembly = CapturedFamilyInputScratch::default();
        let groups = assemble_captured_family_inputs(&prepared, fixed, &mut assembly);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].group.samples.is_empty());
        let [
            family_inputs::CapturedFamilyRequirement::Fixed {
                rank, mask, reason, ..
            },
        ] = &groups[0].requirements[..]
        else {
            panic!("unavailable original must remain an explicit typed requirement")
        };
        assert_eq!(*rank, fixed[0].rank);
        assert_eq!(mask, &fixed[0].mask);
        assert!(matches!(
            reason,
            fixed_masks::FixedMaskRequirement::NativeColorModelUnavailable(_)
        ));
    });
}
