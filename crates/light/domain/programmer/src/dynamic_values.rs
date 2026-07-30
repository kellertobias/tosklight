use crate::ProgrammerRegistry;
use light_core::{AttributeKey, FixtureId, SessionId};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue};
use std::sync::Arc;
use uuid::Uuid;

/// One server-resolved mutation of the first-class Dynamic/FAT Programmer layer.
///
/// `Set` stores Dynamic On, Dynamic Off, or FAT content. `Release` clears only the exact
/// Programmer layer selected by `instance_link`; it never manufactures an ordinary static zero.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicProgrammerValueMutation {
    Set {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        value: DynamicSemanticValue,
    },
    Release {
        fixture_id: FixtureId,
        attribute: AttributeKey,
        instance_link: Option<Uuid>,
    },
}

impl ProgrammerRegistry {
    /// Applies one atomically expanded Dynamic/FAT gesture.
    ///
    /// Blind Preload capture writes to the pending Dynamic layer. Otherwise values become normal
    /// recordable Programmer content. Consecutive encoder samples may share `undo_group`.
    pub fn apply_dynamic_values(
        &self,
        session: SessionId,
        mutations: &[DynamicProgrammerValueMutation],
        undo_group: Option<&str>,
    ) -> bool {
        if mutations.is_empty() {
            return false;
        }
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.close_selection_gesture(session);
        let mut states = self.states.write();
        let Some(state) = states.get_mut(&self.key(session)) else {
            return false;
        };
        let preload = state.blind && state.preload_capture_programmer;
        let values = if preload {
            &state.preload_dynamic_pending
        } else {
            &state.dynamic_values
        };
        if !mutations
            .iter()
            .any(|mutation| mutation_changes(values, mutation))
        {
            return false;
        }
        let continues_group =
            undo_group.is_some() && state.active_value_undo_group.as_deref() == undo_group;
        if !continues_group {
            state.checkpoint();
        }
        state.active_value_undo_group = undo_group.map(str::to_owned);
        let values = if preload {
            Arc::make_mut(&mut state.preload_dynamic_pending)
        } else {
            Arc::make_mut(&mut state.dynamic_values)
        };
        for mutation in mutations {
            apply_mutation(self, values, mutation);
        }
        state.last_activity = self.clock.now();
        let user_id = state.user_id;
        drop(states);
        if preload {
            self.mark_preload_values_changed(user_id);
        } else {
            self.mark_normal_values_changed(user_id);
        }
        true
    }
}

fn mutation_changes(
    values: &[DynamicAddressValue],
    mutation: &DynamicProgrammerValueMutation,
) -> bool {
    match mutation {
        DynamicProgrammerValueMutation::Set {
            fixture_id,
            attribute,
            value,
        } => values
            .iter()
            .find(|stored| {
                same_track(
                    stored,
                    *fixture_id,
                    attribute,
                    semantic_instance_link(value),
                )
            })
            .is_none_or(|stored| stored.value != *value),
        DynamicProgrammerValueMutation::Release {
            fixture_id,
            attribute,
            instance_link,
        } => values
            .iter()
            .any(|stored| same_track(stored, *fixture_id, attribute, *instance_link)),
    }
}

fn apply_mutation(
    registry: &ProgrammerRegistry,
    values: &mut Vec<DynamicAddressValue>,
    mutation: &DynamicProgrammerValueMutation,
) {
    let (fixture_id, attribute, instance_link) = match mutation {
        DynamicProgrammerValueMutation::Set {
            fixture_id,
            attribute,
            value,
        } => (*fixture_id, attribute, semantic_instance_link(value)),
        DynamicProgrammerValueMutation::Release {
            fixture_id,
            attribute,
            instance_link,
        } => (*fixture_id, attribute, *instance_link),
    };
    values.retain(|stored| !same_track(stored, fixture_id, attribute, instance_link));
    if let DynamicProgrammerValueMutation::Set { value, .. } = mutation {
        values.push(DynamicAddressValue {
            fixture_id,
            attribute: attribute.clone(),
            value: value.clone(),
            programmer_order: registry.next_programmer_order(),
            changed_at_millis: u64::try_from(registry.clock.now().timestamp_millis())
                .unwrap_or_default(),
        });
    }
}

fn same_track(
    stored: &DynamicAddressValue,
    fixture_id: FixtureId,
    attribute: &AttributeKey,
    instance_link: Option<Uuid>,
) -> bool {
    stored.fixture_id == fixture_id
        && stored.attribute == *attribute
        && semantic_instance_link(&stored.value) == instance_link
}

fn semantic_instance_link(value: &DynamicSemanticValue) -> Option<Uuid> {
    match value {
        DynamicSemanticValue::DynamicOn { instance_link, .. }
        | DynamicSemanticValue::DynamicOff { instance_link, .. } => Some(*instance_link),
        DynamicSemanticValue::Static { .. }
        | DynamicSemanticValue::FixAt { .. }
        | DynamicSemanticValue::Release => None,
    }
}
