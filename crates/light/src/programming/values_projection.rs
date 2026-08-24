use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{AttributeKey, FixtureId};
use light_core::{SessionId, UserId};
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue};
use light_programmer::{ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerRegistry};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PROJECTION_READS: Cell<usize> = const { Cell::new(0) };
}

/// Complete user-owned normal Programmer values used by recordable UI projections.
///
/// Selection, command interaction, Preload, modes, priority, connectivity, Highlight, and
/// transient control actions deliberately live outside this boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesProjection {
    pub user_id: UserId,
    pub revision: u64,
    pub fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub group_values: Vec<ProgrammerGroupUpdate>,
    pub dynamic_values: Arc<Vec<DynamicAddressValue>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingFixtureValueAddress {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingGroupValueAddress {
    pub group_id: String,
    pub attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingDynamicValueAddress {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub instance_link: Option<uuid::Uuid>,
}

/// One ordered normal-value transition. The retained projection remains available to command
/// outcomes, while event transport publishes only these address-level changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgrammingValuesDelta {
    pub fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub removed_fixture_values: Vec<ProgrammingFixtureValueAddress>,
    pub group_values: Vec<ProgrammerGroupUpdate>,
    pub removed_group_values: Vec<ProgrammingGroupValueAddress>,
    pub dynamic_values: Vec<DynamicAddressValue>,
    pub removed_dynamic_values: Vec<ProgrammingDynamicValueAddress>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesChange {
    pub projection: Arc<ProgrammingValuesProjection>,
    pub delta: ProgrammingValuesDelta,
}

/// Authoritative gap-repair snapshot for one authenticated user's normal Programmer values.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingValuesProjection,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ProgrammingValuesContent {
    pub(super) fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub(super) group_values: Vec<ProgrammerGroupUpdate>,
    pub(super) dynamic_values: Arc<Vec<DynamicAddressValue>>,
}

impl ProgrammingValuesContent {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        Self::read_for_diff(programmers, session)
    }

    pub(super) fn read_for_diff(
        programmers: &ProgrammerRegistry,
        session: SessionId,
    ) -> Result<Self, ActionError> {
        let state = programmers
            .get(session)
            .ok_or_else(programmer_values_unavailable)?;
        if !programmers.knows_session(session) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        let (fixture_values, group_values, dynamic_values) = state.update_projection_content();
        Ok(Self {
            fixture_values,
            group_values,
            dynamic_values,
        })
    }

    pub(super) fn projection(self, user_id: UserId, revision: u64) -> ProgrammingValuesProjection {
        ProgrammingValuesProjection {
            user_id,
            revision,
            fixture_values: self.fixture_values,
            group_values: self.group_values,
            dynamic_values: self.dynamic_values,
        }
    }

    pub(super) fn change(
        self,
        before: &Self,
        user_id: UserId,
        revision: u64,
    ) -> ProgrammingValuesChange {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        use std::collections::HashMap;

        let before_fixture = before
            .fixture_values
            .iter()
            .map(|value| ((value.fixture_id, value.attribute.clone()), value))
            .collect::<HashMap<_, _>>();
        let after_fixture = self
            .fixture_values
            .iter()
            .map(|value| ((value.fixture_id, value.attribute.clone()), value))
            .collect::<HashMap<_, _>>();
        let before_group = before
            .group_values
            .iter()
            .map(|value| ((value.group_id.clone(), value.attribute.clone()), value))
            .collect::<HashMap<_, _>>();
        let after_group = self
            .group_values
            .iter()
            .map(|value| ((value.group_id.clone(), value.attribute.clone()), value))
            .collect::<HashMap<_, _>>();
        let dynamic_changed = !Arc::ptr_eq(&self.dynamic_values, &before.dynamic_values);
        let before_dynamic = if dynamic_changed {
            before
                .dynamic_values
                .iter()
                .map(|value| (dynamic_address(value), value))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::default()
        };
        let after_dynamic = if dynamic_changed {
            self.dynamic_values
                .iter()
                .map(|value| (dynamic_address(value), value))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::default()
        };

        let delta = ProgrammingValuesDelta {
            fixture_values: self
                .fixture_values
                .iter()
                .filter(|value| {
                    before_fixture
                        .get(&(value.fixture_id, value.attribute.clone()))
                        .copied()
                        != Some(*value)
                })
                .cloned()
                .collect(),
            removed_fixture_values: before_fixture
                .keys()
                .filter(|key| !after_fixture.contains_key(*key))
                .map(|(fixture_id, attribute)| ProgrammingFixtureValueAddress {
                    fixture_id: *fixture_id,
                    attribute: attribute.clone(),
                })
                .collect(),
            group_values: self
                .group_values
                .iter()
                .filter(|value| {
                    before_group
                        .get(&(value.group_id.clone(), value.attribute.clone()))
                        .copied()
                        != Some(*value)
                })
                .cloned()
                .collect(),
            removed_group_values: before_group
                .keys()
                .filter(|key| !after_group.contains_key(*key))
                .map(|(group_id, attribute)| ProgrammingGroupValueAddress {
                    group_id: group_id.clone(),
                    attribute: attribute.clone(),
                })
                .collect(),
            dynamic_values: if dynamic_changed {
                self.dynamic_values
                    .iter()
                    .filter(|value| {
                        before_dynamic.get(&dynamic_address(value)).copied() != Some(*value)
                    })
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            },
            removed_dynamic_values: if dynamic_changed {
                before_dynamic
                    .keys()
                    .filter(|key| !after_dynamic.contains_key(*key))
                    .map(
                        |(fixture_id, attribute, instance_link)| ProgrammingDynamicValueAddress {
                            fixture_id: *fixture_id,
                            attribute: attribute.clone(),
                            instance_link: *instance_link,
                        },
                    )
                    .collect()
            } else {
                Vec::new()
            },
        };
        let projection = Arc::new(self.projection(user_id, revision));
        ProgrammingValuesChange { projection, delta }
    }
}

