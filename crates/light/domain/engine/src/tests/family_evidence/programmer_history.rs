use super::*;
use crate::{ContributionProgrammerLane as Lane, ContributionSourceDescriptor as Source};
use light_core::programming::{
    ColorIntent, ColorProgram, PositionIntent, ScalarIntent, TargetReference, ZoomIntent,
};

struct History {
    engine: Engine,
    registry: ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    clock: Arc<ManualClock>,
    attribute: AttributeKey,
}

impl History {
    fn new(attribute: &str) -> Self {
        let (engine, registry, session, fixture, clock) = super::ordinary::focus_engine();
        Self {
            engine,
            registry,
            session,
            fixture,
            clock,
            attribute: AttributeKey(attribute.into()),
        }
    }

    fn edit(&self, value: AttributeValue, timing: Option<(u64, u64)>) -> ContributionFamilyEntry {
        if let Some((fade, delay)) = timing {
            self.registry.set_faded_with_timing(
                self.session,
                self.fixture,
                self.attribute.clone(),
                value,
                Some(fade),
                Some(delay),
            );
        } else {
            self.registry
                .set(self.session, self.fixture, self.attribute.clone(), value);
        }
        stored_entry(&self.engine, self.fixture, &self.attribute)
    }

    fn commit(&self, samples: &[ContributionBatch]) {
        let output = self
            .engine
            .render_with_contribution_batches(Default::default(), samples)
            .unwrap();
        assert!(
            output
                .resolved_values
                .contribution_family_evidence(self.fixture, &self.attribute)
                .is_none()
        );
    }

    fn observe(&self, advance: i64) -> ObservedSourceFrame {
        self.clock.advance_millis(advance);
        self.engine.observe_source_frame(&[])
    }

    fn evidence(&self, frame: &ObservedSourceFrame) -> Arc<ContributionFamilyEvidence> {
        Arc::clone(
            frame
                .values()
                .contribution_family_evidence(self.fixture, &self.attribute)
                .expect("retained family history"),
        )
    }

    fn assert_scalar(&self, frame: &ObservedSourceFrame, expected: f32) {
        let actual = frame
            .values()
            .value(self.fixture, &self.attribute)
            .unwrap()
            .normalized()
            .unwrap();
        assert!(
            (actual - expected).abs() < 0.000001,
            "expected {expected}, got {actual}"
        );
    }

    fn start_playback(&self, value: AttributeValue) {
        let list = test_cue_list(
            "Static fade underlay",
            vec![CueChange::set(self.fixture, self.attribute.clone(), value)],
        );
        let mut snapshot = (*self.engine.snapshot()).clone();
        snapshot.playbacks = vec![test_playback(1, list.id)].into();
        snapshot.cue_lists = vec![list].into();
        self.engine.replace_snapshot(snapshot).unwrap();
        execute_pool(&self.engine, 1, PoolPlaybackAction::Go);
    }
}

pub(super) fn stored_entry(
    engine: &Engine,
    fixture: FixtureId,
    attribute: &AttributeKey,
) -> ContributionFamilyEntry {
    let frame = engine.prepare_observer_frame(Default::default());
    let stored = frame
        .programmer()
        .output_states
        .iter()
        .flat_map(|state| state.values.iter())
        .find(|value| value.fixture_id == fixture && value.attribute == *attribute)
        .unwrap();
    ContributionFamilyEntry::new(
        ContributionSourceId::programmer(frame.programmer().identity.unwrap()),
        light_core::ProgrammerEditStamp {
            changed_at: stored.changed_at,
            programmer_order: stored.programmer_order,
        },
        ContributionFamilyFootprint::Whole,
        ContributionFamilyRole::Authored,
    )
}

pub(super) fn assert_entries(
    actual: &ContributionFamilyEvidence,
    expected: &[ContributionFamilyEntry],
) {
    // Compare original authorship independently of effective fields; conversion-specific scope
    // is asserted by the field-transfer tests below.
    assert_eq!(actual.entries().len(), expected.len(), "{actual:?}");
    for (actual, expected) in actual.entries().iter().zip(expected) {
        assert_eq!(actual.source(), expected.source());
        assert_eq!(actual.stamp().changed_at, expected.stamp().changed_at);
        assert_eq!(
            actual.stamp().programmer_order,
            expected.stamp().programmer_order
        );
        assert_eq!(actual.transition_ordinal(), expected.transition_ordinal());
        assert_eq!(actual.authored_cue_id(), expected.authored_cue_id());
        assert_eq!(actual.footprint(), expected.footprint());
        assert_eq!(actual.role(), expected.role());
    }
}

