use crate::{ProgrammerRegistry, ProgrammerState};
use light_core::{ProgrammerId, SessionId, UserId};
use std::collections::{HashMap, HashSet};

/// One currently connected control session without its private interaction content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammerLifecycleSession {
    pub session_id: SessionId,
}

/// Lightweight ownership and activity summary for one retained user Programmer.
///
/// Values, selected fixture identities, commands, modes, priority, Highlight, transient values,
/// Preload details, and Undo/Redo snapshots deliberately remain outside this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammerLifecycleSummary {
    pub programmer_id: ProgrammerId,
    pub user_id: UserId,
    pub connected: bool,
    pub connected_sessions: Vec<ProgrammerLifecycleSession>,
    pub selected_fixture_count: u64,
    pub normal_value_count: u64,
}

impl ProgrammerRegistry {
    /// Read one retained user authority without cloning its complete Programmer state.
    pub fn programmer_lifecycle(&self, user_id: UserId) -> Option<ProgrammerLifecycleSummary> {
        self.with_user_serialized(user_id, || self.lifecycle_for_user(user_id))
    }

    /// Read only connected Programmer authorities in deterministic user/Programmer order.
    pub fn active_programmer_lifecycles(&self) -> Vec<ProgrammerLifecycleSummary> {
        self.read_active_programmer_lifecycles(std::convert::identity)
    }

    /// Assemble a safe installation snapshot while every current user mutation gate is held.
    ///
    /// The reader should stay small: this boundary exists so an application cursor/revision can
    /// be paired with the exact summaries without a completed mutation slipping between them.
    pub fn read_active_programmer_lifecycles<R>(
        &self,
        reader: impl FnOnce(Vec<ProgrammerLifecycleSummary>) -> R,
    ) -> R {
        self.with_all_mutation_gates(|| reader(self.lifecycle_summaries(true)))
    }

    /// Resolve connected user authorities sharing one desk interaction context.
    pub fn lifecycle_users_for_interaction(&self, context: SessionId) -> Vec<UserId> {
        let states = self.states.read();
        let sessions = self.sessions.read();
        let command_contexts = self.command_contexts.read();
        let mut users = sessions
            .iter()
            .filter(|(session, _)| {
                command_contexts.get(session).copied().unwrap_or(**session) == context
            })
            .filter_map(|(_, key)| states.get(key).map(|state| state.user_id))
            .collect::<Vec<_>>();
        users.sort_unstable_by_key(|user| user.0);
        users.dedup();
        users
    }

    fn lifecycle_for_user(&self, user_id: UserId) -> Option<ProgrammerLifecycleSummary> {
        let states = self.states.read();
        let (key, state) = states.iter().find(|(_, state)| state.user_id == user_id)?;
        let sessions = self.sessions.read();
        let command_contexts = self.command_contexts.read();
        let selections = self.selection_contexts.read();
        Some(lifecycle_summary(
            *key,
            state,
            &sessions,
            &command_contexts,
            &selections,
        ))
    }

    fn lifecycle_summaries(&self, connected_only: bool) -> Vec<ProgrammerLifecycleSummary> {
        let states = self.states.read();
        let sessions = self.sessions.read();
        let command_contexts = self.command_contexts.read();
        let selections = self.selection_contexts.read();
        let connected_keys = sessions.values().copied().collect::<HashSet<_>>();
        let mut summaries = states
            .iter()
            .filter(|(key, _)| !connected_only || connected_keys.contains(key))
            .map(|(key, state)| {
                lifecycle_summary(*key, state, &sessions, &command_contexts, &selections)
            })
            .collect::<Vec<_>>();
        summaries.sort_unstable_by_key(|summary| (summary.user_id.0, summary.programmer_id.0));
        summaries
    }
}

fn lifecycle_summary(
    key: SessionId,
    state: &ProgrammerState,
    sessions: &HashMap<SessionId, SessionId>,
    command_contexts: &HashMap<SessionId, SessionId>,
    selections: &HashMap<SessionId, crate::selection::SelectionContext>,
) -> ProgrammerLifecycleSummary {
    ProgrammerLifecycleSummary {
        programmer_id: state.id,
        user_id: state.user_id,
        connected: sessions.values().any(|bound| *bound == key),
        connected_sessions: connected_sessions(key, sessions),
        selected_fixture_count: selected_fixture_count(key, sessions, command_contexts, selections),
        normal_value_count: value_count(state.values.len(), &state.group_values),
    }
}

fn connected_sessions(
    key: SessionId,
    sessions: &HashMap<SessionId, SessionId>,
) -> Vec<ProgrammerLifecycleSession> {
    let mut connected = sessions
        .iter()
        .filter_map(|(session, bound)| {
            (*bound == key).then_some(ProgrammerLifecycleSession {
                session_id: *session,
            })
        })
        .collect::<Vec<_>>();
    connected.sort_unstable_by_key(|session| session.session_id.0);
    connected
}

fn selected_fixture_count(
    key: SessionId,
    sessions: &HashMap<SessionId, SessionId>,
    command_contexts: &HashMap<SessionId, SessionId>,
    selections: &HashMap<SessionId, crate::selection::SelectionContext>,
) -> u64 {
    sessions
        .iter()
        .filter(|(_, bound)| **bound == key)
        .map(|(session, _)| command_contexts.get(session).copied().unwrap_or(*session))
        .collect::<HashSet<_>>()
        .into_iter()
        .fold(0, |count, context| {
            count.saturating_add(
                selections
                    .get(&context)
                    .map_or(0, |selection| collection_len(selection.selected.len())),
            )
        })
}

