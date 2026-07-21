use light_application::{
    ActionContext, ActionError, ActionErrorKind, GroupManagementActiveShowPorts,
    GroupManagementCommit, GroupManagementCommitResult, GroupManagementPorts,
    GroupManagementSelection, ProgrammingPorts,
};
use light_core::SessionId;

use super::super::{
    ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy, ProgrammingOwnerHighlightPolicy,
    ServerActiveShowPorts,
};
use super::programming_ports::ServerProgrammingPorts;

impl GroupManagementPorts for ServerProgrammingPorts<'_> {
    fn authorize_group_management(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn commit_group_management(
        &self,
        context: &ActionContext,
        commit: &GroupManagementCommit,
    ) -> Result<GroupManagementCommitResult, ActionError> {
        let state = self.state();
        let session_id = context.session_id.map(SessionId).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Unauthorized,
                "Group management requires an operator session",
            )
        })?;
        let user_id = context.user_id.ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Unauthorized,
                "Group management requires an authenticated user",
            )
        })?;
        let owner = ProgrammingInstallOwner {
            desk_id: context.desk_id,
            user_id: light_core::UserId(user_id),
            gesture: ProgrammingOwnerGesturePolicy::Preserve,
            highlight: ProgrammingOwnerHighlightPolicy::Reconcile,
        };
        let ports = ServerActiveShowPorts::group_management(state.clone(), owner, session_id);
        state
            .active_show_service
            .commit_group_management(context, commit, &ports)
    }
}

impl GroupManagementActiveShowPorts for ServerActiveShowPorts {
    /// Runs inside the held show-mutation gate, strictly before the owning Show event. It takes no
    /// desk or user gate, so a frozen refresh cannot deadlock on its own desk lock.
    fn apply_frozen_group_selection(
        &self,
        context: &ActionContext,
        selection: &GroupManagementSelection,
    ) {
        let Some(session_id) = self.frozen_selection_session() else {
            return;
        };
        self.state()
            .programming
            .install_frozen_group_selection(context, session_id, selection);
    }
}
