use super::*;
use light_core::{
    AttributeKey, AttributeValue, NativeColorBinding, NativeColorValue, PhysicalDataQuality,
    ProgrammerEditStamp, ProgrammerId, programming::*,
};
use light_dynamics::{
    DynamicValue, DynamicValueTiming, FamilyCompositionContext, FamilyCompositionScratch,
    compose_dynamic_family,
};
use light_playback::{CueDynamicSourceKey, PlaybackIdentity, SequenceMasterSource};
use std::sync::atomic::{AtomicUsize, Ordering};

type Row = (Uuid, i16, DynamicAddressValue);
fn focus(value: f32) -> DynamicSemanticValue {
    DynamicSemanticValue::FixAt {
        value,
        timing: Default::default(),
    }
}
fn row(value: DynamicSemanticValue, owner: ProgrammingOwner) -> Row {
    (
        Uuid::from_u128(77),
        100,
        DynamicAddressValue {
            fixture_id: FixtureId(Uuid::from_u128(31)),
            attribute: owner.key(),
            value,
            changed_at_millis: 1_000,
            programmer_order: 3,
        },
    )
}
fn sidecar(row: &Row) -> light_engine::CapturedDynamicProgrammerRow {
    let programmer_id = ProgrammerId(row.0);
    light_engine::CapturedDynamicProgrammerRow {
        programmer_id,
        lane: light_engine::CapturedDynamicProgrammerLane::Live,
        source: light_engine::ContributionSourceId::programmer(programmer_id),
        changed_at_millis: row.2.changed_at_millis,
        programmer_order: row.2.programmer_order,
        stamp: Some(ProgrammerEditStamp {
            changed_at: DateTime::from_timestamp_millis(row.2.changed_at_millis as i64).unwrap(),
            programmer_order: row.2.programmer_order,
        }),
    }
}
fn cue(value: DynamicSemanticValue) -> light_playback::ActiveCueDynamicValue {
    let source = SequenceMasterSource {
        playback_number: Some(1_301),
        playback_identity: Some(PlaybackIdentity::virtual_playback(2, 1_301).unwrap()),
        cue_list_id: light_core::CueListId(Uuid::from_u128(42)),
        temporary: false,
    };
    light_playback::ActiveCueDynamicValue {
        source,
        source_key: CueDynamicSourceKey::Normal { source },
        output_enabled: true,
        sequence_master: 0.0,
        snap_sequence_master: 0.0,
        playback_number: source.playback_number,
        cue_list_id: source.cue_list_id,
        authored_cue_id: Uuid::from_u128(43),
        current_cue_id: Uuid::from_u128(44),
        priority: 100,
        changed_at: DateTime::from_timestamp(1, 42).unwrap(),
        changed_at_millis: 1_000,
        transition_ordinal: 3,
        fixture_id: FixtureId(Uuid::from_u128(31)),
        attribute: ProgrammingOwner::Focus.key(),
        value,
    }
}
fn inputs<'a>(
    programmer: &'a [Row],
    cues: &'a [light_playback::ActiveCueDynamicValue],
) -> CapturedFixedMaskRows<'a> {
    CapturedFixedMaskRows {
        now: DateTime::from_timestamp_millis(2_000).unwrap(),
        programmer_values: programmer,
        programmer_rows: None,
        extra_programmer_values: &[],
        cue_values: cues,
    }
}
fn ready(row: &PreparedFixedMask) -> &FamilySample {
    let CompiledFixedMaskState::Ready(sample) = &row.state else {
        panic!("expected compiled fixed mask")
    };
    sample
}

#[test]
fn unknown_attribution_still_compiles_and_applies_programmer_extra_and_cue_masks() {
    let programmer = [row(focus(0.2), ProgrammingOwner::Focus)];
    let extra = [row(focus(0.3), ProgrammingOwner::Focus)];
    let cues = [cue(focus(0.4))];
    let mut input = inputs(&programmer, &cues);
    input.extra_programmer_values = &extra;
    let mut scratch = FixedMaskCompilationScratch::default();
    let masks = compile_captured_fixed_masks(&input, None, None, &mut scratch).unwrap();
    assert_eq!(masks.len(), 3);
    for (mask, source) in masks.iter().zip([
        FamilyFixedSampleSource::Programmer,
        FamilyFixedSampleSource::ExtraProgrammer,
        FamilyFixedSampleSource::Cue,
    ]) {
        assert_eq!(mask.occurrence, None);
        assert_eq!(
            mask.rank.identity,
            FamilySampleIdentity::Fixed {
                source,
                row_index: 0
            }
        );
        assert!(ready(mask).is_fix_at());
        assert!(mask.participates());
    }
    assert_eq!(masks[2].rank.changed_at_submillis_nanos, 42);
    assert!(masks[0].rank < masks[1].rank && masks[1].rank < masks[2].rank);
    let samples = masks
        .iter()
        .map(|row| ready(row).clone())
        .collect::<Vec<_>>();
    let result = compose_dynamic_family(
        ProgrammingOwner::Focus,
        &AttributeValue::Normalized(0.9),
        &samples,
        &FamilyCompositionContext::default(),
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, AttributeValue::Normalized(0.4));
}

