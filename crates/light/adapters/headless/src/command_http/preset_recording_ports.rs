use light_application::{
    ActionContext, ActionError, ProgrammingPorts, ProgrammingPresetActiveShowPorts,
    ProgrammingPresetCommit, ProgrammingPresetCommitResult, ProgrammingPresetRecordingPorts,
};

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts,
};
use super::programming_ports::ServerProgrammingPorts;

impl ProgrammingPresetActiveShowPorts for ServerActiveShowPorts {}

impl ProgrammingPresetRecordingPorts for ServerProgrammingPorts<'_> {
    fn authorize_preset_recording(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn commit_preset(
        &self,
        context: &ActionContext,
        request: &ProgrammingPresetCommit,
    ) -> Result<ProgrammingPresetCommitResult, ActionError> {
        commit(self.state(), context, request)
    }
}

pub(super) fn commit(
    state: &AppState,
    context: &ActionContext,
    commit: &ProgrammingPresetCommit,
) -> Result<ProgrammingPresetCommitResult, ActionError> {
    let user_id = context.user_id.ok_or_else(|| {
        ActionError::new(
            light_application::ActionErrorKind::Unauthorized,
            "Preset recording requires an authenticated user",
        )
    })?;
    let ports = ServerActiveShowPorts::show_objects_with_programming_owner(
        state.clone(),
        ProgrammingInstallOwner {
            desk_id: context.desk_id,
            user_id: light_core::UserId(user_id),
            gesture: ProgrammingOwnerGesturePolicy::Preserve,
            highlight: ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction,
        },
    );
    state
        .active_show_service
        .commit_programming_preset(context, commit, &ports)
}
