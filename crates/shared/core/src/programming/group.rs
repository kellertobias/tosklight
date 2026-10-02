//! A live Group keeps one template and complete per-member exceptions. Exceptions stay scoped
//! to membership, so adding/removing a lamp cannot turn retained intent into orphan fixture writes.
use super::*;
use crate::{AttributeValue, FixtureId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupFamilyAssignment {
    pub owner: ProgrammingOwner,
    pub template: AttributeValue,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub members: BTreeMap<Uuid, AttributeValue>,
}
impl GroupFamilyAssignment {
    pub fn validate(&self) -> Result<(), IntentError> {
        require(
            self.members.len() <= 10_000,
            "too many Group member exceptions",
        )?;
        self.validate_member(&self.template)?;
        for (fixture, value) in &self.members {
            require(
                !fixture.is_nil(),
                "Group member must have a stable fixture identity",
            )?;
            self.validate_member(value)?;
        }
        Ok(())
    }
    fn validate_member(&self, value: &AttributeValue) -> Result<(), IntentError> {
        require(
            !matches!(value, AttributeValue::GroupFamily(_)),
            "Group family assignments cannot nest",
        )?;
        value.validate_programming_address(&self.owner.key())?;
        if self.owner == ProgrammingOwner::Focus {
            match value {
                AttributeValue::Normalized(value) => {
                    ScalarIntent::Value(*value).validate(ScalarDomain::UNIT)
                }
                AttributeValue::Spread(points) => {
                    ScalarIntent::Spread(points.clone()).validate(ScalarDomain::UNIT)
                }
                _ => Err(IntentError(
                    "Focus Group members must be normalized values".into(),
                )),
            }
        } else {
            require(
                value.programming_owner() == Some(self.owner),
                "Group member must contain its complete declared family",
            )
        }
    }
    /// Call only for a fixture in the currently resolved Group membership. Rank evaluation is a
    /// subsequent operation and therefore preserves different bases at equal spatial ranks.
    pub fn for_member(&self, fixture: FixtureId) -> &AttributeValue {
        self.members.get(&fixture.0).unwrap_or(&self.template)
    }
    pub fn spread_control_points(&self) -> usize {
        self.members
            .values()
            .map(AttributeValue::spread_control_points)
            .fold(self.template.spread_control_points(), usize::max)
    }
    pub fn remove_redundant_exceptions(&mut self) {
        self.members.retain(|_, value| *value != self.template);
    }
}

impl super::value::ValidateIntent for GroupFamilyAssignment {
    fn validate_intent(&self) -> Result<(), IntentError> {
        self.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn member_exceptions_retain_different_target_frames_and_new_members_use_template() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let target = |point| {
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Point { point_id: point },
                [1.0, 2.0, 3.0],
            )))
        };
        let template = AttributeValue::Position(Arc::new(PositionIntent::angles(20.0, 30.0)));
        let assignment = GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: template.clone(),
            members: BTreeMap::from([
                (first.0, target(Uuid::new_v4())),
                (second.0, target(Uuid::new_v4())),
            ]),
        };
        assignment.validate().unwrap();
        assert_ne!(assignment.for_member(first), assignment.for_member(second));
        assert_eq!(assignment.for_member(FixtureId::new()), &template);
        let value = AttributeValue::GroupFamily(Arc::new(assignment));
        assert_eq!(
            serde_json::from_value::<AttributeValue>(serde_json::to_value(&value).unwrap())
                .unwrap(),
            value
        );
    }
    #[test]
    fn mixed_owners_and_nested_assignments_are_rejected() {
        let mut assignment = GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0))),
            members: BTreeMap::new(),
        };
        assignment
            .members
            .insert(Uuid::new_v4(), AttributeValue::Normalized(0.5));
        assert!(assignment.validate().is_err());
        assignment.members.clear();
        assignment.members.insert(
            Uuid::new_v4(),
            AttributeValue::GroupFamily(Arc::new(assignment.clone())),
        );
        assert!(assignment.validate().is_err());
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn valid_group_payload_is_rejected_at_independent_owners() {
        let value = AttributeValue::GroupFamily(std::sync::Arc::new(GroupFamilyAssignment {
            owner: ProgrammingOwner::Position,
            template: AttributeValue::Position(std::sync::Arc::new(PositionIntent::angles(
                0.0, 0.0,
            ))),
            members: BTreeMap::new(),
        }));
        let key = ProgrammingOwner::Position.key();
        assert!(
            validate_programming_entries(ProgrammingValueScope::LiveGroup, [(&key, &value)])
                .is_ok()
        );
        assert!(
            validate_programming_entries(ProgrammingValueScope::Fixture, [(&key, &value)]).is_err()
        );
    }
}
