use light_core::{ProgrammerId, SessionId, UserId};
use light_programmer::{
    ProgrammerLifecycleSession, ProgrammerLifecycleSummary, ProgrammerRegistry,
};
use std::sync::Arc;

/// One connected session reduced to lifecycle information safe for cross-user views.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleSession {
    pub session_id: SessionId,
}

/// One connected user's Programmer without recordable content or private interaction details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleProgrammer {
    pub programmer_id: ProgrammerId,
    pub user_id: UserId,
    pub connected: bool,
    pub selected_fixture_count: u64,
    pub normal_value_count: u64,
    pub sessions: Vec<ProgrammingLifecycleSession>,
}

/// Installation-scoped authoritative list of active Programmers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleProjection {
    pub revision: u64,
    pub programmers: Vec<ProgrammingLifecycleProgrammer>,
}

impl ProgrammingLifecycleProjection {
    pub fn active(programmers: &ProgrammerRegistry, revision: u64) -> Self {
        Self {
            revision,
            programmers: programmers
                .active_programmer_lifecycles()
                .into_iter()
                .map(ProgrammingLifecycleProgrammer::from)
                .collect(),
        }
    }
}

/// One lossless lifecycle delta. Invalid both/neither upsert/remove states are unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingLifecycleDelta {
    Upsert {
        programmer: Arc<ProgrammingLifecycleProgrammer>,
    },
    Remove {
        programmer_id: ProgrammerId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleChange {
    pub revision: u64,
    pub delta: ProgrammingLifecycleDelta,
}

impl ProgrammingLifecycleChange {
    pub fn upsert(revision: u64, programmer: ProgrammingLifecycleProgrammer) -> Self {
        Self {
            revision,
            delta: ProgrammingLifecycleDelta::Upsert {
                programmer: Arc::new(programmer),
            },
        }
    }

    pub const fn remove(revision: u64, programmer_id: ProgrammerId) -> Self {
        Self {
            revision,
            delta: ProgrammingLifecycleDelta::Remove { programmer_id },
        }
    }
}

/// Cursor-bound gap-repair snapshot for the installation lifecycle stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLifecycleSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingLifecycleProjection,
}

impl From<ProgrammerLifecycleSession> for ProgrammingLifecycleSession {
    fn from(session: ProgrammerLifecycleSession) -> Self {
        Self {
            session_id: session.session_id,
        }
    }
}

impl From<ProgrammerLifecycleSummary> for ProgrammingLifecycleProgrammer {
    fn from(summary: ProgrammerLifecycleSummary) -> Self {
        Self {
            programmer_id: summary.programmer_id,
            user_id: summary.user_id,
            connected: summary.connected,
            selected_fixture_count: summary.selected_fixture_count,
            normal_value_count: summary.normal_value_count,
            sessions: summary
                .connected_sessions
                .into_iter()
                .map(ProgrammingLifecycleSession::from)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ApplicationEvent, DeliveryPolicy, EventCapability, EventDraft, EventSource,
        ProgrammingEvent,
    };
    use light_core::{SessionId, UserId};
    use uuid::Uuid;

    #[test]
    fn active_projection_preserves_domain_order_and_omits_disconnected_users() {
        let registry = ProgrammerRegistry::default();
        let disconnected = UserId(Uuid::from_u128(30));
        let later = UserId(Uuid::from_u128(20));
        let earlier = UserId(Uuid::from_u128(10));
        let disconnected_session = SessionId(Uuid::from_u128(1));
        registry.start(disconnected_session, disconnected);
        registry.start(SessionId(Uuid::from_u128(2)), later);
        registry.start(SessionId(Uuid::from_u128(3)), earlier);
        registry.disconnect(disconnected_session);

        let projection = ProgrammingLifecycleProjection::active(&registry, 7);

        assert_eq!(projection.revision, 7);
        assert_eq!(projection.programmers.len(), 2);
        assert_eq!(projection.programmers[0].user_id, earlier);
        assert_eq!(projection.programmers[1].user_id, later);
        assert!(projection.programmers.iter().all(|row| row.connected));
    }

    #[test]
    fn change_models_exactly_one_upsert_or_remove() {
        let programmer_id = ProgrammerId(Uuid::from_u128(1));
        let programmer = ProgrammingLifecycleProgrammer {
            programmer_id,
            user_id: UserId(Uuid::from_u128(2)),
            connected: true,
            selected_fixture_count: 0,
            normal_value_count: 0,
            sessions: Vec::new(),
        };

        assert!(matches!(
            ProgrammingLifecycleChange::upsert(1, programmer).delta,
            ProgrammingLifecycleDelta::Upsert { .. }
        ));
        assert_eq!(
            ProgrammingLifecycleChange::remove(2, programmer_id).delta,
            ProgrammingLifecycleDelta::Remove { programmer_id }
        );
    }

    #[test]
    fn lifecycle_event_is_global_lossless_and_not_user_scoped() {
        let change = ProgrammingLifecycleChange::remove(2, ProgrammerId(Uuid::from_u128(1)));
        let draft =
            EventDraft::programming_lifecycle_changed(change.clone(), EventSource::Runtime, None);

        assert_eq!(draft.desk_id, None);
        assert_eq!(draft.delivery, DeliveryPolicy::Lossless);
        let object = draft.object.unwrap();
        assert_eq!(object.capability, EventCapability::Programmer);
        assert_eq!(object.id, "programming-lifecycle");
        assert_eq!(object.programming_user_id(), None);
        assert_eq!(
            draft.payload,
            ApplicationEvent::Programming(ProgrammingEvent::LifecycleChanged(change))
        );
    }
}
