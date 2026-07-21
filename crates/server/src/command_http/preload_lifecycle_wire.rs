use light_application as application;
use light_core::ShowId;
use light_wire::v2::preload_lifecycle as wire;

pub(super) fn command(
    request: &wire::ProgrammingPreloadLifecycleRequest,
) -> application::ProgrammingPreloadLifecycleRequest {
    let exact = application::ProgrammingPreloadRevisionExpectation::Exact;
    application::ProgrammingPreloadLifecycleRequest {
        expected_capture_mode_revision: exact(request.expected_capture_mode_revision),
        expected_values_revision: exact(request.expected_values_revision),
        expected_queue_revision: exact(request.expected_queue_revision),
        expected_selection_revision: exact(request.expected_selection_revision),
        action: match request.action {
            wire::ProgrammingPreloadLifecycleAction::Enter {} => {
                application::ProgrammingPreloadLifecycleAction::Enter
            }
            wire::ProgrammingPreloadLifecycleAction::Go {
                show_id,
                expected_show_revision,
                expected_playback_event_sequence,
            } => application::ProgrammingPreloadLifecycleAction::Go {
                show_id: ShowId(show_id),
                expected_show_revision: exact(expected_show_revision),
                expected_playback_event_sequence: exact(expected_playback_event_sequence),
            },
            wire::ProgrammingPreloadLifecycleAction::ClearPending {} => {
                application::ProgrammingPreloadLifecycleAction::ClearPending
            }
            wire::ProgrammingPreloadLifecycleAction::Release {} => {
                application::ProgrammingPreloadLifecycleAction::Release
            }
        },
    }
}

pub(super) fn outcome(
    result: application::ProgrammingPreloadLifecycleResult,
) -> wire::ProgrammingPreloadLifecycleOutcome {
    wire::ProgrammingPreloadLifecycleOutcome {
        request_id: result.request_id,
        correlation_id: result.context.correlation_id,
        replayed: result.replayed,
        status: match result.state {
            application::ProgrammingPreloadLifecycleState::Changed => {
                wire::ProgrammingPreloadLifecycleState::Changed
            }
            application::ProgrammingPreloadLifecycleState::NoChange => {
                wire::ProgrammingPreloadLifecycleState::NoChange
            }
        },
        active: result.active,
        capture_mode: super::values_wire::capture_mode_projection(&result.capture_mode),
        capture_mode_event_sequence: result.capture_mode_event_sequence,
        values_revision: result.values_revision,
        values_projection: result
            .values_projection
            .as_deref()
            .map(super::preload_values_wire::projection_from_application),
        values_event_sequence: result.values_event_sequence,
        queue_revision: result.queue_revision,
        queue_projection: result
            .queue_projection
            .as_deref()
            .map(super::preload_playback_queue_wire::projection),
        queue_event_sequence: result.queue_event_sequence,
        interaction_event_sequence: result.interaction_event_sequence,
        selection_revision: result.selection_revision,
        commit: result.commit.map(commit),
        warning: result.warning,
    }
}

fn commit(
    commit: application::ProgrammingPreloadCommitResult,
) -> wire::ProgrammingPreloadCommitOutcome {
    wire::ProgrammingPreloadCommitOutcome {
        show_id: commit.show_id.0,
        show_revision: commit.show_revision,
        playback_event_sequence_before: commit.playback_event_sequence_before,
        playback_event_sequence_after: commit.playback_event_sequence_after,
        committed_at: commit.committed_at.to_rfc3339(),
        programmer_fade_millis: commit.programmer_fade_millis,
        executed_playback_actions: commit.executed_playback_actions as u64,
        executed: commit
            .executed
            .iter()
            .map(|action| {
                super::preload_playback_queue_wire::queue_item(
                    application::ProgrammingPreloadPlaybackQueueItem {
                        playback_number: action.playback_number,
                        page: action.page,
                        action: action.action,
                        surface: action.surface,
                    },
                )
            })
            .collect(),
        runtime_changes: commit
            .runtime_changes
            .iter()
            .map(|change| wire::ProgrammingPreloadRuntimeOutcome {
                projection: super::super::playback_v2::runtime_projection(&change.projection),
                event_sequence: change.event_sequence,
            })
            .collect(),
    }
}
