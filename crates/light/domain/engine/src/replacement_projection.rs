mod validation;
pub(crate) use validation::validate_migrations;
// Explicit destinations of an existing authored replacement source. Nothing here changes
// selection, Group membership, or untagged parent/master programming.
use crate::{fixture::profile_mode, profile_projection_plan::ProfileProjectionIndex};
use light_core::{
    AttributeKey, FixtureId, ReplacementProgramProjection, programming::ProgrammingOwner,
};
use light_fixture::PatchedFixture;

/// Cold, immutable capability plan used when compiling an operator-consented replacement.
/// It shares the physical family model with live rendering instead of guessing by slot names.
pub struct ReplacementDestinationPlan {
    fixtures: Vec<PatchedFixture>,
    profiles: ProfileProjectionIndex,
}

impl ReplacementDestinationPlan {
    pub fn compile(fixtures: &[PatchedFixture]) -> Result<Self, crate::EngineError> {
        let snapshot = crate::EngineSnapshot {
            fixtures: fixtures.to_vec().into(),
            ..Default::default()
        };
        Ok(Self {
            profiles: ProfileProjectionIndex::compile(&snapshot)?,
            fixtures: fixtures.to_vec(),
        })
    }

    pub fn destinations(
        &self,
        projection: &ReplacementProgramProjection,
        attribute: &AttributeKey,
    ) -> Vec<FixtureId> {
        destinations(&self.fixtures, &self.profiles, projection, attribute)
    }
}

pub(crate) fn destinations(
    fixtures: &[PatchedFixture],
    profiles: &ProfileProjectionIndex,
    projection: &ReplacementProgramProjection,
    attribute: &AttributeKey,
) -> Vec<FixtureId> {
    if projection.validate().is_err() {
        return Vec::new();
    }
    let Some((_, index)) = profiles.owner(projection.source_owner) else {
        return Vec::new();
    };
    let Some(fixture) = fixtures.get(index) else {
        return Vec::new();
    };
    let Some(profile) = fixture.definition.profile_snapshot.as_deref() else {
        return Vec::new();
    };
    let Some(mode) = profile_mode(fixture) else {
        return Vec::new();
    };
    if profile.id != projection.target_profile.profile_id
        || u64::from(profile.revision) != projection.target_profile.profile_revision
        || mode.id != projection.target_profile.mode_id
    {
        // A later unmatched replacement/deletion keeps stored sources dormant. A profile
        // match by manufacturer, name, channel or index would fabricate correspondence.
        return Vec::new();
    }
    projection
        .targets
        .iter()
        .filter_map(|target| {
            let (head_index, head) = mode
                .heads
                .iter()
                .enumerate()
                .find(|(_, head)| head.id == target.profile_head_id)?;
            let owner = crate::fixture::profile_head_owner(fixture, head_index, head);
            if owner != target.fixture_id {
                return None;
            }
            let family = match attribute.0.as_ref() {
                "color" => Some(ProgrammingOwner::Color),
                "position" => Some(ProgrammingOwner::Position),
                "focus" => Some(ProgrammingOwner::Focus),
                "zoom" => Some(ProgrammingOwner::Zoom),
                _ => None,
            };
            let supported = if let Some(family) = family {
                profiles
                    .family_footprint(fixture, mode, (owner, family), fixture.fixture_id.0)
                    .is_some_and(|channels| !channels.is_empty())
            } else {
                mode.channels.iter().any(|channel| channel.head_id == head.id
                && (&channel.attribute == attribute || &channel.fixture_attribute == attribute
                    || channel.functions.iter().any(|function| &function.attribute == attribute)))
                // An explicitly modeled virtual dimmer controls a real head's optical output.
                || (attribute.is_intensity() && fixture.definition.heads.get(head_index)
                    .is_some_and(|head| head.parameters.iter().any(|parameter|
                        parameter.attribute.is_intensity() && parameter.virtual_dimmer)))
            };
            supported.then_some(owner)
        })
        .collect()
}
