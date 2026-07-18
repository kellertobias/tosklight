use crate::EngineSnapshot;
use light_core::{AttributeKey, FixtureId};
use light_fixture::{FixtureMode, PatchedFixture};

pub(crate) fn profile_mode(fixture: &PatchedFixture) -> Option<&FixtureMode> {
    if fixture.definition.schema_version != light_fixture::FIXTURE_PROFILE_SCHEMA_VERSION {
        return None;
    }
    let profile = fixture.definition.profile_snapshot.as_deref()?;
    profile.mode(fixture.definition.mode_id?)
}

pub(crate) fn profile_head_owner(
    fixture: &PatchedFixture,
    head_index: usize,
    head: &light_fixture::FixtureHead,
) -> FixtureId {
    if head.master_shared {
        return fixture.fixture_id;
    }
    let persisted_head_index = fixture
        .definition
        .heads
        .get(head_index)
        .map(|head| head.index)
        .unwrap_or(head_index as u16);
    fixture
        .logical_heads
        .iter()
        .find(|patched| patched.head_index == persisted_head_index)
        .or_else(|| {
            fixture
                .logical_heads
                .iter()
                .find(|patched| usize::from(patched.head_index) == head_index)
        })
        .or_else(|| {
            fixture
                .logical_heads
                .iter()
                .find(|patched| usize::from(patched.head_index) == head_index + 1)
        })
        .map(|patched| patched.fixture_id)
        .unwrap_or(fixture.fixture_id)
}

pub(crate) fn snapshot_attribute_is_snap(
    snapshot: &EngineSnapshot,
    fixture_id: FixtureId,
    attribute: &AttributeKey,
) -> bool {
    snapshot.fixtures.iter().any(|fixture| {
        let Some(mode) = profile_mode(fixture) else {
            return false;
        };
        mode.heads.iter().enumerate().any(|(head_index, head)| {
            profile_head_owner(fixture, head_index, head) == fixture_id
                && mode.head_attribute_is_snap(head.id, attribute)
        })
    })
}