#[test]
fn fixed_rank_keeps_cue_submillisecond_recency_before_transition_order() {
    let mut a = cue(focus(0.2));
    a.transition_ordinal = 100;
    let mut b = cue(focus(0.8));
    b.changed_at += chrono::Duration::nanoseconds(1);
    b.transition_ordinal = 1;
    b.source.playback_number = Some(1_302);
    b.source.playback_identity = Some(PlaybackIdentity::virtual_playback(2, 1_302).unwrap());
    b.playback_number = b.source.playback_number;
    b.source_key = CueDynamicSourceKey::Normal { source: b.source };
    let cues = [a, b];
    let mut scratch = FixedMaskCompilationScratch::default();
    let rows = compile_captured_fixed_masks(&inputs(&[], &cues), None, None, &mut scratch).unwrap();
    assert!(rows[1].rank > rows[0].rank);
    assert_eq!(rows[1].stamp.exact_changed_at(), Some(cues[1].changed_at));
}

#[test]
fn captured_occurrences_are_optional_but_supplied_mismatches_cannot_relabel_values() {
    let rows = [row(focus(0.4), ProgrammingOwner::Focus)];
    let sidecars = [sidecar(&rows[0])];
    let mut origins = DynamicSourceOrigins::default();
    let captured = origins
        .reconcile_captured_programming_fixed_sources(&rows, Some(&sidecars), &[])
        .unwrap();
    let mut input = inputs(&rows, &[]);
    input.programmer_rows = Some(&sidecars);
    let mut scratch = FixedMaskCompilationScratch::default();
    let prepared =
        compile_captured_fixed_masks(&input, Some(&origins), None, &mut scratch).unwrap();
    assert_eq!(prepared[0].occurrence, Some(captured[0].occurrence_id));
    assert_eq!(prepared[0].original, rows[0].2.value);
    let mut changed = rows.clone();
    changed[0].2.value = focus(0.8);
    input.programmer_values = &changed;
    assert!(compile_captured_fixed_masks(&input, Some(&origins), None, &mut scratch).is_err());
    assert!(scratch.rows.is_empty() && scratch.cache.is_empty());
    // Missing catalogue evidence has no effect on the valid new value's participation.
    assert_eq!(
        compile_captured_fixed_masks(&input, None, None, &mut scratch).unwrap()[0].occurrence,
        None
    );
}

#[test]
fn authored_delay_fade_and_captured_disable_are_independent_of_sequence_master() {
    let mut stored = cue(DynamicSemanticValue::Static {
        value: AttributeValue::Normalized(0.7),
        timing: DynamicValueTiming {
            delay_millis: Some(100),
            fade_millis: Some(400),
        },
    });
    let mut scratch = FixedMaskCompilationScratch::default();
    for (at, mix) in [(1_050, 0.0), (1_300, 0.5), (1_500, 1.0)] {
        let cues = [stored.clone()];
        let mut input = inputs(&[], &cues);
        input.now = DateTime::from_timestamp_millis(at).unwrap();
        let prepared = compile_captured_fixed_masks(&input, None, None, &mut scratch).unwrap();
        assert_eq!(prepared[0].authored_activation_mix, mix);
        assert_eq!(
            ready(&prepared[0]).activation_mix,
            mix,
            "non-intensity master zero does not disable a Cue mask"
        );
    }
    stored.output_enabled = false;
    let cues = [stored];
    let prepared =
        compile_captured_fixed_masks(&inputs(&[], &cues), None, None, &mut scratch).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].authored_activation_mix, 1.0);
    assert_eq!(ready(&prepared[0]).activation_mix, 0.0);
    assert!(!prepared[0].participates());
}

