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
        plan.preview.items[0].outcome,
        UpdateItemOutcome::ChangeAtSource {
            source: CueSource {
                cue_number: 2.0,
                cue_index: 1,
                ..
            }
        }
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
        fixture_values: vec![fixture_update(fixture, "intensity", 0.8, 1)],
        group_values: vec![ProgrammerGroupUpdate {
            group_id: "front".into(),
            attribute: attribute("intensity"),
            value: normalized(0.9),
            programmer_order: 2,
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