#[test]
fn repeated_interruptions_sample_old_history_before_adopting_new_timing() {
    let h = History::new("focus");
    let first = h.edit(AttributeValue::Normalized(0.0), None);
    h.commit(&[]);
    h.clock.advance_millis(1);
    let second = h.edit(AttributeValue::Normalized(0.8), Some((1_000, 200)));
    h.commit(&[]);
    h.clock.advance_millis(500);
    let third = h.edit(AttributeValue::Normalized(0.4), Some((2_000, 100)));
    h.commit(&[]);
    // The interrupted 1s fade has spent 300ms moving after its original 200ms delay.
    let interrupted = h.observe(0);
    h.assert_scalar(&interrupted, 0.24);
    let old_history = h.evidence(&interrupted);
    assert_entries(&old_history, &[first.clone(), second.clone()]);
    let boundary = h.observe(100);
    h.assert_scalar(&boundary, 0.24);
    assert!(Arc::ptr_eq(&h.evidence(&boundary), &old_history));
    let interior = h.observe(500);
    h.assert_scalar(&interior, 0.28);
    assert_entries(
        &h.evidence(&interior),
        &[first.clone(), second.clone(), third.clone()],
    );
    let fourth = h.edit(AttributeValue::Normalized(0.0), Some((500, 0)));
    h.commit(&[]);
    let twice_interrupted = h.observe(250);
    h.assert_scalar(&twice_interrupted, 0.14);
    assert_entries(
        &h.evidence(&twice_interrupted),
        &[first, second, third, fourth.clone()],
    );
    let completed = h.observe(250);
    h.assert_scalar(&completed, 0.0);
    assert_entries(&h.evidence(&completed), &[fourth]);
}

#[test]
fn same_time_equal_values_keep_distinct_edit_orders_and_reuse_cached_history() {
    let h = History::new("focus");
    let first = h.edit(AttributeValue::Normalized(0.25), None);
    h.commit(&[]);
    let first_frame = h.observe(0);
    let first_history = h.evidence(&first_frame);
    let second = h.edit(AttributeValue::Normalized(0.25), Some((1_000, 0)));
    assert_eq!(first.stamp().changed_at, second.stamp().changed_at);
    assert_ne!(
        first.stamp().programmer_order,
        second.stamp().programmer_order
    );
    h.commit(&[]);
    assert!(Arc::ptr_eq(&h.evidence(&h.observe(0)), &first_history));
    let before = h.observe(250);
    h.assert_scalar(&before, 0.25);
    let history = h.evidence(&before);
    assert_entries(&history, &[first, second.clone()]);
    h.commit(&[]);
    let after = h.observe(250);
    assert!(
        Arc::ptr_eq(&history, &h.evidence(&after)),
        "ordinary commits and observers reuse the blend's immutable evidence"
    );
    let completed = h.observe(500);
    assert_entries(&h.evidence(&completed), &[second]);
    let target_history = h.evidence(&completed);
    h.commit(&[]);
    assert!(Arc::ptr_eq(&target_history, &h.evidence(&h.observe(100))));
}

#[test]
fn completed_playback_underlay_keeps_its_stamp_and_ordinal_after_playback_is_off() {
    let h = History::new("focus");
    h.start_playback(AttributeValue::Normalized(0.25));
    let playback = h.evidence(&h.observe(0)).entries()[0].clone();
    assert!(matches!(
        playback.source().descriptor(),
        Source::Playback(_)
    ));
    assert!(playback.transition_ordinal().is_some());
    h.clock.advance_millis(1);
    let incoming = h.edit(AttributeValue::Normalized(0.75), Some((1_000, 0)));
    h.commit(&[]);
    execute_pool(&h.engine, 1, PoolPlaybackAction::Off);
    let interior = h.observe(500);
    h.assert_scalar(&interior, 0.5);
    assert_entries(&h.evidence(&interior), &[playback, incoming.clone()]);
    assert_entries(&h.evidence(&h.observe(500)), &[incoming]);
}

