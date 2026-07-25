use light_application::{
    ActionContext, ActionError, ProgrammingCueActiveShowPorts, ProgrammingCueCommit,
    ProgrammingCueCommitResult,
};

use super::super::{
    AppState, ProgrammingInstallOwner, ProgrammingOwnerGesturePolicy,
    ProgrammingOwnerHighlightPolicy, ServerActiveShowPorts,
};

impl ProgrammingCueActiveShowPorts for ServerActiveShowPorts {}

pub(super) fn commit(
    state: &AppState,
    context: &ActionContext,
    commit: &ProgrammingCueCommit,
) -> Result<ProgrammingCueCommitResult, ActionError> {
    let user_id = context.user_id.ok_or_else(|| {
        ActionError::new(
            light_application::ActionErrorKind::Unauthorized,
            "Cue recording requires an authenticated user",
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
    let result = state
        .active_show_service
        .commit_programming_cue(context, commit, &ports)?;
    publish_legacy_cue_list_change(state, &result);
    Ok(result)
}

/// Mirrors a real retained Show transition to the transient v1 socket without publishing a
/// second authoritative application event. Replay and no-change results have no event sequence.
fn publish_legacy_cue_list_change(state: &AppState, result: &ProgrammingCueCommitResult) {
    let Some(event_sequence) = result.event_sequence else {
        return;
    };
    let projection = &result.projections.cue_list;
    super::super::emit(
        state,
        "show_object_changed",
        serde_json::json!({
            "show_id": result.projections.show_id,
            "kind": projection.kind.as_str(),
            "id": projection.object_id,
            "revision": projection.object_revision,
            "application_event_sequence": event_sequence,
        }),
    );
}
