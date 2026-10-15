use super::*;

pub(super) type PlaybackIdentity = light_application::PlaybackRuntimeIdentity;
pub(super) type PlaybackProjection = light_application::PlaybackRuntimeProjection;

pub(super) struct PreparedPreloadCommit<'a> {
    pub(super) timecodes: Option<light_application::timeline::PreparedTimecodeBatch<'a>>,
    pub(super) pending: Vec<light_programmer::PreloadPlaybackAction>,
    pub(super) committed_at: chrono::DateTime<chrono::Utc>,
    pub(super) programmer_fade_millis: u64,
    pub(super) prepared_playback: light_engine::PreparedPlaybackBatch,
    pub(super) staged_actions: Vec<StagedPreloadPlaybackAction>,
    pub(super) before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    pub(super) context: light_application::ActionContext,
}

pub(super) fn prepare_preload_commit<'a>(
    state: &'a AppState,
    session: &Session,
    context: light_application::ActionContext,
    resources: Option<PreloadResources>,
) -> Result<PreparedPreloadCommit<'a>, String> {
    let pending = pending_actions(state, session)?;
    validate_playback_definitions(&pending, &state.output.snapshot())?;
    let committed_at = state.programming.clock().now();
    let transport_micros = state.timecodes.capture_transport_micros();
    let programmer_fade_millis = state.installation.configuration().programmer_fade_millis;
    let playback_context = CapturedPreloadPlaybackContext::new(session.desk.id);
    let commands = playback_context.commands(&pending, committed_at)?;
    let (prepared_playback, timecodes, committed_at, programmer_fade_millis) = timecodes::prepare(
        state,
        session,
        &commands,
        committed_at,
        transport_micros,
        programmer_fade_millis,
        resources,
    )?;
    let identities = changed_identities(&prepared_playback);
    let before = read_projections(state, &context, &identities)?;
    let staged_actions = staged_preload_actions(&pending, &prepared_playback);
    Ok(PreparedPreloadCommit {
        timecodes,
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        staged_actions,
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

pub(super) fn validate_playback_definitions(
    pending: &[light_programmer::PreloadPlaybackAction],
    snapshot: &EngineSnapshot,
) -> Result<(), String> {
    for action in pending {
        let exists = snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == action.address.page())
            .is_some_and(|page| page.assignments.contains_key(&action.address.position()));
        if !exists {
            return Err(format!(
                "playback {}.{} no longer exists",
                action.address.page(),
                action.address.position()
            ));
        }
    }
    Ok(())
}

fn changed_identities(prepared: &light_engine::PreparedPlaybackBatch) -> Vec<PlaybackIdentity> {
    let addresses = prepared
        .changed_playback_addresses()
        .chain(
            prepared
                .outcomes()
                .iter()
                .flat_map(|outcome| outcome.released_playbacks.iter().copied()),
        )
        .collect::<std::collections::BTreeSet<_>>();
    addresses
        .into_iter()
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
        if projection.identity != *identity || Some(projection.scope) != scope {
            return Err("Playback projection batch returned mismatched authority".into());
        }
    }
    Ok(())
}
