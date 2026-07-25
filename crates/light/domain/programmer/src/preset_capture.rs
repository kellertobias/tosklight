use crate::{Preset, PresetAddress, ProgrammerRegistry};
use light_core::SessionId;

impl ProgrammerRegistry {
    /// Captures only normal recordable values for one Preset family.
    ///
    /// The narrow read deliberately excludes selection, Preload, transient values, Highlight,
    /// modes, priority, connectivity, history, and timing metadata. It also avoids cloning the
    /// complete [`crate::ProgrammerState`]. Callers that require an action-time snapshot must hold
    /// this user's serialization gate for the complete capture-and-commit operation.
    pub fn capture_normal_preset(
        &self,
        session: SessionId,
        address: PresetAddress,
        name: String,
    ) -> Option<Preset> {
        let key = self.key(session);
        let states = self.states.read();
        let state = states.get(&key)?;
        let mut preset = Preset {
            name,
            family: address.family,
            number: address.number,
            ..Preset::default()
        };
        for value in &state.values {
            if address.family.accepts(&value.attribute) {
                preset
                    .values
                    .entry(value.fixture_id)
                    .or_default()
                    .insert(value.attribute.clone(), value.value.clone());
            }
        }
        for (group_id, values) in &state.group_values {
            for (attribute, value) in values {
                if address.family.accepts(attribute) {
                    preset
                        .group_values
                        .entry(group_id.clone())
                        .or_default()
                        .insert(attribute.clone(), value.value.clone());
                }
            }
        }
        Some(preset)
    }
}
