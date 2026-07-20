use super::*;

#[test]
fn preset_update_existing_and_add_new_follow_exact_addresses() {
    let fixtures = [fixture(1), fixture(2), fixture(3), fixture(4)];
    let preset = Preset {
        name: "Color 1".into(),
        family: light_programmer::PresetFamily::Color,
        number: 1,
        values: fixtures[..2]
            .iter()
            .map(|fixture_id| {
                (
                    *fixture_id,
                    HashMap::from([(attribute("color.red"), normalized(0.1))]),
                )
            })
            .collect(),
        group_values: HashMap::new(),
    };
    let programmer = content(
        fixtures
            .iter()
            .enumerate()
            .map(|(index, fixture_id)| fixture_update(*fixture_id, "color.red", 0.8, index as u64))
            .collect(),
    );

    let existing = plan_preset_update(
        "1",
        &preset,
        4,
        4,
        ExistingContentMode::UpdateExisting,
        &programmer,
    )
    .unwrap();
    assert_eq!(existing.preview.changed_count(), 2);
    assert_eq!(existing.preview.ignored_count(), 2);
    let existing = planned_preset(existing);
    assert_eq!(existing.values.len(), 2);
    assert!(
        existing
            .values
            .values()
            .all(|attributes| { attributes[&attribute("color.red")].normalized() == Some(0.8) })
    );

    let added = planned_preset(
        plan_preset_update("1", &preset, 4, 4, ExistingContentMode::AddNew, &programmer).unwrap(),
    );
    assert_eq!(added.values.len(), 4);
}

#[test]
fn group_add_new_preserves_order_and_existing_only_never_mutates_membership() {
    let first = fixture(1);
    let second = fixture(2);
    let third = fixture(3);
    let fourth = fixture(4);
    let group = GroupDefinition {
        id: "1".into(),
        name: "Group 1".into(),
        fixtures: vec![second, first],
        ..Default::default()
    };
    let programmer = ProgrammerUpdateContent {
        selected_fixtures: vec![first, third, second, fourth, third],
        ..Default::default()
    };

    let existing = preview_group_update(
        &group,
        &[second, first],
        ExistingContentMode::UpdateExisting,
        &programmer,
    )
    .unwrap();
    assert_eq!(existing.changed_count(), 0);
    assert_eq!(existing.eligible_count(), 2);
    assert_eq!(existing.ignored_count(), 2);
    assert!(matches!(
        plan_group_update(
            &group,
            &[second, first],
            3,
            3,
            ExistingContentMode::UpdateExisting,
            &programmer,
        ),
        Err(UpdateError::NoOp { .. })
    ));

    let updated = planned_group(
        plan_group_update(
            &group,
            &[second, first],
            3,
            3,
            ExistingContentMode::AddNew,
            &programmer,
        )
        .unwrap(),
    );
    assert_eq!(updated.fixtures, vec![second, first, third, fourth]);
}

#[test]
fn stale_and_no_op_updates_produce_no_mutation_plan() {
    let fixture = fixture(1);
    let preset = Preset {
        name: "Intensity".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(attribute("intensity"), normalized(0.5))]),
        )]),
        group_values: HashMap::new(),
    };
    let changed = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);
    assert!(matches!(
        plan_preset_update(
            "1",
            &preset,
            9,
            8,
            ExistingContentMode::UpdateExisting,
            &changed
        ),
        Err(UpdateError::StaleRevision {
            expected: 8,
            current: 9
        })
    ));

    let unchanged = content(vec![fixture_update(fixture, "intensity", 0.5, 1)]);
    assert!(matches!(
        plan_preset_update(
            "1",
            &preset,
            9,
            9,
            ExistingContentMode::UpdateExisting,
            &unchanged
        ),
        Err(UpdateError::NoOp { .. })
    ));
    assert_eq!(
        preset.values[&fixture][&attribute("intensity")].normalized(),
        Some(0.5)
    );
}

#[test]
fn preset_update_ignores_attributes_outside_the_stored_family() {
    let fixture = fixture(1);
    let preset = Preset {
        name: "Color".into(),
        family: light_programmer::PresetFamily::Color,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(attribute("color.red"), normalized(0.2))]),
        )]),
        group_values: HashMap::new(),
    };
    let programmer = content(vec![
        fixture_update(fixture, "color.red", 0.8, 1),
        fixture_update(fixture, "pan", 0.6, 2),
    ]);

    let plan = plan_preset_update(
        "2.1",
        &preset,
        3,
        3,
        ExistingContentMode::AddNew,
        &programmer,
    )
    .unwrap();
    assert_eq!(plan.preview.items.len(), 1);
    let PlannedUpdateObject::Preset(updated) = plan.object else {
        panic!("expected preset update")
    };
    assert_eq!(updated.values[&fixture].len(), 1);
    assert_eq!(
        updated.values[&fixture][&attribute("color.red")],
        normalized(0.8)
    );
}
