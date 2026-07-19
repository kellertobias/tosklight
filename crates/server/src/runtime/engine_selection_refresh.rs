use super::{AppState, Session, reconcile_highlight_selection};
use light_application::{
    ActionContext, ProgrammingSelectionRefreshResult, ProgrammingSelectionTarget,
};
use light_core::{SessionId, UserId};
use light_engine::PreparedEngineSnapshot;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(super) struct ProgrammingInstallOwner {
    pub(super) desk_id: Uuid,
    pub(super) user_id: UserId,
}

#[derive(Clone, Copy)]
pub(super) enum PlaybackInstallPolicy {
    Preserve,
    Release,
}

#[derive(Clone, Copy)]
pub(super) enum HighlightInstallPolicy {
    Reconcile,
    Clear,
}

/// Installs one already-prepared runtime while publishing every desk selection that the new Group
/// generation changes. The caller owns the activation boundary. `owner` identifies an outer
/// Programming interaction whose actor desk must not be re-locked. Its final selection is
/// published inside the install boundary, and the outer interaction suppresses that already-sent
/// component while still publishing any command-line change. The exact owner Highlight context is
/// likewise reconciled only by the outer boundary.
pub(super) fn install_prepared_snapshot_with_selection_refresh(
    state: &AppState,
    context: &ActionContext,
    prepared: PreparedEngineSnapshot,
    owner: Option<ProgrammingInstallOwner>,
    playback: PlaybackInstallPolicy,
    highlight: HighlightInstallPolicy,
) -> ProgrammingSelectionRefreshResult<()> {
    let groups_changed = selection_topology(&state.engine.snapshot().groups)
        != selection_topology(&prepared.snapshot().groups);
    if !groups_changed {
        install(state, prepared, playback);
        return ProgrammingSelectionRefreshResult {
            output: (),
            events: Vec::new(),
        };
    }

    let owned_target = owner.and_then(|owner| selection_target(state, owner.desk_id));
    let targets = selection_targets(state, owner.map(|owner| owner.desk_id));
    let highlight_sessions = matches!(highlight, HighlightInstallPolicy::Reconcile)
        .then(|| highlight_sessions(state, owner))
        .unwrap_or_default();
    let install = || {
        install(state, prepared, playback);
        for session in highlight_sessions {
            reconcile_highlight_selection(state, &session, "show_selection_refresh");
        }
    };
    match owned_target {
        Some(owned_target) => state.programming.run_selection_refresh_with_owned_target(
            context,
            owned_target,
            targets,
            install,
        ),
        None => state
            .programming
            .run_selection_refresh(context, targets, install),
    }
}

fn install(state: &AppState, prepared: PreparedEngineSnapshot, policy: PlaybackInstallPolicy) {
    match policy {
        PlaybackInstallPolicy::Preserve => state.engine.install_prepared_snapshot(prepared),
        PlaybackInstallPolicy::Release => state
            .engine
            .install_prepared_snapshot_releasing_playback(prepared),
    }
}

fn selection_targets(
    state: &AppState,
    owned_desk: Option<Uuid>,
) -> Vec<ProgrammingSelectionTarget> {
    let mut sessions = state
        .sessions
        .read()
        .values()
        .filter(|session| Some(session.desk.id) != owned_desk)
        .cloned()
        .collect::<Vec<_>>();
    sessions.sort_unstable_by_key(|session| (session.desk.id, session.id.0));
    sessions.dedup_by_key(|session| session.desk.id);
    sessions
        .into_iter()
        .filter_map(|session| selection_target(state, session.desk.id))
        .collect()
}

fn selection_target(state: &AppState, desk_id: Uuid) -> Option<ProgrammingSelectionTarget> {
    let interaction_id = SessionId(desk_id);
    state
        .programmers
        .selection(interaction_id)
        .map(|_| ProgrammingSelectionTarget {
            desk_id,
            interaction_id,
        })
}

fn highlight_sessions(state: &AppState, owner: Option<ProgrammingInstallOwner>) -> Vec<Session> {
    let mut sessions = state
        .sessions
        .read()
        .values()
        .filter(|session| {
            owner.is_none_or(|owner| {
                (session.desk.id, session.user.id) != (owner.desk_id, owner.user_id)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    sessions.sort_unstable_by_key(|session| (session.desk.id, session.user.id.0, session.id.0));
    let mut seen = HashSet::new();
    sessions.retain(|session| seen.insert((session.desk.id, session.user.id)));
    sessions
}

fn selection_topology(
    definitions: &[light_programmer::GroupDefinition],
) -> BTreeMap<String, Option<Vec<light_core::FixtureId>>> {
    let groups = definitions
        .iter()
        .cloned()
        .map(|group| (group.id.clone(), group))
        .collect::<HashMap<_, _>>();
    groups
        .keys()
        .map(|group_id| {
            (
                group_id.clone(),
                light_programmer::resolve_group(group_id, &groups).ok(),
            )
        })
        .collect()
}
