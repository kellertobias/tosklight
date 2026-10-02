use crate::ActionError;
use light_core::{AttributeKey, AttributeValue, FixtureId, NativeColorIdentity, programming::*};
use light_dynamics::DynamicNativeModelResolver;
use light_programmer::{
    NormalProgrammerValueMutation, NormalProgrammerValueTiming, PreloadProgrammerValueMutation,
    PreloadProgrammerValueTiming, Preset, ProgrammerSelection, SelectionExpression,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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

pub(super) fn plan_with_positions(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
    fade_millis: u64,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    plan_with_positions_and_native_models(selection, preset, groups, positions, fade_millis, None)
}

fn plan_with_positions_and_native_models(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
    fade_millis: u64,
    native_models: Option<&dyn DynamicNativeModelResolver>,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    preset.validate_programming().map_err(invalid_intent)?;
    let live_groups = live_group_targets(selection);
    if selection.selected.is_empty() && live_groups.is_empty() {
        return Ok(Vec::new());
    }
    let timing = NormalProgrammerValueTiming {
        fade: true,
        fade_millis: Some(fade_millis),
        delay_millis: None,
    };
    let mut planned = Vec::new();
    if live_groups.is_empty() {
        for attribute in sorted_attributes(&preset.universal_values) {
            let native = native_for_spread(&preset.universal_values[attribute], native_models)
                .map_err(invalid_intent)?;
            let mut context = virtual_context();
            context.native_model = native
                .as_deref()
                .map(|model| model as &dyn NativeColorEditModel);
            let ranked = compile_programming_spread(
                &preset.universal_values[attribute],
                selection.selected.len(),
                &context,
            )
            .map_err(invalid_intent)?;
            for (rank, fixture_id) in selection.selected.iter().enumerate() {
                planned.push(NormalProgrammerValueMutation::SetFixture {
                    fixture_id: *fixture_id,
                    attribute: attribute.clone(),
                    value: ranked
                        .at_rank(rank)
                        .expect("selection rank compiled")
                        .clone(),
                    timing,
                });
            }
        }
    }
    for fixture_id in &selection.selected {
        append_fixture_values(&mut planned, preset, *fixture_id, timing);
    }
    let selected = selection.selected.iter().copied().collect::<HashSet<_>>();
    let mut ids = preset
        .group_values
        .keys()
        .filter(|id| !live_groups.contains(id))
        .collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        let Ok(resolved) = light_programmer::resolve_group_spatial(id, groups, positions) else {
            continue;
        };
        let ranking = resolved.ranked_selection;
        // Sampling retains the full Group rank domain even when only some members are selected.
        let members = ranking
            .ordered_fixture_ids
            .iter()
            .filter(|id| selected.contains(id))
            .map(|id| (*id, ranking.rank_by_fixture[id]))
            .collect::<Vec<_>>();
        for attribute in sorted_attributes(&preset.group_values[id]) {
            let value = &preset.group_values[id][attribute];
            let native =
                group_native_models(value, &members, native_models).map_err(invalid_intent)?;
            let values =
                compile_group_member_values(value, &members, ranking.rank_count, |value| {
                    let mut context = virtual_context();
                    if let Some(source) = direct_spread_source(value) {
                        context.native_model = native
                            .iter()
                            .find(|(identity, _)| identity == source)
                            .map(|(_, model)| model.as_ref() as &dyn NativeColorEditModel);
                    }
                    Ok(context)
                })
                .map_err(invalid_intent)?;
            for (fixture_id, value) in values {
                planned.push(NormalProgrammerValueMutation::SetFixture {
                    fixture_id,
                    attribute: attribute.clone(),
                    value,
                    timing,
                });
            }
        }
    }
    append_live_group_values(&mut planned, preset, &live_groups, timing);
    let mut planned = retain_last_address(planned);
    let order = selection
        .selected
        .iter()
        .enumerate()
        .map(|(rank, id)| (*id, rank))
        .collect::<HashMap<_, _>>();
    // Rank compilation is batched by source, but mutations retain the operator's selection
    // order and the established per-fixture last-source precedence.
    planned.sort_by_key(|mutation| match mutation {
        NormalProgrammerValueMutation::SetFixture { fixture_id, .. }
        | NormalProgrammerValueMutation::ReleaseFixture { fixture_id, .. } => {
            order.get(fixture_id).copied().unwrap_or(usize::MAX)
        }
        _ => usize::MAX,
    });
    Ok(planned)
}

