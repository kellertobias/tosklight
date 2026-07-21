use super::*;
use std::{collections::HashMap, sync::Arc};

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
    validate_playback_definitions(&pending, &state.engine.snapshot())?;
    let committed_at = state.programmers.clock().now();
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let mut commands = preload_batch_commands(&pending)?;
    attach_shared_exclusions(state, session, committed_at, &pending, &mut commands);
    let prepared_playback =
        state
            .engine
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
        .programmers
        .preload_playback_actions(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())
}

fn validate_playback_definitions(
    pending: &[light_programmer::PreloadPlaybackAction],
    snapshot: &EngineSnapshot,
) -> Result<(), String> {
    for action in pending {
        if !snapshot
            .playbacks
            .iter()
            .any(|definition| definition.number == action.playback_number)
        {
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
    let mut resolvers = HashMap::<uuid::Uuid, VirtualPlaybackExclusionResolver>::new();
    let mut cached = HashMap::<(uuid::Uuid, Option<u8>), ResolvedExclusions>::new();
    for (pending, command) in pending.iter().zip(commands) {
        let desk_id = pending.origin_desk_id.unwrap_or(session.desk.id);
        let key = (desk_id, pending.page);
        let exclusions = cached
            .entry(key)
            .or_insert_with(|| resolve_exclusions(state, desk_id, pending.page, &mut resolvers));
        command.exclusion_zones = Arc::clone(&exclusions.zones);
        command.activation_origin = Some(light_playback::PlaybackActivationOrigin {
            at: committed_at,
            desk_id: Some(desk_id),
            surface: activation_surface(pending.surface),
            exclusion_scope: exclusions.scope,
        });
    }
}

struct ResolvedExclusions {
    zones: Arc<[Vec<u16>]>,
    scope: light_playback::PlaybackExclusionScope,
}

fn resolve_exclusions(
    state: &AppState,
    desk_id: uuid::Uuid,
    page: Option<u8>,
    resolvers: &mut HashMap<uuid::Uuid, VirtualPlaybackExclusionResolver>,
) -> ResolvedExclusions {
    let desk_exists = state
        .desk
        .lock()
        .control_desk(desk_id)
        .ok()
        .flatten()
        .is_some();
    if !desk_exists {
        return ResolvedExclusions {
            zones: Arc::default(),
            scope: light_playback::PlaybackExclusionScope::None,
        };
    }
    let resolver = resolvers
        .entry(desk_id)
        .or_insert_with(|| VirtualPlaybackExclusionResolver::read(state, desk_id));
    let scope = if resolver.applies_to_page(page) {
        light_playback::PlaybackExclusionScope::OriginatingDesk
    } else {
        light_playback::PlaybackExclusionScope::None
    };
    ResolvedExclusions {
        zones: resolver.zone_numbers(page).into(),
        scope,
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
    prepared
        .changed_playback_numbers()
        .map(PlaybackIdentity::Playback)
        .collect()
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
