use super::candidate::{PreparedGroupManagement, prepare_group_management};
use super::{GroupManagementActiveShowPorts, GroupManagementCommit, GroupManagementCommitResult};
use crate::active_show::CompletedActiveShowTransaction;
use crate::{
    ActionContext, ActionError, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft,
};

impl ActiveShowService {
    pub fn commit_group_management<P>(
        &self,
        context: &ActionContext,
        commit: &GroupManagementCommit,
        ports: &P,
    ) -> Result<GroupManagementCommitResult, ActionError>
    where
        P: GroupManagementActiveShowPorts,
    {
        self.transact_with_unit(
            context,
            commit.show_id,
            ports,
            commit.operation().backup_label(),
            |unit| prepare_group_management(ports, unit, commit),
            complete_group_management,
        )
    }
}

/// Publishes the desk selection produced by a frozen refresh strictly before the owning Show
/// event, while the show-mutation ordering gate is still held.
fn complete_group_management<P: GroupManagementActiveShowPorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedGroupManagement>,
) -> GroupManagementCommitResult {
    let mut result = completed.state.result;
    if let Some(selection) = result.selection.as_ref() {
        ports.apply_frozen_group_selection(context, selection);
    }
    let Some(commit) = completed.commit else {
        return result;
    };
    result.show_revision = commit.revision();
    let change = ActiveShowObjectChange {
        kind: ActiveShowObjectKind::Group,
        object_id: result.projection.object_id.clone(),
        object_revision: result.projection.object_revision,
        body: Some(result.projection.raw_body.as_ref().clone()),
        deleted: false,
    };
    result.event_sequence = Some(
        events
            .publish(EventDraft::active_show_objects_changed(
                context,
                ActiveShowObjectsChange {
                    show_id: result.projection.show_id,
                    show_revision: result.show_revision,
                    changes: vec![change],
                },
            ))
            .sequence,
    );
    result
}
