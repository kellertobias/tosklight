use crate::{ActionContext, ActionError, ActiveShowObjectKind};
use light_core::{Revision, SessionId, ShowId, UserId};
use std::collections::HashMap;
use uuid::Uuid;

const SHOW_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingShowUndoTarget {
    pub show_id: ShowId,
    pub objects: Vec<ProgrammingShowUndoObject>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingShowUndoObject {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
    pub operation: ProgrammingShowUndoOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingShowUndoOperation {
    RestorePrevious,
    DeleteCreated,
}
#[derive(Clone)]
struct ShowHistoryEntry {
    target: ProgrammingShowUndoTarget,
    programmer_undo_depth: usize,
}

#[derive(Default)]
pub(super) struct ShowHistory {
    entries: HashMap<(UserId, Uuid), Vec<ShowHistoryEntry>>,
}

impl ShowHistory {
    pub(super) fn record(
        &mut self,
        user_id: UserId,
        desk_id: Uuid,
        programmer_undo_depth: usize,
        target: ProgrammingShowUndoTarget,
    ) {
        let entries = self.entries.entry((user_id, desk_id)).or_default();
        entries.push(ShowHistoryEntry {
            target,
            programmer_undo_depth,
        });
        if entries.len() > SHOW_HISTORY_LIMIT {
            entries.remove(0);
        }
    }

    pub(super) fn next(
        &self,
        user_id: UserId,
        desk_id: Uuid,
        programmer_undo_depth: usize,
    ) -> Option<ProgrammingShowUndoTarget> {
        self.entries
            .get(&(user_id, desk_id))
            .and_then(|entries| entries.last())
            .filter(|entry| programmer_undo_depth <= entry.programmer_undo_depth)
            .map(|entry| entry.target.clone())
    }

    pub(super) fn finish(
        &mut self,
        user_id: UserId,
        desk_id: Uuid,
        target: &ProgrammingShowUndoTarget,
        current_revision: Revision,
    ) {
        let key = (user_id, desk_id);
        let remove_key = self.entries.get_mut(&key).is_some_and(|entries| {
            if entries.last().is_some_and(|entry| entry.target == *target) {
                entries.pop();
            }
            if let Some(previous) = entries.last_mut()
                && previous.target.show_id == target.show_id
                && previous.target.objects.len() == 1
                && target.objects.len() == 1
                && previous.target.objects[0].kind == target.objects[0].kind
                && previous.target.objects[0].object_id == target.objects[0].object_id
            {
                previous.target.objects[0].expected_object_revision = current_revision;
            }
            entries.is_empty()
        });
        if remove_key {
            self.entries.remove(&key);
        }
    }
}

pub(super) fn history_identity(
    context: &ActionContext,
) -> Result<(SessionId, UserId), ActionError> {
    let session_id = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            crate::ActionErrorKind::Unauthorized,
            "Programmer history requires an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            crate::ActionErrorKind::Unauthorized,
            "Programmer history requires an authenticated user",
        )
    })?;
    Ok((session_id, user_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(show_id: ShowId, revision: Revision) -> ProgrammingShowUndoTarget {
        ProgrammingShowUndoTarget {
            show_id,
            objects: vec![ProgrammingShowUndoObject {
                kind: ActiveShowObjectKind::Group,
                object_id: "1".into(),
                expected_object_revision: revision,
                operation: ProgrammingShowUndoOperation::RestorePrevious,
            }],
        }
    }

    #[test]
    fn recording_history_is_desk_scoped_and_waits_for_newer_programmer_steps() {
        let user = UserId::new();
        let desk = Uuid::new_v4();
        let other_desk = Uuid::new_v4();
        let mut history = ShowHistory::default();
        let target = target(ShowId::new(), 2);
        history.record(user, desk, 3, target.clone());

        assert!(history.next(user, other_desk, 3).is_none());
        assert!(history.next(user, desk, 4).is_none());
        assert_eq!(history.next(user, desk, 3), Some(target));
    }

    #[test]
    fn undo_retargets_the_previous_entry_for_the_new_object_revision() {
        let user = UserId::new();
        let desk = Uuid::new_v4();
        let show_id = ShowId::new();
        let mut history = ShowHistory::default();
        history.record(user, desk, 0, target(show_id, 1));
        history.record(user, desk, 0, target(show_id, 2));

        history.finish(user, desk, &target(show_id, 2), 3);

        assert_eq!(history.next(user, desk, 0), Some(target(show_id, 3)));
    }
}
