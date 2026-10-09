use super::*;

#[test]
fn existing_only_changes_the_latest_authoritative_tracked_source() {
    let fixture = fixture(1);
    let list = cue_list(vec![
        cue(1.0, vec![change(fixture, "intensity", 0.2)]),
        cue(2.0, vec![change(fixture, "intensity", 0.4)]),
        cue(3.0, vec![]),
    ]);
    let target = target(&list, 2, Some(1));
    let programmer = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);

    let plan = plan_cue_update(
        &list,
        7,
        7,
        &target,
        CueUpdateMode::ExistingOnly,
        &programmer,
    )
    .unwrap();
    assert!(matches!(
        &plan.preview.items[0].outcome,
        UpdateItemOutcome::ChangeAtSource {
            source: CueSource {
                cue_number,
                cue_index: 1,
                ..
            }
        } if cue_number == &crate::CueNumber::try_from_legacy_f64(2.0).unwrap()
    ));
    let updated = planned_cue_list(plan);
    assert_eq!(
        stored_value(&updated.cues[0], fixture, "intensity"),
        Some(0.2)
    );
    assert_eq!(
        stored_value(&updated.cues[1], fixture, "intensity"),
        Some(0.8)
    );
    assert_eq!(stored_value(&updated.cues[2], fixture, "intensity"), None);
}

#[test]
fn a_later_release_prevents_existing_only_from_rewriting_an_unrelated_earlier_value() {
    let fixture = fixture(1);
    let mut release = change(fixture, "intensity", 0.0);
    release.value = None;
    let list = cue_list(vec![
        cue(1.0, vec![change(fixture, "intensity", 0.2)]),
        cue(2.0, vec![release]),
        cue(3.0, vec![]),
    ]);
    let target = target(&list, 2, Some(1));
    let programmer = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);

    let preview =
        preview_cue_update(&list, &target, CueUpdateMode::ExistingOnly, &programmer).unwrap();
    assert_eq!(preview.changed_count(), 0);
    assert_eq!(
        preview.items[0].outcome,
        UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NotInActiveTrackedState
        }
    );
    assert!(matches!(
        plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer
        ),
        Err(UpdateError::NoOp { .. })
    ));
}

fn color_hold(
    component: Option<light_core::programming::ProgrammingComponent>,
    white_blend: f32,
) -> DynamicSemanticValue {
    use light_core::programming::{ColorIntent, ColorProgram, ProgrammingOwner};
    let family = AttributeValue::ColorProgram(std::sync::Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            white_blend,
            ..Default::default()
        },
    }));
    DynamicSemanticValue::ProgrammingFixAt {
        mask: light_dynamics::ProgrammingFamilyFixAt::from_family(
            ProgrammingOwner::Color,
            component,
            family,
        )
        .unwrap(),
        timing: DynamicValueTiming::default(),
    }
}

fn color_dynamic_change(fixture_id: FixtureId, value: DynamicSemanticValue) -> CueDynamicChange {
    CueDynamicChange {
        fixture_id,
        attribute: light_core::programming::ProgrammingOwner::Color.key(),
        value,
        automatic_restore: false,
    }
}

fn color_programmer(fixture_id: FixtureId, value: DynamicSemanticValue) -> ProgrammerUpdateContent {
    ProgrammerUpdateContent {
        dynamic_values: vec![DynamicAddressValue {
            fixture_id,
            attribute: light_core::programming::ProgrammingOwner::Color.key(),
            value,
            programmer_order: 1,
            changed_at_millis: 10,
        }],
        ..Default::default()
    }
}

fn playback_dynamic_count_at(list: &CueList, cue_index: usize) -> usize {
    let mut engine = light_playback::PlaybackEngine::default();
    engine.register(list.clone()).unwrap();
    for _ in 0..=cue_index {
        engine.go_at(list.id, chrono::Utc::now()).unwrap();
    }
    engine.active_cue_dynamic_values().len()
}

