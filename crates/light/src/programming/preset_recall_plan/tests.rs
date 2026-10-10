use super::*;
use light_core::AttributeValue;
use light_programmer::SelectionRule;
use light_programmer::{GroupDefinition, PresetFamily};

fn red() -> AttributeValue {
    AttributeValue::ColorXyz(light_core::Xyz {
        x: 0.4124,
        y: 0.2126,
        z: 0.0193,
    })
}

fn blue() -> AttributeValue {
    AttributeValue::ColorXyz(light_core::Xyz {
        x: 0.1805,
        y: 0.0722,
        z: 0.9505,
    })
}

#[test]
fn a_universal_colour_reaches_selected_fixtures_the_preset_never_named() {
    let named = FixtureId::new();
    let unnamed = FixtureId::new();
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Color,
        number: 1,
        universal_values: HashMap::from([(AttributeKey::color(), red())]),
        ..Preset::default()
    };
    let planned = plan(
        &selection(vec![named, unnamed]),
        &preset,
        &HashMap::new(),
        0,
    )
    .unwrap();
    assert_eq!(
        fixture_writes(&planned),
        vec![
            (named, "color".into(), red()),
            (unnamed, "color".into(), red()),
        ]
    );
}

#[test]
fn fixture_specific_colours_never_extend_to_unrelated_fixtures() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let unrelated = FixtureId::new();
    let mut preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Color,
        number: 2,
        values: HashMap::from([
            (first, HashMap::from([(AttributeKey::color(), red())])),
            (second, HashMap::from([(AttributeKey::color(), blue())])),
        ]),
        ..Preset::default()
    };
    preset.consolidate_universal_color();
    assert!(
        !preset.is_universal(),
        "differing colours stay fixture-specific"
    );
    let planned = plan(
        &selection(vec![first, second, unrelated]),
        &preset,
        &HashMap::new(),
        0,
    )
    .unwrap();
    assert_eq!(
        fixture_writes(&planned),
        vec![
            (first, "color".into(), red()),
            (second, "color".into(), blue()),
        ],
        "the unrelated fixture receives nothing"
    );
}

#[test]
fn a_named_fixture_keeps_its_own_colour_over_the_universal_one() {
    let named = FixtureId::new();
    let other = FixtureId::new();
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Color,
        number: 3,
        values: HashMap::from([(named, HashMap::from([(AttributeKey::color(), blue())]))]),
        universal_values: HashMap::from([(AttributeKey::color(), red())]),
        ..Preset::default()
    };
    let planned = plan(&selection(vec![named, other]), &preset, &HashMap::new(), 0).unwrap();
    assert_eq!(
        fixture_writes(&planned),
        vec![
            (named, "color".into(), blue()),
            (other, "color".into(), red())
        ]
    );
}

#[test]
fn a_universal_preset_without_a_selection_is_silent() {
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Color,
        number: 1,
        universal_values: HashMap::from([(AttributeKey::color(), red())]),
        ..Preset::default()
    };
    let fixture = FixtureId::new();
    let plan = target_selection(
        &preset,
        &HashMap::new(),
        &[fixture],
        &HashMap::from([(fixture, vec![fixture])]),
    );
    assert!(plan.selected.is_empty());
    assert!(plan.warning.is_none());
}

#[test]
fn overlapping_fixture_and_group_values_have_deterministic_last_source_precedence() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let intensity = AttributeKey::intensity();
    let pan = AttributeKey("pan".into());
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Mixed,
        aim_at_fixture_number: None,
        number: 1,
        values: HashMap::from([
            (
                first,
                HashMap::from([
                    (intensity.clone(), normalized(0.1)),
                    (pan.clone(), normalized(0.4)),
                ]),
            ),
            (
                second,
                HashMap::from([(intensity.clone(), normalized(0.2))]),
            ),
        ]),
        group_values: HashMap::from([
            (
                "10".into(),
                HashMap::from([(intensity.clone(), normalized(0.6))]),
            ),
            (
                "2".into(),
                HashMap::from([(intensity.clone(), normalized(0.8))]),
            ),
        ]),
        ..Preset::default()
    };
    let groups = HashMap::from([
        ("10".into(), group("10", vec![first, second])),
        ("2".into(), group("2", vec![first, second])),
    ]);
    let selection = selection(vec![second, first]);

    let planned = plan(&selection, &preset, &groups, 750).unwrap();

    assert_eq!(
        fixture_writes(&planned),
        vec![
            (second, "intensity".into(), normalized(0.8)),
            (first, "pan".into(), normalized(0.4)),
            (first, "intensity".into(), normalized(0.8)),
        ]
    );
    assert!(
        planned
            .iter()
            .all(|mutation| timing(mutation).is_some_and(|timing| timing.fade
                && timing.fade_millis == Some(750)
                && timing.delay_millis.is_none()))
    );
}

