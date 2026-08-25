use crate::ProgrammerRegistry;
use light_core::{ProgrammerId, SessionId};
use std::collections::HashSet;

/// One currently connected control session without its private interaction content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammerLifecycleSession {
    pub session_id: SessionId,
}

/// Lightweight activity summary for the desk's retained Programmer.
///
/// Values, selected fixture identities, commands, modes, priority, Highlight, transient values,
/// Preload details, and Undo/Redo snapshots deliberately remain outside this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammerLifecycleSummary {
    pub programmer_id: ProgrammerId,
    pub connected: bool,
    pub connected_sessions: Vec<ProgrammerLifecycleSession>,
    pub selected_fixture_count: u64,
    pub normal_value_count: u64,
    /// Whether retained active Preload fixture or Group values currently contribute to output.
    /// Pending values, capture mode, and queued Playback actions deliberately do not count.
    pub preload_active: bool,
}

impl ProgrammerRegistry {
    /// The lifecycle of the desk's one Programmer, without cloning its complete state.
    pub fn programmer_lifecycle(&self) -> Option<ProgrammerLifecycleSummary> {
        self.serialized(|| self.lifecycle_for_desk())
    }

    /// Read only connected Programmer authorities in deterministic Programmer order.
    pub fn active_programmer_lifecycles(&self) -> Vec<ProgrammerLifecycleSummary> {
        self.read_active_programmer_lifecycles(std::convert::identity)
    }

    /// Assemble a safe installation snapshot while the desk's mutation gate is held.
    ///
    /// The reader should stay small: this boundary exists so an application cursor/revision can
    /// be paired with the exact summaries without a completed mutation slipping between them.
    pub fn read_active_programmer_lifecycles<R>(
        &self,
        reader: impl FnOnce(Vec<ProgrammerLifecycleSummary>) -> R,
    ) -> R {
        self.with_all_mutation_gates(|| reader(self.lifecycle_summaries(true)))
    }

    fn lifecycle_for_desk(&self) -> Option<ProgrammerLifecycleSummary> {
        let state = self.state.read();
        let state = state.as_ref()?;
        let sessions = self.sessions.read();
        Some(ProgrammerLifecycleSummary {
            programmer_id: state.id,
            connected: !sessions.is_empty(),
            connected_sessions: connected_sessions(&sessions),
            // One desk, one selection: every connected surface shares it, so it counts once.
            selected_fixture_count: collection_len(self.selection_context.read().selected.len()),
            normal_value_count: value_count(
                state
                    .values
                    .len()
                    .saturating_add(state.dynamic_values.len()),
                &state.group_values,
            ),
            preload_active: !state.preload_active.is_empty()
                || !state.preload_dynamic_active.is_empty()
                || !state.preload_group_active.is_empty()
                || state.preload_playback_active,
        })
    }

    fn lifecycle_summaries(&self, connected_only: bool) -> Vec<ProgrammerLifecycleSummary> {
        if connected_only && self.sessions.read().is_empty() {
            return Vec::new();
        }
        self.lifecycle_for_desk().into_iter().collect()
    }
}

