//! Explicit correspondence attached to an existing authored programming source.
//!
//! This is not parent inheritance. A later direct edit has no correspondence unless it
//! actually recalls a source carrying one. Groups retain their original member identities.
use crate::{FixtureId, Revision};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplacementProfileContext {
    pub profile_id: FixtureId,
    pub profile_revision: Revision,
    pub mode_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplacementHeadTarget {
    pub profile_head_id: Uuid,
    pub fixture_id: FixtureId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplacementProgramProjection {
    pub source_owner: FixtureId,
    pub source_profile: ReplacementProfileContext,
    pub source_head_id: Uuid,
    pub target_profile: ReplacementProfileContext,
    /// Explicit destinations chosen during replacement, never inferred by head index/name.
    /// Empty is the operator's deliberate leave-unmatched/dormant decision, not absence.
    pub targets: Vec<ReplacementHeadTarget>,
}

/// For a live Group source, keys remain original members and rank is sampled before routing.
pub type ReplacementProjectionMap = HashMap<FixtureId, ReplacementProgramProjection>;

/// Transient, generation-scoped consent for already captured runtime sources. This is never
/// persisted on a fixture and never applies to future Programmer writes.
#[derive(Clone, Debug)]
pub struct ReplacementRuntimeMigration {
    pub source_owner: FixtureId,
    pub source_profile: ReplacementProfileContext,
    pub target_profile: ReplacementProfileContext,
    pub root_attributes: HashSet<crate::AttributeKey>,
    pub root_projections: HashMap<crate::AttributeKey, ReplacementProgramProjection>,
    pub head_targets: HashMap<FixtureId, ReplacementHeadTarget>,
    pub source_head_owners: HashMap<Uuid, FixtureId>,
}

impl ReplacementRuntimeMigration {
    /// Migrate a captured source. An existing envelope retains its original authorship, while
    /// its destination context follows explicit replacement consent. Unmatched targets become
    /// dormant. An absent envelope stays absent unless this is an old shared-root contribution.
    pub fn project_existing(
        &self,
        attribute: &crate::AttributeKey,
        owner: FixtureId,
        existing: Option<&ReplacementProgramProjection>,
    ) -> Result<Option<ReplacementProgramProjection>, crate::programming::IntentError> {
        if let Some(existing) = existing {
            existing.validate()?;
            if existing.source_owner != self.source_owner
                || existing.target_profile != self.source_profile
            {
                return Ok(Some(existing.clone()));
            }
            if existing.targets.iter().any(|target| {
                self.source_head_owners.get(&target.profile_head_id) != Some(&target.fixture_id)
            }) {
                return Err(crate::programming::IntentError("replacement source envelope has a stale effective head or physical owner; nothing was changed".into()));
            }
            let mut migrated = existing.clone();
            migrated.target_profile = self.target_profile.clone();
            let mut destinations = Vec::new();
            for target in &existing.targets {
                if target.fixture_id == self.source_owner {
                    let root = self.root_decision(attribute)?;
                    destinations.extend(root.targets.iter().cloned());
                } else if let Some(target) = self.head_targets.get(&target.fixture_id) {
                    destinations.push(target.clone());
                }
            }
            let mut seen = HashSet::new();
            destinations.retain(|target| seen.insert(target.fixture_id));
            migrated.targets = destinations;
            return Ok(Some(migrated));
        }
        if owner != self.source_owner || !self.root_attributes.contains(attribute) {
            return Ok(None);
        }
        Ok(Some(self.root_decision(attribute)?.clone()))
    }

    fn root_decision(
        &self,
        attribute: &crate::AttributeKey,
    ) -> Result<&ReplacementProgramProjection, crate::programming::IntentError> {
        self.root_projections.get(attribute).ok_or_else(|| {
            crate::programming::IntentError(format!(
                    "choose replacement destinations or explicitly leave unmatched for existing root programming at {} on fixture {}",
                    attribute.0, self.source_owner.0,
                ))
        })
    }
}

impl ReplacementProgramProjection {
    pub fn validate(&self) -> Result<(), crate::programming::IntentError> {
        use crate::programming::IntentError;
        if self.source_owner.0.is_nil()
            || self.source_head_id.is_nil()
            || self.source_profile.profile_id.0.is_nil()
            || self.source_profile.mode_id.is_nil()
            || self.target_profile.profile_id.0.is_nil()
            || self.target_profile.mode_id.is_nil()
            || self.source_profile.profile_revision == 0
            || self.target_profile.profile_revision == 0
        {
            return Err(IntentError(
                "replacement projection has an invalid source or profile identity".into(),
            ));
        }
        let mut heads = HashSet::new();
        let mut owners = HashSet::new();
        for target in &self.targets {
            if target.profile_head_id.is_nil()
                || target.fixture_id.0.is_nil()
                || !heads.insert(target.profile_head_id)
                || !owners.insert(target.fixture_id)
            {
                return Err(IntentError(
                    "replacement projection has invalid or duplicate destinations".into(),
                ));
            }
        }
        Ok(())
    }

    /// Recall may select one child of a migrated whole-fixture source. Do not illuminate its
    /// siblings, and do not interpret selection of the root master as selection of children.
    pub fn restricted_to(&self, selected: &HashSet<FixtureId>) -> Option<Self> {
        let mut projection = self.clone();
        projection
            .targets
            .retain(|target| selected.contains(&target.fixture_id));
        (!projection.targets.is_empty()).then_some(projection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> ReplacementProgramProjection {
        ReplacementProgramProjection {
            source_owner: FixtureId::new(),
            source_profile: ReplacementProfileContext {
                profile_id: FixtureId::new(),
                profile_revision: 1,
                mode_id: Uuid::new_v4(),
            },
            source_head_id: Uuid::new_v4(),
            target_profile: ReplacementProfileContext {
                profile_id: FixtureId::new(),
                profile_revision: 2,
                mode_id: Uuid::new_v4(),
            },
            targets: (0..2)
                .map(|_| ReplacementHeadTarget {
                    profile_head_id: Uuid::new_v4(),
                    fixture_id: FixtureId::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn replacement_projection_restricts_recall_without_master_or_sibling_fan_out() {
        let projection = projection();
        let chosen = projection.targets[1].fixture_id;
        let scoped = projection.restricted_to(&HashSet::from([chosen])).unwrap();
        assert_eq!(scoped.targets, vec![projection.targets[1].clone()]);
        assert_eq!(scoped.source_owner, projection.source_owner);
        assert!(
            projection
                .restricted_to(&HashSet::from([projection.source_owner]))
                .is_none()
        );
    }

    #[test]
    fn replacement_projection_rejects_duplicates_and_preserves_explicit_dormancy() {
        let mut projection = projection();
        projection.validate().unwrap();
        let json = serde_json::to_string(&projection).unwrap();
        assert_eq!(
            serde_json::from_str::<ReplacementProgramProjection>(&json).unwrap(),
            projection
        );
        projection.targets.push(projection.targets[0].clone());
        assert!(projection.validate().is_err());
        projection.targets.clear();
        projection.validate().unwrap();
        let json = serde_json::to_string(&projection).unwrap();
        assert!(
            serde_json::from_str::<ReplacementProgramProjection>(&json)
                .unwrap()
                .targets
                .is_empty()
        );
    }
    #[test]
    fn replacement_projection_stale_effective_head_cannot_be_upgraded_by_owner_uuid() {
        let next = projection();
        let mut old = next.clone();
        old.target_profile = old.source_profile.clone();
        old.targets = vec![ReplacementHeadTarget {
            profile_head_id: Uuid::new_v4(),
            fixture_id: old.source_owner,
        }];
        let attribute = crate::AttributeKey::intensity();
        let plan = ReplacementRuntimeMigration {
            source_owner: next.source_owner,
            source_profile: next.source_profile.clone(),
            target_profile: next.target_profile.clone(),
            root_attributes: HashSet::from([attribute.clone()]),
            source_head_owners: HashMap::from([(next.source_head_id, next.source_owner)]),
            root_projections: HashMap::from([(attribute.clone(), next)]),
            head_targets: HashMap::new(),
        };
        assert!(
            plan.project_existing(&attribute, old.source_owner, Some(&old))
                .is_err(),
            "a matching fixture UUID is not proof of the old effective head"
        );
    }
}
