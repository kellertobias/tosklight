use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{AttributeKey, FixtureId};
use light_core::{SessionId, UserId};
use light_dynamics::DynamicAddressValue;
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

/// One ordered normal-value transition. The retained projection remains available to command
/// outcomes, while event transport publishes only these address-level changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgrammingValuesDelta {
    pub fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub removed_fixture_values: Vec<ProgrammingFixtureValueAddress>,
    pub group_values: Vec<ProgrammerGroupUpdate>,
    pub removed_group_values: Vec<ProgrammingGroupValueAddress>,
    pub dynamic_values: Vec<DynamicAddressValue>,
    pub removed_dynamic_values: Vec<ProgrammingFixtureValueAddress>,
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
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        Self::read_for_diff(programmers, session, user_id)
    }

    pub(super) fn read_for_diff(
        programmers: &ProgrammerRegistry,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        let state = programmers
            .get(session)
            .ok_or_else(programmer_values_unavailable)?;
        if state.user_id != user_id {
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
                .map(|value| ((value.fixture_id, value.attribute.clone()), value))
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::default()
        };
        let after_dynamic = if dynamic_changed {
            self.dynamic_values
                .iter()
                .map(|value| ((value.fixture_id, value.attribute.clone()), value))
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
                        before_dynamic
                            .get(&(value.fixture_id, value.attribute.clone()))
                            .copied()
                            != Some(*value)
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
                    .map(|(fixture_id, attribute)| ProgrammingFixtureValueAddress {
                        fixture_id: *fixture_id,
                        attribute: attribute.clone(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
        };
        let projection = Arc::new(self.projection(user_id, revision));
        ProgrammingValuesChange { projection, delta }
    }
}

#[cfg(test)]
pub(super) fn reset_projection_read_count() {
    PROJECTION_READS.set(0);
}

#[cfg(test)]
pub(super) fn projection_read_count() -> usize {
    PROJECTION_READS.get()
}

impl ProgrammingService {
    pub fn values_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingValuesSnapshot, ActionError> {
        let (session, user_id) = values_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            // Reading the cursor first permits a duplicate after repair, but cannot skip a
            // same-user mutation because that transition uses this same user gate.
            let event_sequence = self.events.latest_sequence();
            let content = ProgrammingValuesContent::read(&self.programmers, session, user_id)?;
            let revision = self.programmers.normal_values_revision(user_id);
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