#[test]
fn rich_sampled_underlay_retains_original_footprints_dependencies_and_ordinals() {
    let h = History::new("focus");
    let at = h.clock.now();
    let rich = Arc::new(ContributionFamilyEvidence::new(vec![
        ContributionFamilyEntry::new(
            ContributionSourceId::programmer_group(ProgrammerId::new(), "original"),
            light_core::ProgrammerEditStamp {
                changed_at: at,
                programmer_order: 8,
            },
            ContributionFamilyFootprint::Component(ProgrammingComponent::Focus),
            ContributionFamilyRole::Authored,
        )
        .with_transition_ordinal(Some(17)),
        ContributionFamilyEntry::new(
            ContributionSourceId::preload(ProgrammerId::new()),
            light_core::ProgrammerEditStamp {
                changed_at: at - chrono::Duration::seconds(1),
                programmer_order: 3,
            },
            ContributionFamilyFootprint::Whole,
            ContributionFamilyRole::CalculationDependency,
        ),
    ]));
    let sample = sampled(h.fixture, "focus", at, 1).with_family_evidence(Arc::clone(&rich));
    let target = h.edit(AttributeValue::Normalized(0.8), Some((1_000, 200)));
    h.commit(&[ContributionBatch::new([sample])]);
    let delayed = h.observe(100);
    h.assert_scalar(&delayed, 0.4);
    assert!(Arc::ptr_eq(&h.evidence(&delayed), &rich));
    // The original sample is absent now, but its complete evidence was captured on first output.
    let interior = h.observe(600);
    h.assert_scalar(&interior, 0.6);
    let mut expected = rich.entries().to_vec();
    expected.push(target.clone());
    assert_entries(&h.evidence(&interior), &expected);
    assert_entries(&h.evidence(&h.observe(500)), &[target]);
}

#[test]
fn unknown_winning_underlay_does_not_borrow_evidence_from_equal_known_playback() {
    for empty_sidecar in [false, true] {
        let h = History::new("focus");
        h.start_playback(AttributeValue::Normalized(0.4));
        assert_eq!(h.evidence(&h.observe(0)).entries().len(), 1);
        let mut unknown = sampled(h.fixture, "focus", h.clock.now(), 20);
        if empty_sidecar {
            unknown =
                unknown.with_family_evidence(Arc::new(ContributionFamilyEvidence::new(Vec::new())));
        }
        let target = h.edit(AttributeValue::Normalized(0.8), Some((1_000, 200)));
        h.commit(&[ContributionBatch::new([unknown])]);
        for (advance, expected) in [(100, 0.4), (600, 0.6)] {
            let frame = h.observe(advance);
            h.assert_scalar(&frame, expected);
            let evidence = frame
                .values()
                .contribution_family_evidence(h.fixture, &h.attribute);
            assert!(
                evidence.is_none_or(|evidence| evidence.entries().is_empty()),
                "unknown input cannot gain an authored source: {evidence:?}"
            );
        }
        assert_entries(&h.evidence(&h.observe(500)), &[target]);
    }
}

#[test]
fn rounded_float_progress_uses_the_same_exact_endpoint_for_value_and_evidence() {
    let h = History::new("focus");
    h.edit(AttributeValue::Normalized(0.25), None);
    h.commit(&[]);
    h.clock.advance_millis(1);
    let target = h.edit(AttributeValue::Normalized(0.75), Some((40_000_000, 0)));
    h.commit(&[]);
    let almost_complete = h.observe(39_999_999);
    assert_eq!(
        almost_complete.values().value(h.fixture, &h.attribute),
        Some(&AttributeValue::Normalized(0.75))
    );
    assert_entries(&h.evidence(&almost_complete), &[target]);
}

#[test]
fn numeric_angles_and_zoom_blend_both_authored_families() {
    let angles = |pan, tilt| AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)));
    let zoom = |opening| {
        AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(opening),
            convention: light_core::OpeningConvention::Beam,
        }))
    };
    for (attribute, from, to, midpoint) in [
        (
            "position",
            angles(0.0, -20.0),
            angles(100.0, 20.0),
            angles(50.0, 0.0),
        ),
        ("zoom", zoom(10.0), zoom(50.0), zoom(30.0)),
    ] {
        let h = History::new(attribute);
        let first = h.edit(from, None);
        h.commit(&[]);
        h.clock.advance_millis(1);
        let second = h.edit(to, Some((1_000, 0)));
        h.commit(&[]);
        let interior = h.observe(500);
        assert_eq!(
            interior.values().value(h.fixture, &h.attribute),
            Some(&midpoint)
        );
        assert_entries(&h.evidence(&interior), &[first, second]);
    }
}

