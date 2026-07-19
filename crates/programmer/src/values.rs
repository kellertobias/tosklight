use crate::ProgrammerRegistry;
use crate::state::{ProgrammerValueTiming, TransientProgrammerAction};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, TimedValue};
use std::collections::HashSet;

impl ProgrammerRegistry {
    pub fn set(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_with_fade(session, fixture_id, attribute, value, false);
    }

    /// Apply a fixture-level macro as one Programmer mutation. All values share one undo
    /// checkpoint and become visible to the renderer together, so a multi-channel control action
    /// can never be observed half-applied.
    pub fn set_many(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_many_with_checkpoint(session, assignments, true, ProgrammerValueTiming::default());
    }

    /// Apply several normalized values as one normal faded Programmer gesture.
    pub fn set_many_faded_with_timing(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_many_with_checkpoint(
            session,
            assignments,
            true,
            ProgrammerValueTiming {
                fade: true,
                fade_millis,
                delay_millis,
            },
        );
    }

    /// Complete a momentary/timed action without creating a second Undo point. The active edge
    /// already captured the pre-action state; Undo after the inactive edge therefore returns to
    /// that state instead of unexpectedly firing the action again.
    pub fn set_many_transient(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_many_with_checkpoint(
            session,
            assignments,
            false,
            ProgrammerValueTiming::default(),
        );
    }

    /// Install or retrigger one runtime-only fixture-control action. Returning the generation lets
    /// a timed release avoid clearing a newer retrigger of the same action.
    pub fn set_transient_action(
        &self,
        session: SessionId,
        source: String,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
    ) -> Option<u64> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let assignments = assignments.into_iter().collect::<Vec<_>>();
        if assignments.is_empty() {
            return None;
        }
        let generation = self.next_programmer_order();
        let changed_at = self.clock.now();
        let mut states = self.states.write();
        let state = states.get_mut(&self.key(session))?;
        state
            .transient_values
            .retain(|action| action.source != source);
        let values = assignments
            .into_iter()
            .map(|(fixture_id, attribute, value)| TimedValue {
                fixture_id,
                attribute,
                value,
                priority: state.priority,
                changed_at,
                programmer_order: self.next_programmer_order(),
                merge_mode: light_core::MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect();
        state.transient_values.push(TransientProgrammerAction {
            source,
            generation,
            values,
        });
        state.last_activity = changed_at;
        Some(generation)
    }

    /// Release a runtime-only action. Timers provide a generation; a pointer-up release passes
    /// `None` to release whichever generation is currently active.
    pub fn release_transient_action(
        &self,
        session: SessionId,
        source: &str,
        generation: Option<u64>,
    ) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let before = state.transient_values.len();
        state.transient_values.retain(|action| {
            action.source != source
                || generation.is_some_and(|generation| action.generation != generation)
        });
        let changed = state.transient_values.len() != before;
        if changed {
            state.last_activity = self.clock.now();
        }
        changed
    }

    fn set_many_with_checkpoint(
        &self,
        session: SessionId,
        assignments: impl IntoIterator<Item = (FixtureId, AttributeKey, AttributeValue)>,
        checkpoint: bool,
        timing: ProgrammerValueTiming,
    ) {
        let assignments = assignments.into_iter().collect::<Vec<_>>();
        if assignments.is_empty() {
            return;
        }
        self.close_selection_gesture(session);
        let changed_user = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return;
            };
            if checkpoint {
                state.checkpoint();
            }
            let changed_at = self.clock.now();
            let touched = assignments
                .iter()
                .map(|(fixture_id, attribute, _)| (*fixture_id, attribute.clone()))
                .collect::<HashSet<_>>();
            {
                let preload = state.blind && state.preload_capture_programmer;
                let values = if preload {
                    &mut state.preload_pending
                } else {
                    &mut state.values
                };
                for (fixture_id, attribute, value) in assignments {
                    values.retain(|existing| {
                        existing.fixture_id != fixture_id || existing.attribute != attribute
                    });
                    values.push(TimedValue {
                        fixture_id,
                        attribute,
                        value,
                        priority: state.priority,
                        changed_at,
                        programmer_order: self.next_programmer_order(),
                        merge_mode: light_core::MergeMode::Ltp,
                        fade: timing.fade,
                        fade_millis: timing.fade_millis,
                        delay_millis: timing.delay_millis,
                    });
                }
            }
            // A latched mode change made while a pulse is active belongs underneath that pulse.
            // Restamp the runtime override so it stays on top and reveals the new latched value
            // only when the momentary/timed action ends.
            for value in state
                .transient_values
                .iter_mut()
                .flat_map(|action| action.values.iter_mut())
                .filter(|value| touched.contains(&(value.fixture_id, value.attribute.clone())))
            {
                value.changed_at = changed_at;
                value.programmer_order = self.next_programmer_order();
            }
            state.last_activity = changed_at;
            (!state.blind || !state.preload_capture_programmer).then_some(state.user_id)
        };
        if let Some(user_id) = changed_user {
            self.mark_normal_values_changed(user_id);
        }
    }
    pub fn set_faded(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_with_fade(session, fixture_id, attribute, value, true);
    }
    pub fn set_faded_with_timing(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        fade_millis: Option<u64>,
        delay_millis: Option<u64>,
    ) {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.set_with_timing(
            session,
            fixture_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade: true,
                fade_millis,
                delay_millis,
            },
        );
    }
    fn set_with_fade(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        fade: bool,
    ) {
        self.set_with_timing(
            session,
            fixture_id,
            attribute,
            value,
            ProgrammerValueTiming {
                fade,
                fade_millis: None,
                delay_millis: None,
            },
        );
    }
    fn set_with_timing(
        &self,
        session: SessionId,
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: AttributeValue,
        timing: ProgrammerValueTiming,
    ) {
        self.close_selection_gesture(session);
        let changed_user = {
            let mut states = self.states.write();
            let Some(state) = states.get_mut(&self.key(session)) else {
                return;
            };
            state.checkpoint();
            let merge_mode = light_core::MergeMode::Ltp;
            let preload = state.blind && state.preload_capture_programmer;
            let values = if preload {
                &mut state.preload_pending
            } else {
                &mut state.values
            };
            values.retain(|v| !(v.fixture_id == fixture_id && v.attribute == attribute));
            values.push(TimedValue {
                fixture_id,
                attribute,
                value,
                priority: state.priority,
                changed_at: self.clock.now(),
                programmer_order: self.next_programmer_order(),
                merge_mode,
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            });
            state.last_activity = self.clock.now();
            (!preload).then_some(state.user_id)
        };
        if let Some(user_id) = changed_user {
            self.mark_normal_values_changed(user_id);
        }
    }
}
