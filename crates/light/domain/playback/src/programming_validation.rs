use crate::CueList;
use light_core::programming::{
    IntentError, ProgrammingValueScope, validate_targeted_programming_entries,
};

impl crate::ActivePlayback {
    pub fn required_programming_contract(&self) -> u16 {
        self.deleted_cue_hold
            .iter()
            .flat_map(|hold| hold.contributions.iter())
            .chain(self.deleted_cue_transition_source.iter().flatten())
            .map(|value| value.value.required_programming_contract())
            .max()
            .unwrap_or(0)
    }
}

impl CueList {
    pub fn required_programming_contract(&self) -> u16 {
        self.cues
            .iter()
            .map(|cue| {
                cue.changes
                    .iter()
                    .filter_map(|v| v.value.as_ref())
                    .chain(cue.group_changes.iter().filter_map(|v| v.value.as_ref()))
                    .map(light_core::AttributeValue::required_programming_contract)
                    .chain(
                        cue.dynamic_changes
                            .iter()
                            .map(|v| v.value.required_programming_contract()),
                    )
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
    }

    /// Cue changes remain sparse, but each written value is a complete, valid semantic owner.
    pub fn validate_programming(&self) -> Result<(), IntentError> {
        for cue in &self.cues {
            let mut masks = std::collections::HashSet::new();
            for change in &cue.dynamic_changes {
                change.value.validate_programming_at(&change.attribute)?;
                if matches!(
                    change.value,
                    light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
                        | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
                ) && !masks.insert((
                    change.fixture_id,
                    &change.attribute,
                    change.value.track_key(),
                    change.automatic_restore,
                )) {
                    return Err(IntentError(
                        "duplicate typed component hold in Cue changes".into(),
                    ));
                }
            }
            validate_targeted_programming_entries(
                ProgrammingValueScope::Fixture,
                cue.changes.iter().filter_map(|change| {
                    change
                        .value
                        .as_ref()
                        .map(|value| (change.fixture_id, &change.attribute, value))
                }),
            )?;
            validate_targeted_programming_entries(
                ProgrammingValueScope::LiveGroup,
                cue.group_changes.iter().filter_map(|change| {
                    change
                        .value
                        .as_ref()
                        .map(|value| (&change.group_id, &change.attribute, value))
                }),
            )?;
            validate_targeted_programming_entries(
                ProgrammingValueScope::Fixture,
                cue.dynamic_changes
                    .iter()
                    .filter_map(|change| match &change.value {
                        light_dynamics::DynamicSemanticValue::Static { value, .. } => {
                            Some((change.fixture_id, &change.attribute, value))
                        }
                        _ => None,
                    }),
            )?;
        }
        Ok(())
    }
}
