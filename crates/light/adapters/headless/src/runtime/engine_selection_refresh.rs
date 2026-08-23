use super::{AppState, Session, reconcile_highlight_selection};
use light_application::{ActionContext, ProgrammingSelectionRefreshResult};
use light_core::SessionId;
use light_engine::PreparedEngineSnapshot;
use std::collections::{BTreeMap, HashMap};

/// An outer Programming interaction already holding the desk, on whose behalf this install runs.
#[derive(Clone, Copy)]
pub(super) struct ProgrammingInstallOwner {
    pub(super) gesture: ProgrammingOwnerGesturePolicy,
    pub(super) highlight: ProgrammingOwnerHighlightPolicy,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ProgrammingOwnerGesturePolicy {
    Preserve,
    Finish(SessionId),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ProgrammingOwnerHighlightPolicy {
    Reconcile,
    DeferToOuterInteraction,
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

/// Installs one already-prepared runtime while publishing the desk selection that the new Group
/// generation changes. The caller owns the activation boundary. `owner` marks an outer Programming
/// interaction already holding the desk, which must not be re-locked. The desk's final selection is
/// published inside the install boundary, and the outer interaction suppresses that already-sent
/// component while still publishing any command-line change. The desk's Highlight context is
/// either reconciled here or deferred to the outer boundary according to the owner policy.
pub(super) fn install_prepared_snapshot_with_selection_refresh(
    state: &AppState,
    context: &ActionContext,
    prepared: PreparedEngineSnapshot,
    owner: Option<ProgrammingInstallOwner>,
    playback: PlaybackInstallPolicy,
    highlight: HighlightInstallPolicy,
) -> ProgrammingSelectionRefreshResult<()> {
    let groups_changed = selection_topology(&state.output.snapshot().groups)
        != selection_topology(&prepared.snapshot().groups);
    let finish_owner = owner
        .is_some_and(|owner| matches!(owner.gesture, ProgrammingOwnerGesturePolicy::Finish(_)));
    let desk_context = state.programming.desk_interaction_context();
    // An owner already holds the desk's interaction, so its own pending choice is not a reason to
    // publish again; without one, any pending choice on the desk is.
    let pending_choice = state
        .programming
        .has_pending_command_choices_except_context(owner.and(desk_context));
    if !groups_changed && !finish_owner && !pending_choice {
        install(state, prepared, playback);
        return ProgrammingSelectionRefreshResult {
            output: (),
            events: Vec::new(),
        };
    }

    let highlight_sessions =
        if groups_changed && matches!(highlight, HighlightInstallPolicy::Reconcile) {
            highlight_sessions(state, owner)
        } else {
            Vec::new()
        };
    let install = || {
        install(state, prepared, playback);
        state
            .programming
            .clear_pending_command_choices_except_context(owner.and(desk_context));
        finish_owned_selection_gesture(state, owner);
        for session in highlight_sessions {
            reconcile_highlight_selection(state, &session, "show_selection_refresh");
        }
    };
    match owner {
        Some(_) => state
            .programming
            .run_selection_refresh_within_interaction(context, install),
        None => state.programming.run_selection_refresh(context, install),
    }
}

fn finish_owned_selection_gesture(state: &AppState, owner: Option<ProgrammingInstallOwner>) {
    if let Some(ProgrammingInstallOwner {
        gesture: ProgrammingOwnerGesturePolicy::Finish(session_id),
        ..
    }) = owner
    {
        state
            .programming
            .finish_selection_gesture_within_interaction(session_id);
    }
}

fn install(state: &AppState, prepared: PreparedEngineSnapshot, policy: PlaybackInstallPolicy) {
    match policy {
        PlaybackInstallPolicy::Preserve => state.output.install_prepared_snapshot(prepared),
        PlaybackInstallPolicy::Release => state
            .output
            .install_prepared_snapshot_releasing_playback(prepared),
    }
}

/// The desk's Highlight context, reconciled here unless an outer interaction will do it.
///
/// One desk means one Highlight context, so this is at most one session — the lowest-numbered
/// connected one, chosen deterministically so repeated installs reconcile the same surface.
fn highlight_sessions(state: &AppState, owner: Option<ProgrammingInstallOwner>) -> Vec<Session> {
    if owner.is_some_and(|owner| {
        owner.highlight == ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction
    }) {
        return Vec::new();
    }
    let mut sessions = state.sessions.sessions();
    sessions.sort_unstable_by_key(|session| session.id.0);
    sessions.into_iter().take(1).collect()
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