#[test]
fn missing_empty_and_unresolved_groups_do_not_perturb_selection_order() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let attribute = AttributeKey::intensity();
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Intensity,
        aim_at_fixture_number: None,
        number: 1,
        values: HashMap::from([
            (first, HashMap::from([(attribute.clone(), normalized(0.1))])),
            (
                second,
                HashMap::from([(attribute.clone(), normalized(0.2))]),
            ),
        ]),
        group_values: HashMap::from([
            (
                "missing".into(),
                HashMap::from([(attribute.clone(), normalized(0.3))]),
            ),
            (
                "empty".into(),
                HashMap::from([(attribute.clone(), normalized(0.4))]),
            ),
            (
                "cycle".into(),
                HashMap::from([(attribute.clone(), normalized(0.5))]),
            ),
        ]),
        ..Preset::default()
    };
    let groups = HashMap::from([
        ("empty".into(), group("empty", Vec::new())),
        (
            "cycle".into(),
            GroupDefinition {
                id: "cycle".into(),
                derived_from: Some(light_programmer::DerivedGroup {
                    source_group_id: "cycle".into(),
                    rule: SelectionRule::All,
                }),
                ..GroupDefinition::default()
            },
        ),
    ]);

    let planned = plan(&selection(vec![second, first]), &preset, &groups, 100).unwrap();

    assert_eq!(
        fixture_writes(&planned),
        vec![
            (second, "intensity".into(), normalized(0.2)),
            (first, "intensity".into(), normalized(0.1)),
        ]
    );
}

#[test]
fn target_selection_expands_parents_deduplicates_unions_and_uses_desk_order() {
    let parent = FixtureId::new();
    let head_a = FixtureId::new();
    let head_b = FixtureId::new();
    let standalone = FixtureId::new();
    let missing = FixtureId::new();
    let intensity = AttributeKey::intensity();
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        family: PresetFamily::Mixed,
        aim_at_fixture_number: None,
        number: 1,
        values: HashMap::from([
            (
                parent,
                HashMap::from([(intensity.clone(), normalized(0.1))]),
            ),
            (
                standalone,
                HashMap::from([(intensity.clone(), normalized(0.2))]),
            ),
            (
                missing,
                HashMap::from([(intensity.clone(), normalized(0.3))]),
            ),
        ]),
        group_values: HashMap::from([
            (
                "front".into(),
                HashMap::from([(intensity.clone(), normalized(0.4))]),
            ),
            ("gone".into(), HashMap::from([(intensity, normalized(0.5))])),
        ]),
        ..Preset::default()
    };
    let groups = HashMap::from([("front".into(), group("front", vec![standalone, head_b]))]);
    let desk_order = vec![head_b, standalone, head_a];
    let expansions = HashMap::from([
        (parent, vec![head_a, head_b]),
        (head_a, vec![head_a]),
        (head_b, vec![head_b]),
        (standalone, vec![standalone]),
    ]);

    let planned = target_selection(&preset, &groups, &desk_order, &expansions);

    assert_eq!(planned.selected, desk_order);
    let warning = planned.warning.unwrap();
    assert!(warning.contains("1 missing fixture target"));
    assert!(warning.contains("1 missing Group (gone)"));
}

#[test]
fn target_selection_ignores_empty_values_and_empty_groups_without_warning() {
    let fixture = FixtureId::new();
    let preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        values: HashMap::from([(fixture, HashMap::new())]),
        group_values: HashMap::from([("empty".into(), HashMap::new())]),
        aim_at_fixture_number: None,
        ..Preset::default()
    };
    let expansions = HashMap::from([(fixture, vec![fixture])]);

    let planned = target_selection(&preset, &HashMap::new(), &[fixture], &expansions);

    assert!(planned.selected.is_empty());
    assert_eq!(planned.warning, None);
}

