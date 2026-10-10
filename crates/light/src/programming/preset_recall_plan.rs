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
    let selected = selection.selected.iter().copied().collect::<HashSet<_>>();
    let mut sources = preset.values.keys().copied().collect::<Vec<_>>();
    sources.sort_by_key(|id| id.0);
    for fixture_id in sources {
        append_fixture_values(&mut planned, preset, fixture_id, timing, &selected);
    }
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
            .map(|id| (*id, ranking.rank_by_fixture[id]))
            .collect::<Vec<_>>();
        for attribute in sorted_attributes(&preset.group_values[id]) {
            let members = members
                .iter()
                .copied()
                .filter(|(member, _)| {
                    group_source_selected(preset, id, attribute, *member, &selected)
                })
                .collect::<Vec<_>>();
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
    let projection_order = replacement_value_origins(selection, preset, groups)
        .into_iter()
        .filter_map(|(owner, attribute, map)| {
            let light_core::PresetValueOwner::Fixture { fixture_id } = owner else {
                return None;
            };
            let rank = map
                .get(&fixture_id)
                .into_iter()
                .flat_map(|projection| &projection.targets)
                .filter_map(|target| order.get(&target.fixture_id).copied())
                .min();
            Some(((fixture_id, attribute), rank))
        })
        .collect::<HashMap<_, _>>();
    // Replacement values remain authored on their root, ordered by the first selected explicit
    // destination. Group rank compilation still uses the original membership domain.
    planned.sort_by_key(|mutation| match mutation {
        NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            ..
        }
        | NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
            ..
        } => projection_order
            .get(&(*fixture_id, attribute.clone()))
            .copied()
            .flatten()
            .or_else(|| order.get(fixture_id).copied())
            .unwrap_or(usize::MAX),
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
    selected: &HashSet<FixtureId>,
) {
    let Some(attributes) = preset.values.get(&fixture_id) else {
        return;
    };
    for attribute in sorted_attributes(attributes) {
        if !fixture_source_selected(preset, fixture_id, attribute, selected) {
            continue;
        }
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
mod tests;

#[cfg(test)]
mod family_tests;

/// Provenance follows exactly the recall owner's precedence; it is never inferred from equality.
pub fn preset_value_origins(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    positions: &HashMap<FixtureId, light_dynamics::Position3d>,
) -> Vec<(
    light_core::PresetValueOwner,
    AttributeKey,
    light_core::PresetValueReference,
)> {
    use light_core::{PresetValueOwner as Owner, PresetValueReference};
    let Some(instance) = preset.instance_id else {
        return Vec::new();
    };
    let live = live_group_targets(selection);
    let mut origins = Vec::new();
    let mut append = |target: Owner, source: Owner, attribute: &AttributeKey, rank, member| {
        let reference = PresetValueReference {
            preset_instance_id: instance,
            source_owner: source,
            source_attribute: attribute.clone(),
            sample_rank: rank,
            member_fixture: member,
        };
        origins.retain(|(existing, key, _)| existing != &target || key != attribute);
        origins.push((target, attribute.clone(), reference));
    };
    if live.is_empty() {
        for (rank, fixture) in selection.selected.iter().enumerate() {
            for attribute in sorted_attributes(&preset.universal_values) {
                append(
                    Owner::Fixture {
                        fixture_id: *fixture,
                    },
                    Owner::Universal,
                    attribute,
                    Some((rank, selection.selected.len())),
                    Some(*fixture),
                );
            }
        }
    }
    let selected = selection.selected.iter().copied().collect::<HashSet<_>>();
    let mut fixtures = preset.values.keys().collect::<Vec<_>>();
    fixtures.sort_by_key(|fixture| fixture.0);
    for fixture in fixtures {
        if let Some(values) = preset.values.get(fixture) {
            for attribute in sorted_attributes(values) {
                if !fixture_source_selected(preset, *fixture, attribute, &selected) {
                    continue;
                }
                append(
                    Owner::Fixture {
                        fixture_id: *fixture,
                    },
                    Owner::Fixture {
                        fixture_id: *fixture,
                    },
                    attribute,
                    None,
                    Some(*fixture),
                );
            }
        }
    }
    let selected = selection.selected.iter().copied().collect::<HashSet<_>>();
    let mut group_ids = preset
        .group_values
        .keys()
        .filter(|id| !live.contains(id))
        .collect::<Vec<_>>();
    group_ids.sort();
    for group in group_ids {
        if let Ok(resolved) = light_programmer::resolve_group_spatial(group, groups, positions) {
            let ranking = resolved.ranked_selection;
            for fixture in ranking.ordered_fixture_ids.iter() {
                for attribute in sorted_attributes(&preset.group_values[group]) {
                    if !group_source_selected(preset, group, attribute, *fixture, &selected) {
                        continue;
                    }
                    append(
                        Owner::Fixture {
                            fixture_id: *fixture,
                        },
                        Owner::Group {
                            group_id: group.clone(),
                        },
                        attribute,
                        Some((ranking.rank_by_fixture[fixture], ranking.rank_count)),
                        Some(*fixture),
                    );
                }
            }
        }
    }
    for group in live {
        for attribute in sorted_attributes(&preset.universal_values) {
            append(
                Owner::Group {
                    group_id: group.clone(),
                },
                Owner::Universal,
                attribute,
                None,
                None,
            );
        }
        if let Some(values) = preset.group_values.get(&group) {
            for attribute in sorted_attributes(values) {
                append(
                    Owner::Group {
                        group_id: group.clone(),
                    },
                    Owner::Group {
                        group_id: group.clone(),
                    },
                    attribute,
                    None,
                    None,
                );
            }
        }
    }
    origins
}

fn fixture_source_selected(
    preset: &Preset,
    fixture: FixtureId,
    attribute: &AttributeKey,
    selected: &HashSet<FixtureId>,
) -> bool {
    preset
        .fixture_replacement_projections
        .get(&fixture)
        .and_then(|values| values.get(attribute))
        .map_or_else(
            || selected.contains(&fixture),
            |projection| {
                projection
                    .targets
                    .iter()
                    .any(|target| selected.contains(&target.fixture_id))
            },
        )
}
fn group_source_selected(
    preset: &Preset,
    group: &str,
    attribute: &AttributeKey,
    member: FixtureId,
    selected: &HashSet<FixtureId>,
) -> bool {
    preset
        .group_replacement_projections
        .get(group)
        .and_then(|values| values.get(attribute))
        .and_then(|projections| projections.get(&member))
        .map_or_else(
            || selected.contains(&member),
            |projection| {
                projection
                    .targets
                    .iter()
                    .any(|target| selected.contains(&target.fixture_id))
            },
        )
}

pub(super) fn replacement_value_origins(
    selection: &ProgrammerSelection,
    preset: &Preset,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
) -> Vec<(
    light_core::PresetValueOwner,
    AttributeKey,
    light_core::ReplacementProjectionMap,
)> {
    use light_core::PresetValueOwner as Owner;
    let selected = selection.selected.iter().copied().collect::<HashSet<_>>();
    let live = live_group_targets(selection);
    let mut origins = Vec::new();
    if live.is_empty() {
        for fixture in &selection.selected {
            for attribute in sorted_attributes(&preset.universal_values) {
                origins.push((
                    Owner::Fixture {
                        fixture_id: *fixture,
                    },
                    attribute.clone(),
                    HashMap::new(),
                ));
            }
        }
    }
    let mut fixtures = preset.values.keys().copied().collect::<Vec<_>>();
    fixtures.sort_by_key(|id| id.0);
    for fixture in fixtures {
        for attribute in sorted_attributes(&preset.values[&fixture]) {
            if fixture_source_selected(preset, fixture, attribute, &selected) {
                let projection = preset
                    .fixture_replacement_projections
                    .get(&fixture)
                    .and_then(|values| values.get(attribute))
                    .and_then(|projection| projection.restricted_to(&selected));
                origins.push((
                    Owner::Fixture {
                        fixture_id: fixture,
                    },
                    attribute.clone(),
                    projection
                        .map(|projection| HashMap::from([(fixture, projection)]))
                        .unwrap_or_default(),
                ));
            }
        }
    }
    let mut group_ids = preset.group_values.keys().collect::<Vec<_>>();
    group_ids.sort();
    for group in group_ids {
        let members = light_programmer::resolve_group(group, groups).unwrap_or_default();
        for attribute in sorted_attributes(&preset.group_values[group]) {
            let projections = preset
                .group_replacement_projections
                .get(group)
                .and_then(|values| values.get(attribute));
            if live.contains(group) {
                let map = projections
                    .into_iter()
                    .flat_map(|map| map.iter())
                    .filter(|(member, _)| members.contains(member))
                    .map(|(member, projection)| (*member, projection.clone()))
                    .collect();
                origins.push((
                    Owner::Group {
                        group_id: group.clone(),
                    },
                    attribute.clone(),
                    map,
                ));
            } else {
                for member in &members {
                    if group_source_selected(preset, group, attribute, *member, &selected) {
                        let projection = projections
                            .and_then(|map| map.get(member))
                            .and_then(|projection| projection.restricted_to(&selected));
                        origins.push((
                            Owner::Fixture {
                                fixture_id: *member,
                            },
                            attribute.clone(),
                            projection
                                .map(|projection| HashMap::from([(*member, projection)]))
                                .unwrap_or_default(),
                        ));
                    }
                }
            }
        }
    }
    origins
}