fn native_source() -> NativeColorIdentity {
    NativeColorIdentity {
        profile_id: Uuid::from_u128(1),
        profile_revision: 2,
        profile_digest: "original".into(),
        mode_id: Uuid::from_u128(2),
        head_id: Uuid::from_u128(3),
        path_id: Uuid::from_u128(4),
        model_revision: 1,
        native_layout_signature: "emitter-wheel".into(),
    }
}
fn native_binding(channel: u128) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: Uuid::from_u128(channel),
        function_id: Uuid::from_u128(channel + 10),
    }
}
fn portable() -> PortableColorEstimate {
    PortableColorEstimate {
        model_revision: 1,
        visible: None,
        uv: Some(PortableUv {
            amount: 0.8,
            quality: PhysicalDataQuality::Estimated,
        }),
        quality: PhysicalDataQuality::Unknown,
        limitations: vec!["unknown appearance".into()],
    }
}
fn native_mask(component: Option<ProgrammingComponent>) -> DynamicSemanticValue {
    let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: native_source(),
            channels: vec![
                NativeColorValue {
                    channel_id: native_binding(5).channel_id,
                    function_id: native_binding(5).function_id,
                    raw: u32::MAX - 1,
                },
                NativeColorValue {
                    channel_id: native_binding(6).channel_id,
                    function_id: native_binding(6).function_id,
                    raw: 17,
                },
            ],
            spreads: vec![],
        },
        portable: portable(),
    }));
    DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, component, family)
            .unwrap(),
        timing: Default::default(),
    }
}
struct Model {
    source: NativeColorIdentity,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        [5, 6]
            .into_iter()
            .find(|channel| native_binding(*channel) == binding)
            .map(|channel| NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: if channel == 5 { u32::MAX } else { 255 },
                continuous: channel == 5,
            })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.fetch_add(1, Ordering::Relaxed);
        if recipe.source != self.source || recipe.channels.len() != 2 {
            return Err(IntentError("invalid original recipe".into()));
        }
        for channel in &recipe.channels {
            let descriptor = self
                .descriptor(NativeColorBinding {
                    channel_id: channel.channel_id,
                    function_id: channel.function_id,
                })
                .ok_or_else(|| IntentError("invalid native function".into()))?;
            if channel.raw > descriptor.raw_to {
                return Err(IntentError("native raw outside original range".into()));
            }
        }
        Ok(portable())
    }
}
struct Models {
    model: Arc<Model>,
    unavailable: bool,
    resolutions: AtomicUsize,
}
impl DynamicNativeModelResolver for Models {
    fn resolve(&self, source: &NativeColorIdentity) -> Result<ModelArc, IntentError> {
        self.resolve_capability(source)?.require_available()
    }
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        self.resolutions.fetch_add(1, Ordering::Relaxed);
        Ok(if self.unavailable {
            NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
                source: source.clone(),
                reason: NativeColorUnavailableReason::MissingRevision,
                detail: "original revision unavailable".into(),
            })
        } else {
            NativeColorModelCapability::Available(self.model.clone())
        })
    }
}
type ModelArc = Arc<dyn NativeColorEditModel + Send + Sync>;
fn models() -> Models {
    Models {
        model: Arc::new(Model {
            source: native_source(),
            predictions: AtomicUsize::new(0),
        }),
        unavailable: false,
        resolutions: AtomicUsize::new(0),
    }
}

#[test]
fn missing_native_is_a_retained_scoped_requirement_and_can_recover_without_new_authorship() {
    let rows = [row(native_mask(None), ProgrammingOwner::Color)];
    let mut models = models();
    models.unavailable = true;
    let mut scratch = FixedMaskCompilationScratch::default();
    let prepared =
        compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch)
            .unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].target, rows[0].2.fixture_id);
    assert_eq!(prepared[0].owner, ProgrammingOwner::Color);
    assert_eq!(prepared[0].original, rows[0].2.value);
    assert!(prepared[0].participates());
    assert!(matches!(
        prepared[0].state,
        CompiledFixedMaskState::Requires(FixedMaskRequirement::NativeColorModelUnavailable(_))
    ));
    models.unavailable = false;
    let prepared =
        compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch)
            .unwrap();
    assert!(ready(&prepared[0]).is_fix_at());
}