fn value_count(fixture_values: usize, group_values: &crate::groups::GroupProgrammerValues) -> u64 {
    group_values
        .values()
        .fold(collection_len(fixture_values), |count, attributes| {
            count.saturating_add(collection_len(attributes.len()))
        })
}

fn collection_len(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NormalProgrammerValueMutation, NormalProgrammerValueTiming, ProgrammerSnapshot};
    use light_core::{AttributeKey, AttributeValue, FixtureId};
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn groups_same_user_sessions_and_counts_only_owned_addresses() {
        let registry = ProgrammerRegistry::default();
        let user = UserId(Uuid::from_u128(10));
        let first = SessionId(Uuid::from_u128(11));
        let second = SessionId(Uuid::from_u128(12));
        let first_desk = SessionId(Uuid::from_u128(21));
        let second_desk = SessionId(Uuid::from_u128(22));
        registry.start(first, user);
        registry.start(second, user);
        assert!(registry.attach_command_context(first, first_desk));
        assert!(registry.attach_command_context(second, second_desk));
        registry.select(first, [FixtureId(Uuid::from_u128(31))]);
        registry.select(
            second,
            [
                FixtureId(Uuid::from_u128(32)),
                FixtureId(Uuid::from_u128(33)),
            ],
        );
        assert!(registry.apply_normal_values(first, &normal_values()));

        let summary = registry.programmer_lifecycle(user).unwrap();
        assert_eq!(summary.normal_value_count, 2);
        assert_eq!(summary.selected_fixture_count, 3);
        assert!(summary.connected);
        assert_eq!(summary.connected_sessions.len(), 2);
    }

    #[test]
    fn attached_surfaces_count_one_shared_desk_selection_once() {
        let registry = ProgrammerRegistry::default();
        let user = UserId(Uuid::from_u128(10));
        let application = SessionId(Uuid::from_u128(11));
        let osc = SessionId(Uuid::from_u128(12));
        let desk = SessionId(Uuid::from_u128(21));
        registry.start(application, user);
        registry.start(osc, user);
        assert!(registry.attach_command_context(application, desk));
        assert!(registry.attach_command_context(osc, desk));
        registry.select(application, [FixtureId(Uuid::from_u128(31))]);

        let summary = registry.programmer_lifecycle(user).unwrap();

        assert_eq!(summary.connected_sessions.len(), 2);
        assert_eq!(summary.selected_fixture_count, 1);
    }

    #[test]
    fn active_rows_include_foreign_users_in_deterministic_order() {
        let registry = ProgrammerRegistry::default();
        let later_user = UserId(Uuid::from_u128(20));
        let earlier_user = UserId(Uuid::from_u128(10));
        registry.start(SessionId(Uuid::from_u128(2)), later_user);
        registry.start(SessionId(Uuid::from_u128(3)), earlier_user);

        let summaries = registry.active_programmer_lifecycles();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].user_id, earlier_user);
        assert_eq!(summaries[1].user_id, later_user);
    }

    #[test]
    fn direct_summary_retains_disconnected_authority_while_active_view_removes_it() {
        let registry = ProgrammerRegistry::default();
        let user = UserId(Uuid::from_u128(10));
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session, user);
        registry.disconnect(session);

        let retained = registry.programmer_lifecycle(user).unwrap();
        assert!(!retained.connected);
        assert!(retained.connected_sessions.is_empty());
        assert!(registry.active_programmer_lifecycles().is_empty());
    }

    #[test]
    fn session_identity_is_the_selection_context_fallback() {
        let registry = ProgrammerRegistry::default();
        let user = UserId(Uuid::from_u128(10));
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session, user);
        registry.select(session, [FixtureId(Uuid::from_u128(12))]);
        registry.command_contexts.write().remove(&session);

        let summary = registry.programmer_lifecycle(user).unwrap();

        assert_eq!(summary.selected_fixture_count, 1);
    }

    #[test]
    fn summary_does_not_materialize_state_or_history() {
        let registry = ProgrammerRegistry::default();
        let user = UserId(Uuid::from_u128(10));
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session, user);
        let snapshot = Arc::new(ProgrammerSnapshot::default());
        registry
            .states
            .write()
            .get_mut(&session)
            .unwrap()
            .undo
            .push(Arc::clone(&snapshot));
        let before = Arc::strong_count(&snapshot);

        let summary = registry.programmer_lifecycle(user).unwrap();

        assert_eq!(Arc::strong_count(&snapshot), before);
        assert_eq!(summary.normal_value_count, 0);
    }

    fn normal_values() -> Vec<NormalProgrammerValueMutation> {
        vec![
            NormalProgrammerValueMutation::SetFixture {
                fixture_id: FixtureId(Uuid::from_u128(41)),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.5),
                timing: NormalProgrammerValueTiming::default(),
            },
            NormalProgrammerValueMutation::SetGroup {
                group_id: "1".into(),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.5),
                timing: NormalProgrammerValueTiming::default(),
            },
        ]
    }
}
