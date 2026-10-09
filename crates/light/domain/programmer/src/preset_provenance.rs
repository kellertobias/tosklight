use crate::ProgrammerRegistry;
use light_core::{AttributeKey, PresetValueOwner, PresetValueReference, SessionId};
use std::sync::Arc;

impl ProgrammerRegistry {
    /// Attach only provenance from a captured, actual recall. Orders are unique edit identities:
    /// ordinary subsequent writes cannot reuse an origin belonging to an earlier edit.
    pub fn attach_preset_provenance(
        &self,
        session: SessionId,
        origins: &[(PresetValueOwner, AttributeKey, PresetValueReference)],
        preload: bool,
        checkpoint: bool,
    ) -> bool {
        if !self.knows_session(session) {
            return false;
        }
        let gate = self.mutation_gate();
        let _guard = gate.lock();
        let mut states = self.state.write();
        let Some(state) = states.as_mut() else {
            return false;
        };
        let values = if preload {
            state.preload_pending.as_slice()
        } else {
            state.values.as_slice()
        };
        let groups = if preload {
            &state.preload_group_pending
        } else {
            &state.group_values
        };
        let mut updates = Vec::new();
        for (owner, attribute, origin) in origins {
            let order = match owner {
                PresetValueOwner::Fixture { fixture_id } => values
                    .iter()
                    .find(|value| value.fixture_id == *fixture_id && value.attribute == *attribute)
                    .map(|value| value.programmer_order),
                PresetValueOwner::Group { group_id } => groups
                    .get(group_id)
                    .and_then(|values| values.get(attribute))
                    .map(|value| value.programmer_order),
                PresetValueOwner::Universal => None,
            };
            if let Some(order) = order
                && order != 0
                && state.preset_provenance.get(&order) != Some(origin)
            {
                updates.push((order, origin.clone()));
            }
        }
        if updates.is_empty() {
            return false;
        }
        if checkpoint {
            state.checkpoint();
        }
        let live_orders = state
            .values
            .iter()
            .chain(state.preload_pending.iter())
            .chain(state.preload_active.iter())
            .map(|value| value.programmer_order)
            .chain(
                state
                    .group_values
                    .values()
                    .chain(state.preload_group_pending.values())
                    .chain(state.preload_group_active.values())
                    .flat_map(|values| values.values())
                    .map(|value| value.programmer_order),
            )
            .collect::<std::collections::HashSet<_>>();
        let origins = Arc::make_mut(&mut state.preset_provenance);
        origins.retain(|order, _| live_orders.contains(order));
        origins.extend(updates);
        drop(states);
        if preload {
            self.mark_preload_values_changed();
        } else {
            self.mark_normal_values_changed();
        }
        true
    }
}