/// A typed Dynamic release must stop Tracked Update exactly like an ordinary release does, so
/// the programmer cannot resurrect a hold that active playback no longer tracks.
#[test]
fn a_later_typed_dynamic_release_prevents_existing_only_from_reviving_the_released_hold() {
    use light_core::programming::{ColorComponent, ProgrammingComponent};
    let red = Some(ProgrammingComponent::Color(ColorComponent::Red));
    for (label, hold_component, release) in [
        (
            "component release",
            red,
            DynamicSemanticValue::ProgrammingRelease { component: red },
        ),
        (
            "whole-family release",
            None,
            DynamicSemanticValue::ProgrammingRelease { component: None },
        ),
        (
            "owner release sweeping a component",
            red,
            DynamicSemanticValue::Release,
        ),
    ] {
        let fixture = fixture(1);
        let mut first = cue(1.0, vec![]);
        first.dynamic_changes = vec![color_dynamic_change(
            fixture,
            color_hold(hold_component, 0.2),
        )];
        let mut second = cue(2.0, vec![]);
        second.dynamic_changes = vec![color_dynamic_change(fixture, release.clone())];
        let list = cue_list(vec![first, second, cue(3.0, vec![])]);
        assert_eq!(
            playback_dynamic_count_at(&list, 2),
            0,
            "{label}: playback no longer tracks the hold"
        );
        let target = target(&list, 2, Some(1));
        let programmer = color_programmer(fixture, color_hold(hold_component, 0.8));

        let preview =
            preview_cue_update(&list, &target, CueUpdateMode::ExistingOnly, &programmer).unwrap();
        assert_eq!(preview.changed_count(), 0, "{label}");
        assert_eq!(
            preview.items[0].outcome,
            UpdateItemOutcome::Ignored {
                reason: UpdateIgnoreReason::NotInActiveTrackedState
            },
            "{label}"
        );
        assert!(
            matches!(
                plan_cue_update(
                    &list,
                    1,
                    1,
                    &target,
                    CueUpdateMode::ExistingOnly,
                    &programmer
                ),
                Err(UpdateError::NoOp { .. })
            ),
            "{label}: applied update agrees with the preview"
        );
        assert_eq!(list.cues[1].dynamic_changes[0].value, release, "{label}");
    }
}

/// Without the release, the same typed hold stays tracked and Tracked Update rewrites its source.
#[test]
fn existing_only_changes_an_active_typed_dynamic_hold_at_its_source() {
    use light_core::programming::{ColorComponent, ProgrammingComponent};
    let red = Some(ProgrammingComponent::Color(ColorComponent::Red));
    for hold_component in [red, None] {
        let fixture = fixture(1);
        let mut first = cue(1.0, vec![]);
        first.dynamic_changes = vec![color_dynamic_change(
            fixture,
            color_hold(hold_component, 0.2),
        )];
        let list = cue_list(vec![first, cue(2.0, vec![]), cue(3.0, vec![])]);
        assert_eq!(playback_dynamic_count_at(&list, 2), 1);
        let target = target(&list, 2, Some(1));
        let incoming = color_hold(hold_component, 0.8);
        let programmer = color_programmer(fixture, incoming.clone());

        let plan = plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer,
        )
        .unwrap();
        assert!(matches!(
            &plan.preview.items[0].outcome,
            UpdateItemOutcome::ChangeAtSource {
                source: CueSource { cue_index: 0, .. }
            }
        ));
        let updated = planned_cue_list(plan);
        assert_eq!(updated.cues[0].dynamic_changes[0].value, incoming);
        assert!(updated.cues[1].dynamic_changes.is_empty());
        assert!(updated.cues[2].dynamic_changes.is_empty());
    }
}