fn direct_spread_source(value: &AttributeValue) -> Option<&NativeColorIdentity> {
    if value.spread_control_points() == 0 {
        return None;
    }
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Direct { recipe, .. } => Some(&recipe.source),
            ColorProgram::Semantic { .. } => None,
        },
        _ => None,
    }
}

fn native_for_spread(
    value: &AttributeValue,
    resolver: Option<&dyn DynamicNativeModelResolver>,
) -> Result<Option<Arc<dyn NativeColorEditModel + Send + Sync>>, IntentError> {
    let Some(source) = direct_spread_source(value) else {
        return Ok(None);
    };
    let Some(resolver) = resolver else {
        // The original entry point keeps its previous behavior: Direct spreads fail in
        // compile_programming_spread with its established missing-model error.
        return Ok(None);
    };
    let model = resolver.resolve(source)?;
    if model.source() != source {
        return Err(IntentError("native spread source identity changed".into()));
    }
    Ok(Some(model))
}

fn group_native_models(
    value: &AttributeValue,
    members: &[(FixtureId, usize)],
    resolver: Option<&dyn DynamicNativeModelResolver>,
) -> Result<
    Vec<(
        NativeColorIdentity,
        Arc<dyn NativeColorEditModel + Send + Sync>,
    )>,
    IntentError,
> {
    let mut native = Vec::new();
    for (fixture_id, _) in members {
        let member = match value {
            AttributeValue::GroupFamily(assignment) => assignment
                .members
                .get(&fixture_id.0)
                .unwrap_or(&assignment.template),
            _ => value,
        };
        let Some(source) = direct_spread_source(member) else {
            continue;
        };
        if native.iter().any(|(identity, _)| identity == source) {
            continue;
        }
        if let Some(model) = native_for_spread(member, resolver)? {
            native.push((source.clone(), model));
        }
    }
    Ok(native)
}

fn virtual_context() -> FamilyEditContext<'static> {
    FamilyEditContext {
        color_model: Some(&VirtualColorAuthoringV1),
        ..Default::default()
    }
}
fn invalid_intent(error: IntentError) -> ActionError {
    ActionError::new(crate::ActionErrorKind::Invalid, error.to_string())
}

/// Shared materialization for FixAT and other fixture-scoped consumers of stored Group presets.
pub fn materialize_preset_fixture_values(
    preset: &Preset,
    targets: &[FixtureId],
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    let selection = ProgrammerSelection {
        selected: targets.to_vec(),
        ..Default::default()
    };
    plan_with_positions(&selection, preset, groups, positions, 0)
}

/// Materialize Direct native spreads against the immutable original source named by the stored
/// recipe. The resolver must never substitute a selected fixture or current library revision.
pub fn materialize_preset_fixture_values_with_native_models(
    preset: &Preset,
    targets: &[FixtureId],
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
    native_models: &dyn DynamicNativeModelResolver,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    let selection = ProgrammerSelection {
        selected: targets.to_vec(),
        ..Default::default()
    };
    plan_with_positions_and_native_models(
        &selection,
        preset,
        groups,
        positions,
        0,
        Some(native_models),
    )
}

/// Plan a command recall using the same spread, precedence and live Group ownership as v2
/// recall. The caller captures the selection/geometry before entering its existing atomic
/// command transaction; this function never changes Programmer state or bakes output values.
pub fn plan_preset_selection_values(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
    fade_millis: u64,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    plan_with_positions(selection, preset, groups, positions, fade_millis)
}