#[test]
fn held_position_keeps_previous_evidence_and_compatible_color_target_blends_keep_field_scopes() {
    use light_core::programming::ProgrammingTraceField as F;
    let target = |x| {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [x, 0.0, 0.0],
        )))
    };
    let color = |uv| {
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                uv: light_core::programming::UvIntent { amount: uv },
                ..Default::default()
            },
        }))
    };
    for (attribute, from, to, held) in [
        (
            "position",
            AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0))),
            target(1.0),
            true,
        ),
        ("position", target(0.0), target(1.0), false),
        ("color", color(0.0), color(1.0), false),
    ] {
        let h = History::new(attribute);
        let first = h.edit(from.clone(), None);
        h.commit(&[]);
        h.clock.advance_millis(1);
        let second = h.edit(to, Some((1_000, 0)));
        h.commit(&[]);
        let interior = h.observe(500);
        if held {
            assert_eq!(
                interior.values().value(h.fixture, &h.attribute),
                Some(&from)
            );
            assert_entries(&h.evidence(&interior), &[first]);
        } else {
            let history = h.evidence(&interior);
            assert_entries(&history, &[first, second.clone()]);
            let old = history.entries()[0].effective_fields().unwrap();
            let new = history.entries()[1].effective_fields().unwrap();
            let (held, blended) = if attribute == "color" {
                assert!(old.contains(F::ColorWheel(0)));
                assert!(!new.contains(F::ColorWheel(0)));
                (F::Allocation, F::Uv)
            } else {
                (F::TargetReference, F::TargetX)
            };
            assert!(old.contains(held));
            assert!(!new.contains(held));
            assert!(old.contains(blended));
            assert!(new.contains(blended));
        }
        assert_entries(&h.evidence(&h.observe(500)), &[second]);
    }
}

#[test]
fn amber_source_becomes_rgb_dependency_without_acquiring_the_generated_amber_field() {
    use light_core::programming::{
        ColorAllocation, ColorAuthoringModel, ColorComponent, ProgrammingTraceField as F,
        VirtualColorAuthoringV1,
    };
    let h = History::new("color");
    let mut outgoing = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_base_component(&mut outgoing, ColorComponent::Amber, 0.8)
        .unwrap();
    outgoing.allocation = ColorAllocation::PreferColoredEmitters;
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: outgoing.clone(),
    }));
    let original = ContributionFamilyEntry::new(
        ContributionSourceId::programmer_group(ProgrammerId::new(), "amber group"),
        light_core::ProgrammerEditStamp {
            changed_at: h.clock.now(),
            programmer_order: 7,
        },
        ContributionFamilyFootprint::Component(ProgrammingComponent::Color(ColorComponent::Amber)),
        ContributionFamilyRole::CalculationDependency,
    );
    let allocation = ContributionFamilyEntry::new(
        ContributionSourceId::preload(ProgrammerId::new()),
        light_core::ProgrammerEditStamp {
            changed_at: h.clock.now(),
            programmer_order: 4,
        },
        ContributionFamilyFootprint::Whole,
        ContributionFamilyRole::Authored,
    );
    let old_evidence = Arc::new(ContributionFamilyEvidence::new(vec![
        original.clone(),
        allocation.clone(),
    ]));
    let underlying = ContributionSample::independent(TimedValue {
        fixture_id: h.fixture,
        attribute: h.attribute.clone(),
        value,
        priority: 1,
        changed_at: h.clock.now(),
        programmer_order: 1,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
    .with_family_evidence(old_evidence);
    let incoming = h.edit(
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                allocation: ColorAllocation::PreferWhite,
                ..Default::default()
            },
        })),
        Some((1_000, 0)),
    );
    h.commit(&[ContributionBatch::new([underlying])]);
    let middle = h.observe(500);
    let history = h.evidence(&middle);
    assert_entries(
        &history,
        &[original.clone(), allocation.clone(), incoming.clone()],
    );
    let amber_fields = history.entries()[0].effective_fields().unwrap();
    for field in [
        F::ColorXyz,
        F::ColorRecipeRed,
        F::ColorRecipeGreen,
        F::ColorRecipeBlue,
    ] {
        assert!(amber_fields.contains(field));
    }
    assert!(!amber_fields.contains(F::ColorRecipeAmber));
    assert!(!amber_fields.contains(F::Uv));
    assert!(!amber_fields.contains(F::Allocation));
    assert!(
        history.entries()[1]
            .effective_fields()
            .unwrap()
            .contains(F::Allocation)
    );
    assert!(
        !history.entries()[2]
            .effective_fields()
            .unwrap()
            .contains(F::Allocation)
    );
    let AttributeValue::ColorProgram(actual) =
        middle.values().value(h.fixture, &h.attribute).unwrap()
    else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = actual.as_ref() else {
        panic!()
    };
    assert_eq!(intent.recipe.amber, 0.0);
    assert_eq!(intent.allocation, ColorAllocation::PreferColoredEmitters);

    // A second edit must retain the converted scopes, not recreate a Whole source for Amber.
    let final_target = h.edit(
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                uv: light_core::programming::UvIntent { amount: 0.5 },
                ..Default::default()
            },
        })),
        Some((1_000, 0)),
    );
    h.commit(&[]);
    let interrupted = h.observe(500);
    let retained = h.evidence(&interrupted);
    assert_entries(
        &retained,
        &[original, allocation, incoming, final_target.clone()],
    );
    let fields = retained.entries()[0].effective_fields().unwrap();
    assert!(fields.contains(F::ColorRecipeRed));
    assert!(!fields.contains(F::ColorRecipeAmber));
    assert!(!fields.contains(F::Allocation));
    assert_entries(&h.evidence(&h.observe(500)), &[final_target]);
}

