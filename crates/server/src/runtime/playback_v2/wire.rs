//! Explicit translation between transport-independent Playback values and v2 DTOs.

use light_application as application;
use light_wire::v2::playback as wire;

#[path = "wire/projection.rs"]
mod projection;

use projection::target_projection;

pub(in crate::runtime) fn application_command(
    request: wire::PlaybackActionRequest,
) -> Result<(String, application::PlaybackCommand), String> {
    validate_request_id(&request.request_id)?;
    Ok((
        request.request_id,
        application::PlaybackCommand {
            address: application_address(request.address)?,
            action: application_action(request.action)?,
            surface: match request.surface {
                wire::PlaybackSurface::Virtual => application::PlaybackSurface::Virtual,
                wire::PlaybackSurface::Physical => application::PlaybackSurface::Physical,
            },
        },
    ))
}

pub(in crate::runtime) fn application_identities(
    identities: Vec<wire::PlaybackRuntimeIdentity>,
) -> Result<Vec<application::PlaybackRuntimeIdentity>, String> {
    identities
        .into_iter()
        .map(|identity| match identity {
            wire::PlaybackRuntimeIdentity::Playback { playback_number }
                if (1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
            {
                Ok(application::PlaybackRuntimeIdentity::Playback(
                    playback_number,
                ))
            }
            wire::PlaybackRuntimeIdentity::Playback { .. } => {
                Err("playback_number must be within 1-1000".into())
            }
            wire::PlaybackRuntimeIdentity::CueList { cue_list_id } if !cue_list_id.is_nil() => Ok(
                application::PlaybackRuntimeIdentity::CueList(light_core::CueListId(cue_list_id)),
            ),
            wire::PlaybackRuntimeIdentity::CueList { .. } => {
                Err("cue_list_id must not be nil".into())
            }
        })
        .collect()
}

fn application_address(
    address: wire::PlaybackAddress,
) -> Result<application::PlaybackAddress, String> {
    match address {
        wire::PlaybackAddress::CueList { cue_list_id } if !cue_list_id.is_nil() => Ok(
            application::PlaybackAddress::CueList(light_core::CueListId(cue_list_id)),
        ),
        wire::PlaybackAddress::CueList { .. } => Err("cue_list_id must not be nil".into()),
        wire::PlaybackAddress::Playback { playback_number }
            if (1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
        {
            Ok(application::PlaybackAddress::Pool(playback_number))
        }
        wire::PlaybackAddress::Playback { .. } => {
            Err("playback_number must be within 1-1000".into())
        }
        wire::PlaybackAddress::CurrentPage { slot }
            if (1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            Ok(application::PlaybackAddress::CurrentPage { slot })
        }
        wire::PlaybackAddress::CurrentPage { .. } => {
            Err("current-page slot must be within 1-127".into())
        }
        wire::PlaybackAddress::ExplicitPage { page, slot }
            if (1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
                && (1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            Ok(application::PlaybackAddress::ExplicitPage { page, slot })
        }
        wire::PlaybackAddress::ExplicitPage { .. } => {
            Err("explicit page and slot must be within 1-127".into())
        }
    }
}

fn application_action(action: wire::PlaybackAction) -> Result<application::PlaybackAction, String> {
    use application::PlaybackAction as App;
    use wire::PlaybackAction as Wire;
    Ok(match action {
        Wire::Go { pressed } => App::Go { pressed },
        Wire::Back { pressed } => App::Back { pressed },
        Wire::Pause { pressed } => App::Pause { pressed },
        Wire::Release => App::Release,
        Wire::On { pressed } => App::On { pressed },
        Wire::Off { pressed } => App::Off { pressed },
        Wire::Toggle { pressed } => App::Toggle { pressed },
        Wire::FastForward { pressed } => App::FastForward { pressed },
        Wire::FastRewind { pressed } => App::FastRewind { pressed },
        Wire::Flash { pressed } => App::Flash { pressed },
        Wire::Temp { pressed } => App::Temp { pressed },
        Wire::Swap { pressed } => App::Swap { pressed },
        Wire::Select { pressed } => App::Select { pressed },
        Wire::SelectContents { pressed } => App::SelectContents { pressed },
        Wire::SelectDereferenced { pressed } => App::SelectDereferenced { pressed },
        Wire::Learn { pressed } => App::Learn { pressed },
        Wire::Double { pressed } => App::Double { pressed },
        Wire::Half { pressed } => App::Half { pressed },
        Wire::Blackout { pressed } => App::Blackout { pressed },
        Wire::PauseDynamics { pressed } => App::PauseDynamics { pressed },
        Wire::None { pressed } => App::None { pressed },
        Wire::Master { value } if value.is_finite() && (0.0..=1.0).contains(&value) => {
            App::Master(application::PlaybackLevel::new(value))
        }
        Wire::Master { .. } => return Err("master value must be finite and within 0-1".into()),
        Wire::GoTo { cue_number } if cue_number.is_finite() && cue_number > 0.0 => {
            App::GoTo(application::CueNumber::new(cue_number))
        }
        Wire::GoTo { .. } => return Err("cue_number must be finite and greater than zero".into()),
        Wire::Load { cue_number } if cue_number.is_finite() && cue_number > 0.0 => {
            App::Load(application::CueNumber::new(cue_number))
        }
        Wire::Load { .. } => return Err("cue_number must be finite and greater than zero".into()),
        Wire::Crossfade { enabled } => App::Crossfade { enabled },
        Wire::Temporary { enabled, pressed } => App::Temporary { enabled, pressed },
        Wire::ConfiguredButton { number, pressed } if (1..=3).contains(&number) => {
            App::ConfiguredButton { number, pressed }
        }
        Wire::ConfiguredButton { .. } => return Err("button number must be within 1-3".into()),
    })
}

pub(in crate::runtime) fn action_outcome(
    result: application::PlaybackResult,
) -> wire::PlaybackActionOutcome {
    wire::PlaybackActionOutcome {
        request_id: result.context.request_id.clone().unwrap_or_default(),
        correlation_id: result.context.correlation_id,
        requested: requested_address(result.requested),
        resolved: resolved_address(result.resolved),
        outcome: outcome(result.outcome),
        durability: match result.durability {
            application::PlaybackDurability::Durable => wire::PlaybackDurability::Durable,
            application::PlaybackDurability::PersistencePending => {
                wire::PlaybackDurability::PersistencePending
            }
        },
        projection: runtime_projection(&result.projection),
        related: result
            .related
            .iter()
            .map(|related| wire::PlaybackRelatedOutcome {
                projection: runtime_projection(&related.projection),
                event_sequence: related.event_sequence,
            })
            .collect(),
        desk: result.desk.map(desk_projection),
        event_sequence: result.event_sequence,
        desk_event_sequence: result.desk_event_sequence,
        replayed: result.replayed,
    }
}

pub(in crate::runtime) fn runtime_snapshot(
    snapshot: application::PlaybackRuntimeSnapshot,
) -> wire::PlaybackRuntimeSnapshot {
    wire::PlaybackRuntimeSnapshot {
        cursor: light_wire::v2::events::EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        desk: desk_projection(snapshot.desk),
        projections: snapshot
            .projections
            .iter()
            .map(runtime_projection)
            .collect(),
    }
}

pub(in crate::runtime) fn runtime_change(
    change: &application::PlaybackRuntimeChange,
) -> wire::PlaybackRuntimeChange {
    wire::PlaybackRuntimeChange {
        projection: runtime_projection(&change.projection),
        transition: change.transition.as_ref().map(cue_transition),
    }
}

pub(in crate::runtime) fn runtime_projection(
    projection: &application::PlaybackRuntimeProjection,
) -> wire::PlaybackRuntimeProjection {
    wire::PlaybackRuntimeProjection {
        scope: show_scope(projection.scope),
        requested: wire_identity(projection.requested),
        playback_number: projection.playback_number,
        target: target_projection(&projection.target),
    }
}

pub(in crate::runtime) fn desk_projection(
    projection: application::PlaybackDeskProjection,
) -> wire::PlaybackDeskProjection {
    wire::PlaybackDeskProjection {
        scope: show_scope(projection.scope),
        desk_id: projection.desk_id,
        active_page: projection.active_page,
        selected_playback: projection.selected_playback,
    }
}

const fn show_scope(scope: application::PlaybackShowScope) -> wire::PlaybackShowScope {
    wire::PlaybackShowScope {
        show_id: scope.show_id,
        show_revision: scope.show_revision,
    }
}

fn cue_transition(transition: &application::PlaybackCueTransition) -> wire::PlaybackCueTransition {
    wire::PlaybackCueTransition {
        playback_number: transition.playback_number,
        cue_list_id: transition.cue_list_id,
        previous: transition.previous.as_ref().map(cue_reference),
        current: transition.current.as_ref().map(cue_reference),
        cause: transition_cause(transition.cause),
        advanced_steps: transition.advanced_steps,
    }
}

fn cue_reference(cue: &application::PlaybackCueReference) -> wire::PlaybackCueReference {
    wire::PlaybackCueReference {
        id: cue.id,
        number: cue.number,
    }
}

fn transition_cause(cause: application::PlaybackTransitionCause) -> wire::PlaybackTransitionCause {
    use application::PlaybackTransitionCause as App;
    match cause {
        App::Go => wire::PlaybackTransitionCause::Go,
        App::Back => wire::PlaybackTransitionCause::Back,
        App::Jump => wire::PlaybackTransitionCause::Jump,
        App::Chaser => wire::PlaybackTransitionCause::Chaser,
        App::Follow => wire::PlaybackTransitionCause::Follow,
        App::Wait => wire::PlaybackTransitionCause::Wait,
        App::Timecode => wire::PlaybackTransitionCause::Timecode,
    }
}

fn wire_identity(identity: application::PlaybackRuntimeIdentity) -> wire::PlaybackRuntimeIdentity {
    match identity {
        application::PlaybackRuntimeIdentity::Playback(playback_number) => {
            wire::PlaybackRuntimeIdentity::Playback { playback_number }
        }
        application::PlaybackRuntimeIdentity::CueList(cue_list_id) => {
            wire::PlaybackRuntimeIdentity::CueList {
                cue_list_id: cue_list_id.0,
            }
        }
    }
}

fn requested_address(address: application::PlaybackAddress) -> wire::PlaybackAddress {
    match address {
        application::PlaybackAddress::CueList(cue_list_id) => wire::PlaybackAddress::CueList {
            cue_list_id: cue_list_id.0,
        },
        application::PlaybackAddress::Pool(playback_number) => {
            wire::PlaybackAddress::Playback { playback_number }
        }
        application::PlaybackAddress::CurrentPage { slot } => {
            wire::PlaybackAddress::CurrentPage { slot }
        }
        application::PlaybackAddress::ExplicitPage { page, slot } => {
            wire::PlaybackAddress::ExplicitPage { page, slot }
        }
    }
}

fn resolved_address(
    address: application::ResolvedPlaybackAddress,
) -> wire::ResolvedPlaybackAddress {
    match address {
        application::ResolvedPlaybackAddress::CueList(cue_list_id) => {
            wire::ResolvedPlaybackAddress::CueList {
                cue_list_id: cue_list_id.0,
            }
        }
        application::ResolvedPlaybackAddress::Pool { number, page, slot } => {
            wire::ResolvedPlaybackAddress::Playback {
                playback_number: number,
                page,
                slot,
            }
        }
    }
}

fn outcome(outcome: application::PlaybackOutcome) -> wire::PlaybackOutcome {
    match outcome {
        application::PlaybackOutcome::Applied => wire::PlaybackOutcome::Applied,
        application::PlaybackOutcome::NoChange => wire::PlaybackOutcome::NoChange,
        application::PlaybackOutcome::Captured(pending) => wire::PlaybackOutcome::Captured {
            pending: pending_action(pending),
        },
    }
}

fn pending_action(action: application::PendingPlaybackAction) -> wire::PendingPlaybackAction {
    match action {
        application::PendingPlaybackAction::Toggle => wire::PendingPlaybackAction::Toggle,
        application::PendingPlaybackAction::Go => wire::PendingPlaybackAction::Go,
        application::PendingPlaybackAction::Back => wire::PendingPlaybackAction::Back,
        application::PendingPlaybackAction::Off => wire::PendingPlaybackAction::Off,
        application::PendingPlaybackAction::On => wire::PendingPlaybackAction::On,
        application::PendingPlaybackAction::TemporaryOn => wire::PendingPlaybackAction::TemporaryOn,
        application::PendingPlaybackAction::TemporaryOff => {
            wire::PendingPlaybackAction::TemporaryOff
        }
    }
}

fn validate_request_id(request_id: &str) -> Result<(), String> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err("request_id must contain 1-128 printable bytes".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
