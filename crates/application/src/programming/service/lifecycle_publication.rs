use super::ProgrammingService;
use crate::{
    ActionContext, EventDraft, EventSource, ProgrammingLifecycleChange,
    ProgrammingLifecycleProgrammer, ProgrammingLifecycleProjection, ProgrammingLifecycleSnapshot,
    ProgrammingPorts,
};
use light_core::UserId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct LifecyclePublicationGate {
    revision: u64,
    last_published: HashMap<UserId, Option<ProgrammingLifecycleProgrammer>>,
}

impl ProgrammingService {
    /// Run one adapter-owned session lifecycle transition under the target user's authority gate.
    pub fn run_lifecycle_transition<T>(
        &self,
        context: &ActionContext,
        user_id: UserId,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.programmers.with_user_serialized(user_id, || {
            let before = self.active_lifecycle_programmer(user_id);
            let output = operation();
            self.publish_lifecycle_for_user(context, user_id, before);
            output
        })
    }

    /// Return an authenticated installation-wide lifecycle snapshot with one safe event cursor.
    pub fn lifecycle_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingLifecycleSnapshot, crate::ActionError> {
        ports.authorize(context)?;
        Ok(self
            .programmers
            .read_active_programmer_lifecycles(|summaries| {
                let publication = self.lifecycle_publication.lock();
                let programmers = summaries.into_iter().map(Into::into).collect();
                ProgrammingLifecycleSnapshot {
                    event_sequence: self.events.latest_sequence(),
                    projection: ProgrammingLifecycleProjection {
                        revision: publication.revision,
                        programmers,
                    },
                }
            }))
    }

    pub(in crate::programming) fn active_lifecycle_programmer(
        &self,
        user_id: UserId,
    ) -> Option<ProgrammingLifecycleProgrammer> {
        self.programmers
            .programmer_lifecycle(user_id)
            .filter(|summary| summary.connected)
            .map(Into::into)
    }

    pub(super) fn publish_lifecycle_for_context(
        &self,
        context: &ActionContext,
        before: Option<ProgrammingLifecycleProgrammer>,
    ) -> Option<u64> {
        let user_id = context.user_id.map(UserId)?;
        self.publish_lifecycle_for_user(context, user_id, before)
    }

    pub(in crate::programming) fn publish_lifecycle_for_user(
        &self,
        context: &ActionContext,
        user_id: UserId,
        before: Option<ProgrammingLifecycleProgrammer>,
    ) -> Option<u64> {
        let after = self.active_lifecycle_programmer(user_id);
        self.publish_lifecycle_transition(
            before,
            after,
            EventSource::Action(context.source),
            Some(context.correlation_id),
        )
    }

    fn publish_lifecycle_transition(
        &self,
        before: Option<ProgrammingLifecycleProgrammer>,
        after: Option<ProgrammingLifecycleProgrammer>,
        source: EventSource,
        correlation_id: Option<uuid::Uuid>,
    ) -> Option<u64> {
        if before == after {
            return None;
        }
        let user_id = lifecycle_user(&before, &after)?;
        let mut publication = self.lifecycle_publication.lock();
        if publication
            .last_published
            .get(&user_id)
            .is_some_and(|published| published == &after)
        {
            return None;
        }
        publication.revision = publication.revision.saturating_add(1);
        let change = lifecycle_change(publication.revision, before, after.clone())?;
        let sequence = self
            .events
            .publish(EventDraft::programming_lifecycle_changed(
                change,
                source,
                correlation_id,
            ))
            .sequence;
        publication.last_published.insert(user_id, after);
        Some(sequence)
    }
}

fn lifecycle_user(
    before: &Option<ProgrammingLifecycleProgrammer>,
    after: &Option<ProgrammingLifecycleProgrammer>,
) -> Option<UserId> {
    after
        .as_ref()
        .or(before.as_ref())
        .map(|programmer| programmer.user_id)
}

fn lifecycle_change(
    revision: u64,
    before: Option<ProgrammingLifecycleProgrammer>,
    after: Option<ProgrammingLifecycleProgrammer>,
) -> Option<ProgrammingLifecycleChange> {
    match (before, after) {
        (_, Some(programmer)) => Some(ProgrammingLifecycleChange::upsert(revision, programmer)),
        (Some(programmer), None) => Some(ProgrammingLifecycleChange::remove(
            revision,
            programmer.programmer_id,
        )),
        (None, None) => None,
    }
}
