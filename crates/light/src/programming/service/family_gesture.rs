//! One bounded, runtime-only adoption per shared Programmer. A continuous touch keeps the first
//! coherent pose/source models even when its first sample is numerically neutral. Authored
//! values still come from the latest same-lane state on each sample.
use super::ProgrammingService;
use crate::{
    ActionContext, ActionError, ProgrammingFamilyContext, ProgrammingPorts, ProgrammingValueIntent,
    ProgrammingValueOperation, ProgrammingValuesEnvironment,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, ProgrammerId, SessionId};
use std::{collections::HashMap, sync::Arc};

type GestureStamp = (ProgrammerId, u64, u64, u64);
type FixtureValues = HashMap<(FixtureId, AttributeKey), AttributeValue>;
type GroupValues = HashMap<(String, AttributeKey), AttributeValue>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct GestureKey {
    stamp: GestureStamp,
    desk: uuid::Uuid,
    session: SessionId,
    preload: bool,
    caller_id: String,
    attribute: AttributeKey,
    group: Option<String>,
    members: Vec<FixtureId>,
}

struct FamilyCapture {
    seeds: HashMap<FixtureId, AttributeValue>,
    contexts: HashMap<FixtureId, ProgrammingFamilyContext>,
    group_context: Option<ProgrammingFamilyContext>,
    template: Option<AttributeValue>,
}

#[derive(Clone)]
pub(super) struct PreparedFamilyGesture {
    key: GestureKey,
    capture: Arc<FamilyCapture>,
    // Never coalesce across changed selection, history, membership or capture mode merely
    // because a surface recycled its caller ID.
    pub(super) undo_group: String,
}

struct ValueTransactionScope<'a>(&'a std::sync::atomic::AtomicUsize);
impl Drop for ValueTransactionScope<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl ProgrammingService {
    pub(super) fn prepare_family_gesture(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        preload: bool,
        intent: &ProgrammingValueIntent,
        environment: &mut ProgrammingValuesEnvironment,
        active: &FixtureValues,
        groups: &GroupValues,
    ) -> Result<Option<PreparedFamilyGesture>, ActionError> {
        let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
            return Ok(None);
        };
        if edits.is_empty() || (intent.group_id.is_none() && intent.fixture_ids.is_empty()) {
            return Ok(None);
        }
        if let Some(group) = &intent.group_id {
            super::values_validation::validate_identifier(group, "group_id")?;
            if !environment.group_memberships.contains_key(group) {
                return Err(ActionError::new(
                    crate::ActionErrorKind::Invalid,
                    "Group does not exist",
                ));
            }
        }
        let requested_members = intent.group_id.as_ref().map_or_else(
            || intent.fixture_ids.clone(),
            |id| {
                environment
                    .group_members
                    .get(id)
                    .cloned()
                    .unwrap_or_default()
            },
        );
        if requested_members
            .iter()
            .any(|fixture| !environment.fixture_ids.contains(fixture))
        {
            return Err(ActionError::new(
                crate::ActionErrorKind::Invalid,
                "fixture does not exist",
            ));
        }
        let lane = if preload {
            light_programmer::ProgrammerAlignmentLane::Preload
        } else {
            light_programmer::ProgrammerAlignmentLane::Normal
        };
        let cohort = context.session_id.and_then(|session| {
            self.family_alignment_fixture_cohort(
                SessionId(session),
                lane,
                intent,
                environment,
                active,
            )
        });
        let mut capture_intent = intent.clone();
        if let Some(cohort) = &cohort {
            capture_intent.fixture_ids = cohort.clone();
        }
        let members = cohort.unwrap_or(requested_members);
        if members
            .iter()
            .any(|fixture| !environment.fixture_ids.contains(fixture))
        {
            return Err(ActionError::new(
                crate::ActionErrorKind::Invalid,
                "fixture does not exist",
            ));
        }
        let capture = |environment: &mut ProgrammingValuesEnvironment| {
            // An intentionally empty Group can still carry a declarative template, but
            // there is no physical member whose current pose/model needs capturing.
            if members.is_empty() {
                return Ok(());
            }
            ports.prepare_family_edit_context(context, preload, &capture_intent, environment)?;
            super::zoom_adoption::prepare_zoom_adoption(
                &capture_intent,
                &members,
                environment,
                active,
            );
            super::color_adoption::prepare_color_adoption(
                ports,
                context,
                &capture_intent,
                &members,
                environment,
                active,
                groups,
            )
        };
        let (Some(stamp), Some(session), Some(caller_id)) = (
            self.programmers.value_gesture_stamp(),
            context.session_id,
            intent.undo_group.clone(),
        ) else {
            capture(environment)?;
            return Ok(None);
        };
        let key = GestureKey {
            stamp,
            desk: context.desk_id,
            session: SessionId(session),
            preload,
            caller_id,
            attribute: intent.attribute.clone(),
            group: intent.group_id.clone(),
            members: members.clone(),
        };
        let retained = self
            .family_gesture
            .lock()
            .as_ref()
            .filter(|entry| entry.key == key)
            .cloned();
        let prepared = if let Some(retained) = retained {
            retained
        } else {
            capture(environment)?;
            let capture = capture_family(&key, environment, active);
            PreparedFamilyGesture {
                key,
                capture: Arc::new(capture),
                undo_group: uuid::Uuid::new_v4().to_string(),
            }
        };
        install_captured_family(&prepared, environment);
        Ok(Some(prepared))
    }

    /// Called under the Programmer/desk mutation gate, after authorization and replay.
    /// A delayed End cannot retire another surface's capture or a newer Undo owner.
    pub(super) fn finish_captured_family_gesture(
        &self,
        context: &ActionContext,
        preload: bool,
        attribute: &AttributeKey,
        undo_group: &str,
    ) -> Result<(), ActionError> {
        super::values_validation::validate_identifier(&attribute.0, "attribute")?;
        if undo_group.is_empty() || undo_group.len() > 128 {
            return Err(ActionError::new(
                crate::ActionErrorKind::Invalid,
                "encoder undo_group must contain 1-128 bytes",
            ));
        }
        let stamp = self.programmers.value_gesture_stamp();
        let mut retained = self.family_gesture.lock();
        let matches = retained.as_ref().is_some_and(|entry| {
            context.session_id == Some(entry.key.session.0)
                && context.desk_id == entry.key.desk
                && preload == entry.key.preload
                && attribute == &entry.key.attribute
                && undo_group == entry.key.caller_id
                && stamp == Some(entry.key.stamp)
        });
        if matches {
            *retained = None;
            drop(retained);
            self.programmers.finish_value_gesture();
        }
        Ok(())
    }

    pub(super) fn finish_family_gesture(
        &self,
        prepared: Option<PreparedFamilyGesture>,
        changed: bool,
    ) {
        if let Some(mut prepared) = prepared {
            let Some(stamp) = self.programmers.value_gesture_stamp() else {
                return;
            };
            // The first real sample creates its Undo checkpoint and may close selection.
            prepared.key.stamp = stamp;
            *self.family_gesture.lock() = Some(prepared);
        } else if changed {
            *self.family_gesture.lock() = None;
        }
    }

    /// Roll back domain-only compound operations and their adoption lifecycle together. Public
    /// value actions also persist/publish/cache responses and must use isolated staging instead;
    /// reject those before any work, rather than retaining a false success after rollback.
    pub fn with_value_gesture_transaction<T, E>(
        &self,
        session: SessionId,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        self.programmers.with_transaction(session, || {
            self.value_transaction_depth
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _scope = ValueTransactionScope(&self.value_transaction_depth);
            let before = self.family_gesture.lock().clone();
            let result = operation();
            if result.is_err() {
                *self.family_gesture.lock() = before;
            }
            result
        })
    }

    pub(super) fn assert_value_action_boundary(&self) -> Result<(), crate::ActionError> {
        if self
            .value_transaction_depth
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return Ok(());
        }
        Err(crate::ActionError::new(
            crate::ActionErrorKind::Invalid,
            "publishing value actions require an isolated staged command, not a domain rollback transaction",
        ))
    }

    /// Install a prepared show atomically with ending the old show's adoption. Preparation or
    /// failed activation must not call this boundary.
    pub fn run_value_gesture_boundary<T>(
        &self,
        context: &crate::ActionContext,
        operation: impl FnOnce() -> T,
    ) -> T {
        self.programmers.serialized(|| {
            let result = operation();
            self.forget_value_gesture(None);
            // This boundary is used only by committed show replacement. Align is desk-wide;
            // its deactivation argument does not select an owner.
            self.finish_alignment(context, SessionId(uuid::Uuid::nil()));
            result
        })
    }

    /// Called by committed lifecycle operations. An unrelated connected surface leaving the
    /// desk must not drop the initiating surface's continuous touch.
    pub fn forget_value_gesture(&self, session: Option<SessionId>) {
        self.programmers.serialized(|| {
            let mut retained = self.family_gesture.lock();
            if session.is_none()
                || retained
                    .as_ref()
                    .is_some_and(|entry| Some(entry.key.session) == session)
            {
                *retained = None;
                self.programmers.finish_value_gesture();
            }
        });
    }
}

