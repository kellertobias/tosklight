use crate::{ActionError, ActionErrorKind};
use light_core::{AttributeKey, FixtureId};
use light_programmer::{
    NormalProgrammerValueMutation, NormalProgrammerValueTiming, Preset, ProgrammerSelection,
    SelectionExpression, SelectionReference, SelectionRule,
};
use std::collections::{HashMap, HashSet};

pub(super) fn plan(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    fade_millis: u64,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    if selection.selected.is_empty() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Preset recall requires a current selection",
        ));
    }
    let live_groups = live_group_targets(selection);
    let expanded_groups = expanded_group_memberships(preset, groups, &live_groups);
    let timing = NormalProgrammerValueTiming {
        fade: true,
        fade_millis: Some(fade_millis),
        delay_millis: None,
    };
    let mut planned = Vec::new();
    for fixture_id in &selection.selected {
        append_fixture_values(&mut planned, preset, *fixture_id, timing);
        append_expanded_group_values(&mut planned, preset, &expanded_groups, *fixture_id, timing);
    }
    append_live_group_values(&mut planned, preset, &live_groups, timing);
    Ok(retain_last_address(planned))
}

fn live_group_targets(selection: &ProgrammerSelection) -> Vec<String> {
    match &selection.expression {
        Some(SelectionExpression::LiveGroup {
            group_id,
            rule: SelectionRule::All,
        }) => vec![group_id.clone()],
        Some(SelectionExpression::Sources { items })
            if items
                .iter()
                .all(|item| matches!(item, SelectionReference::LiveGroup { .. })) =>
        {
            items
                .iter()
                .filter_map(|item| match item {
                    SelectionReference::LiveGroup { group_id } => Some(group_id.clone()),
                    _ => None,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn expanded_group_memberships(
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    live_groups: &[String],
) -> Vec<(String, HashSet<FixtureId>)> {
    let mut ids = preset
        .group_values
        .keys()
        .filter(|id| !live_groups.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids.into_iter()
        .filter_map(|group_id| {
            light_programmer::resolve_group(&group_id, groups)
                .ok()
                .map(|members| (group_id, members.into_iter().collect()))
        })
        .collect()
}

fn append_fixture_values(
    planned: &mut Vec<NormalProgrammerValueMutation>,
    preset: &Preset,
    fixture_id: FixtureId,
    timing: NormalProgrammerValueTiming,
) {
    let Some(attributes) = preset.values.get(&fixture_id) else {
        return;
    };
    for attribute in sorted_attributes(attributes) {
        planned.push(NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute: attribute.clone(),
            value: attributes[attribute].clone(),
            timing,
        });
    }
}

fn append_expanded_group_values(
    planned: &mut Vec<NormalProgrammerValueMutation>,
    preset: &Preset,
    groups: &[(String, HashSet<FixtureId>)],
    fixture_id: FixtureId,
    timing: NormalProgrammerValueTiming,
) {
    for (group_id, members) in groups {
        if !members.contains(&fixture_id) {
            continue;
        }
        let attributes = &preset.group_values[group_id];
        for attribute in sorted_attributes(attributes) {
            planned.push(NormalProgrammerValueMutation::SetFixture {
                fixture_id,
                attribute: attribute.clone(),
                value: attributes[attribute].clone(),
                timing,
            });
        }
    }
}

fn append_live_group_values(
    planned: &mut Vec<NormalProgrammerValueMutation>,
    preset: &Preset,
    live_groups: &[String],
    timing: NormalProgrammerValueTiming,
) {
    for group_id in live_groups {
        let Some(attributes) = preset.group_values.get(group_id) else {
            continue;
        };
        for attribute in sorted_attributes(attributes) {
            planned.push(NormalProgrammerValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: attributes[attribute].clone(),
                timing,
            });
        }
    }
}

fn sorted_attributes<V>(attributes: &HashMap<AttributeKey, V>) -> Vec<&AttributeKey> {
    let mut attributes = attributes.keys().collect::<Vec<_>>();
    attributes.sort_by(|left, right| left.0.cmp(&right.0));
    attributes
}

fn retain_last_address(
    planned: Vec<NormalProgrammerValueMutation>,
) -> Vec<NormalProgrammerValueMutation> {
    let mut seen = HashSet::new();
    let mut retained = planned
        .into_iter()
        .rev()
        .filter(|mutation| seen.insert(address(mutation)))
        .collect::<Vec<_>>();
    retained.reverse();
    retained
}

#[derive(Eq, Hash, PartialEq)]
enum PlannedAddress {
    Fixture(FixtureId, AttributeKey),
    Group(String, AttributeKey),
}

fn address(mutation: &NormalProgrammerValueMutation) -> PlannedAddress {
    match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            ..
        }
        | NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => PlannedAddress::Fixture(*fixture_id, attribute.clone()),
        NormalProgrammerValueMutation::SetGroup {
            group_id,
            attribute,
            ..
        }
        | NormalProgrammerValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => PlannedAddress::Group(group_id.clone(), attribute.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::AttributeValue;
    use light_programmer::{GroupDefinition, PresetFamily};

    #[test]
    fn overlapping_fixture_and_group_values_have_deterministic_last_source_precedence() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let intensity = AttributeKey::intensity();
        let pan = AttributeKey("pan".into());
        let preset = Preset {
            family: PresetFamily::Mixed,
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
            family: PresetFamily::Intensity,
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
                } => Some((*fixture_id, attribute.0.clone(), value.clone())),
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
}
