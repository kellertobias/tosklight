use super::values_replay_memory::{
    ENTRY_CONTAINER_OVERHEAD, ReplayLimits, preload_projection_retained_bytes,
    preload_queue_projection_retained_bytes,
};
use crate::{
    ActionError, ActionErrorKind, ProgrammingPreloadLifecycleRequest,
    ProgrammingPreloadLifecycleResult,
};
use light_core::{SessionId, UserId};
use std::{
    collections::{HashMap, VecDeque},
    mem::size_of,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    user_id: UserId,
    desk_id: uuid::Uuid,
    session_id: SessionId,
    request_id: String,
}

pub(super) struct PreloadLifecycleReplayIdentity {
    pub(super) user_id: UserId,
    pub(super) desk_id: uuid::Uuid,
    pub(super) session_id: SessionId,
    pub(super) request_id: String,
}

struct ReplayEntry {
    request: ProgrammingPreloadLifecycleRequest,
    result: ProgrammingPreloadLifecycleResult,
    retained_bytes: usize,
}

#[derive(Default)]
pub(super) struct PreloadLifecycleReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
    retained_bytes: usize,
    limits: ReplayLimits,
}

impl PreloadLifecycleReplayCache {
    pub(super) fn get(
        &self,
        identity: &PreloadLifecycleReplayIdentity,
        request: &ProgrammingPreloadLifecycleRequest,
    ) -> Result<Option<ProgrammingPreloadLifecycleResult>, ActionError> {
        let key = key(identity);
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.request != *request {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Preload lifecycle action",
            ));
        }
        let mut result = entry.result.clone();
        result.replayed = true;
        Ok(Some(result))
    }

    pub(super) fn insert(
        &mut self,
        identity: PreloadLifecycleReplayIdentity,
        request: ProgrammingPreloadLifecycleRequest,
        result: ProgrammingPreloadLifecycleResult,
    ) {
        let key = key(&identity);
        let retained_bytes = retained_entry_bytes(&key, &request, &result);
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        if let Some(previous) = self.entries.insert(
            key,
            ReplayEntry {
                request,
                result,
                retained_bytes,
            },
        ) {
            self.retained_bytes = self.retained_bytes.saturating_sub(previous.retained_bytes);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(retained_bytes);
        self.truncate();
    }

    fn truncate(&mut self) {
        while self.entries.len() > self.limits.entries || self.retained_bytes > self.limits.bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.retained_bytes = self.retained_bytes.saturating_sub(entry.retained_bytes);
            }
        }
    }

    #[cfg(test)]
    fn with_limits(entries: usize, bytes: usize) -> Self {
        Self {
            limits: ReplayLimits { entries, bytes },
            ..Self::default()
        }
    }
}

fn key(identity: &PreloadLifecycleReplayIdentity) -> ReplayKey {
    ReplayKey {
        user_id: identity.user_id,
        desk_id: identity.desk_id,
        session_id: identity.session_id,
        request_id: identity.request_id.clone(),
    }
}

fn retained_entry_bytes(
    key: &ReplayKey,
    request: &ProgrammingPreloadLifecycleRequest,
    result: &ProgrammingPreloadLifecycleResult,
) -> usize {
    let mut bytes = ENTRY_CONTAINER_OVERHEAD
        .saturating_add(2 * size_of::<ReplayKey>())
        .saturating_add(key.request_id.capacity() + key.request_id.len())
        .saturating_add(size_of::<ReplayEntry>())
        .saturating_add(size_of_val(request))
        .saturating_add(result.request_id.capacity())
        .saturating_add(result.warning.as_ref().map_or(0, String::capacity));
    if let Some(projection) = &result.values_projection {
        bytes = bytes.saturating_add(preload_projection_retained_bytes(projection));
    }
    if let Some(projection) = &result.queue_projection {
        bytes = bytes.saturating_add(preload_queue_projection_retained_bytes(projection));
    }
    if let Some(commit) = &result.commit {
        bytes = bytes
            .saturating_add(commit.warnings.capacity() * size_of::<String>())
            .saturating_add(commit.warnings.iter().map(String::capacity).sum::<usize>())
            .saturating_add(
                commit.executed.capacity()
                    * size_of::<crate::ProgrammingPreloadExecutedPlaybackAction>(),
            )
            .saturating_add(
                commit.runtime_changes.capacity()
                    * size_of::<crate::ProgrammingPreloadRuntimeChange>(),
            );
        for change in &commit.runtime_changes {
            bytes = bytes.saturating_add(runtime_projection_retained_bytes(&change.projection));
        }
    }
    bytes
}

