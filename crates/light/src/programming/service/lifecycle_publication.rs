use super::ProgrammingService;
use crate::{
    ActionContext, EventDraft, EventSource, ProgrammingLifecycleChange,
    ProgrammingLifecycleProgrammer, ProgrammingLifecycleProjection, ProgrammingLifecycleSnapshot,
    ProgrammingPorts,
};

#[derive(Default)]
pub(super) struct LifecyclePublicationGate {
    revision: u64,
    /// What the desk last told its surfaces. `None` means nothing has been published yet, which
    /// is not the same as having published that there is no Programmer.
    last_published: Option<Option<ProgrammingLifecycleProgrammer>>,
}

impl ProgrammingService {
    /// Run one adapter-owned session lifecycle transition under the desk's authority gate.
    pub fn run_lifecycle_transition<T>(
        &self,
        context: &ActionContext,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.programmers.serialized(|| {
            let before = self.active_lifecycle_programmer();
            let output = operation();
            self.publish_lifecycle(context, before);
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
    ) -> Option<ProgrammingLifecycleProgrammer> {
        self.programmers
            .programmer_lifecycle()
            .filter(|summary| summary.connected)
            .map(Into::into)
    }

    pub(in crate::programming) fn publish_lifecycle(
        &self,
        context: &ActionContext,
        before: Option<ProgrammingLifecycleProgrammer>,
    ) -> Option<u64> {
        let after = self.active_lifecycle_programmer();
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
        let mut publication = self.lifecycle_publication.lock();
        if publication
            .last_published
            .as_ref()
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
        publication.last_published = Some(after);
        Some(sequence)
    }
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
