use crate::ProgrammerRegistry;
use crate::selection::{SelectionRule, apply_selection_rule};
use crate::state::ProgrammerValueTiming;
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId};
use light_dynamics::{
    Position3d, RankedSelection, SpatialMappingWarning, SpatialSelectionMapping, SpatialTarget,
    evaluate_spatial_mapping,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupProgrammerValue {
    pub value: AttributeValue,
    pub changed_at: DateTime<Utc>,
    #[serde(default)]
    pub programmer_order: u64,
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}
impl<'de> Deserialize<'de> for GroupProgrammerValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Scoped {
                value: AttributeValue,
                changed_at: DateTime<Utc>,
                #[serde(default)]
                programmer_order: u64,
                #[serde(default)]
                fade: bool,
                #[serde(default)]
                fade_millis: Option<u64>,
                #[serde(default)]
                delay_millis: Option<u64>,
            },
            Legacy(AttributeValue),
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Scoped {
                value,
                changed_at,
                programmer_order,
                fade,
                fade_millis,
                delay_millis,
            } => Self {
                value,
                changed_at,
                programmer_order,
                fade,
                fade_millis,
                delay_millis,
            },
            Repr::Legacy(value) => Self {
                value,
                changed_at: Utc::now(),
                programmer_order: 0,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
        })
    }
}
pub(crate) type GroupProgrammerValues =
    HashMap<String, HashMap<AttributeKey, GroupProgrammerValue>>;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GroupDefinition {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub fixtures: Vec<FixtureId>,
    /// Canonical live membership authority. Legacy files without this field are interpreted from
    /// `fixtures` and `derived_from` without changing their observable membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<GroupFixtureSource>,
    /// Optional portable projection-plus-shape mapping. Legacy `grid` state is deliberately not
    /// promoted because it did not affect Group order or value spreading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<SpatialSelectionMapping>,
    pub derived_from: Option<DerivedGroup>,
    pub frozen_from: Option<FrozenGroup>,
    pub programming: HashMap<AttributeKey, AttributeValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupFixtureSource {
    Explicit { fixture_ids: Vec<FixtureId> },
    References { references: Vec<GroupReference> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupReference {
    pub group_id: String,
    pub rule: SelectionRule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DerivedGroup {
    pub source_group_id: String,
    pub rule: SelectionRule,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FrozenGroup {
    pub source_group_id: String,
    pub source_revision: u64,
    pub captured_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupMappingProvenance {
    None,
    Local { group_id: String },
    Inherited { source_group_ids: Vec<String> },
    MixedSourceMappings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedGroup {
    pub source_order: Vec<FixtureId>,
    pub effective_mapping: Option<SpatialSelectionMapping>,
    pub ranked_selection: RankedSelection,
    pub mapping_provenance: GroupMappingProvenance,
}

/// Public evaluation bounds for canonical Group sources. These cover the supported 4,000-fixture
/// stress tier while bounding hostile or accidentally explosive reference graphs.
pub const MAX_GROUP_REFERENCE_DEPTH: usize = 64;
pub const MAX_GROUP_REFERENCE_EVALUATIONS: usize = 4096;
pub const MAX_GROUP_RESOLVED_FIXTURES: usize = 4096;

#[derive(Default)]
struct GroupEvaluationBudget {
    reference_evaluations: usize,
}

impl GroupEvaluationBudget {
    fn count_reference(&mut self) -> Result<(), String> {
        if self.reference_evaluations >= MAX_GROUP_REFERENCE_EVALUATIONS {
            return Err(format!(
                "group reference evaluation limit exceeded: maximum {MAX_GROUP_REFERENCE_EVALUATIONS} references"
            ));
        }
        self.reference_evaluations += 1;
        Ok(())
    }
}

// @tour ordered-selection:20 Resolve a Group without losing emptiness
// An existing Group resolves in stored membership order, including a valid empty result. An
// absent Group is an error, while derived Groups preserve the source's ordered rule.
pub fn resolve_group(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
) -> Result<Vec<FixtureId>, String> {
    resolve_group_membership(
        id,
        groups,
        &mut Vec::new(),
        &mut GroupEvaluationBudget::default(),
        false,
    )
}

pub fn resolve_group_spatial(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<FixtureId, Position3d>,
) -> Result<ResolvedGroup, String> {
    resolve_group_spatial_inner(
        id,
        groups,
        stage_positions,
        &mut Vec::new(),
        &mut GroupEvaluationBudget::default(),
        false,
    )
}

fn resolve_group_membership(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
    stack: &mut Vec<String>,
    budget: &mut GroupEvaluationBudget,
    is_reference: bool,
) -> Result<Vec<FixtureId>, String> {
    push_group(id, groups, stack)?;
    if is_reference {
        budget.count_reference()?;
    }
    let group = groups
        .get(id)
        .ok_or_else(|| format!("group {id} does not exist"))?;
    let resolved = match effective_source(group) {
        GroupFixtureSource::Explicit { fixture_ids } => limited_deduplicate(fixture_ids)?,
        GroupFixtureSource::References { references } => {
            let mut combined = Vec::new();
            let mut seen = HashSet::new();
            for reference in references {
                let source =
                    resolve_group_membership(&reference.group_id, groups, stack, budget, true)?;
                for fixture_id in apply_selection_rule(&source, &reference.rule) {
                    if seen.insert(fixture_id) {
                        ensure_fixture_limit(seen.len())?;
                        combined.push(fixture_id);
                    }
                }
            }
            combined
        }
    };
    stack.pop();
    Ok(resolved)
}

fn resolve_group_spatial_inner(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
    stage_positions: &HashMap<FixtureId, Position3d>,
    stack: &mut Vec<String>,
    budget: &mut GroupEvaluationBudget,
    is_reference: bool,
) -> Result<ResolvedGroup, String> {
    push_group(id, groups, stack)?;
    if is_reference {
        budget.count_reference()?;
    }
    let group = groups
        .get(id)
        .ok_or_else(|| format!("group {id} does not exist"))?;

    let (source_order, inherited_mapping, inherited_from, mixed) = match effective_source(group) {
        GroupFixtureSource::Explicit { fixture_ids } => {
            (limited_deduplicate(fixture_ids)?, None, Vec::new(), false)
        }
        GroupFixtureSource::References { references } => {
            let mut combined = Vec::new();
            let mut seen = HashSet::new();
            let mut evaluated_sources = Vec::new();
            for reference in references {
                let resolved = resolve_group_spatial_inner(
                    &reference.group_id,
                    groups,
                    stage_positions,
                    stack,
                    budget,
                    true,
                )?;
                let selected = apply_selection_rule(
                    &resolved.ranked_selection.ordered_fixture_ids,
                    &reference.rule,
                );
                for fixture_id in selected.iter().copied() {
                    if seen.insert(fixture_id) {
                        ensure_fixture_limit(seen.len())?;
                        combined.push(fixture_id);
                    }
                }
                evaluated_sources.push((
                    reference.group_id,
                    !selected.is_empty(),
                    resolved.effective_mapping,
                ));
            }
            let (mapping, sources, mixed) = inherited_mapping(&evaluated_sources);
            (combined, mapping, sources, mixed)
        }
    };

    let (effective_mapping, mapping_provenance) = if let Some(mapping) = group.mapping.clone() {
        (
            Some(mapping),
            GroupMappingProvenance::Local {
                group_id: id.to_owned(),
            },
        )
    } else if mixed {
        (None, GroupMappingProvenance::MixedSourceMappings)
    } else if let Some(mapping) = inherited_mapping {
        (
            Some(mapping),
            GroupMappingProvenance::Inherited {
                source_group_ids: inherited_from,
            },
        )
    } else {
        (None, GroupMappingProvenance::None)
    };

    let ranked_selection = if let Some(mapping) = effective_mapping.as_ref() {
        let targets = source_order
            .iter()
            .copied()
            .map(|fixture_id| SpatialTarget {
                fixture_id,
                position: stage_positions.get(&fixture_id).copied(),
            })
            .collect::<Vec<_>>();
        evaluate_spatial_mapping(mapping, &targets).map_err(|error| error.to_string())?
    } else {
        source_order_ranks(&source_order)
    };
    stack.pop();
    Ok(ResolvedGroup {
        source_order,
        effective_mapping,
        ranked_selection,
        mapping_provenance,
    })
}

fn effective_source(group: &GroupDefinition) -> GroupFixtureSource {
    group.source.clone().unwrap_or_else(|| {
        group.derived_from.as_ref().map_or_else(
            || GroupFixtureSource::Explicit {
                fixture_ids: group.fixtures.clone(),
            },
            |derived| GroupFixtureSource::References {
                references: vec![GroupReference {
                    group_id: derived.source_group_id.clone(),
                    rule: derived.rule.clone(),
                }],
            },
        )
    })
}

fn push_group(
    id: &str,
    groups: &HashMap<String, GroupDefinition>,
    stack: &mut Vec<String>,
) -> Result<(), String> {
    if !groups.contains_key(id) {
        return Err(format!("group {id} does not exist"));
    }
    if let Some(index) = stack.iter().position(|entry| entry == id) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(id.to_owned());
        return Err(format!(
            "group reference cycle detected: {}",
            cycle.join(" -> ")
        ));
    }
    if stack.len() > MAX_GROUP_REFERENCE_DEPTH {
        return Err(format!(
            "group reference depth limit exceeded while resolving group {id}: maximum depth {MAX_GROUP_REFERENCE_DEPTH}"
        ));
    }
    stack.push(id.to_owned());
    Ok(())
}

fn limited_deduplicate(fixture_ids: Vec<FixtureId>) -> Result<Vec<FixtureId>, String> {
    let mut seen = HashSet::new();
    let mut deduplicated = Vec::new();
    for fixture_id in fixture_ids {
        if seen.insert(fixture_id) {
            ensure_fixture_limit(seen.len())?;
            deduplicated.push(fixture_id);
        }
    }
    Ok(deduplicated)
}

fn ensure_fixture_limit(unique_fixtures: usize) -> Result<(), String> {
    if unique_fixtures > MAX_GROUP_RESOLVED_FIXTURES {
        Err(format!(
            "group resolved fixture limit exceeded: maximum {MAX_GROUP_RESOLVED_FIXTURES} unique fixtures"
        ))
    } else {
        Ok(())
    }
}

fn inherited_mapping(
    sources: &[(String, bool, Option<SpatialSelectionMapping>)],
) -> (Option<SpatialSelectionMapping>, Vec<String>, bool) {
    let considered = if sources.iter().any(|(_, non_empty, _)| *non_empty) {
        sources
            .iter()
            .filter(|(_, non_empty, _)| *non_empty)
            .collect::<Vec<_>>()
    } else {
        sources.iter().collect::<Vec<_>>()
    };
    let Some((_, _, first_mapping)) = considered.first().copied() else {
        return (None, Vec::new(), false);
    };
    if considered
        .iter()
        .any(|(_, _, mapping)| mapping != first_mapping)
    {
        return (None, Vec::new(), true);
    }
    let Some(mapping) = first_mapping.clone() else {
        return (None, Vec::new(), false);
    };
    (
        Some(mapping),
        considered
            .into_iter()
            .map(|(group_id, _, _)| group_id.clone())
            .collect(),
        false,
    )
}

fn source_order_ranks(source_order: &[FixtureId]) -> RankedSelection {
    RankedSelection {
        ordered_fixture_ids: source_order.to_vec(),
        rank_by_fixture: source_order
            .iter()
            .copied()
            .enumerate()
            .map(|(rank, fixture_id)| (fixture_id, rank))
            .collect(),
        rank_count: source_order.len(),
        warnings: Vec::<SpatialMappingWarning>::new(),
    }
}

/// Apply the desk's ordered Group Merge rule: retain the existing membership exactly, then append
/// each previously absent incoming fixture in operator selection order. Duplicate incoming
/// fixtures do not reorder or duplicate an existing member.
pub fn merge_ordered_group_membership(
    existing: &[FixtureId],
    incoming: &[FixtureId],
) -> Vec<FixtureId> {
    let mut merged = existing.to_vec();
    let mut seen = existing.iter().copied().collect::<HashSet<_>>();
    for fixture_id in incoming {
        if seen.insert(*fixture_id) {
            merged.push(*fixture_id);
        }
    }
    merged
}

impl ProgrammerRegistry {
    pub fn set_group(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_group_with_fade(session, group_id, attribute, value, false)
    }
    pub fn set_group_faded(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_group_with_fade(session, group_id, attribute, value, true)
    }
    pub fn set_group_faded_with_timing(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_group_with_timing(
            session,
            group_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade: true,
                fade_millis,
                delay_millis,
            },
        )
    }
    pub fn set_group_immediate_with_delay(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        delay_millis: Option<u64>,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_group_with_timing(
            session,
            group_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade: false,
                fade_millis: None,
                delay_millis,
            },
        )
    }
    fn set_group_with_fade(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
    ) -> bool {
        self.set_group_with_timing(
            session,
            group_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade,
                fade_millis: None,
                delay_millis: None,
            },
        )
    }
    fn set_group_with_timing(
        &self,
        session: SessionId,
        group_id: String,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammerValueTiming,
    ) -> bool {
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        state.checkpoint();
        let programmer_order = self.next_programmer_order();
        let preload = state.blind && state.preload_capture_programmer;
        let target = if preload {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        target.entry(group_id).or_default().insert(
            attribute,
            GroupProgrammerValue {
                value,
                changed_at: self.clock.now(),
                programmer_order,
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            },
        );
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if preload {
            self.mark_preload_values_changed(user_id);
        } else {
            self.mark_normal_values_changed(user_id);
        }
        true
    }

    /// Release exactly one fixture-scoped programmer attribute. Contributions at every other
    /// fixture, Group, and attribute remain intact so resolved output falls back naturally.
    pub fn release_fixture_attribute(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let preload = state.blind && state.preload_capture_programmer;
        let values = if preload {
            &mut state.preload_pending
        } else {
            &mut state.values
        };
        let before = values.len();
        if values
            .iter()
            .all(|value| value.fixture_id != fixture_id || value.attribute != *attribute)
        {
            return false;
        }
        state.checkpoint();
        let values = if preload {
            &mut state.preload_pending
        } else {
            &mut state.values
        };
        values.retain(|value| value.fixture_id != fixture_id || value.attribute != *attribute);
        debug_assert!(values.len() < before);
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if preload {
            self.mark_preload_values_changed(user_id);
        } else {
            self.mark_normal_values_changed(user_id);
        }
        true
    }

    /// Release exactly one Group-scoped programmer attribute. The Group entry itself is removed
    /// only when it has no remaining attributes.
    pub fn release_group_attribute(
        &self,
        session: SessionId,
        group_id: &str,
        attribute: &AttributeKey,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let preload = state.blind && state.preload_capture_programmer;
        let target = if preload {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        if !target
            .get(group_id)
            .is_some_and(|attributes| attributes.contains_key(attribute))
        {
            return false;
        }
        state.checkpoint();
        let target = if preload {
            &mut state.preload_group_pending
        } else {
            &mut state.group_values
        };
        if let Some(attributes) = target.get_mut(group_id) {
            attributes.remove(attribute);
            if attributes.is_empty() {
                target.remove(group_id);
            }
        }
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if preload {
            self.mark_preload_values_changed(user_id);
        } else {
            self.mark_normal_values_changed(user_id);
        }
        true
    }
}

#[cfg(test)]
mod group_resolution_tests {
    use super::*;
    use light_dynamics::{
        Position3d, ProjectionPreset, RankDirection, SpatialProjection, SpatialSelectionShape,
    };
    use uuid::Uuid;

    fn fixture(value: u128) -> FixtureId {
        FixtureId(Uuid::from_u128(value))
    }

    fn explicit(id: &str, fixture_ids: Vec<FixtureId>) -> GroupDefinition {
        GroupDefinition {
            id: id.to_owned(),
            name: id.to_owned(),
            source: Some(GroupFixtureSource::Explicit { fixture_ids }),
            ..GroupDefinition::default()
        }
    }

    fn reference(id: &str, references: Vec<GroupReference>) -> GroupDefinition {
        GroupDefinition {
            id: id.to_owned(),
            name: id.to_owned(),
            source: Some(GroupFixtureSource::References { references }),
            ..GroupDefinition::default()
        }
    }

    fn top_grid(direction: RankDirection) -> SpatialSelectionMapping {
        SpatialSelectionMapping {
            projection: SpatialProjection::from_preset(
                ProjectionPreset::Top,
                Position3d::default(),
            ),
            shape: SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction,
            },
        }
    }

    #[test]
    fn canonical_and_legacy_sources_preserve_order_rules_and_first_occurrence() {
        let first = fixture(1);
        let second = fixture(2);
        let legacy = GroupDefinition {
            id: "legacy".into(),
            name: "Legacy".into(),
            fixtures: vec![first, second, first],
            ..GroupDefinition::default()
        };
        let derived = GroupDefinition {
            id: "derived".into(),
            name: "Derived".into(),
            derived_from: Some(DerivedGroup {
                source_group_id: "legacy".into(),
                rule: SelectionRule::Odd,
            }),
            ..GroupDefinition::default()
        };
        let canonical = reference(
            "canonical",
            vec![
                GroupReference {
                    group_id: "derived".into(),
                    rule: SelectionRule::All,
                },
                GroupReference {
                    group_id: "legacy".into(),
                    rule: SelectionRule::Even,
                },
            ],
        );
        let groups = HashMap::from([
            ("legacy".into(), legacy),
            ("derived".into(), derived),
            ("canonical".into(), canonical),
        ]);

        assert_eq!(resolve_group("legacy", &groups).unwrap(), [first, second]);
        assert_eq!(resolve_group("derived", &groups).unwrap(), [first]);
        assert_eq!(
            resolve_group("canonical", &groups).unwrap(),
            [first, second]
        );
    }

    #[test]
    fn canonical_source_wins_over_disagreeing_legacy_fields() {
        let mut group = explicit("group", vec![fixture(2)]);
        group.fixtures = vec![fixture(1)];
        group.derived_from = Some(DerivedGroup {
            source_group_id: "absent".into(),
            rule: SelectionRule::All,
        });
        assert_eq!(
            resolve_group("group", &HashMap::from([("group".into(), group)])).unwrap(),
            [fixture(2)]
        );
    }

    #[test]
    fn cycles_name_the_complete_reference_path() {
        let groups = HashMap::from([
            (
                "a".into(),
                reference(
                    "a",
                    vec![GroupReference {
                        group_id: "b".into(),
                        rule: SelectionRule::All,
                    }],
                ),
            ),
            (
                "b".into(),
                reference(
                    "b",
                    vec![GroupReference {
                        group_id: "c".into(),
                        rule: SelectionRule::All,
                    }],
                ),
            ),
            (
                "c".into(),
                reference(
                    "c",
                    vec![GroupReference {
                        group_id: "a".into(),
                        rule: SelectionRule::All,
                    }],
                ),
            ),
        ]);
        assert_eq!(
            resolve_group("a", &groups).unwrap_err(),
            "group reference cycle detected: a -> b -> c -> a"
        );
    }

    #[test]
    fn inherited_and_local_mappings_recompute_over_live_membership() {
        let first = fixture(1);
        let second = fixture(2);
        let mut source = explicit("source", vec![first, second]);
        source.mapping = Some(top_grid(RankDirection::Ascending));
        let derived = reference(
            "derived",
            vec![GroupReference {
                group_id: "source".into(),
                rule: SelectionRule::All,
            }],
        );
        let mut local = derived.clone();
        local.id = "local".into();
        local.mapping = Some(top_grid(RankDirection::Descending));
        let groups = HashMap::from([
            ("source".into(), source),
            ("derived".into(), derived),
            ("local".into(), local),
        ]);
        let positions = HashMap::from([
            (
                first,
                Position3d {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
            ),
            (
                second,
                Position3d {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            ),
        ]);

        let inherited = resolve_group_spatial("derived", &groups, &positions).unwrap();
        assert_eq!(
            inherited.ranked_selection.ordered_fixture_ids,
            [second, first]
        );
        assert_eq!(
            inherited.mapping_provenance,
            GroupMappingProvenance::Inherited {
                source_group_ids: vec!["source".into()]
            }
        );

        let local = resolve_group_spatial("local", &groups, &positions).unwrap();
        assert_eq!(local.source_order, [second, first]);
        assert_eq!(local.ranked_selection.ordered_fixture_ids, [first, second]);
        assert_eq!(
            local.mapping_provenance,
            GroupMappingProvenance::Local {
                group_id: "local".into()
            }
        );
    }

    #[test]
    fn mixed_reference_mappings_fall_back_to_concatenated_source_order() {
        let first = fixture(1);
        let second = fixture(2);
        let mut mapped = explicit("mapped", vec![first]);
        mapped.mapping = Some(top_grid(RankDirection::Ascending));
        let plain = explicit("plain", vec![second]);
        let combined = reference(
            "combined",
            vec![
                GroupReference {
                    group_id: "mapped".into(),
                    rule: SelectionRule::All,
                },
                GroupReference {
                    group_id: "plain".into(),
                    rule: SelectionRule::All,
                },
            ],
        );
        let groups = HashMap::from([
            ("mapped".into(), mapped),
            ("plain".into(), plain),
            ("combined".into(), combined),
        ]);

        let resolved = resolve_group_spatial("combined", &groups, &HashMap::new()).unwrap();
        assert_eq!(resolved.effective_mapping, None);
        assert_eq!(
            resolved.ranked_selection.ordered_fixture_ids,
            [first, second]
        );
        assert_eq!(
            resolved.mapping_provenance,
            GroupMappingProvenance::MixedSourceMappings
        );
    }

    #[test]
    fn reference_depth_accepts_64_and_rejects_65_without_masking_other_diagnostics() {
        let mut groups = HashMap::new();
        for depth in 0..MAX_GROUP_REFERENCE_DEPTH {
            groups.insert(
                depth.to_string(),
                reference(
                    &depth.to_string(),
                    vec![GroupReference {
                        group_id: (depth + 1).to_string(),
                        rule: SelectionRule::All,
                    }],
                ),
            );
        }
        groups.insert(
            MAX_GROUP_REFERENCE_DEPTH.to_string(),
            explicit(&MAX_GROUP_REFERENCE_DEPTH.to_string(), vec![fixture(1)]),
        );
        assert_eq!(resolve_group("0", &groups).unwrap(), [fixture(1)]);

        groups.insert(
            MAX_GROUP_REFERENCE_DEPTH.to_string(),
            reference(
                &MAX_GROUP_REFERENCE_DEPTH.to_string(),
                vec![GroupReference {
                    group_id: (MAX_GROUP_REFERENCE_DEPTH + 1).to_string(),
                    rule: SelectionRule::All,
                }],
            ),
        );
        groups.insert(
            (MAX_GROUP_REFERENCE_DEPTH + 1).to_string(),
            explicit(
                &(MAX_GROUP_REFERENCE_DEPTH + 1).to_string(),
                vec![fixture(1)],
            ),
        );
        assert_eq!(
            resolve_group("0", &groups).unwrap_err(),
            format!(
                "group reference depth limit exceeded while resolving group {}: maximum depth {MAX_GROUP_REFERENCE_DEPTH}",
                MAX_GROUP_REFERENCE_DEPTH + 1
            )
        );

        assert_eq!(
            resolve_group("missing", &HashMap::<String, GroupDefinition>::new()).unwrap_err(),
            "group missing does not exist"
        );
    }

    #[test]
    fn reference_evaluation_count_accepts_4096_and_rejects_the_next_reference() {
        let references = (0..MAX_GROUP_REFERENCE_EVALUATIONS)
            .map(|_| GroupReference {
                group_id: "leaf".into(),
                rule: SelectionRule::All,
            })
            .collect::<Vec<_>>();
        let mut groups = HashMap::from([
            ("leaf".into(), explicit("leaf", vec![fixture(1)])),
            ("root".into(), reference("root", references.clone())),
        ]);
        assert_eq!(resolve_group("root", &groups).unwrap(), [fixture(1)]);

        let mut overflowing = references;
        overflowing.push(GroupReference {
            group_id: "leaf".into(),
            rule: SelectionRule::All,
        });
        groups.insert("root".into(), reference("root", overflowing));
        assert_eq!(
            resolve_group_spatial("root", &groups, &HashMap::new()).unwrap_err(),
            format!(
                "group reference evaluation limit exceeded: maximum {MAX_GROUP_REFERENCE_EVALUATIONS} references"
            )
        );
    }

    #[test]
    fn resolved_fixture_count_accepts_4096_and_rejects_the_next_unique_fixture() {
        let fixtures = (1..=MAX_GROUP_RESOLVED_FIXTURES)
            .map(|value| fixture(value as u128))
            .collect::<Vec<_>>();
        let mut group = explicit("group", fixtures.clone());
        let mut groups = HashMap::from([("group".into(), group.clone())]);
        assert_eq!(resolve_group("group", &groups).unwrap(), fixtures);

        group
            .source
            .as_mut()
            .and_then(|source| match source {
                GroupFixtureSource::Explicit { fixture_ids } => Some(fixture_ids),
                GroupFixtureSource::References { .. } => None,
            })
            .unwrap()
            .push(fixture((MAX_GROUP_RESOLVED_FIXTURES + 1) as u128));
        groups.insert("group".into(), group);
        assert_eq!(
            resolve_group_spatial("group", &groups, &HashMap::new()).unwrap_err(),
            format!(
                "group resolved fixture limit exceeded: maximum {MAX_GROUP_RESOLVED_FIXTURES} unique fixtures"
            )
        );
    }
}
