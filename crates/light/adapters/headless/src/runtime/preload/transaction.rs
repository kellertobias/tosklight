use super::*;

pub(super) fn commit_preload_transaction(
    state: &AppState,
    session: &Session,
    context: light_application::ActionContext,
) -> Result<CommittedPreload, String> {
    let preparation::PreparedPreloadCommit {
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        staged_actions,
        before,
        context,
    } = preparation::prepare_preload_commit(state, session, context)?;
    let playback_runtime_changed = prepared_playback.effect().durable();
    // The outer Programmer transaction restores activation/queue on any preparation error.
    // Final Dynamic owners and all fallible event projections are built before either Live swap.
    let (changes, dynamic_runtime_changed) = install_preload_commit(
        state,
        session,
        pending,
        committed_at,
        programmer_fade_millis,
        prepared_playback,
        |snapshot, playback, runtime| {
            let interval = 1_000_u64.div_ceil(u64::from(state.output.frame_rate_hz().max(1)));
            let after = playback_service::prepared_runtime_projections(
                snapshot, &before, playback, runtime, interval,
            )?;
            events::preload_change_events(&context, before, after, &staged_actions)
        },
    )?;
    if dynamic_runtime_changed {
        state
            .events
            .publish(light_application::EventDraft::dynamic_runtime_changed(
                Some(&context),
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::PreloadCommitted,
                    dynamic_id: None,
                    runtime_instance_id: None,
                    controller_id: None,
                    winning_controller_id: None,
                    occurred_at_millis: u64::try_from(committed_at.timestamp_millis())
                        .unwrap_or_default(),
                    message: None,
                },
            ));
    }
    events::emit_exclusions(state, session, &changes);
    let executed_projection = executed_preload_projection(&staged_actions);
    let executed = executed_preload_actions(staged_actions, committed_at, programmer_fade_millis);
    let warnings = persist_preload_commit(
        state,
        session,
        playback_runtime_changed,
        dynamic_runtime_changed,
    );
    Ok(CommittedPreload {
        committed_at,
        programmer_fade_millis,
        executed,
        warnings,
        runtime_changes: changes.changes,
        executed_projection,
    })
}

pub(super) struct CommittedPreload {
    pub(super) committed_at: chrono::DateTime<chrono::Utc>,
    pub(super) programmer_fade_millis: u64,
    pub(super) executed: Vec<serde_json::Value>,
    pub(super) warnings: Vec<String>,
    pub(super) runtime_changes: Vec<(
        light_application::EventDraft,
        light_application::PlaybackRuntimeProjection,
    )>,
    pub(super) executed_projection:
        Vec<light_application::ProgrammingPreloadExecutedPlaybackAction>,
}

fn executed_preload_projection(
    actions: &[StagedPreloadPlaybackAction],
) -> Vec<light_application::ProgrammingPreloadExecutedPlaybackAction> {
    actions
        .iter()
        .map(
            |action| light_application::ProgrammingPreloadExecutedPlaybackAction {
                playback_number: action.playback_number,
                page: action.page,
                action: application_queue_action(action.action),
                surface: application_queue_surface(action.surface),
            },
        )
        .collect()
}

const fn application_queue_action(
    action: light_programmer::PreloadPlaybackQueueAction,
) -> light_application::ProgrammingPreloadPlaybackAction {
    use light_application::ProgrammingPreloadPlaybackAction as Application;
    use light_programmer::PreloadPlaybackQueueAction as Domain;
    match action {
        Domain::Toggle => Application::Toggle,
        Domain::Go => Application::Go,
        Domain::Back => Application::Back,
        Domain::Off => Application::Off,
        Domain::On => Application::On,
        Domain::TemporaryOn => Application::TemporaryOn,
        Domain::TemporaryOff => Application::TemporaryOff,
        Domain::DynamicPause => Application::DynamicPause,
        Domain::DynamicRestart => Application::DynamicRestart,
        Domain::DynamicDoubleSpeed => Application::DynamicDoubleSpeed,
        Domain::DynamicHalfSpeed => Application::DynamicHalfSpeed,
        Domain::DynamicLearnSpeed => Application::DynamicLearnSpeed,
        Domain::Fader { value_permyriad } => Application::Fader { value_permyriad },
    }
}

const fn application_queue_surface(
    surface: light_programmer::PreloadPlaybackQueueSurface,
) -> light_application::ProgrammingPreloadPlaybackSurface {
    use light_application::ProgrammingPreloadPlaybackSurface as Application;
    use light_programmer::PreloadPlaybackQueueSurface as Domain;
    match surface {
        Domain::Physical => Application::Physical,
        Domain::Virtual => Application::Virtual,
        Domain::Osc => Application::Osc,
        Domain::Matter => Application::Matter,
    }
}

fn install_preload_commit<T>(
    state: &AppState,
    session: &Session,
    pending: Vec<light_programmer::PreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
    prepared_playback: light_engine::PreparedPlaybackBatch,
    prepare: impl FnOnce(
        &EngineSnapshot,
        &light_playback::PlaybackEngine,
        &light_dynamics::DynamicRuntimeSnapshot,
    ) -> Result<T, String>,
) -> Result<(T, bool), String> {
    state.programming.activate_preload_at_with_fade(
        session.id,
        committed_at,
        programmer_fade_millis,
    );
    let drained = state.programming.take_preload_playback_actions(session.id);
    if drained != pending {
        return Err("the Preload queue changed while GO was being prepared".into());
    }
    state
        .output
        .install_preload_playback_batch(prepared_playback, committed_at, prepare)
}

fn executed_preload_actions(
    actions: Vec<StagedPreloadPlaybackAction>,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> Vec<serde_json::Value> {
    actions
        .into_iter()
        .map(|action| executed_preload_action(action, committed_at, programmer_fade_millis))
        .collect()
}

fn executed_preload_action(
    action: StagedPreloadPlaybackAction,
    committed_at: chrono::DateTime<chrono::Utc>,
    programmer_fade_millis: u64,
) -> serde_json::Value {
    let mut executed = serde_json::json!({
        "playback_number":action.playback_number,
        "action":action.action.legacy_name(),
        "surface":action.surface.name(),
        "started_at":committed_at,
        "fallback_millis":programmer_fade_millis
    });
    if let Some(page) = action.page {
        executed["page"] = page.into();
    }
    executed
}

fn persist_preload_commit(
    state: &AppState,
    session: &Session,
    active_playbacks_changed: bool,
    dynamic_runtime_changed: bool,
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
    if dynamic_runtime_changed && let Err(error) = persist_output_runtime(state) {
        warnings.push(record_preload_persistence_failure(
            state,
            session,
            "Dynamic output runtime",
            error,
        ));
    }
    warnings
}