fn dynamic_address(value: &DynamicAddressValue) -> (FixtureId, AttributeKey, Option<uuid::Uuid>) {
    let instance_link = match &value.value {
        DynamicSemanticValue::DynamicOn { instance_link, .. }
        | DynamicSemanticValue::DynamicOff { instance_link, .. } => Some(*instance_link),
        _ => None,
    };
    (value.fixture_id, value.attribute.clone(), instance_link)
}

#[cfg(test)]
pub(super) fn reset_projection_read_count() {
    PROJECTION_READS.set(0);
}

#[cfg(test)]
pub(super) fn projection_read_count() -> usize {
    PROJECTION_READS.get()
}

#[cfg(test)]
mod dynamic_delta_tests {
    use super::*;
    use light_core::{AttributeKey, FixtureId};
    use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
    use uuid::Uuid;

    fn dynamic_value(instance_link: Uuid, programmer_order: u64) -> DynamicAddressValue {
        DynamicAddressValue {
            fixture_id: FixtureId(Uuid::from_u128(1)),
            attribute: AttributeKey("intensity".into()),
            value: DynamicSemanticValue::DynamicOff {
                instance_link,
                timing: DynamicValueTiming::default(),
            },
            programmer_order,
            changed_at_millis: programmer_order,
        }
    }

    #[test]
    fn dynamic_delta_keeps_concurrent_instance_tracks_distinct() {
        let first = Uuid::from_u128(11);
        let second = Uuid::from_u128(12);
        let before = ProgrammingValuesContent {
            dynamic_values: Arc::new(vec![dynamic_value(first, 1), dynamic_value(second, 2)]),
            ..Default::default()
        };
        let after = ProgrammingValuesContent {
            dynamic_values: Arc::new(vec![dynamic_value(second, 3)]),
            ..Default::default()
        };

        let change = after.change(&before, UserId(Uuid::from_u128(2)), 2);

        assert_eq!(change.delta.dynamic_values, vec![dynamic_value(second, 3)]);
        assert_eq!(
            change.delta.removed_dynamic_values,
            vec![ProgrammingDynamicValueAddress {
                fixture_id: FixtureId(Uuid::from_u128(1)),
                attribute: AttributeKey("intensity".into()),
                instance_link: Some(first),
            }]
        );
    }
}

impl ProgrammingService {
    pub fn values_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingValuesSnapshot, ActionError> {
        // The identity must be there; which identity it is no longer selects anything.
        let (session, _) = values_identity(context)?;
        self.with_programmer_and_desk_gate(context.desk_id, || {
            ports.authorize(context)?;
            // Reading the cursor first permits a duplicate after repair, but cannot skip a
            // mutation, because that transition uses this same gate.
            let event_sequence = self.events.latest_sequence();
            let content = ProgrammingValuesContent::read(&self.programmers, session)?;
            // Report the Programmer the session operates, not the name it asked under.
            let user_id = self
                .programmers
                .operated_desk_user(session)
                .ok_or_else(programmer_values_unavailable)?;
            let revision = self.programmers.normal_values_revision();
            Ok(ProgrammingValuesSnapshot {
                event_sequence,
                projection: content.projection(user_id, revision),
            })
        })
    }
}

fn values_identity(context: &ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer value snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer value snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn programmer_values_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Programmer values are unavailable",
    )
}
