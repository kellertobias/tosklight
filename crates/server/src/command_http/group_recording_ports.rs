use light_application::{
    ActionContext, ActionError, ProgrammingGroupActiveShowPorts, ProgrammingGroupCommit,
    ProgrammingGroupCommitResult, ProgrammingGroupRecordingPorts, ProgrammingPorts,
};

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts,
};
use super::programming_ports::ServerProgrammingPorts;

impl ProgrammingGroupActiveShowPorts for ServerActiveShowPorts {}

impl ProgrammingGroupRecordingPorts for ServerProgrammingPorts<'_> {
    fn authorize_group_recording(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn commit_group(
        &self,
        context: &ActionContext,
        request: &ProgrammingGroupCommit,
    ) -> Result<ProgrammingGroupCommitResult, ActionError> {
        commit(self.state(), context, request)
    }
}

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