#[test]
fn cached_complete_native_compilation_reuses_prediction_but_refreshes_time_and_evidence() {
    let rows = [row(native_mask(None), ProgrammingOwner::Color)];
    let models = models();
    let mut scratch = FixedMaskCompilationScratch::default();
    compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch).unwrap();
    let predictions = models.model.predictions.load(Ordering::Relaxed);
    assert!(predictions > 0);
    let mut input = inputs(&rows, &[]);
    input.now = DateTime::from_timestamp_millis(5_000).unwrap();
    let sidecars = [sidecar(&rows[0])];
    let mut origins = DynamicSourceOrigins::default();
    let captured = origins
        .reconcile_captured_programming_fixed_sources(&rows, Some(&sidecars), &[])
        .unwrap();
    input.programmer_rows = Some(&sidecars);
    let prepared =
        compile_captured_fixed_masks(&input, Some(&origins), Some(&models), &mut scratch).unwrap();
    assert_eq!(prepared[0].occurrence, Some(captured[0].occurrence_id));
    assert_eq!(
        models.model.predictions.load(Ordering::Relaxed),
        predictions
    );
    assert_eq!(scratch.cache.len(), 1);
    compile_captured_fixed_masks(&inputs(&[], &[]), None, Some(&models), &mut scratch).unwrap();
    assert!(scratch.cache.is_empty());
}

#[test]
fn narrow_discrete_native_compiles_a_cached_step_and_keeps_the_eligible_recipe() {
    let rows = [
        row(
            native_mask(Some(ProgrammingComponent::NativeColor(native_binding(6)))),
            ProgrammingOwner::Color,
        ),
        row(native_mask(None), ProgrammingOwner::Color),
    ];
    let models = models();
    let mut scratch = FixedMaskCompilationScratch::default();
    let prepared =
        compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch)
            .unwrap();
    assert_eq!(
        prepared[0].mask.address.component,
        Some(ProgrammingComponent::NativeColor(native_binding(6)))
    );
    let mut narrow = ready(&prepared[0]).clone();
    assert!(ready(&prepared[1]).address().address().component.is_none());
    assert_eq!(
        models.resolutions.load(Ordering::Relaxed),
        1,
        "same original source is resolved once per capture"
    );
    let predictions = models.model.predictions.load(Ordering::Relaxed);
    compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch).unwrap();
    assert_eq!(
        models.model.predictions.load(Ordering::Relaxed),
        predictions,
        "unchanged discrete mask reuses its complete-recipe validation"
    );

    let DynamicSemanticValue::ProgrammingFixAt { mut mask, .. } = native_mask(None) else {
        panic!()
    };
    let AttributeValue::ColorProgram(program) = &mut mask.family else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = Arc::make_mut(program) else {
        panic!()
    };
    recipe.channels[0].raw = 123;
    recipe.channels[1].raw = 3;
    let base = mask.family;
    let mut composition = FamilyCompositionScratch::default();
    narrow.activation_mix = 0.5;
    let held = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &[narrow.clone()],
        &FamilyCompositionContext::default(),
        &mut composition,
    )
    .unwrap();
    assert_eq!(held, base);
    assert_eq!(
        models.model.predictions.load(Ordering::Relaxed),
        predictions,
        "held discrete mask does not re-predict its unchanged underlay"
    );
    narrow.activation_mix = 1.0;
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &[narrow],
        &FamilyCompositionContext::default(),
        &mut composition,
    )
    .unwrap();
    let AttributeValue::ColorProgram(program) = result else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
        panic!()
    };
    assert_eq!(
        recipe.channels[0].raw, 123,
        "unrelated emitter stays in the eligible underlay"
    );
    assert_eq!(recipe.channels[1].raw, 17);
    assert_eq!(portable.uv.as_ref().unwrap().amount, 0.8);
    assert_eq!(
        models.model.predictions.load(Ordering::Relaxed),
        predictions + 1
    );
}

#[test]
fn invalid_native_or_wrong_original_model_fails_without_exposing_partial_results() {
    let rows = [
        row(focus(0.2), ProgrammingOwner::Focus),
        row(native_mask(None), ProgrammingOwner::Color),
    ];
    let mut models = models();
    Arc::get_mut(&mut models.model)
        .unwrap()
        .source
        .profile_revision += 1;
    let mut scratch = FixedMaskCompilationScratch::default();
    assert!(
        compile_captured_fixed_masks(&inputs(&rows, &[]), None, Some(&models), &mut scratch)
            .is_err()
    );
    assert!(scratch.rows.is_empty());
    let mut invalid = rows[0].clone();
    invalid.2.attribute = AttributeKey::intensity();
    invalid.2.value = DynamicSemanticValue::ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Focus,
            None,
            AttributeValue::Normalized(0.7),
        )
        .unwrap(),
        timing: Default::default(),
    };
    assert!(
        compile_captured_fixed_masks(&inputs(&[invalid], &[]), None, None, &mut scratch).is_err()
    );
}

