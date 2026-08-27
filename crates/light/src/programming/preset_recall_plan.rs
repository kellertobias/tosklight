use crate::{ActionError, ActionErrorKind};
use light_core::{AttributeKey, FixtureId};
use light_programmer::{
    NormalProgrammerValueMutation, NormalProgrammerValueTiming, PreloadProgrammerValueMutation,
    PreloadProgrammerValueTiming, Preset, ProgrammerSelection, SelectionExpression,
    SelectionReference, SelectionRule,
};
use std::collections::{HashMap, HashSet};

pub(super) struct PresetTargetPlan {
    pub(super) selected: Vec<FixtureId>,
    pub(super) warning: Option<String>,
}

/// Resolve a Preset's stored owners into one frozen programmer selection.
///
/// `Preset` currently stores fixture and Group owners in maps, so it has no stored cross-owner
/// order to preserve. The active desk's selectable catalog is therefore the deterministic fallback
/// order. Whole-fixture owners expand through the same logical-head map used by ordinary selection.
pub(super) fn target_selection(
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    selectable_targets: &[FixtureId],
    target_expansions: &HashMap<FixtureId, Vec<FixtureId>>,
) -> PresetTargetPlan {
    let selectable = selectable_targets.iter().copied().collect::<HashSet<_>>();
    let mut requested = HashSet::new();
    let mut missing_fixture_targets = 0usize;
    let mut missing_groups = Vec::new();
    let mut missing_group_members = 0usize;

    for (fixture_id, attributes) in &preset.values {
        if attributes.is_empty() {
            continue;
        }
        if !append_expanded_target(&mut requested, *fixture_id, target_expansions, &selectable) {
            missing_fixture_targets += 1;
        }
    }

    let mut group_ids = preset
        .group_values
        .iter()
        .filter(|(_, attributes)| !attributes.is_empty())
        .map(|(group_id, _)| group_id)
        .collect::<Vec<_>>();
    group_ids.sort();
    for group_id in group_ids {
        match light_programmer::resolve_group(group_id, groups) {
            Ok(members) => {
                for fixture_id in members {
                    if !append_expanded_target(
                        &mut requested,
                        fixture_id,
                        target_expansions,
                        &selectable,
                    ) {
                        missing_group_members += 1;
                    }
                }
            }
            Err(_) => missing_groups.push(group_id.clone()),
        }
    }

    let selected = selectable_targets
        .iter()
        .copied()
        .filter(|fixture_id| requested.contains(fixture_id))
        .collect();
    let warning = target_warning(
        missing_fixture_targets,
        missing_group_members,
        &missing_groups,
    );
    PresetTargetPlan { selected, warning }
}

fn append_expanded_target(
    requested: &mut HashSet<FixtureId>,
    owner: FixtureId,
    target_expansions: &HashMap<FixtureId, Vec<FixtureId>>,
    selectable: &HashSet<FixtureId>,
) -> bool {
    let Some(expanded) = target_expansions.get(&owner) else {
        return false;
    };
    let mut found = false;
    for target in expanded {
        if selectable.contains(target) {
            requested.insert(*target);
            found = true;
        }
    }
    found
}

fn target_warning(
    missing_fixture_targets: usize,
    missing_group_members: usize,
    missing_groups: &[String],
) -> Option<String> {
    let missing_targets = missing_fixture_targets + missing_group_members;
    if missing_targets == 0 && missing_groups.is_empty() {
        return None;
    }
    let mut skipped = Vec::new();
    if missing_targets > 0 {
        skipped.push(format!(
            "{missing_targets} missing fixture target{}",
            if missing_targets == 1 { "" } else { "s" }
        ));
    }
    if !missing_groups.is_empty() {
        skipped.push(format!(
            "{} missing Group{} ({})",
            missing_groups.len(),
            if missing_groups.len() == 1 { "" } else { "s" },
            missing_groups.join(", ")
        ));
    }
    Some(format!(
        "Preset skipped {}. Restore the missing show objects or update the Preset.",
        skipped.join(" and ")
    ))
}

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

pub(super) fn as_preload(
    mutations: &[NormalProgrammerValueMutation],
) -> Vec<PreloadProgrammerValueMutation> {
    mutations
        .iter()
        .map(|mutation| match mutation {
            NormalProgrammerValueMutation::SetFixture {
                fixture_id,
                attribute,
                value,
                timing,
            } => PreloadProgrammerValueMutation::SetFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                timing: preload_timing(*timing),
            },
            NormalProgrammerValueMutation::ReleaseFixture {
                fixture_id,
                attribute,
            } => PreloadProgrammerValueMutation::ReleaseFixture {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
            },
            NormalProgrammerValueMutation::SetGroup {
                group_id,
                attribute,
                value,
                timing,
            } => PreloadProgrammerValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: value.clone(),
                timing: preload_timing(*timing),
            },
            NormalProgrammerValueMutation::ReleaseGroup {
                group_id,
                attribute,
            } => PreloadProgrammerValueMutation::ReleaseGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
            },
        })
        .collect()
}

const fn preload_timing(timing: NormalProgrammerValueTiming) -> PreloadProgrammerValueTiming {
    PreloadProgrammerValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
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
}
