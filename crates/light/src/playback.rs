use light_playback::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause,
    PlaybackCueReference as DomainPlaybackCueReference,
};
pub use light_playback::{
    PhysicalPlaybackNumber, PlaybackIdentity, VirtualPlaybackAddress, VirtualPlaybackNumber,
};
use std::sync::Arc;

mod command;
mod event;
mod group_id;
mod operation;
mod ports;
mod projection;
mod service;
mod telemetry;
mod transition_set;

pub use command::{
    CueNumber, PendingPlaybackAction, PlaybackAction, PlaybackAddress, PlaybackCommand,
    PlaybackDurability, PlaybackExecution, PlaybackLevel, PlaybackOutcome, PlaybackRelatedResult,
    PlaybackResult, PlaybackSurface, ResolvedPlaybackAddress,
};
pub use event::{
    PlaybackCueTransition, PlaybackRuntimeChange, PlaybackTransitionCause,
    committed_playback_effect_event, committed_playback_event,
};
pub use group_id::{MAX_PLAYBACK_GROUP_ID_BYTES, PlaybackGroupId, PlaybackGroupIdError};
pub use operation::{PlaybackOperation, PlaybackOperationResult, PlaybackUnitOfWork};
pub use ports::PlaybackPorts;
pub use projection::{
    CueListRuntimeProjection, CueTimingRuntimeProjection, CueTriggerTimingKind,
    CueTriggerTimingProjection, DeletedCueHoldProjection, DynamicPlaybackControllerStatus,
    DynamicPlaybackRuntimeProjection, DynamicPlaybackRuntimeState, DynamicPlaybackSpeedSource,
    GrandMasterRuntimeProjection, MAX_PLAYBACK_SNAPSHOT_IDENTITIES, ManualXFadeDirection,
    PlaybackCueReference, PlaybackDeskProjection, PlaybackRuntimeIdentity,
    PlaybackRuntimeProjection, PlaybackRuntimeSnapshot, PlaybackShowScope,
    PlaybackTargetProjection, SoundLossReason, SoundStatus, SpeedGroupRuntimeProjection,
    SpeedSource,
};
pub use service::PlaybackService;
pub use telemetry::{PlaybackTelemetryDeltas, PlaybackTelemetryTick, telemetry_frame_divider};

#[cfg(test)]
#[path = "playback/service_tests.rs"]
mod service_tests;

use crate::{EventBus, EventDraft, EventEnvelope, EventSource};

#[derive(Clone, Debug, PartialEq)]
pub struct AutomaticPlaybackProjection {
    pub transition: AutomaticPlaybackTransition,
    pub projection: PlaybackRuntimeProjection,
}

/// Maps scheduler-owned domain transitions to installation-global application events.
///
/// Callers collect the domain transitions while advancing playback, release their domain lock,
/// and only then publish these drafts through the event bus.
pub fn automatic_playback_events(
    changes: impl IntoIterator<Item = AutomaticPlaybackProjection>,
) -> Vec<EventDraft> {
    changes.into_iter().map(automatic_playback_event).collect()
}

/// Publishes automatic transitions after the caller has released its playback/render locks.
pub fn publish_automatic_playback_events(
    bus: &EventBus,
    changes: impl IntoIterator<Item = AutomaticPlaybackProjection>,
) -> Vec<Arc<EventEnvelope>> {
    automatic_playback_events(changes)
        .into_iter()
        .map(|draft| bus.publish(draft))
        .collect()
}

fn automatic_playback_event(change: AutomaticPlaybackProjection) -> EventDraft {
    let transition = change.transition;
    EventDraft::playback_runtime_changed(
        None,
        PlaybackRuntimeChange {
            projection: change.projection,
            transition: Some(PlaybackCueTransition {
                playback_number: transition.playback_number,
                cue_list_id: transition.cue_list_id.0,
                previous: Some(cue_reference(transition.previous)),
                current: Some(cue_reference(transition.current)),
                cause: transition_cause(transition.cause),
                transition_ordinal: transition.transition_ordinal,
                advanced_steps: transition.advanced_steps,
            }),
        },
        EventSource::Runtime,
        None,
    )
}

