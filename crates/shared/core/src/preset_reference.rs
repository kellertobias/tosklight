//! Stable provenance for an individual value recalled from a portable Preset.
use crate::{AttributeKey, FixtureId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PresetValueOwner {
    Universal,
    Fixture { fixture_id: FixtureId },
    Group { group_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PresetValueReference {
    /// Immutable instance identity; a pool number may be reused or moved.
    pub preset_instance_id: Uuid,
    pub source_owner: PresetValueOwner,
    pub source_attribute: AttributeKey,
    /// The original sampling domain for a frozen selection. Live Group values leave this unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rank: Option<(usize, usize)>,
    /// Group-family members are addressed by fixture identity, independently of rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_fixture: Option<FixtureId>,
}

impl PresetValueReference {
    pub fn validate(
        &self,
        target_attribute: &AttributeKey,
    ) -> Result<(), crate::programming::IntentError> {
        use crate::programming::IntentError;
        if self.preset_instance_id.is_nil()
            || self.source_attribute.0.is_empty()
            || &self.source_attribute != target_attribute
        {
            return Err(IntentError(
                "live Preset reference has an invalid identity or source address".into(),
            ));
        }
        if self
            .sample_rank
            .is_some_and(|(rank, count)| count == 0 || rank >= count)
        {
            return Err(IntentError(
                "live Preset reference has an invalid sampling rank".into(),
            ));
        }
        if matches!(&self.source_owner, PresetValueOwner::Group { group_id } if group_id.is_empty())
        {
            return Err(IntentError(
                "live Preset reference has an empty Group identity".into(),
            ));
        }
        Ok(())
    }
}
