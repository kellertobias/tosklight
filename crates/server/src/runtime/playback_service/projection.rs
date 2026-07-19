use std::sync::Arc;

use light_application::{
    ActionContext, ActionError, AutomaticPlaybackProjection, PlaybackDeskProjection,
    PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection,
};
use light_core::CueListId;
use light_engine::EngineSnapshot;
use light_playback::{PlaybackRuntimeStatus, PlaybackTarget};

use super::{ServerPlaybackPorts, invalid};

#[path = "projection/targets.rs"]
mod targets;
use targets::{cue_list_projection, grand_master_projection, group_projection, speed_projection};

pub(in crate::runtime) fn automatic_changes(
    engine: &light_engine::Engine,
    scope: PlaybackShowScope,
    transitions: Vec<light_playback::AutomaticPlaybackTransition>,
) -> Vec<AutomaticPlaybackProjection> {
    if transitions.is_empty() {
        return Vec::new();
    }
    let runtime = engine.playback().read().runtime_status();
    transitions
        .into_iter()
        .map(|transition| {
            let requested = transition.playback_number.map_or(
                PlaybackRuntimeIdentity::CueList(transition.cue_list_id),
                PlaybackRuntimeIdentity::Playback,
            );
            let status = runtime.iter().find(|status| {
                transition.playback_number.map_or_else(
                    || {
                        status.playback.playback_number.is_none()
                            && status.playback.cue_list_id == transition.cue_list_id
                    },
                    |number| status.playback.playback_number == Some(number),
                )
            });
            AutomaticPlaybackProjection {
                projection: cue_list_projection(
                    scope,
                    requested,
                    transition.playback_number,
                    transition.cue_list_id,
                    status,
                ),
                transition,
            }
        })
        .collect()
}

pub(super) fn projection(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identity: PlaybackRuntimeIdentity,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let snapshot = ports.state.engine.snapshot();
    let scope = show_scope(ports, &snapshot);
    let runtime = ports.state.engine.playback().read().runtime_status();
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            project_playback(ports, &snapshot, &runtime, scope, identity, number)
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => Ok(cue_list_projection(
            scope,
            identity,
            None,
            cue_list_id,
            direct_cue_list_runtime(&runtime, cue_list_id),
        )),
    }
}

pub(super) fn projections(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identities: &[PlaybackRuntimeIdentity],
) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
    let snapshot = ports.state.engine.snapshot();
    let scope = show_scope(ports, &snapshot);
    let runtime = ports.state.engine.playback().read().runtime_status();
    let mut result = Vec::with_capacity(identities.len());
    for identity in identities {
        project_identity(ports, &snapshot, &runtime, scope, *identity, &mut result)?;
    }
    Ok(result)
}

pub(super) fn desk_projection(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
) -> Result<Option<PlaybackDeskProjection>, ActionError> {
    if context.desk_id.is_nil() {
        return Ok(None);
    }
    let snapshot = ports.state.engine.snapshot();
    let Some(show) = ports.state.active_show.read().clone() else {
        // Test-bench and startup compatibility paths may operate a prepared in-memory runtime
        // before its show index entry is installed. Nil makes that transient scope explicit.
        return Ok(Some(PlaybackDeskProjection {
            scope: PlaybackShowScope {
                show_id: uuid::Uuid::nil(),
                show_revision: snapshot.revision,
            },
            desk_id: context.desk_id,
            active_page: 1,
            selected_playback: None,
        }));
    };
    let store = ports.state.desk.lock();
    let active_page = store
        .desk_page(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?;
    let selected_playback = store
        .selected_playback(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(Some(PlaybackDeskProjection {
        scope: PlaybackShowScope {
            show_id: show.id.0,
            show_revision: snapshot.revision,
        },
        desk_id: context.desk_id,
        active_page,
        selected_playback,
    }))
}

fn project_identity(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &Arc<EngineSnapshot>,
    runtime: &[PlaybackRuntimeStatus],
    scope: PlaybackShowScope,
    identity: PlaybackRuntimeIdentity,
    result: &mut Vec<PlaybackRuntimeProjection>,
) -> Result<(), ActionError> {
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            result.push(project_playback(
                ports, snapshot, runtime, scope, identity, number,
            )?);
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => {
            project_cue_list(scope, snapshot, runtime, identity, cue_list_id, result);
        }
    }
    Ok(())
}

fn project_cue_list(
    scope: PlaybackShowScope,
    snapshot: &EngineSnapshot,
    runtime: &[PlaybackRuntimeStatus],
    requested: PlaybackRuntimeIdentity,
    cue_list_id: CueListId,
    result: &mut Vec<PlaybackRuntimeProjection>,
) {
    let start = result.len();
    for definition in snapshot.playbacks.iter().filter(|definition| {
        matches!(definition.target, PlaybackTarget::CueList { cue_list_id: id } if id == cue_list_id)
    }) {
        result.push(cue_list_projection(
            scope,
            requested,
            Some(definition.number),
            cue_list_id,
            runtime
                .iter()
                .find(|status| status.playback.playback_number == Some(definition.number)),
        ));
    }
    let direct = direct_cue_list_runtime(runtime, cue_list_id);
    if direct.is_some() || result.len() == start {
        result.push(cue_list_projection(
            scope,
            requested,
            None,
            cue_list_id,
            direct,
        ));
    }
}

fn direct_cue_list_runtime(
    runtime: &[PlaybackRuntimeStatus],
    cue_list_id: CueListId,
) -> Option<&PlaybackRuntimeStatus> {
    runtime.iter().find(|status| {
        status.playback.playback_number.is_none() && status.playback.cue_list_id == cue_list_id
    })
}

fn project_playback(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    runtime: &[PlaybackRuntimeStatus],
    scope: PlaybackShowScope,
    requested: PlaybackRuntimeIdentity,
    number: u16,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let Some(definition) = snapshot
        .playbacks
        .iter()
        .find(|definition| definition.number == number)
    else {
        return Ok(PlaybackRuntimeProjection {
            scope,
            requested,
            playback_number: Some(number),
            target: PlaybackTargetProjection::Missing,
        });
    };
    let target = match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => {
            return Ok(cue_list_projection(
                scope,
                requested,
                Some(number),
                *cue_list_id,
                runtime
                    .iter()
                    .find(|status| status.playback.playback_number == Some(number)),
            ));
        }
        PlaybackTarget::Group { group_id } => group_projection(ports, snapshot, group_id)?,
        PlaybackTarget::SpeedGroup { group } => speed_projection(ports, group)?,
        PlaybackTarget::GrandMaster => grand_master_projection(ports),
        PlaybackTarget::ProgrammerFade => PlaybackTargetProjection::ProgrammerFade {
            millis: ports.state.configuration.read().programmer_fade_millis,
        },
        PlaybackTarget::CueFade => PlaybackTargetProjection::CueFade {
            millis: ports.state.configuration.read().sequence_master_fade_millis,
        },
    };
    Ok(PlaybackRuntimeProjection {
        scope,
        requested,
        playback_number: Some(number),
        target,
    })
}

fn show_scope(ports: &ServerPlaybackPorts<'_>, snapshot: &EngineSnapshot) -> PlaybackShowScope {
    let show_id = ports
        .state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id.0)
        .unwrap_or_else(uuid::Uuid::nil);
    PlaybackShowScope {
        show_id,
        show_revision: snapshot.revision,
    }
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
