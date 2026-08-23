use crate::ProgrammerRegistry;
use light_core::{AttributeKey, FixtureId, SessionId};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseProgrammerFixtureValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseProgrammerGroupValue {
    pub group_id: String,
    pub attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct GroupReleaseProgrammerValue {
    pub group_id: String,
    pub attribute: AttributeKey,
    #[serde(default)]
    pub programmer_order: u64,
    #[serde(default)]
    pub changed_at_millis: u64,
}

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

    /// Stores explicit RELEASE instructions while removing superseded normal Programmer values.
    ///
    /// RELEASE remains recordable in the scalar Dynamic/FixAT layer, where it is visible to Cue
    /// capture but contributes no live value. Fixture and live-Group values are removed in the
    /// same Undo checkpoint so RELEASE never behaves like a zero or require two Undo presses.
    pub fn apply_release_values(
        &self,
        session: SessionId,
        fixtures: &[ReleaseProgrammerFixtureValue],
        groups: &[ReleaseProgrammerGroupValue],
    ) -> bool {
        if fixtures.is_empty() && groups.is_empty() {
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
        let (normal_values, group_values, group_releases, dynamic_values) = if preload {
            (
                &state.preload_pending,
                &state.preload_group_pending,
                &state.preload_group_release_pending,
                state.preload_dynamic_pending.as_ref(),
            )
        } else {
            (
                state.values.as_ref(),
                state.group_values.as_ref(),
                &state.group_release_values,
                state.dynamic_values.as_ref(),
            )
        };
        let fixture_change = fixtures.iter().any(|release| {
            normal_values.iter().any(|stored| {
                stored.fixture_id == release.fixture_id && stored.attribute == release.attribute
            }) || dynamic_values
                .iter()
                .find(|stored| same_track(stored, release.fixture_id, &release.attribute, None))
                .is_none_or(|stored| !matches!(stored.value, DynamicSemanticValue::Release))
        });
        let group_change = groups.iter().any(|release| {
            group_values
                .get(&release.group_id)
                .is_some_and(|attributes| attributes.contains_key(&release.attribute))
                || !group_releases.iter().any(|stored| {
                    stored.group_id == release.group_id && stored.attribute == release.attribute
                })
        });
        if !fixture_change && !group_change {
            return false;
        }
        state.checkpoint();
        state.active_value_undo_group = None;
        let changed_at = self.clock.now();
        let changed_at_millis = u64::try_from(changed_at.timestamp_millis()).unwrap_or_default();

        let (normal_values, group_values, group_releases, dynamic_values) = if preload {
            (
                &mut state.preload_pending,
                &mut state.preload_group_pending,
                &mut state.preload_group_release_pending,
                Arc::make_mut(&mut state.preload_dynamic_pending),
            )
        } else {
            (
                Arc::make_mut(&mut state.values),
                Arc::make_mut(&mut state.group_values),
                &mut state.group_release_values,
                Arc::make_mut(&mut state.dynamic_values),
            )
        };
        for release in fixtures {
            normal_values.retain(|stored| {
                stored.fixture_id != release.fixture_id || stored.attribute != release.attribute
            });
            dynamic_values
                .retain(|stored| !same_track(stored, release.fixture_id, &release.attribute, None));
            dynamic_values.push(DynamicAddressValue {
                fixture_id: release.fixture_id,
                attribute: release.attribute.clone(),
                value: DynamicSemanticValue::Release,
                programmer_order: self.next_programmer_order(),
                changed_at_millis,
            });
        }
        for release in groups {
            if let Some(attributes) = group_values.get_mut(&release.group_id) {
                attributes.remove(&release.attribute);
                if attributes.is_empty() {
                    group_values.remove(&release.group_id);
                }
            }
            group_releases.retain(|stored| {
                stored.group_id != release.group_id || stored.attribute != release.attribute
            });
            group_releases.push(GroupReleaseProgrammerValue {
                group_id: release.group_id.clone(),
                attribute: release.attribute.clone(),
                programmer_order: self.next_programmer_order(),
                changed_at_millis,
            });
        }
        state.last_activity = changed_at;
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