#[test]
fn captured_and_preload_observations_keep_history_isolated_from_later_live_edits() {
    use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};
    let h = History::new("focus");
    let first = h.edit(AttributeValue::Normalized(0.25), None);
    h.commit(&[]);
    h.clock.advance_millis(1);
    let second = h.edit(AttributeValue::Normalized(0.75), Some((1_000, 0)));
    h.commit(&[]);
    h.clock.advance_millis(250);
    h.registry.arm_preload(h.session, true);
    assert!(h.registry.apply_preload_values(
        h.session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: h.fixture,
            attribute: h.attribute.clone(),
            value: AttributeValue::Normalized(0.0),
            timing: PreloadProgrammerValueTiming::default(),
        }]
    ));
    let captured = h.engine.prepare_output_frame(Default::default());
    let live = h.engine.observe_prepared_frame(&captured, &[]);
    h.assert_scalar(&live, 0.375);
    let live_history = h.evidence(&live);
    assert_entries(&live_history, &[first, second]);
    let pending = h.engine.prepare_preload_frame(&captured, None);
    let mut state = PreloadFrameState::default();
    let (revision_before, _) = h.engine.capture_output_continuity();
    let preview = h
        .engine
        .render_prepared_preload(&pending, &[], &[], &mut state)
        .unwrap();
    h.assert_scalar(&preview.source, 0.0);
    let preview_history = h.evidence(&preview.source);
    assert_eq!(preview_history.entries().len(), 1);
    assert!(matches!(
        preview_history.entries()[0].source().descriptor(),
        Source::Programmer {
            lane: Lane::Preload,
            ..
        }
    ));
    assert_eq!(revision_before, h.engine.capture_output_continuity().0);
    assert!(Arc::ptr_eq(
        &live_history,
        &h.evidence(&h.engine.observe_prepared_frame(&captured, &[]))
    ));
    h.registry.arm_preload(h.session, false);
    h.clock.advance_millis(250);
    let later = h.edit(AttributeValue::Normalized(0.5), None);
    h.commit(&[]);
    assert_entries(&h.evidence(&h.observe(0)), &[later]);
    let retained = h.engine.observe_prepared_frame(&captured, &[]);
    h.assert_scalar(&retained, 0.375);
    assert!(Arc::ptr_eq(&live_history, &h.evidence(&retained)));
    let retained_preview = h
        .engine
        .observe_prepared_preload(&pending, &[], &state, false);
    h.assert_scalar(&retained_preview, 0.0);
    assert_entries(&h.evidence(&retained_preview), preview_history.entries());
}
