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

pub(super) fn projection(
    projection: &application::ProgrammingPreloadPlaybackQueueProjection,
) -> wire::ProgrammingPreloadPlaybackQueueProjection {
    wire::ProgrammingPreloadPlaybackQueueProjection {
        revision: projection.revision,
        actions: projection.actions.iter().cloned().map(queue_item).collect(),
    }
}

pub(super) fn queue_item(
    item: application::ProgrammingPreloadPlaybackQueueItem,
) -> wire::ProgrammingPreloadPlaybackQueueItem {
    wire::ProgrammingPreloadPlaybackQueueItem {
        address: light_wire::v2::playback::PlaybackAddressValue {
            page: item.address.page(),
            position: item.address.position(),
        },
        control_source: light_wire::v2::playback::PlaybackControlSource {
            surface_id: item.control_source.surface_id,
            control_id: item.control_source.control_id,
        },
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
            application::ProgrammingPreloadPlaybackAction::DynamicPause => {
                wire::ProgrammingPreloadPlaybackAction::DynamicPause
            }
            application::ProgrammingPreloadPlaybackAction::DynamicRestart => {
                wire::ProgrammingPreloadPlaybackAction::DynamicRestart
            }
            application::ProgrammingPreloadPlaybackAction::DynamicDoubleSpeed => {
                wire::ProgrammingPreloadPlaybackAction::DynamicDoubleSpeed
            }
            application::ProgrammingPreloadPlaybackAction::DynamicHalfSpeed => {
                wire::ProgrammingPreloadPlaybackAction::DynamicHalfSpeed
            }
            application::ProgrammingPreloadPlaybackAction::DynamicLearnSpeed => {
                wire::ProgrammingPreloadPlaybackAction::DynamicLearnSpeed
            }
        },
    }
}