#[test]
fn target_mask_compiles_original_reference_without_requesting_current_or_geometry() {
    let family = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(99),
        },
        [1.0, 2.0, 3.0],
    )));
    let rows = [row(
        DynamicSemanticValue::ProgrammingFixAt {
            mask: ProgrammingFamilyFixAt::from_family(
                ProgrammingOwner::Position,
                Some(ProgrammingComponent::TargetY),
                family.clone(),
            )
            .unwrap(),
            timing: Default::default(),
        },
        ProgrammingOwner::Position,
    )];
    let mut scratch = FixedMaskCompilationScratch::default();
    let prepared =
        compile_captured_fixed_masks(&inputs(&rows, &[]), None, None, &mut scratch).unwrap();
    assert_eq!(prepared[0].mask.family, family);
    assert_eq!(
        ready(&prepared[0]).materialized_value(),
        Some(&DynamicValue::Scalar(2.0))
    );
}

#[test]
fn source_validation_and_duplicate_rejection_do_not_depend_on_catalogue_or_sidecars() {
    let valid = row(focus(0.3), ProgrammingOwner::Focus);
    let mut scratch = FixedMaskCompilationScratch::default();
    let repeated = [valid.clone(), row(focus(0.7), ProgrammingOwner::Focus)];
    // An unknown lane is not proof of duplicate identity. Both captured rows still apply.
    let prepared =
        compile_captured_fixed_masks(&inputs(&repeated, &[]), None, None, &mut scratch).unwrap();
    assert_eq!(prepared.len(), 2);
    assert!(prepared.iter().all(|row| row.occurrence.is_none()));
    assert_ne!(prepared[0].rank.identity, prepared[1].rank.identity);
    assert!(prepared[0].participates() && prepared[1].participates());
    assert!(prepared[0].rank < prepared[1].rank);
    let value = compose_dynamic_family(
        ProgrammingOwner::Focus,
        &AttributeValue::Normalized(0.9),
        &prepared
            .iter()
            .map(|row| ready(row).clone())
            .collect::<Vec<_>>(),
        &FamilyCompositionContext::default(),
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(value, AttributeValue::Normalized(0.7));
    let same_lane = [sidecar(&valid), sidecar(&valid)];
    let mut known = inputs(&repeated, &[]);
    known.programmer_rows = Some(&same_lane);
    assert!(compile_captured_fixed_masks(&known, None, None, &mut scratch).is_err());
    assert!(scratch.rows.is_empty());
    let mut distinct_lanes = same_lane.clone();
    distinct_lanes[1].lane = light_engine::CapturedDynamicProgrammerLane::Preload;
    distinct_lanes[1].source =
        light_engine::ContributionSourceId::preload(distinct_lanes[1].programmer_id);
    known.programmer_rows = Some(&distinct_lanes);
    assert_eq!(
        compile_captured_fixed_masks(&known, None, None, &mut scratch)
            .unwrap()
            .len(),
        2
    );
    let cues = [cue(focus(0.2)), cue(focus(0.3))];
    assert!(compile_captured_fixed_masks(&inputs(&[], &cues), None, None, &mut scratch).is_err());
    let mut invalid = valid.clone();
    invalid.0 = Uuid::nil();
    assert!(
        compile_captured_fixed_masks(&inputs(&[invalid], &[]), None, None, &mut scratch).is_err()
    );
    let mut invalid_cue = cue(focus(0.4));
    invalid_cue.authored_cue_id = Uuid::nil();
    assert!(
        compile_captured_fixed_masks(&inputs(&[], &[invalid_cue]), None, None, &mut scratch)
            .is_err()
    );
    let mut invalid_cue = cue(focus(0.4));
    invalid_cue.source.cue_list_id.0 = Uuid::nil();
    invalid_cue.cue_list_id = invalid_cue.source.cue_list_id;
    invalid_cue.source_key = CueDynamicSourceKey::Normal {
        source: invalid_cue.source,
    };
    assert!(
        compile_captured_fixed_masks(&inputs(&[], &[invalid_cue]), None, None, &mut scratch)
            .is_err()
    );
    // Invalid capture did not poison a subsequent valid compile.
    assert_eq!(
        compile_captured_fixed_masks(&inputs(&[valid], &[]), None, None, &mut scratch)
            .unwrap()
            .len(),
        1
    );
}