fn runtime_projection_retained_bytes(projection: &crate::PlaybackRuntimeProjection) -> usize {
    use crate::{PlaybackRuntimeIdentity, PlaybackTargetProjection};
    let mut bytes = size_of_val(projection);
    if let PlaybackRuntimeIdentity::Group(group) = &projection.requested {
        bytes = bytes.saturating_add(group.as_str().len());
    }
    bytes.saturating_add(match &projection.target {
        PlaybackTargetProjection::CueList {
            runtime: Some(runtime),
            ..
        } => size_of_val(runtime.as_ref()),
        PlaybackTargetProjection::Group { group_id, .. } => group_id.capacity(),
        PlaybackTargetProjection::SpeedGroup { group, runtime } => {
            group.capacity() + size_of_val(runtime.as_ref())
        }
        PlaybackTargetProjection::Missing
        | PlaybackTargetProjection::CueList { runtime: None, .. }
        | PlaybackTargetProjection::GrandMaster(_)
        | PlaybackTargetProjection::ProgrammerFade { .. }
        | PlaybackTargetProjection::CueFade { .. } => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionContext, ActionSource, PlaybackRuntimeIdentity, PlaybackRuntimeProjection,
        PlaybackShowScope, PlaybackTargetProjection, ProgrammingCaptureModeProjection,
        ProgrammingPreloadCommitResult, ProgrammingPreloadLifecycleAction,
        ProgrammingPreloadLifecycleState, ProgrammingPreloadRevisionExpectation,
        ProgrammingPreloadRuntimeChange,
    };
    use light_core::{ShowId, UserId};
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn byte_budget_evicts_deep_runtime_projection_strings() {
        let user_id = UserId::new();
        let desk_id = Uuid::new_v4();
        let session_id = SessionId::new();
        let first = result(user_id, desk_id, session_id, "first", 512 * 1024);
        let second = result(user_id, desk_id, session_id, "second", 512 * 1024);
        let first_key = replay_key(user_id, desk_id, session_id, "first");
        let second_key = replay_key(user_id, desk_id, session_id, "second");
        let request = request();
        let budget = retained_entry_bytes(&first_key, &request, &first)
            + retained_entry_bytes(&second_key, &request, &second)
            - 1;
        let mut cache = PreloadLifecycleReplayCache::with_limits(10, budget);

        cache.insert(
            identity(user_id, desk_id, session_id, "first"),
            request.clone(),
            first,
        );
        cache.insert(
            identity(user_id, desk_id, session_id, "second"),
            request.clone(),
            second,
        );

        assert_eq!(cache.entries.len(), 1);
        assert!(cache.retained_bytes <= budget);
        assert!(
            cache
                .get(&identity(user_id, desk_id, session_id, "first"), &request)
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .get(&identity(user_id, desk_id, session_id, "second"), &request)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn retained_size_includes_warning_vector_storage_and_fixed_allocation_headers() {
        let user_id = UserId::new();
        let desk_id = Uuid::new_v4();
        let session_id = SessionId::new();
        let request = request();
        let key = replay_key(user_id, desk_id, session_id, "warnings");
        let mut outcome = result(user_id, desk_id, session_id, "warnings", 0);
        let baseline = retained_entry_bytes(&key, &request, &outcome);
        outcome.commit.as_mut().unwrap().warnings = Vec::with_capacity(32);

        assert!(
            retained_entry_bytes(&key, &request, &outcome) >= baseline + 32 * size_of::<String>()
        );
        assert!(ENTRY_CONTAINER_OVERHEAD >= 16 * size_of::<usize>());
    }

    fn request() -> ProgrammingPreloadLifecycleRequest {
        let current = ProgrammingPreloadRevisionExpectation::Current;
        ProgrammingPreloadLifecycleRequest {
            expected_capture_mode_revision: current,
            expected_values_revision: current,
            expected_queue_revision: current,
            expected_selection_revision: current,
            action: ProgrammingPreloadLifecycleAction::Enter,
        }
    }

    fn result(
        user_id: UserId,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        group_bytes: usize,
    ) -> ProgrammingPreloadLifecycleResult {
        let show_id = ShowId::new();
        let runtime = ProgrammingPreloadRuntimeChange {
            projection: PlaybackRuntimeProjection {
                scope: PlaybackShowScope {
                    show_id: show_id.0,
                    show_revision: 1,
                },
                requested: PlaybackRuntimeIdentity::Playback(1),
                playback_number: Some(1),
                target: PlaybackTargetProjection::Group {
                    group_id: "g".repeat(group_bytes),
                    master: 1.0,
                    flash_level: 1.0,
                },
            },
            event_sequence: 1,
        };
        ProgrammingPreloadLifecycleResult {
            context: ActionContext::operator(desk_id, user_id.0, session_id.0, ActionSource::Http)
                .with_request_id(request_id),
            request_id: request_id.into(),
            replayed: false,
            state: ProgrammingPreloadLifecycleState::Changed,
            active: false,
            capture_mode: Arc::new(ProgrammingCaptureModeProjection {
                user_id,
                revision: 1,
                blind: false,
                preview: false,
                preload_capture_programmer: true,
            }),
            capture_mode_event_sequence: None,
            values_revision: 0,
            values_projection: None,
            values_event_sequence: None,
            queue_revision: 0,
            queue_projection: None,
            queue_event_sequence: None,
            interaction_event_sequence: None,
            selection_revision: 0,
            commit: Some(ProgrammingPreloadCommitResult {
                show_id,
                show_revision: 1,
                playback_event_sequence_before: 0,
                playback_event_sequence_after: 1,
                committed_at: chrono::Utc::now(),
                programmer_fade_millis: 0,
                executed_playback_actions: 0,
                executed: Vec::new(),
                runtime_changes: vec![runtime],
                warnings: Vec::new(),
            }),
            warning: None,
        }
    }

    fn identity(
        user_id: UserId,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
    ) -> PreloadLifecycleReplayIdentity {
        PreloadLifecycleReplayIdentity {
            user_id,
            desk_id,
            session_id,
            request_id: request_id.into(),
        }
    }

    fn replay_key(
        user_id: UserId,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
    ) -> ReplayKey {
        ReplayKey {
            user_id,
            desk_id,
            session_id,
            request_id: request_id.into(),
        }
    }
}
