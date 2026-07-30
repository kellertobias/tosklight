use crate::{
    DerivedGroup, GridMethodConfiguration, GroupDefinition, ProgrammerRegistry,
    ProgrammerSelection, SelectionExpression, SelectionReference, SelectionRule,
    merge_ordered_group_membership, resolve_group,
};
use light_core::{FixtureId, SessionId};
use std::collections::{HashMap, HashSet};

/// Action-time, desk-local selection captured for one Group recording operation.
///
/// Programmer values, Preload, timing, Highlight, modes, and transient state are deliberately
/// absent. Relationship metadata is retained only when the complete selection has one supported
/// source; mixed and subtractive expressions are materialized as concrete ordered fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupRecordingCapture {
    fixtures: Vec<FixtureId>,
    relationship: GroupRecordingRelationship,
    grid: GridMethodConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GroupRecordingRelationship {
    Materialized,
    Live {
        source_group_id: String,
        rule: SelectionRule,
    },
}

impl GroupRecordingCapture {
    pub fn fixtures(&self) -> &[FixtureId] {
        &self.fixtures
    }

    /// Overwrite while retaining a valid single-source Group relationship.
    ///
    /// A self-reference, missing source, or transitive live cycle is materialized. Dereferenced
    /// selections are already concrete ordered fixtures and record without any source reference.
    pub fn overwrite(
        &self,
        group_id: &str,
        existing: Option<&GroupDefinition>,
        groups: &HashMap<String, GroupDefinition>,
    ) -> GroupDefinition {
        let mut group = existing.cloned().unwrap_or_else(|| new_group(group_id));
        group.id = group_id.to_owned();
        group.fixtures.clone_from(&self.fixtures);
        group.grid = self.grid;
        group.derived_from = None;
        group.frozen_from = None;
        self.apply_relationship(&mut group, groups);
        group
    }

    /// Merge always materializes the target's currently resolved ordered membership.
    pub fn merge(
        &self,
        group_id: &str,
        existing: &GroupDefinition,
        groups: &HashMap<String, GroupDefinition>,
    ) -> Result<GroupDefinition, String> {
        let membership = resolve_group(group_id, groups)?;
        let mut group = existing.clone();
        group.id = group_id.to_owned();
        group.fixtures = merge_ordered_group_membership(&membership, &self.fixtures);
        materialize(&mut group);
        Ok(group)
    }

    /// Subtract always materializes the target while preserving remaining relative order.
    pub fn subtract(
        &self,
        group_id: &str,
        existing: &GroupDefinition,
        groups: &HashMap<String, GroupDefinition>,
    ) -> Result<GroupDefinition, String> {
        let mut membership = resolve_group(group_id, groups)?;
        let removed = self.fixtures.iter().copied().collect::<HashSet<_>>();
        membership.retain(|fixture_id| !removed.contains(fixture_id));
        let mut group = existing.clone();
        group.id = group_id.to_owned();
        group.fixtures = membership;
        materialize(&mut group);
        Ok(group)
    }

    fn from_selection(selection: ProgrammerSelection) -> Self {
        Self {
            fixtures: ordered_unique(selection.selected),
            relationship: relationship(selection.expression),
            grid: selection.grid.configuration,
        }
    }

    fn apply_relationship(
        &self,
        group: &mut GroupDefinition,
        groups: &HashMap<String, GroupDefinition>,
    ) {
        match &self.relationship {
            GroupRecordingRelationship::Materialized => {}
            GroupRecordingRelationship::Live {
                source_group_id,
                rule,
            } if source_group_id != &group.id => {
                preserve_live_relationship(group, source_group_id, rule, groups);
            }
            GroupRecordingRelationship::Live { .. } => {}
        }
    }
}

impl ProgrammerRegistry {
    /// Capture only the authoritative desk-local selection for Group recording.
    ///
    /// The Programming application boundary must hold the user-then-desk serialization gates.
    pub fn capture_group_recording_selection(
        &self,
        session: SessionId,
    ) -> Option<GroupRecordingCapture> {
        self.selection(session)
            .map(GroupRecordingCapture::from_selection)
    }

    /// Close a recording gesture when the caller already owns the Programming interaction.
    /// This avoids acquiring a second desk boundary while retaining the registry's reentrant user
    /// serialization for direct domain callers.
    pub fn finish_selection_gesture_within_interaction(&self, session: SessionId) -> bool {
        self.close_selection_gesture(session)
    }
}

/// Return the first direct live dependency that prevents safe deletion.
pub fn group_delete_blocker<'a>(
    group_id: &str,
    groups: &'a HashMap<String, GroupDefinition>,
) -> Option<&'a str> {
    groups
        .values()
        .filter(|group| {
            group
                .derived_from
                .as_ref()
                .is_some_and(|derived| derived.source_group_id == group_id)
        })
        .map(|group| group.id.as_str())
        .min()
}

fn new_group(group_id: &str) -> GroupDefinition {
    GroupDefinition {
        id: group_id.to_owned(),
        name: format!("Group {group_id}"),
        ..GroupDefinition::default()
    }
}

fn materialize(group: &mut GroupDefinition) {
    group.derived_from = None;
    group.frozen_from = None;
}

fn ordered_unique(fixtures: Vec<FixtureId>) -> Vec<FixtureId> {
    let mut seen = HashSet::new();
    fixtures
        .into_iter()
        .filter(|fixture_id| seen.insert(*fixture_id))
        .collect()
}

fn relationship(expression: Option<SelectionExpression>) -> GroupRecordingRelationship {
    match expression {
        Some(SelectionExpression::LiveGroup { group_id, rule }) => {
            GroupRecordingRelationship::Live {
                source_group_id: group_id,
                rule,
            }
        }
        Some(SelectionExpression::Sources { items }) => single_live_source(items),
        _ => GroupRecordingRelationship::Materialized,
    }
}

fn single_live_source(items: Vec<SelectionReference>) -> GroupRecordingRelationship {
    match items.as_slice() {
        [SelectionReference::LiveGroup { group_id }] => GroupRecordingRelationship::Live {
            source_group_id: group_id.clone(),
            rule: SelectionRule::All,
        },
        _ => GroupRecordingRelationship::Materialized,
    }
}

fn preserve_live_relationship(
    group: &mut GroupDefinition,
    source_group_id: &str,
    rule: &SelectionRule,
    groups: &HashMap<String, GroupDefinition>,
) {
    group.derived_from = Some(DerivedGroup {
        source_group_id: source_group_id.to_owned(),
        rule: rule.clone(),
    });
    let mut candidate = groups.clone();
    candidate.insert(group.id.clone(), group.clone());
    if resolve_group(&group.id, &candidate).is_err() {
        group.derived_from = None;
    }
}
