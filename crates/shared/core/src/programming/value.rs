//! Validation shared by mutation, persisted-show and restore boundaries. Native controls are
//! adapter inputs; a complete owner can only be stored at its own semantic address.
use super::{ColorProgram, IntentError, PositionIntent, ProgrammingOwner, ScalarIntent, require};
use crate::{AttributeKey, AttributeValue};

impl AttributeValue {
    /// Fixture physical metadata has its own schema. This version concerns authored values only.
    pub fn required_programming_contract(&self) -> u16 {
        if self.programming_owner().is_some() {
            super::PROGRAMMING_CONTRACT_VERSION
        } else {
            0
        }
    }

    pub fn programming_owner(&self) -> Option<ProgrammingOwner> {
        match self {
            Self::ColorProgram(_) => Some(ProgrammingOwner::Color),
            Self::Position(_) => Some(ProgrammingOwner::Position),
            Self::Zoom(_) => Some(ProgrammingOwner::Zoom),
            Self::GroupFamily(value) => Some(value.owner),
            _ => None,
        }
    }

    /// Existing non-semantic domains keep their current validation. Rich owners always validate
    /// both payload and address so a wrong key cannot silently route a Position as a Color.
    pub fn validate_programming_address(
        &self,
        attribute: &AttributeKey,
    ) -> Result<(), IntentError> {
        if let Some(owner) = self.programming_owner() {
            require(
                attribute.0.as_ref() == owner.id(),
                "complete intent must use its own family address",
            )?;
        }
        match self {
            Self::ColorProgram(value) => value.validate(),
            Self::Position(value) => value.validate(),
            Self::Zoom(value) => value.validate(),
            Self::GroupFamily(value) => value.validate(),
            _ => Ok(()),
        }
    }

    /// Maximum control-point count, including component curves inside an atomic family.
    /// No allocation is needed at the mutation boundary.
    pub fn spread_control_points(&self) -> usize {
        let count = |value: &ScalarIntent| match value {
            ScalarIntent::Value(_) => 0,
            ScalarIntent::Spread(points) => points.len(),
        };
        match self {
            Self::Spread(points) => points.len(),
            Self::ColorProgram(value) => match value.as_ref() {
                ColorProgram::Semantic { intent } => intent
                    .spreads
                    .iter()
                    .map(|s| s.points.len())
                    .max()
                    .unwrap_or(0),
                ColorProgram::Direct { recipe, .. } => recipe
                    .spreads
                    .iter()
                    .map(|s| s.points.len())
                    .max()
                    .unwrap_or(0),
            },
            Self::Position(value) => match value.as_ref() {
                PositionIntent::Angles {
                    pan_degrees,
                    tilt_degrees,
                } => count(pan_degrees).max(count(tilt_degrees)),
                PositionIntent::Target { offset_metres, .. } => {
                    offset_metres.iter().map(count).max().unwrap_or(0)
                }
            },
            Self::Zoom(value) => count(&value.opening_degrees),
            Self::GroupFamily(value) => value.spread_control_points(),
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingValueScope {
    Fixture,
    Universal,
    LiveGroup,
}
impl AttributeValue {
    pub fn validate_programming_scope(
        &self,
        scope: ProgrammingValueScope,
    ) -> Result<(), IntentError> {
        require(
            scope == ProgrammingValueScope::LiveGroup || !matches!(self, Self::GroupFamily(_)),
            "Group family assignments require a live Group owner",
        )?;
        require(
            scope != ProgrammingValueScope::Fixture
                || (!matches!(self, Self::Spread(_)) && self.spread_control_points() == 0),
            "fixture values must be materialized before storage",
        )
    }
}

/// Descriptor-backed family membership for staged legacy/component addresses. Unknown native
/// profile keys require a feature adapter; do not classify arbitrary prefixes as Color.
pub fn independent_programming_component(
    attribute: &AttributeKey,
    owner: ProgrammingOwner,
) -> bool {
    *attribute != owner.key()
        && (super::programming_component(&attribute.0, super::ProgrammingTargetRole::LightHead)
            .is_some_and(|component| component.owner() == owner)
            || (owner == ProgrammingOwner::Color
                && crate::attribute_descriptor(attribute).family == crate::AttributeClass::Color))
}

/// Validate one stored target's values, including mixed complete-owner/component conflicts.
/// Only the newly tagged families participate in this rule during the staged cutover.
pub fn validate_programming_entries<'a>(
    scope: ProgrammingValueScope,
    entries: impl IntoIterator<Item = (&'a AttributeKey, &'a AttributeValue)>,
) -> Result<(), IntentError> {
    let mut owners = 0_u8;
    let mut components = 0_u8;
    let bit = |owner: ProgrammingOwner| match owner {
        ProgrammingOwner::Color => 1,
        ProgrammingOwner::Position => 2,
        ProgrammingOwner::Focus => 4,
        ProgrammingOwner::Zoom => 8,
    };
    for (key, value) in entries {
        value.validate_programming_scope(scope)?;
        value.validate_programming_address(key)?;
        if let Some(owner) = value.programming_owner() {
            owners |= bit(owner);
        } else if let Some(component) =
            super::programming_component(&key.0, super::ProgrammingTargetRole::LightHead)
        {
            components |= bit(component.owner());
        } else if crate::attribute_descriptor(key).family == crate::AttributeClass::Color {
            components |= bit(ProgrammingOwner::Color);
        }
    }
    require(
        owners & components == 0,
        "complete intent cannot coexist with independent components of its family",
    )
}

pub(crate) trait ValidateIntent {
    fn validate_intent(&self) -> Result<(), IntentError>;
}
impl ValidateIntent for ColorProgram {
    fn validate_intent(&self) -> Result<(), IntentError> {
        self.validate()
    }
}
impl ValidateIntent for PositionIntent {
    fn validate_intent(&self) -> Result<(), IntentError> {
        self.validate()
    }
}
impl ValidateIntent for super::ZoomIntent {
    fn validate_intent(&self) -> Result<(), IntentError> {
        self.validate()
    }
}
pub(crate) fn deserialize_intent<'de, D, T>(deserializer: D) -> Result<std::sync::Arc<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + ValidateIntent,
{
    let value = <T as serde::Deserialize>::deserialize(deserializer)?;
    value.validate_intent().map_err(serde::de::Error::custom)?;
    Ok(std::sync::Arc::new(value))
}

/// Validate complete ownership independently for each target in a retained value collection.
pub fn validate_targeted_programming_entries<'a, K: Eq + std::hash::Hash>(
    scope: ProgrammingValueScope,
    entries: impl IntoIterator<Item = (K, &'a AttributeKey, &'a AttributeValue)>,
) -> Result<(), IntentError> {
    let mut targets = std::collections::HashMap::<K, Vec<_>>::new();
    for (target, attribute, value) in entries {
        targets.entry(target).or_default().push((attribute, value));
    }
    for entries in targets.into_values() {
        validate_programming_entries(scope, entries)?;
    }
    Ok(())
}
