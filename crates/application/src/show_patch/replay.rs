use super::{PatchFixturesCommand, PatchFixturesResult};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_fixture::{MultiPatchInstance, PatchedFixturePatch};
use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
    sync::Arc,
};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;
const REQUEST_CACHE_BYTE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    desk_id: Uuid,
    session_id: Option<Uuid>,
    request_id: String,
}

impl ReplayKey {
    pub(super) fn from_context(context: &ActionContext) -> Option<Self> {
        Some(Self {
            desk_id: context.desk_id,
            session_id: context.session_id,
            request_id: context.request_id.clone()?,
        })
    }

    pub(super) fn request_id(&self) -> &str {
        &self.request_id
    }
}

#[derive(Clone)]
struct ReplayEntry {
    expected_revision: Option<u64>,
    command: Arc<PatchFixturesCommand>,
    result: Arc<PatchFixturesResult>,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct ReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
}

impl ReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        context: &ActionContext,
        command: &PatchFixturesCommand,
    ) -> Result<Option<PatchFixturesResult>, ActionError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.expected_revision != context.expected_revision || entry.command.as_ref() != command
        {
            return Err(request_collision());
        }
        let mut replay = entry.result.as_ref().clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        context: &ActionContext,
        command: PatchFixturesCommand,
        result: PatchFixturesResult,
    ) {
        let retained_bytes = retained_bytes(&command, &result);
        if retained_bytes > REQUEST_CACHE_BYTE_LIMIT {
            return;
        }
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(previous) = self.entries.insert(
            key,
            ReplayEntry {
                expected_revision: context.expected_revision,
                command: Arc::new(command),
                result: Arc::new(result),
                retained_bytes,
            },
        ) {
            self.retained_bytes = self.retained_bytes.saturating_sub(previous.retained_bytes);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        self.truncate();
    }

    fn truncate(&mut self) {
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT
            || self.retained_bytes > REQUEST_CACHE_BYTE_LIMIT
        {
            if let Some(oldest) = self.order.pop_front()
                && let Some(entry) = self.entries.remove(&oldest)
            {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.retained_bytes);
            }
        }
    }
}

fn retained_bytes(command: &PatchFixturesCommand, result: &PatchFixturesResult) -> usize {
    size_of::<ReplayEntry>()
        .saturating_add(command_bytes(command))
        .saturating_add(result_bytes(result))
}

fn command_bytes(command: &PatchFixturesCommand) -> usize {
    command
        .fixtures
        .iter()
        .map(|fixture| size_of_val_and_patch(&fixture.patch))
        .sum::<usize>()
        .saturating_add(command.fixtures.capacity() * size_of::<super::PatchFixtureCandidate>())
        .saturating_add(command.remove_fixture_ids.capacity() * size_of::<light_core::FixtureId>())
}

fn result_bytes(result: &PatchFixturesResult) -> usize {
    let change = &result.change;
    let fixtures = change
        .fixtures
        .iter()
        .map(|fixture| size_of_val_and_patch(&fixture.patch))
        .sum::<usize>();
    let profiles = change
        .profile_revisions
        .iter()
        .map(|profile| {
            profile.content_digest.capacity()
                + profile.manufacturer.capacity()
                + profile.name.capacity()
                + profile.fixture_type.capacity()
                + profile
                    .referenced_modes
                    .iter()
                    .map(|mode| {
                        mode.name.capacity()
                            + mode.splits.capacity() * size_of::<light_fixture::FixtureSplit>()
                    })
                    .sum::<usize>()
        })
        .sum::<usize>();
    size_of::<PatchFixturesResult>()
        .saturating_add(result.request_id.capacity())
        .saturating_add(fixtures)
        .saturating_add(profiles)
        .saturating_add(change.fixtures.capacity() * size_of::<super::PatchFixtureProjection>())
        .saturating_add(change.removed_fixture_ids.capacity() * size_of::<light_core::FixtureId>())
        .saturating_add(
            change.profile_revisions.capacity()
                * size_of::<super::PatchProfileRevisionProjection>(),
        )
}

fn size_of_val_and_patch(patch: &PatchedFixturePatch) -> usize {
    size_of::<PatchedFixturePatch>()
        .saturating_add(patch.name.capacity())
        .saturating_add(patch.layer_id.capacity())
        .saturating_add(patch.split_patches.capacity() * size_of::<light_fixture::SplitPatch>())
        .saturating_add(patch.logical_heads.capacity() * size_of::<light_fixture::PatchedHead>())
        .saturating_add(patch.multipatch.iter().map(multipatch_bytes).sum::<usize>())
        .saturating_add(patch.multipatch.capacity() * size_of::<MultiPatchInstance>())
        .saturating_add(
            patch.highlight_overrides.len() * (size_of::<(Uuid, u32)>() + 4 * size_of::<usize>()),
        )
}

fn multipatch_bytes(instance: &MultiPatchInstance) -> usize {
    instance
        .name
        .capacity()
        .saturating_add(instance.split_patches.capacity() * size_of::<light_fixture::SplitPatch>())
}

fn request_collision() -> ActionError {
    ActionError::new(
        ActionErrorKind::Conflict,
        "request_id was already used for a different patch operation or expected revision",
    )
}