fn connected_sessions(sessions: &HashSet<SessionId>) -> Vec<ProgrammerLifecycleSession> {
    let mut connected = sessions
        .iter()
        .map(|session| ProgrammerLifecycleSession {
            session_id: *session,
        })
        .collect::<Vec<_>>();
    connected.sort_unstable_by_key(|session| session.session_id.0);
    connected
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
    use crate::{
        NormalProgrammerValueMutation, NormalProgrammerValueTiming, PreloadPlaybackQueueAction,
        PreloadPlaybackQueueSurface, ProgrammerSnapshot,
    };
    use light_core::{AttributeKey, AttributeValue, FixtureId};
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn a_second_surface_replaces_the_shared_selection_rather_than_adding_its_own() {
        let registry = ProgrammerRegistry::default();
        let first = SessionId(Uuid::from_u128(11));
        let second = SessionId(Uuid::from_u128(12));
        registry.start(first);
        registry.start(second);
        registry.select(first, [FixtureId(Uuid::from_u128(31))]);
        registry.select(
            second,
            [
                FixtureId(Uuid::from_u128(32)),
                FixtureId(Uuid::from_u128(33)),
            ],
        );
        assert!(registry.apply_normal_values(first, &normal_values()));

        let summary = registry.programmer_lifecycle().unwrap();
        assert_eq!(summary.normal_value_count, 2);
        assert_eq!(
            summary.selected_fixture_count, 2,
            "two surfaces share one selection; the second replaced the first"
        );
        assert!(summary.connected);
        assert_eq!(summary.connected_sessions.len(), 2);
    }

    #[test]
    fn attached_surfaces_count_one_shared_desk_selection_once() {
        let registry = ProgrammerRegistry::default();
        let application = SessionId(Uuid::from_u128(11));
        let osc = SessionId(Uuid::from_u128(12));
        let desk = SessionId(Uuid::from_u128(21));
        registry.start(application);
        registry.start(osc);
        assert!(registry.attach_command_context(application, desk));
        assert!(registry.attach_command_context(osc, desk));
        registry.select(application, [FixtureId(Uuid::from_u128(31))]);

        let summary = registry.programmer_lifecycle().unwrap();

        assert_eq!(summary.connected_sessions.len(), 2);
        assert_eq!(summary.selected_fixture_count, 1);
    }

    #[test]
    /// However many surfaces connect, the desk projects one Programmer. There is no second
    /// authority to order against a first.
    fn every_connected_surface_projects_the_one_desk_programmer() {
        let registry = ProgrammerRegistry::default();
        registry.start(SessionId(Uuid::from_u128(2)));
        registry.start(SessionId(Uuid::from_u128(3)));

        let summaries = registry.active_programmer_lifecycles();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].connected_sessions.len(), 2);
    }

    #[test]
    fn direct_summary_retains_disconnected_authority_while_active_view_removes_it() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session);
        registry.disconnect(session);

        let retained = registry.programmer_lifecycle().unwrap();
        assert!(!retained.connected);
        assert!(retained.connected_sessions.is_empty());
        assert!(registry.active_programmer_lifecycles().is_empty());
    }

    #[test]
    fn the_desks_one_selection_is_counted_once() {
        let registry = ProgrammerRegistry::default();
        let first = SessionId(Uuid::from_u128(11));
        let second = SessionId(Uuid::from_u128(12));
        registry.start(first);
        registry.start(second);
        registry.select(first, [FixtureId(Uuid::from_u128(13))]);

        let summary = registry.programmer_lifecycle().unwrap();

        assert_eq!(summary.connected_sessions.len(), 2);
        assert_eq!(summary.selected_fixture_count, 1);
    }

    #[test]
    fn summary_does_not_materialize_state_or_history() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session);
        let snapshot = Arc::new(ProgrammerSnapshot::default());
        registry
            .state
            .write()
            .as_mut()
            .unwrap()
            .undo
            .push(Arc::clone(&snapshot));
        let before = Arc::strong_count(&snapshot);

        let summary = registry.programmer_lifecycle().unwrap();

        assert_eq!(Arc::strong_count(&snapshot), before);
        assert_eq!(summary.normal_value_count, 0);
    }

    #[test]
    fn preload_active_counts_only_retained_active_fixture_or_group_values() {
        let registry = ProgrammerRegistry::default();
        let session = SessionId(Uuid::from_u128(11));
        registry.start(session);

        assert!(!preload_active(&registry));
        assert!(registry.arm_preload(session, true));
        registry.select(session, [FixtureId(Uuid::from_u128(12))]);
        assert!(!preload_active(&registry));

        registry.set(
            session,
            FixtureId(Uuid::from_u128(13)),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        assert!(!preload_active(&registry));
        assert!(registry.activate_preload(session));
        assert!(preload_active(&registry));
        assert!(registry.release_preload(session));
        assert!(!preload_active(&registry));

        assert!(registry.queue_preload_playback_action(
            session,
            1,
            None,
            PreloadPlaybackQueueAction::Go,
            PreloadPlaybackQueueSurface::Physical,
        ));
        assert!(!preload_active(&registry));
        assert_eq!(registry.take_preload_playback_actions(session).len(), 1);

        registry.set(
            session,
            FixtureId(Uuid::from_u128(14)),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
        );
        assert!(!preload_active(&registry));
        assert!(registry.set_preload_group(
            session,
            "7".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        ));
        assert!(!preload_active(&registry));
        assert!(registry.activate_preload(session));
        assert!(preload_active(&registry));
        assert!(registry.release_preload(session));
        assert!(!preload_active(&registry));
    }

    fn preload_active(registry: &ProgrammerRegistry) -> bool {
        registry.programmer_lifecycle().unwrap().preload_active
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