#[test]
fn four_cue_modes_keep_tracked_source_current_cue_and_new_addresses_distinct() {
    let fixture = fixture(1);
    let list = cue_list(vec![
        cue(1.0, vec![change(fixture, "intensity", 0.5)]),
        cue(2.0, vec![change(fixture, "pan", 0.25)]),
    ]);
    let target = target(&list, 1, Some(1));
    let programmer = content(vec![
        fixture_update(fixture, "intensity", 0.8, 1),
        fixture_update(fixture, "color.red", 0.6, 2),
    ]);

    let existing = plan_cue_update(
        &list,
        1,
        1,
        &target,
        CueUpdateMode::ExistingOnly,
        &programmer,
    )
    .unwrap();
    assert_eq!(existing.preview.changed_count(), 1);
    assert_eq!(existing.preview.ignored_count(), 1);
    let existing = planned_cue_list(existing);
    assert_eq!(
        stored_value(&existing.cues[0], fixture, "intensity"),
        Some(0.8)
    );
    assert_eq!(stored_value(&existing.cues[1], fixture, "intensity"), None);

    let current = preview_cue_update(
        &list,
        &target,
        CueUpdateMode::ExistingInCurrentCue,
        &programmer,
    )
    .unwrap();
    assert_eq!(current.changed_count(), 0);
    assert_eq!(current.ignored_count(), 2);

    let add_current = plan_cue_update(
        &list,
        1,
        1,
        &target,
        CueUpdateMode::AddToCurrentCue,
        &programmer,
    )
    .unwrap();
    assert_eq!(add_current.preview.added_count(), 1);
    assert_eq!(add_current.preview.ignored_count(), 1);
    let add_current = planned_cue_list(add_current);
    assert_eq!(
        stored_value(&add_current.cues[1], fixture, "intensity"),
        Some(0.8)
    );
    assert_eq!(
        stored_value(&add_current.cues[1], fixture, "color.red"),
        None
    );

    let add_new =
        plan_cue_update(&list, 1, 1, &target, CueUpdateMode::AddNew, &programmer).unwrap();
    assert_eq!(add_new.preview.added_count(), 2);
    let add_new = planned_cue_list(add_new);
    assert_eq!(
        stored_value(&add_new.cues[1], fixture, "intensity"),
        Some(0.8)
    );
    assert_eq!(
        stored_value(&add_new.cues[1], fixture, "color.red"),
        Some(0.6)
    );
}

#[test]
fn cue_eligibility_is_exact_per_fixture_and_attribute() {
    let fixtures = [fixture(1), fixture(2), fixture(3), fixture(4)];
    let list = cue_list(vec![
        cue(
            1.0,
            vec![
                change(fixtures[0], "color.red", 0.1),
                change(fixtures[1], "color.red", 0.1),
            ],
        ),
        cue(2.0, vec![]),
    ]);
    let target = target(&list, 1, Some(1));
    let programmer = content(
        fixtures
            .iter()
            .enumerate()
            .map(|(index, fixture_id)| fixture_update(*fixture_id, "color.red", 0.8, index as u64))
            .collect(),
    );

    let preview =
        preview_cue_update(&list, &target, CueUpdateMode::AddToCurrentCue, &programmer).unwrap();
    assert_eq!(preview.changed_count(), 2);
    assert_eq!(preview.ignored_count(), 2);
    let updated = planned_cue_list(
        plan_cue_update(
            &list,
            2,
            2,
            &target,
            CueUpdateMode::AddToCurrentCue,
            &programmer,
        )
        .unwrap(),
    );
    assert_eq!(updated.cues[1].changes.len(), 2);
    assert!(
        updated.cues[1]
            .changes
            .iter()
            .all(|change| fixtures[..2].contains(&change.fixture_id))
    );
}

