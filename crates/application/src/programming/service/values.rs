use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPorts, ProgrammingValueMutation,
    ProgrammingValueTiming, ProgrammingValuesCommand, ProgrammingValuesOutcome,
    ProgrammingValuesRequest, ProgrammingValuesResult,
};
use light_core::{SessionId, UserId};
use light_programmer::{NormalProgrammerValueMutation, NormalProgrammerValueTiming};
use std::sync::Arc;

use super::values_replay_fingerprint::{RequestFingerprint, values_request_fingerprint};
use super::values_validation::{validate_request_id, validate_value_mutations};

impl ProgrammingService {
    pub fn handle_values(
        &self,
        action: ActionEnvelope<ProgrammingValuesRequest>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingValuesResult, ActionError> {
        let (session, user_id, request_id, expected_revision) = values_context(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, user_id, || {
            ports.authorize(&action.context)?;
            self.assert_values_owner(session, user_id)?;
            let fingerprint = values_request_fingerprint(expected_revision, &action.command);
            if let Some(cached) = self.cached_values(
                user_id,
                action.context.desk_id,
                session,
                &request_id,
                fingerprint,
            )? {
                return Ok(cached);
            }
            self.assert_values_revision(user_id, expected_revision)?;
            let capture_mode_revision = self.assert_capture_mode_precondition(
                session,
                user_id,
                action.command.expected_capture_mode_revision,
            )?;
            let result = self.apply_values_action(
                &action,
                ports,
                session,
                user_id,
                expected_revision,
                capture_mode_revision,
            )?;
            self.remember_values(
                user_id,
                action.context.desk_id,
                session,
                request_id,
                fingerprint,
                result.clone(),
            );
            Ok(result)
        })
    }

    fn apply_values_action(
        &self,
        action: &ActionEnvelope<ProgrammingValuesRequest>,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
        user_id: UserId,
        revision_before: u64,
        capture_mode_revision: u64,
    ) -> Result<ProgrammingValuesResult, ActionError> {
        let lifecycle_before = self.active_lifecycle_programmer(user_id);
        let before = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let mutations = action.command.command.mutations();
        if !mutations.is_empty() {
            let environment = ports.values_environment(&action.context)?;
            validate_value_mutations(mutations.as_ref(), &environment)?;
        }
        let changed =
            self.mutate_normal_values(session, &action.command.command, mutations.as_ref());
        let warning = changed
            .then(|| ports.persist(&action.context, "programmer.values"))
            .flatten();
        let after = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            session,
            &before,
            &after,
        );
        let values = self.values_change(
            user_id,
            session,
            before.values_generation,
            after.values_generation,
        )?;
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let outcome = self.values_outcome(&action.context, values, revision_before);
        self.publish_lifecycle_for_context(&action.context, lifecycle_before);
        Ok(ProgrammingValuesResult {
            context: action.context.clone(),
            outcome,
            capture_mode_revision,
            interaction_event_sequence,
            replayed: false,
            warning,
        })
    }

    fn mutate_normal_values(
        &self,
        session: SessionId,
        command: &ProgrammingValuesCommand,
        mutations: &[ProgrammingValueMutation],
    ) -> bool {
        if command.is_clear() {
            self.programmers.clear_normal_values(session)
        } else {
            let mutations = mutations.iter().map(domain_mutation).collect::<Vec<_>>();
            self.programmers.apply_normal_values(session, &mutations)
        }
    }

    fn values_outcome(
        &self,
        context: &crate::ActionContext,
        change: Option<crate::ProgrammingValuesChange>,
        revision_before: u64,
    ) -> ProgrammingValuesOutcome {
        let Some(change) = change else {
            return ProgrammingValuesOutcome::NoChange {
                revision: revision_before,
            };
        };
        let projection = Arc::clone(&change.projection);
        let event_sequence = self
            .publish_values(context, Some(change))
            .expect("a values change always publishes one event");
        ProgrammingValuesOutcome::Changed {
            projection,
            event_sequence,
        }
    }

    fn assert_values_owner(&self, session: SessionId, user_id: UserId) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(ActionError::new(
                ActionErrorKind::NotFound,
                "Programmer values are unavailable",
            )),
        }
    }

    fn assert_values_revision(&self, user_id: UserId, expected: u64) -> Result<(), ActionError> {
        let actual = self.programmers.normal_values_revision(user_id);
        if expected == actual {
            Ok(())
        } else {
            Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "Programmer values revision conflict: expected {expected}, actual {actual}"
                ),
            )
            .at_revision(actual))
        }
    }

    fn assert_capture_mode_precondition(
        &self,
        session: SessionId,
        user_id: UserId,
        expected: u64,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.capture_mode_revision(user_id);
        let values_revision = self.programmers.normal_values_revision(user_id);
        if expected != actual {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "Programmer capture-mode revision conflict: expected {expected}, actual {actual}"
                ),
            )
            .at_revision(values_revision)
            .at_related_revision(actual));
        }
        let mode = self.programmers.capture_mode(session).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                "Programmer values are unavailable",
            )
        })?;
        if mode.redirects_normal_values_to_preload() {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "normal Programmer values cannot be changed while Programmer capture is redirected to Preload",
            )
            .at_revision(values_revision)
            .at_related_revision(actual));
        }
        Ok(actual)
    }

    fn cached_values(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        fingerprint: RequestFingerprint,
    ) -> Result<Option<ProgrammingValuesResult>, ActionError> {
        self.values_replay
            .lock()
            .get(user_id, desk_id, session_id, request_id, fingerprint)
    }

    fn remember_values(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: String,
        fingerprint: RequestFingerprint,
        result: ProgrammingValuesResult,
    ) {
        self.values_replay.lock().insert(
            user_id,
            desk_id,
            session_id,
            request_id,
            fingerprint,
            result,
        );
    }
}

fn values_context(
    action: &ActionEnvelope<ProgrammingValuesRequest>,
) -> Result<(SessionId, UserId, String, u64), ActionError> {
    let session = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer values actions require an operator session",
        )
    })?;
    let user_id = action.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer values actions require an authenticated user",
        )
    })?;
    let request_id = action.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer values actions require a request_id",
        )
    })?;
    validate_request_id(request_id)?;
    let expected_revision = action.context.expected_revision.ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer values actions require an expected revision",
        )
    })?;
    Ok((session, user_id, request_id.to_owned(), expected_revision))
}

fn domain_mutation(mutation: &ProgrammingValueMutation) -> NormalProgrammerValueMutation {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => NormalProgrammerValueMutation::SetFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
        },
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => NormalProgrammerValueMutation::SetGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => NormalProgrammerValueMutation::ReleaseGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
        },
    }
}

fn domain_timing(timing: ProgrammingValueTiming) -> NormalProgrammerValueTiming {
    NormalProgrammerValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
}