#[test]
fn target_selection_is_shared_by_color_position_and_mixed_presets() {
    let fixture = FixtureId::new();
    let expansions = HashMap::from([(fixture, vec![fixture])]);

    for (family, attribute) in [
        (PresetFamily::Color, AttributeKey("red".into())),
        (PresetFamily::Position, AttributeKey("pan".into())),
        (PresetFamily::Mixed, AttributeKey::intensity()),
    ] {
        let preset = Preset {
            fixture_replacement_projections: Default::default(),
            group_replacement_projections: Default::default(),
            family,
            values: HashMap::from([(fixture, HashMap::from([(attribute, normalized(0.5))]))]),
            aim_at_fixture_number: None,
            ..Preset::default()
        };

        let planned = target_selection(&preset, &HashMap::new(), &[fixture], &expansions);

        assert_eq!(planned.selected, vec![fixture]);
        assert_eq!(planned.warning, None);
    }
}

fn selection(selected: Vec<FixtureId>) -> ProgrammerSelection {
    ProgrammerSelection {
        selected,
        expression: Some(SelectionExpression::Static),
        revision: 7,
        gesture_open: false,
    }
}

fn group(id: &str, fixtures: Vec<FixtureId>) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        fixtures,
        ..GroupDefinition::default()
    }
}

fn normalized(value: f32) -> AttributeValue {
    AttributeValue::Normalized(value)
}

fn fixture_writes(
    planned: &[NormalProgrammerValueMutation],
) -> Vec<(FixtureId, String, AttributeValue)> {
    planned
        .iter()
        .filter_map(|mutation| match mutation {
            NormalProgrammerValueMutation::SetFixture {
                fixture_id,
                attribute,
                value,
                ..
            } => Some((*fixture_id, attribute.0.to_string(), value.clone())),
            _ => None,
        })
        .collect()
}

fn timing(mutation: &NormalProgrammerValueMutation) -> Option<NormalProgrammerValueTiming> {
    match mutation {
        NormalProgrammerValueMutation::SetFixture { timing, .. }
        | NormalProgrammerValueMutation::SetGroup { timing, .. } => Some(*timing),
        _ => None,
    }
}

fn replacement_projection(
    source: FixtureId,
    targets: &[FixtureId],
) -> light_core::ReplacementProgramProjection {
    let profile = light_core::ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: uuid::Uuid::new_v4(),
    };
    light_core::ReplacementProgramProjection {
        source_owner: source,
        source_profile: profile.clone(),
        source_head_id: uuid::Uuid::new_v4(),
        target_profile: profile,
        targets: targets
            .iter()
            .map(|fixture| light_core::ReplacementHeadTarget {
                profile_head_id: uuid::Uuid::new_v4(),
                fixture_id: *fixture,
            })
            .collect(),
    }
}

#[test]
fn replacement_projection_recall_targets_exact_selected_head_without_master_or_sibling() {
    let source = FixtureId::new();
    let heads = [FixtureId::new(), FixtureId::new()];
    let attribute = AttributeKey::intensity();
    let preset = Preset {
        instance_id: Some(uuid::Uuid::new_v4()),
        family: PresetFamily::Intensity,
        values: HashMap::from([(
            source,
            HashMap::from([(attribute.clone(), AttributeValue::Normalized(0.4))]),
        )]),
        fixture_replacement_projections: HashMap::from([(
            source,
            HashMap::from([(attribute.clone(), replacement_projection(source, &heads))]),
        )]),
        ..Preset::default()
    };
    let selected = selection(vec![heads[1]]);
    let planned = plan(&selected, &preset, &HashMap::new(), 0).unwrap();
    assert_eq!(
        fixture_writes(&planned),
        vec![(source, "intensity".into(), AttributeValue::Normalized(0.4))]
    );
    let origins = replacement_value_origins(&selected, &preset, &HashMap::new());
    assert_eq!(origins[0].2[&source].targets[0].fixture_id, heads[1]);
    assert_eq!(origins[0].2[&source].targets.len(), 1);
    let references = preset_value_origins(&selected, &preset, &HashMap::new(), &HashMap::new());
    assert_eq!(
        references[0].2.source_owner,
        light_core::PresetValueOwner::Fixture { fixture_id: source }
    );
    assert!(
        plan(&selection(vec![source]), &preset, &HashMap::new(), 0)
            .unwrap()
            .is_empty(),
        "root .0 is not permission to address replacement heads"
    );
}

