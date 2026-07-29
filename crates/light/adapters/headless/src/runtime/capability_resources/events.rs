use super::*;

impl EventResource {
    pub(in crate::runtime) fn latest_sequence(&self) -> u64 {
        self.application.latest_sequence()
    }

    pub(in crate::runtime) fn publish(
        &self,
        event: light_application::EventDraft,
    ) -> Arc<light_application::EventEnvelope> {
        self.application.publish(event)
    }

    pub(in crate::runtime) fn replay(
        &self,
        after_sequence: u64,
        filter: &light_application::EventFilter,
    ) -> light_application::EventReplay {
        self.application.replay(after_sequence, filter)
    }

    pub(in crate::runtime) fn subscribe(
        &self,
        filter: light_application::EventFilter,
        options: light_application::SubscriptionOptions,
    ) -> light_application::EventSubscription {
        self.application.subscribe(filter, options)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn publish_automatic_playback_events(
        &self,
        changes: impl IntoIterator<Item = light_application::AutomaticPlaybackProjection>,
    ) -> Vec<Arc<light_application::EventEnvelope>> {
        light_application::publish_automatic_playback_events(&self.application, changes)
    }

    pub(in crate::runtime) fn record_audit(&self, kind: &str, payload: serde_json::Value) -> u64 {
        let revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        let event = Event {
            revision,
            kind: kind.into(),
            payload,
        };
        let mut audit = self.audit.lock();
        if audit.len() == Self::AUDIT_CAPACITY {
            audit.pop_front();
        }
        audit.push_back(event);
        revision
    }

    pub(in crate::runtime) fn audit_after(&self, after_revision: u64) -> Vec<Event> {
        self.audit
            .lock()
            .iter()
            .filter(|event| event.revision > after_revision)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn audit_events(&self) -> Vec<Event> {
        self.audit.lock().iter().cloned().collect()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn audit_revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}
