use super::*;

pub(super) fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
) -> Result<CommittedPreload, String> {
    let PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        staged_actions,
        identities,
        before,
        context,
    } = prepare_preload_commit(state, session)?;
    let playback_runtime_changed = prepared_playback.effect().durable();
    install_preload_commit(state, session, pending, committed_at, prepared_playback)?;
    let events = preload_change_events(state, &context, &identities, before, &staged_actions)?;
    emit_preload_exclusions(state, session, &staged_actions);
    let executed = executed_preload_actions(staged_actions, committed_at, programmer_fade_millis);
    let warnings = persist_preload_commit(state, session, playback_runtime_changed);
    Ok(CommittedPreload {
        committed_at,
        programmer_fade_millis,
        executed,
        warnings,
        events,
    })
}

pub(super) struct CommittedPreload {
    pub(super) committed_at: chrono::DateTime<chrono::Utc>,
    pub(super) programmer_fade_millis: u64,
    pub(super) executed: Vec<serde_json::Value>,
    pub(super) warnings: Vec<String>,
    pub(super) events: Vec<light_application::EventDraft>,
}

type PlaybackIdentity = light_application::PlaybackRuntimeIdentity;
type PlaybackProjection = light_application::PlaybackRuntimeProjection;

struct PreparedPreloadCommit {
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    prepared_playback: light_engine::PreparedPlaybackBatch,
    staged_actions: Vec<StagedPreloadPlaybackAction>,
    identities: Vec<PlaybackIdentity>,
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    context: light_application::ActionContext,
}

fn prepare_preload_commit(
    state: &AppState,
    session: &Session,
) -> Result<PreparedPreloadCommit, String> {
    let pending = state
        .programmers
        .preload_playback_actions(session.id)
        .ok_or_else(|| "programmer does not exist".to_owned())?;
    let snapshot = state.engine.snapshot();
    let definitions = preload_definitions(&pending, &snapshot)?;
    let committed_at = state.programmers.clock().now();
    let programmer_fade_millis = state.configuration.read().programmer_fade_millis;
    let exclusions = VirtualPlaybackExclusionResolver::read(state, session.desk.id);
    let context = light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    );
    let mut commands = preload_batch_commands(&pending)?;
    attach_preload_exclusions(&pending, &mut commands, &exclusions);
    let auto_off = preload_auto_off_candidates(state, &commands);
    let identities = preload_identities(&definitions, &commands, &auto_off);
    let before = read_preload_projections(state, &context, &identities)?;
    let prepared_playback =
        state
            .engine
            .prepare_playback_batch(&commands, committed_at, programmer_fade_millis)?;
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

fn preload_definitions(
    pending: &[light_programmer::PreloadPlaybackAction],
    snapshot: &EngineSnapshot,
) -> Result<
    Vec<(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )>,
    String,
> {
    pending
        .iter()
        .map(|action| {
            snapshot
                .playbacks
                .iter()
                .find(|definition| definition.number == action.playback_number)
                .cloned()
                .map(|definition| (action.clone(), definition))
                .ok_or_else(|| format!("playback {} no longer exists", action.playback_number))
        })
        .collect()
}

fn preload_identities(
    definitions: &[(
        light_programmer::PreloadPlaybackAction,
        light_playback::PlaybackDefinition,
    )],
    commands: &[light_engine::PlaybackBatchCommand],
    auto_off: &[u16],
) -> Vec<PlaybackIdentity> {
    let mut identities = definitions
        .iter()
        .map(|(_, definition)| PlaybackIdentity::Playback(definition.number))
        .chain(
            commands
                .iter()
                .flat_map(|command| command.exclusion_zones.iter().flatten())
                .copied()
                .map(PlaybackIdentity::Playback),
        )
        .chain(auto_off.iter().copied().map(PlaybackIdentity::Playback))
        .collect::<Vec<_>>();
    identities.sort_by_key(|identity| match identity {
        PlaybackIdentity::Playback(number) => *number,
        PlaybackIdentity::CueList(_) => 0,
    });
    identities.dedup();
    identities
}

fn preload_auto_off_candidates(
    state: &AppState,
    commands: &[light_engine::PlaybackBatchCommand],
) -> Vec<u16> {
    if commands.iter().any(|command| {
        matches!(
            command.action,
            PlaybackBatchAction::Toggle | PlaybackBatchAction::Go | PlaybackBatchAction::On
        )
    }) {
        state.engine.enabled_auto_off_playbacks()
    } else {
        Vec::new()
    }
}

fn attach_preload_exclusions(
    pending: &[light_programmer::PreloadPlaybackAction],
    commands: &mut [light_engine::PlaybackBatchCommand],
    resolver: &VirtualPlaybackExclusionResolver,
) {
    for (pending, command) in pending.iter().zip(commands) {
        command.exclusion_zones = resolver.zone_numbers(pending.page);
    }
}

fn read_preload_projections(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
) -> Result<Vec<(PlaybackIdentity, PlaybackProjection)>, String> {
    identities
        .iter()
        .copied()
        .map(|identity| {
            playback_service::read_runtime_projection(state, context, identity)
                .map(|projection| (identity, projection))
                .map_err(|error| error.message)
        })
        .collect()
}

