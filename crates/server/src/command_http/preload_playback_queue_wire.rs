use light_application as application;
use light_wire::v2::{events::EventSnapshotCursor, preload_playback_queue as wire};

pub(super) fn snapshot(
    snapshot: application::ProgrammingPreloadPlaybackQueueSnapshot,
) -> wire::ProgrammingPreloadPlaybackQueueSnapshot {
    wire::ProgrammingPreloadPlaybackQueueSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: projection(&snapshot.projection),
    }
}

pub(in crate::runtime) fn change(
    change: &application::ProgrammingPreloadPlaybackQueueChange,
) -> wire::ProgrammingPreloadPlaybackQueueChange {
    wire::ProgrammingPreloadPlaybackQueueChange {
        projection: projection(&change.projection),
    }
}

fn projection(
    projection: &application::ProgrammingPreloadPlaybackQueueProjection,
) -> wire::ProgrammingPreloadPlaybackQueueProjection {
    wire::ProgrammingPreloadPlaybackQueueProjection {
        user_id: projection.user_id.0,
        revision: projection.revision,
        actions: projection.actions.iter().copied().map(queue_item).collect(),
    }
}

fn queue_item(
    item: application::ProgrammingPreloadPlaybackQueueItem,
) -> wire::ProgrammingPreloadPlaybackQueueItem {
    wire::ProgrammingPreloadPlaybackQueueItem {
        playback_number: item.playback_number,
        action: match item.action {
            application::ProgrammingPreloadPlaybackAction::Toggle => {
                wire::ProgrammingPreloadPlaybackAction::Toggle
            }
            application::ProgrammingPreloadPlaybackAction::Go => {
                wire::ProgrammingPreloadPlaybackAction::Go
            }
            application::ProgrammingPreloadPlaybackAction::Back => {
                wire::ProgrammingPreloadPlaybackAction::Back
            }
            application::ProgrammingPreloadPlaybackAction::Off => {
                wire::ProgrammingPreloadPlaybackAction::Off
            }
            application::ProgrammingPreloadPlaybackAction::On => {
                wire::ProgrammingPreloadPlaybackAction::On
            }
            application::ProgrammingPreloadPlaybackAction::TemporaryOn => {
                wire::ProgrammingPreloadPlaybackAction::TemporaryOn
            }
            application::ProgrammingPreloadPlaybackAction::TemporaryOff => {
                wire::ProgrammingPreloadPlaybackAction::TemporaryOff
            }
        },
        surface: match item.surface {
            application::ProgrammingPreloadPlaybackSurface::Physical => {
                wire::ProgrammingPreloadPlaybackSurface::Physical
            }
            application::ProgrammingPreloadPlaybackSurface::Virtual => {
                wire::ProgrammingPreloadPlaybackSurface::Virtual
            }
            application::ProgrammingPreloadPlaybackSurface::Osc => {
                wire::ProgrammingPreloadPlaybackSurface::Osc
            }
            application::ProgrammingPreloadPlaybackSurface::Matter => {
                wire::ProgrammingPreloadPlaybackSurface::Matter
            }
        },
    }
}
