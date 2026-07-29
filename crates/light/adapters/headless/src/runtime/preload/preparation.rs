use super::*;
use std::sync::Arc;

pub(super) type PlaybackIdentity = light_application::PlaybackRuntimeIdentity;
pub(super) type PlaybackProjection = light_application::PlaybackRuntimeProjection;

pub(super) struct PreparedPreloadCommit {
    pub(super) pending: Vec<light_programmer::PreloadPlaybackAction>,
    pub(super) committed_at: chrono::DateTime<chrono::Utc>,
    pub(super) programmer_fade_millis: u64,
    pub(super) prepared_playback: light_engine::PreparedPlaybackBatch,
    pub(super) staged_actions: Vec<StagedPreloadPlaybackAction>,
    pub(super) identities: Vec<PlaybackIdentity>,
    pub(super) before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    pub(super) context: light_application::ActionContext,
}

pub(super) fn prepare_preload_commit(
    state: &AppState,
    session: &Session,
    context: light_application::ActionContext,
) -> Result<PreparedPreloadCommit, String> {
    let pending = pending_actions(state, session)?;
    validate_playback_definitions(&pending, &state.output.snapshot())?;
    let committed_at = state.programming.clock().now();
    let programmer_fade_millis = state.installation.configuration().programmer_fade_millis;
    let mut commands = preload_batch_commands(&pending)?;
    attach_shared_exclusions(state, session, committed_at, &pending, &mut commands);
    let prepared_playback =
        state
            .output
            .prepare_playback_batch(&commands, committed_at, programmer_fade_millis)?;
    let identities = changed_identities(&prepared_playback);
    let before = read_projections(state, &context, &identities)?;
    let staged_actions = staged_preload_actions(&pending, &prepared_playback);
    Ok(PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        staged_actions,
        identities,
        before,
        context,
    })
}

pub(super) fn read_projections(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
) -> Result<Vec<(PlaybackIdentity, PlaybackProjection)>, String> {
    if identities.is_empty() {
        return Ok(Vec::new());
    }
    let projections = playback_service::read_runtime_projections(state, context, identities)
        .map_err(|error| error.message)?;
    validate_projections(identities, &projections)?;
    Ok(identities.iter().cloned().zip(projections).collect())
}

fn pending_actions(
    state: &AppState,
    session: &Session,
) -> Result<Vec<light_programmer::PreloadPlaybackAction>, String> {
    state
        .programming
        .preload_playback_actions(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())
}

fn validate_playback_definitions(
    pending: &[light_programmer::PreloadPlaybackAction],
    snapshot: &EngineSnapshot,
) -> Result<(), String> {
    for action in pending {
        let exists = if action.playback_number >= light_playback::MIN_VIRTUAL_PLAYBACK {
            action.page.is_some_and(|page_number| {
                snapshot
                    .playback_pages
                    .iter()
                    .find(|page| page.number == page_number)
                    .is_some_and(|page| {
                        page.virtual_playbacks.contains_key(&action.playback_number)
                    })
            })
        } else {
            snapshot
                .playbacks
                .iter()
                .any(|definition| definition.number == action.playback_number)
        };
        if !exists {
            return Err(format!(
                "playback {} no longer exists",
                action.playback_number
            ));
        }
    }
    Ok(())
}

fn attach_shared_exclusions(
    state: &AppState,
    session: &Session,
    committed_at: chrono::DateTime<chrono::Utc>,
    pending: &[light_programmer::PreloadPlaybackAction],
    commands: &mut [light_engine::PlaybackBatchCommand],
) {
    let zones: Arc<[Vec<u16>]> = VirtualPlaybackExclusionResolver::read(state)
        .map(|resolver| resolver.zone_numbers())
        .unwrap_or_default()
        .into();
    for (pending, command) in pending.iter().zip(commands) {
        let desk_id = pending.origin_desk_id.unwrap_or(session.desk.id);
        let applies = zones
            .iter()
            .any(|zone| zone.contains(&pending.playback_number));
        command.exclusion_zones = Arc::clone(&zones);
        command.activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: committed_at,
            desk_id: Some(desk_id),
            surface: activation_surface(pending.surface),
            exclusion_scope: if applies {
                light_playback::PlaybackExclusionScope::Show
            } else {
                light_playback::PlaybackExclusionScope::None
            },
        });
    }
}

const fn activation_surface(
    surface: light_programmer::PreloadPlaybackQueueSurface,
) -> light_playback::PlaybackActivationSurface {
    match surface {
        light_programmer::PreloadPlaybackQueueSurface::Physical => {
            light_playback::PlaybackActivationSurface::Physical
        }
        light_programmer::PreloadPlaybackQueueSurface::Virtual => {
            light_playback::PlaybackActivationSurface::Virtual
        }
        light_programmer::PreloadPlaybackQueueSurface::Osc => {
            light_playback::PlaybackActivationSurface::Osc
        }
        light_programmer::PreloadPlaybackQueueSurface::Matter => {
            light_playback::PlaybackActivationSurface::Matter
        }
    }
}

fn changed_identities(prepared: &light_engine::PreparedPlaybackBatch) -> Vec<PlaybackIdentity> {
    let mut identities = prepared
        .changed_playback_numbers()
        .map(PlaybackIdentity::Playback)
        .collect::<Vec<_>>();
    for outcome in prepared.outcomes() {
        if outcome.addressed_effect.changed()
            && let Some(page) = outcome.page
            && let Ok(address) = light_playback::VirtualPlaybackAddress::new(page, outcome.number)
        {
            identities.push(PlaybackIdentity::Virtual(address));
        }
        if let Some(page) = outcome.page {
            identities.extend(outcome.released_playbacks.iter().filter_map(|number| {
                light_playback::VirtualPlaybackAddress::new(page, *number)
                    .ok()
                    .map(PlaybackIdentity::Virtual)
            }));
        }
    }
    identities.sort_by_key(|identity| match identity {
        PlaybackIdentity::Playback(number) => (0_u8, 0_u8, *number),
        PlaybackIdentity::Virtual(address) => (1, address.page(), address.number().get()),
        PlaybackIdentity::CueList(_) | PlaybackIdentity::Group(_) => (2, 0, 0),
    });
    identities.dedup();
    identities
}

fn validate_projections(
    identities: &[PlaybackIdentity],
    projections: &[PlaybackProjection],
) -> Result<(), String> {
    if identities.len() != projections.len() {
        return Err("Playback projection batch returned an incomplete result".into());
    }
    let scope = projections.first().map(|projection| projection.scope);
    for (identity, projection) in identities.iter().zip(projections) {
        if projection.requested != *identity || Some(projection.scope) != scope {
            return Err("Playback projection batch returned mismatched authority".into());
        }
    }
    Ok(())
}