#[test]
fn cue_fixture_and_group_addresses_track_independently() {
    let fixture = fixture(1);
    let mut first = cue(1.0, vec![change(fixture, "intensity", 0.2)]);
    first.group_changes.push(GroupCueChange {
        preset_reference: None,
        group_id: "front".into(),
        attribute: attribute("intensity"),
        value: Some(normalized(0.4)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let list = cue_list(vec![first, cue(2.0, vec![])]);
    let target = target(&list, 1, Some(1));
    let programmer = ProgrammerUpdateContent {
        dynamic_values: Vec::new(),
        fixture_values: vec![fixture_update(fixture, "intensity", 0.8, 1)],
        group_values: vec![ProgrammerGroupUpdate {
            preset_reference: None,
            group_id: "front".into(),
            attribute: attribute("intensity"),
            value: normalized(0.9),
            programmer_order: 2,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }],
        selected_fixtures: vec![],
    };
    let updated = planned_cue_list(
        plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::ExistingOnly,
            &programmer,
        )
        .unwrap(),
    );
    assert_eq!(
        stored_value(&updated.cues[0], fixture, "intensity"),
        Some(0.8)
    );
    assert_eq!(
        updated.cues[0].group_changes[0]
            .value
            .as_ref()
            .and_then(AttributeValue::normalized),
        Some(0.9)
    );
}

#[test]
fn existing_in_current_cue_treats_explicit_release_as_stored_but_not_generated_restore() {
    let explicit_fixture = fixture(1);
    let generated_fixture = fixture(2);
    let mut explicit_release = change(explicit_fixture, "intensity", 0.0);
    explicit_release.value = None;
    let mut generated = change(generated_fixture, "intensity", 0.2);
    generated.automatic_restore = true;
    let list = cue_list(vec![cue(1.0, vec![explicit_release, generated])]);
    let target = target(&list, 0, Some(1));
    let programmer = content(vec![
        fixture_update(explicit_fixture, "intensity", 0.8, 1),
        fixture_update(generated_fixture, "intensity", 0.9, 2),
    ]);
    let preview = preview_cue_update(
        &list,
        &target,
        CueUpdateMode::ExistingInCurrentCue,
        &programmer,
    )
    .unwrap();
    assert!(matches!(
        preview.items[0].outcome,
        UpdateItemOutcome::ChangeInCurrentCue { .. }
    ));
    assert_eq!(
        preview.items[1].outcome,
        UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NotInCurrentCue
        }
    );
}

#[test]
fn focus_and_zoom_update_independently_at_their_own_tracked_sources() {
    use light_core::OpeningConvention;
    use light_core::programming::{ProgrammingOwner, ScalarIntent, ZoomIntent};
    let zoom = |degrees| {
        AttributeValue::Zoom(std::sync::Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(degrees),
            convention: OpeningConvention::Field,
        }))
    };
    let (focus_key, zoom_key) = (ProgrammingOwner::Focus.key(), ProgrammingOwner::Zoom.key());
    let fixture = fixture(1);
    let list = cue_list(vec![
        cue(
            1.0,
            vec![
                CueChange::set(fixture, focus_key.clone(), normalized(0.2)),
                CueChange::set(fixture, zoom_key.clone(), zoom(30.)),
            ],
        ),
        cue(2.0, vec![]),
    ]);
    let target = target(&list, 1, Some(1));
    let stored = |cue: &Cue, key: &AttributeKey| {
        cue.changes
            .iter()
            .find(|change| change.fixture_id == fixture && change.attribute == *key)
            .and_then(|change| change.value.clone())
    };
    let update = |key: &AttributeKey, value: AttributeValue| ProgrammerUpdateContent {
        fixture_values: vec![ProgrammerFixtureUpdate {
            preset_reference: None,
            fixture_id: fixture,
            attribute: key.clone(),
            value,
            programmer_order: 1,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }],
        ..Default::default()
    };

    // Updating Focus rewrites only Focus at its source; the stored Zoom request is untouched.
    let focus_only = update(&focus_key, normalized(0.8));
    let updated = planned_cue_list(
        plan_cue_update(
            &list,
            1,
            1,
            &target,
            CueUpdateMode::ExistingOnly,
            &focus_only,
        )
        .unwrap(),
    );
    assert_eq!(stored(&updated.cues[0], &focus_key), Some(normalized(0.8)));
    assert_eq!(stored(&updated.cues[0], &zoom_key), Some(zoom(30.)));
    assert!(updated.cues[1].changes.is_empty());

    // Updating Zoom rewrites only Zoom (degrees and convention); Focus keeps its value.
    let zoom_only = update(&zoom_key, zoom(20.));
    let updated = planned_cue_list(
        plan_cue_update(
            &updated,
            2,
            2,
            &target,
            CueUpdateMode::ExistingOnly,
            &zoom_only,
        )
        .unwrap(),
    );
    assert_eq!(stored(&updated.cues[0], &zoom_key), Some(zoom(20.)));
    assert_eq!(stored(&updated.cues[0], &focus_key), Some(normalized(0.8)));

    // Add to the current cue: one owner is added without generating the other.
    let added = planned_cue_list(
        plan_cue_update(
            &list,
            3,
            3,
            &target,
            CueUpdateMode::AddToCurrentCue,
            &zoom_only,
        )
        .unwrap(),
    );
    assert_eq!(stored(&added.cues[1], &zoom_key), Some(zoom(20.)));
    assert_eq!(stored(&added.cues[1], &focus_key), None);
    assert_eq!(stored(&added.cues[0], &zoom_key), Some(zoom(30.)));
}
