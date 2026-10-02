use crate::{ProgrammerSnapshot, ProgrammerState};
use light_core::programming::{
    IntentError, ProgrammingValueScope, validate_programming_entries,
    validate_targeted_programming_entries,
};

macro_rules! required_contract {
    ($value:expr) => {{
        let state = $value;
        state
            .values
            .iter()
            .chain(state.preload_pending.iter())
            .chain(state.preload_active.iter())
            .map(|v| v.value.required_programming_contract())
            .chain(
                state
                    .group_values
                    .values()
                    .chain(state.preload_group_pending.values())
                    .chain(state.preload_group_active.values())
                    .flat_map(|v| v.values())
                    .map(|v| v.value.required_programming_contract()),
            )
            .chain(
                state
                    .dynamic_values
                    .iter()
                    .chain(state.preload_dynamic_pending.iter())
                    .chain(state.preload_dynamic_active.iter())
                    .map(|v| v.value.required_programming_contract()),
            )
            .chain([state.preload_released_colors.required_contract()])
            .max()
            .unwrap_or(0)
    }};
}

macro_rules! validate_content {
    ($value:expr) => {{
        let state = $value;
        state.preload_released_colors.validate(
            &state.preload_dynamic_pending,
            &state.preload_group_release_pending,
        )?;
        for values in [
            state.values.as_slice(),
            state.preload_pending.as_slice(),
            state.preload_active.as_slice(),
        ] {
            validate_targeted_programming_entries(
                ProgrammingValueScope::Fixture,
                values
                    .iter()
                    .map(|value| (value.fixture_id, &value.attribute, &value.value)),
            )?;
        }
        for groups in [
            &*state.group_values,
            &state.preload_group_pending,
            &*state.preload_group_active,
        ] {
            for values in groups.values() {
                validate_programming_entries(
                    ProgrammingValueScope::LiveGroup,
                    values
                        .iter()
                        .map(|(attribute, value)| (attribute, &value.value)),
                )?;
            }
        }
        for values in [
            state.dynamic_values.as_slice(),
            state.preload_dynamic_pending.as_slice(),
            state.preload_dynamic_active.as_slice(),
        ] {
            let mut masks = std::collections::HashSet::new();
            for value in values {
                value.value.validate_programming_at(&value.attribute)?;
                if matches!(
                    value.value,
                    light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
                        | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
                ) && !masks.insert((value.fixture_id, &value.attribute, value.value.track_key()))
                {
                    return Err(IntentError(
                        "duplicate typed component hold in Programmer state".into(),
                    ));
                }
            }
            validate_targeted_programming_entries(
                ProgrammingValueScope::Fixture,
                values.iter().filter_map(|value| match &value.value {
                    light_dynamics::DynamicSemanticValue::Static { value: payload, .. } => {
                        Some((value.fixture_id, &value.attribute, payload))
                    }
                    _ => None,
                }),
            )?;
        }
        Ok(())
    }};
}
impl ProgrammerState {
    pub fn required_programming_contract(&self) -> u16 {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|snapshot| snapshot.required_programming_contract())
            .chain([required_contract!(self)])
            .max()
            .unwrap_or(0)
    }
    /// Validate all durable lanes before restore. History is included so an old retained Undo
    /// checkpoint cannot reintroduce an invalid owner after an otherwise successful load.
    pub fn validate_programming(&self) -> Result<(), IntentError> {
        for snapshot in self.undo.iter().chain(&self.redo) {
            snapshot.validate_programming()?;
        }
        validate_content!(self)
    }
}
impl ProgrammerSnapshot {
    pub fn required_programming_contract(&self) -> u16 {
        required_contract!(self)
    }
    pub fn validate_programming(&self) -> Result<(), IntentError> {
        validate_content!(self)
    }
}