fn cue_reference(reference: DomainPlaybackCueReference) -> projection::PlaybackCueReference {
    projection::PlaybackCueReference {
        id: reference.id,
        number: reference.number,
    }
}

fn transition_cause(cause: AutomaticPlaybackTransitionCause) -> PlaybackTransitionCause {
    match cause {
        AutomaticPlaybackTransitionCause::Chaser => PlaybackTransitionCause::Chaser,
        AutomaticPlaybackTransitionCause::Follow => PlaybackTransitionCause::Follow,
        AutomaticPlaybackTransitionCause::Wait => PlaybackTransitionCause::Wait,
        AutomaticPlaybackTransitionCause::Timecode => PlaybackTransitionCause::Timecode,
        AutomaticPlaybackTransitionCause::Link => PlaybackTransitionCause::Link,
    }
}

#[cfg(test)]
mod tests {
    use light_playback::{
        AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
    };
    use uuid::Uuid;

    use super::*;
    use crate::{ApplicationEvent, PlaybackEvent};

    #[test]
    fn automatic_transitions_become_installation_global_runtime_events() {
        let cue_list_id = Default::default();
        let drafts =
            automatic_playback_events([automatic_projection(AutomaticPlaybackTransition {
                playback_number: Some(12),
                cue_list_id,
                previous: domain_cue(2, 1.0),
                current: domain_cue(3, 4.0),
                cause: AutomaticPlaybackTransitionCause::Chaser,
                transition_ordinal: 37,
                advanced_steps: 7,
            })]);

        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].desk_id, None);
        assert_eq!(drafts[0].source, EventSource::Runtime);
        let ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) = &drafts[0].payload
        else {
            panic!("expected playback transition");
        };
        let event = change.transition.as_ref().unwrap();
        assert_eq!(event.cue_list_id, cue_list_id.0);
        assert_eq!(event.cause, PlaybackTransitionCause::Chaser);
        assert_eq!(event.transition_ordinal, 37);
        assert_eq!(event.advanced_steps, 7);
        assert_eq!(
            event.previous.as_ref().map(|cue| cue.number.clone()),
            Some(crate::CueNumber::try_from_legacy_f64(1.0).unwrap())
        );
        assert_eq!(
            event.current.as_ref().map(|cue| cue.number.clone()),
            Some(crate::CueNumber::try_from_legacy_f64(4.0).unwrap())
        );
    }

    #[test]
    fn publishing_occurs_through_the_application_bus() {
        let bus = EventBus::default();
        let published = publish_automatic_playback_events(
            &bus,
            [automatic_projection(AutomaticPlaybackTransition {
                playback_number: None,
                cue_list_id: Default::default(),
                previous: domain_cue(1, 1.0),
                current: domain_cue(2, 2.0),
                cause: AutomaticPlaybackTransitionCause::Follow,
                transition_ordinal: 38,
                advanced_steps: 1,
            })],
        );

        assert_eq!(published.len(), 1);
        assert_eq!(published[0].sequence, 1);
        assert_eq!(bus.latest_sequence(), 1);
    }

    fn domain_cue(value: u128, number: f64) -> PlaybackCueReference {
        PlaybackCueReference {
            id: Uuid::from_u128(value),
            number: crate::CueNumber::try_from_legacy_f64(number).unwrap(),
        }
    }

    fn automatic_projection(
        transition: AutomaticPlaybackTransition,
    ) -> AutomaticPlaybackProjection {
        let requested = transition.playback_number.map_or(
            PlaybackRuntimeIdentity::CueList(transition.cue_list_id),
            PlaybackRuntimeIdentity::Playback,
        );
        AutomaticPlaybackProjection {
            projection: PlaybackRuntimeProjection {
                scope: PlaybackShowScope {
                    show_id: Uuid::nil(),
                    show_revision: 0,
                },
                requested,
                playback_number: transition.playback_number,
                target: PlaybackTargetProjection::CueList {
                    cue_list_id: transition.cue_list_id,
                    runtime: None,
                },
            },
            transition,
        }
    }
}