pub fn as_preload(
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
    selection
        .expression
        .as_ref()
        .map(SelectionExpression::live_group_owners)
        .unwrap_or_default()
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

fn append_live_group_values(
    planned: &mut Vec<NormalProgrammerValueMutation>,
    preset: &Preset,
    live_groups: &[String],
    timing: NormalProgrammerValueTiming,
) {
    for group_id in live_groups {
        for attribute in sorted_attributes(&preset.universal_values) {
            planned.push(NormalProgrammerValueMutation::SetGroup {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: preset.universal_values[attribute].clone(),
                timing,
            });
        }
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
fn plan(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    fade: u64,
) -> Result<Vec<NormalProgrammerValueMutation>, ActionError> {
    plan_with_positions(selection, preset, groups, &HashMap::new(), fade)
}

#[cfg(test)]
mod tests {
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

#[cfg(test)]
mod family_tests {
    use super::*;
    use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};
    use std::{collections::BTreeMap, sync::Arc};

    struct NativeModel {
        identity: NativeColorIdentity,
        binding: NativeColorBinding,
    }
    impl NativeColorEditModel for NativeModel {
        fn source(&self) -> &NativeColorIdentity {
            &self.identity
        }
        fn descriptor(
            &self,
            binding: NativeColorBinding,
        ) -> Option<NativeColorComponentDescriptor> {
            (binding == self.binding).then_some(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: true,
            })
        }
        fn predict(
            &self,
            recipe: &NativeColorRecipe,
        ) -> Result<PortableColorEstimate, IntentError> {
            if recipe.source != self.identity || recipe.channels.len() != 1 {
                return Err(IntentError(
                    "prediction requires the complete original source".into(),
                ));
            }
            let channel = &recipe.channels[0];
            if (channel.channel_id, channel.function_id)
                != (self.binding.channel_id, self.binding.function_id)
            {
                return Err(IntentError("native binding changed".into()));
            }
            Ok(PortableColorEstimate {
                model_revision: self.identity.model_revision,
                visible: None,
                uv: Some(PortableUv {
                    amount: 0.75,
                    quality: PhysicalDataQuality::Estimated,
                }),
                quality: PhysicalDataQuality::Estimated,
                limitations: vec![format!("original raw {}", channel.raw)],
            })
        }
    }
    struct Models(Arc<NativeModel>);
    impl DynamicNativeModelResolver for Models {
        fn resolve(
            &self,
            source: &NativeColorIdentity,
        ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
            if source != &self.0.identity {
                return Err(IntentError("original source unavailable".into()));
            }
            Ok(self.0.clone())
        }
    }
    struct MismatchedModels(Arc<NativeModel>);
    impl DynamicNativeModelResolver for MismatchedModels {
        fn resolve(
            &self,
            _: &NativeColorIdentity,
        ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
            Ok(self.0.clone())
        }
    }
    fn native_model() -> Arc<NativeModel> {
        Arc::new(NativeModel {
            identity: NativeColorIdentity {
                profile_id: uuid::Uuid::new_v4(),
                profile_revision: 3,
                profile_digest: "immutable-original-source".into(),
                mode_id: uuid::Uuid::new_v4(),
                head_id: uuid::Uuid::new_v4(),
                path_id: uuid::Uuid::new_v4(),
                model_revision: 4,
                native_layout_signature: "one-u32-uv-channel".into(),
            },
            binding: NativeColorBinding {
                channel_id: uuid::Uuid::new_v4(),
                function_id: uuid::Uuid::new_v4(),
            },
        })
    }
    fn native_spread(model: &NativeModel) -> AttributeValue {
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
            recipe: NativeColorRecipe {
                source: model.identity.clone(),
                channels: vec![NativeColorValue {
                    channel_id: model.binding.channel_id,
                    function_id: model.binding.function_id,
                    raw: u32::MAX - 2,
                }],
                spreads: vec![NativeColorSpread {
                    binding: model.binding,
                    points: vec![u32::MAX - 2, u32::MAX],
                }],
            },
            portable: PortableColorEstimate {
                model_revision: 4,
                visible: None,
                uv: None,
                quality: PhysicalDataQuality::Unknown,
                limitations: vec![],
            },
        }))
    }

    #[test]
    fn native_universal_spread_uses_original_source_and_predicts_each_exact_u32_and_uv() {
        let original = native_model();
        let targets = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
        let preset = Preset {
            family: light_programmer::PresetFamily::Color,
            number: 1,
            universal_values: HashMap::from([(
                ProgrammingOwner::Color.key(),
                native_spread(&original),
            )]),
            ..Default::default()
        };
        let models = Models(original.clone());
        let planned = materialize_preset_fixture_values_with_native_models(
            &preset,
            &targets,
            &HashMap::new(),
            &HashMap::new(),
            &models,
        )
        .unwrap();
        assert_eq!(planned.len(), targets.len());
        for (rank, mutation) in planned.iter().enumerate() {
            let NormalProgrammerValueMutation::SetFixture {
                fixture_id, value, ..
            } = mutation
            else {
                panic!("one fixture write per selection rank");
            };
            assert_eq!(*fixture_id, targets[rank]);
            let AttributeValue::ColorProgram(program) = value else {
                panic!("Direct Color result");
            };
            let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
                panic!("native recipe result");
            };
            let raw = u32::MAX - 2 + rank as u32;
            assert_eq!(recipe.source, original.identity);
            assert_eq!(recipe.channels[0].raw, raw);
            assert!(recipe.spreads.is_empty());
            assert_eq!(portable.uv.unwrap().amount, 0.75);
            assert_eq!(portable.quality, PhysicalDataQuality::Estimated);
            assert_eq!(portable.limitations, vec![format!("original raw {raw}")]);
        }
        assert!(
            materialize_preset_fixture_values(&preset, &targets, &HashMap::new(), &HashMap::new(),)
                .is_err(),
            "the legacy wrapper keeps its missing-model behavior"
        );

        let unavailable = Models(Arc::new(NativeModel {
            identity: NativeColorIdentity {
                profile_id: uuid::Uuid::new_v4(),
                ..original.identity.clone()
            },
            binding: original.binding,
        }));
        assert!(
            materialize_preset_fixture_values_with_native_models(
                &preset,
                &[],
                &HashMap::new(),
                &HashMap::new(),
                &unavailable,
            )
            .unwrap()
            .is_empty(),
            "empty selection must not require any native source resolution"
        );
        assert!(
            materialize_preset_fixture_values_with_native_models(
                &preset,
                &targets,
                &HashMap::new(),
                &HashMap::new(),
                &unavailable,
            )
            .is_err(),
            "a destination-only resolver cannot replace the original"
        );
        assert!(
            materialize_preset_fixture_values_with_native_models(
                &preset,
                &targets,
                &HashMap::new(),
                &HashMap::new(),
                &MismatchedModels(unavailable.0),
            )
            .is_err(),
            "a resolver returning a different source identity must fail"
        );
    }

    #[test]
    fn native_group_spread_materializes_only_selected_members_in_full_group_rank_domain() {
        let original = native_model();
        let a = FixtureId::new();
        let b = FixtureId::new();
        let c = FixtureId::new();
        let preset = Preset {
            family: light_programmer::PresetFamily::Color,
            number: 2,
            group_values: HashMap::from([(
                "original-group".into(),
                HashMap::from([(ProgrammingOwner::Color.key(), native_spread(&original))]),
            )]),
            ..Default::default()
        };
        let groups = HashMap::from([(
            "original-group".into(),
            light_programmer::GroupDefinition {
                id: "original-group".into(),
                fixtures: vec![a, b, c],
                ..Default::default()
            },
        )]);
        let planned = materialize_preset_fixture_values_with_native_models(
            &preset,
            &[c, b],
            &groups,
            &HashMap::new(),
            &Models(original),
        )
        .unwrap();
        assert_eq!(planned.len(), 2);
        for (mutation, expected) in planned.iter().zip([u32::MAX, u32::MAX - 1]) {
            let NormalProgrammerValueMutation::SetFixture { value, .. } = mutation else {
                panic!()
            };
            let AttributeValue::ColorProgram(program) = value else {
                panic!()
            };
            let ColorProgram::Direct { recipe, portable } = program.as_ref() else {
                panic!()
            };
            assert_eq!(recipe.channels[0].raw, expected);
            assert_eq!(
                portable.limitations,
                vec![format!("original raw {expected}")]
            );
        }
    }
    #[test]
    fn group_preset_materializes_members_in_selection_order_and_retains_full_group_ranks() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let c = FixtureId::new();
        let position = ProgrammingOwner::Position.key();
        let template = AttributeValue::Position(Arc::new(PositionIntent::Angles {
            pan_degrees: ScalarIntent::Spread(vec![-360.0, 360.0]),
            tilt_degrees: ScalarIntent::Value(0.0),
        }));
        let target = AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [1.0, 2.0, 3.0],
        )));
        let assignment = AttributeValue::GroupFamily(Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template,
            members: BTreeMap::from([(b.0, target.clone())]),
        }));
        let preset = Preset {
            group_values: HashMap::from([(
                "1".into(),
                HashMap::from([(position.clone(), assignment.clone())]),
            )]),
            ..Default::default()
        };
        let groups = HashMap::from([(
            "1".into(),
            light_programmer::GroupDefinition {
                id: "1".into(),
                fixtures: vec![a, b, c],
                ..Default::default()
            },
        )]);
        let planned =
            materialize_preset_fixture_values(&preset, &[c, b], &groups, &HashMap::new()).unwrap();
        let values = planned
            .iter()
            .map(|mutation| match mutation {
                NormalProgrammerValueMutation::SetFixture {
                    fixture_id, value, ..
                } => (*fixture_id, value.clone()),
                _ => panic!("fixture value expected"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                (
                    c,
                    AttributeValue::Position(Arc::new(PositionIntent::angles(360.0, 0.0)))
                ),
                (b, target)
            ]
        );
        let mut invalid = preset.clone();
        invalid
            .universal_values
            .insert(position.clone(), assignment.clone());
        assert!(invalid.validate_programming().is_err());
        invalid = preset.clone();
        invalid
            .values
            .insert(a, HashMap::from([(position, assignment)]));
        assert!(invalid.validate_programming().is_err());
    }
    #[test]
    fn fixture_preset_rejects_unsampled_curves_but_universal_materializes_them() {
        let a = FixtureId::new();
        let b = FixtureId::new();
        let key = ProgrammingOwner::Position.key();
        let value = AttributeValue::Position(Arc::new(PositionIntent::Angles {
            pan_degrees: ScalarIntent::Spread(vec![-360.0, 360.0]),
            tilt_degrees: ScalarIntent::Value(0.0),
        }));
        let mut preset = Preset {
            values: HashMap::from([(a, HashMap::from([(key.clone(), value.clone())]))]),
            ..Default::default()
        };
        assert!(preset.validate_programming().is_err());
        preset.values.clear();
        preset.universal_values.insert(key, value);
        assert!(preset.validate_programming().is_ok());
        let values =
            materialize_preset_fixture_values(&preset, &[b, a], &HashMap::new(), &HashMap::new())
                .unwrap();
        for (mutation, expected) in values.iter().zip([-360.0, 360.0]) {
            let NormalProgrammerValueMutation::SetFixture { value, .. } = mutation else {
                unreachable!()
            };
            assert_eq!(
                value,
                &AttributeValue::Position(Arc::new(PositionIntent::angles(expected, 0.0)))
            );
        }
    }
}
