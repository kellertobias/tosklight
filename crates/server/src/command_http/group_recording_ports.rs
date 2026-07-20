use light_application::{
    ActionContext, ActionError, ProgrammingGroupActiveShowPorts, ProgrammingGroupCommit,
    ProgrammingGroupCommitResult,
};

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts,
};

impl ProgrammingGroupActiveShowPorts for ServerActiveShowPorts {}

pub(super) fn commit(
    state: &AppState,
    context: &ActionContext,
    commit: &ProgrammingGroupCommit,
) -> Result<ProgrammingGroupCommitResult, ActionError> {
    let user_id = context.user_id.ok_or_else(|| {
        ActionError::new(
            light_application::ActionErrorKind::Unauthorized,
            "Group recording requires an authenticated user",
        )
    })?;
    let owner = ProgrammingInstallOwner {
        desk_id: context.desk_id,
        user_id: light_core::UserId(user_id),
        gesture: if commit.finishes_actor_gesture() {
            ProgrammingOwnerGesturePolicy::Finish(commit.actor_session_id())
        } else {
            ProgrammingOwnerGesturePolicy::Preserve
        },
        highlight: if commit.within_interaction() {
            ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction
        } else {
            ProgrammingOwnerHighlightPolicy::Reconcile
        },
    };
    let ports = ServerActiveShowPorts::show_objects_with_programming_owner(state.clone(), owner);
    state
        .active_show_service
        .commit_programming_group(context, commit, &ports)
}