#[test]
fn replacement_projection_group_member_recall_keeps_original_rank_and_live_membership() {
    let source = FixtureId::new();
    let unrelated = FixtureId::new();
    let child = FixtureId::new();
    let attribute = AttributeKey::intensity();
    let groups = HashMap::from([(
        "Front".into(),
        GroupDefinition {
            id: "Front".into(),
            fixtures: vec![source, unrelated],
            ..Default::default()
        },
    )]);
    let preset = Preset {
        instance_id: Some(uuid::Uuid::new_v4()),
        family: PresetFamily::Intensity,
        group_values: HashMap::from([(
            "Front".into(),
            HashMap::from([(attribute.clone(), AttributeValue::Normalized(0.4))]),
        )]),
        group_replacement_projections: HashMap::from([(
            "Front".into(),
            HashMap::from([(
                attribute.clone(),
                HashMap::from([(source, replacement_projection(source, &[child]))]),
            )]),
        )]),
        ..Preset::default()
    };
    let selected = selection(vec![child]);
    let references = preset_value_origins(&selected, &preset, &groups, &HashMap::new());
    assert_eq!(references[0].2.member_fixture, Some(source));
    assert_eq!(references[0].2.sample_rank, Some((0, 2)));
    let planned = plan(&selected, &preset, &groups, 0).unwrap();
    assert_eq!(fixture_writes(&planned)[0].0, source);
    let mut selected_group = selection(vec![source, unrelated]);
    selected_group.expression = Some(SelectionExpression::LiveGroup {
        group_id: "Front".into(),
        rule: SelectionRule::All,
    });
    let origins = replacement_value_origins(&selected_group, &preset, &groups);
    assert!(
        matches!(&origins[0].0, light_core::PresetValueOwner::Group { group_id } if group_id == "Front")
    );
    assert_eq!(
        origins[0].2.keys().copied().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(
        light_programmer::resolve_group("Front", &groups).unwrap(),
        vec![source, unrelated]
    );
}

#[test]
fn replacement_projection_recall_orders_roots_by_selected_destinations_and_detaches_later_source() {
    let roots = [FixtureId::new(), FixtureId::new()];
    let children = [FixtureId::new(), FixtureId::new()];
    let attribute = AttributeKey::intensity();
    let mut preset = Preset {
        values: roots
            .iter()
            .map(|root| (*root, HashMap::from([(attribute.clone(), normalized(0.4))])))
            .collect(),
        fixture_replacement_projections: roots
            .iter()
            .zip(children)
            .map(|(root, child)| {
                (
                    *root,
                    HashMap::from([(attribute.clone(), replacement_projection(*root, &[child]))]),
                )
            })
            .collect(),
        ..Default::default()
    };
    let selected = selection(vec![children[1], children[0]]);
    let writes = fixture_writes(&plan(&selected, &preset, &HashMap::new(), 0).unwrap());
    assert_eq!(
        writes.iter().map(|value| value.0).collect::<Vec<_>>(),
        vec![roots[1], roots[0]]
    );
    // An ordinary Group source wins over the earlier direct value at the same selected root.
    // Its empty envelope must explicitly detach the earlier direct projection, never borrow it.
    preset.group_values.insert(
        "Later".into(),
        HashMap::from([(attribute.clone(), normalized(0.7))]),
    );
    let groups = HashMap::from([("Later".into(), group("Later", vec![roots[0]]))]);
    let selected = selection(vec![children[0], roots[0]]);
    let writes = fixture_writes(&plan(&selected, &preset, &groups, 0).unwrap());
    assert_eq!(
        writes,
        vec![(roots[0], "intensity".into(), normalized(0.7))]
    );
    let origins = replacement_value_origins(&selected, &preset, &groups);
    assert!(origins.last().unwrap().2.is_empty());
}