/// Install a gesture's retained capture. Absence is captured too: do not acquire a
/// model/default from a later render frame.
fn install_captured_family(
    prepared: &PreparedFamilyGesture,
    environment: &mut ProgrammingValuesEnvironment,
) {
    for fixture in &prepared.key.members {
        let address = (*fixture, prepared.key.attribute.clone());
        environment.current_values.remove(&address);
        environment.default_values.remove(&address);
        if let Some(seed) = prepared.capture.seeds.get(fixture) {
            environment.current_values.insert(address, seed.clone());
        }
        environment.family_contexts.remove(fixture);
        if let Some(model) = prepared.capture.contexts.get(fixture) {
            environment.family_contexts.insert(*fixture, model.clone());
        }
    }
    if let Some(group) = &prepared.key.group {
        environment.group_family_contexts.remove(group);
        if let Some(context) = &prepared.capture.group_context {
            environment
                .group_family_contexts
                .insert(group.clone(), context.clone());
        }
        let address = (group.clone(), prepared.key.attribute.clone());
        environment.group_family_templates.remove(&address);
        if let Some(template) = &prepared.capture.template {
            environment
                .group_family_templates
                .insert(address, template.clone());
        }
    }
}

/// The seeds and adoption contexts of a gesture's first sample, retained for its later samples.
fn capture_family(
    key: &GestureKey,
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
) -> FamilyCapture {
    let mut seeds = HashMap::new();
    let mut contexts = HashMap::new();
    for fixture in &key.members {
        let address = (*fixture, key.attribute.clone());
        if let Some(value) = active
            .get(&address)
            .or_else(|| environment.current_values.get(&address))
            .or_else(|| environment.default_values.get(&address))
        {
            seeds.insert(*fixture, value.clone());
        }
        if let Some(value) = environment.family_contexts.get(fixture) {
            contexts.insert(*fixture, value.clone());
        }
    }
    FamilyCapture {
        seeds,
        contexts,
        group_context: key
            .group
            .as_ref()
            .and_then(|id| environment.group_family_contexts.get(id))
            .cloned(),
        template: key
            .group
            .as_ref()
            .and_then(|id| {
                environment
                    .group_family_templates
                    .get(&(id.clone(), key.attribute.clone()))
            })
            .cloned(),
    }
}
