use light_application::{
    ActionContext, ActionError, ProgrammingPresetActiveShowPorts, ProgrammingPresetCommit,
    ProgrammingPresetCommitResult,
};

use super::super::{AppState, ProgrammingInstallOwner, ServerActiveShowPorts};

impl ProgrammingPresetActiveShowPorts for ServerActiveShowPorts {}

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
        },
    );
    state
        .active_show_service
        .commit_programming_preset(context, commit, &ports)
}