fn install_preload_commit(
    state: &AppState,
    session: &Session,
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    prepared_playback: light_engine::PreparedPlaybackBatch,
) -> Result<(), String> {
    state
        .programmers
        .activate_preload_at(session.id, committed_at);
    let drained = state.programmers.take_preload_playback_actions(session.id);
    if drained != pending {
        return Err("the Preload queue changed while GO was being prepared".into());
    }

    state
        .engine
        .install_prepared_playback_batch(prepared_playback)
}

fn preload_change_events(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    actions: &[StagedPreloadPlaybackAction],
) -> Result<Vec<light_application::EventDraft>, String> {
    let mut after = read_preload_projections(state, context, identities)?
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    let mut transitions = before
        .into_iter()
        .filter_map(|(identity, before)| {
            let projection = after.remove(&identity)?;
            Some((identity, before, projection))
        })
        .collect::<Vec<_>>();
    transitions.sort_by_key(|(identity, before, after)| {
        (
            !playback_was_released(before, after),
            identity_sort_key(*identity),
        )
    });
    Ok(transitions
        .into_iter()
        .filter_map(|(identity, before, projection)| {
            let action = if playback_was_released(&before, &projection) {
                light_application::PlaybackAction::Off { pressed: true }
            } else {
                preload_event_action(actions, identity)
            };
            light_application::committed_playback_effect_event(
                context,
                action,
                None,
                before,
                projection,
                preload_addressed_event_required(actions, identity),
            )
        })
        .collect())
}

fn preload_addressed_event_required(
    actions: &[StagedPreloadPlaybackAction],
    identity: PlaybackIdentity,
) -> bool {
    let PlaybackIdentity::Playback(number) = identity else {
        return false;
    };
    actions
        .iter()
        .filter(|action| action.playback_number == number)
        .any(|action| action.addressed_event_required)
}

fn playback_was_released(before: &PlaybackProjection, after: &PlaybackProjection) -> bool {
    before
        .cue_list_runtime()
        .is_some_and(|runtime| runtime.enabled)
        && !after
            .cue_list_runtime()
            .is_some_and(|runtime| runtime.enabled)
}

fn identity_sort_key(identity: PlaybackIdentity) -> (u8, u128) {
    match identity {
        PlaybackIdentity::Playback(number) => (0, u128::from(number)),
        PlaybackIdentity::CueList(id) => (1, id.0.as_u128()),
    }
}

fn emit_preload_exclusions(
    state: &AppState,
    session: &Session,
    actions: &[StagedPreloadPlaybackAction],
) {
    for action in actions
        .iter()
        .filter(|action| !action.released_playbacks.is_empty())
    {
        emit(
            state,
            "playback_exclusion_applied",
            serde_json::json!({
                "desk_id":session.desk.id,
                "activated_playback":action.playback_number,
                "released_playbacks":action.released_playbacks,
                "source":"preload",
            }),
        );
    }
}

fn executed_preload_actions(
    actions: Vec<StagedPreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> Vec<serde_json::Value> {
    actions
        .into_iter()
        .map(|action| {
            let mut executed = serde_json::json!({
                "playback_number":action.playback_number,
                "action":action.action,
                "surface":action.surface,
                "started_at":committed_at,
                "fallback_millis":programmer_fade_millis
            });
            if let Some(page) = action.page {
                executed["page"] = page.into();
            }
            executed
        })
        .collect()
}

fn persist_preload_commit(
    state: &AppState,
    session: &Session,
    active_playbacks_changed: bool,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Err(error) = persist_programmer(state, session) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "programmer",
            error,
        ));
    }
    if active_playbacks_changed && let Err(error) = persist_active_playbacks(state) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "active playbacks",
            error,
        ));
    }
    warnings
}

fn preload_event_action(
    actions: &[StagedPreloadPlaybackAction],
    identity: light_application::PlaybackRuntimeIdentity,
) -> light_application::PlaybackAction {
    let light_application::PlaybackRuntimeIdentity::Playback(number) = identity else {
        return light_application::PlaybackAction::None { pressed: true };
    };
    if actions
        .iter()
        .any(|action| action.released_playbacks.contains(&number))
    {
        return light_application::PlaybackAction::Off { pressed: true };
    }
    match actions
        .iter()
        .find(|action| action.playback_number == number)
        .map(|action| action.action.as_str())
    {
        Some("toggle") => light_application::PlaybackAction::Toggle { pressed: true },
        Some("go") => light_application::PlaybackAction::Go { pressed: true },
        Some("go-minus") => light_application::PlaybackAction::Back { pressed: true },
        Some("off") => light_application::PlaybackAction::Off { pressed: true },
        Some("on") => light_application::PlaybackAction::On { pressed: true },
        Some("temp-on") => light_application::PlaybackAction::Temporary {
            enabled: true,
            pressed: true,
        },
        Some("temp-off") => light_application::PlaybackAction::Temporary {
            enabled: false,
            pressed: false,
        },
        _ => light_application::PlaybackAction::None { pressed: true },
    }
}
