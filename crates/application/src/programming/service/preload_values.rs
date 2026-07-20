use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPorts,
    ProgrammingPreloadValueMutation, ProgrammingPreloadValueTiming,
    ProgrammingPreloadValuesOutcome, ProgrammingPreloadValuesRequest,
    ProgrammingPreloadValuesResult,
};
use light_core::{SessionId, UserId};
use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};
use std::sync::Arc;

use super::preload_values_replay::PreloadReplayIdentity;
use super::values_replay_fingerprint::preload_request_fingerprint;
use super::values_validation::{validate_preload_value_mutations, validate_request_id};

impl ProgrammingService {
    pub fn handle_preload_values(
        &self,
        action: ActionEnvelope<ProgrammingPreloadValuesRequest>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPreloadValuesResult, ActionError> {
        let (session, user_id, request_id, expected_revision) = preload_values_context(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, user_id, || {
            ports.authorize(&action.context)?;
            self.assert_preload_values_owner(session, user_id)?;
            let fingerprint = preload_request_fingerprint(expected_revision, &action.command);
            let replay_identity = PreloadReplayIdentity {
                user_id,
                desk_id: action.context.desk_id,
                session_id: session,
                request_id,
            };
            if let Some(cached) = self
                .preload_values_replay
                .lock()
                .get(&replay_identity, fingerprint)?
            {
                return Ok(cached);
            }
            self.assert_preload_values_revision(user_id, expected_revision)?;
            let capture_mode_revision = self.assert_preload_capture_precondition(
                session,
                user_id,
                action.command.expected_capture_mode_revision,
            )?;
            let result = self.apply_preload_values_action(
                &action,
                ports,
                session,
                user_id,
                expected_revision,
                capture_mode_revision,
            )?;
            self.preload_values_replay
                .lock()
                .insert(replay_identity, fingerprint, result.clone());
            Ok(result)
        })
    }

    fn apply_preload_values_action(
        &self,
        action: &ActionEnvelope<ProgrammingPreloadValuesRequest>,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
        user_id: UserId,
        revision_before: u64,
        capture_mode_revision: u64,
    ) -> Result<ProgrammingPreloadValuesResult, ActionError> {
        let before = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let mutations = action.command.command.mutations();
        if !mutations.is_empty() {
            let environment = ports.values_environment(&action.context)?;
            validate_preload_value_mutations(mutations.as_ref(), &environment)?;
        }
        let domain_mutations = mutations.iter().map(domain_mutation).collect::<Vec<_>>();
        let changed = self
            .programmers
            .apply_preload_values(session, &domain_mutations);
        let warning = changed
            .then(|| ports.persist(&action.context, "programmer.preload_values"))
            .flatten();
        let after = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            session,
            &before,
            &after,
        );
        let values = self.preload_values_change(
            user_id,
            session,
            before.preload_values_generation,
            after.preload_values_generation,
        )?;
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let outcome = self.preload_values_outcome(&action.context, values, revision_before);
        Ok(ProgrammingPreloadValuesResult {
            context: action.context.clone(),
            outcome,
            capture_mode_revision,
            interaction_event_sequence,
            replayed: false,
            warning,
        })
    }

    fn preload_values_outcome(
        &self,
        context: &crate::ActionContext,
        change: Option<crate::ProgrammingPreloadValuesChange>,
        revision_before: u64,
    ) -> ProgrammingPreloadValuesOutcome {
        let Some(change) = change else {
            return ProgrammingPreloadValuesOutcome::NoChange {
                revision: revision_before,
            };
        };
        let projection = Arc::clone(&change.projection);
        let event_sequence = self
            .publish_preload_values(context, Some(change))
            .expect("a Preload values change always publishes one event");
        ProgrammingPreloadValuesOutcome::Changed {
            projection,
            event_sequence,
        }
    }

    fn assert_preload_values_owner(
        &self,
        session: SessionId,
        user_id: UserId,
    ) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(ActionError::new(
                ActionErrorKind::NotFound,
                "Preload values are unavailable",
            )),
        }
    }

    fn assert_preload_values_revision(
        &self,
        user_id: UserId,
        expected: u64,
    ) -> Result<(), ActionError> {
        let actual = self.programmers.preload_values_revision(user_id);
        if expected == actual {
            Ok(())
        } else {
            Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!("Preload values revision conflict: expected {expected}, actual {actual}"),
            )
            .at_revision(actual))
        }
    }

    fn assert_preload_capture_precondition(
        &self,
        session: SessionId,
        user_id: UserId,
        expected: u64,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.capture_mode_revision(user_id);
        let values_revision = self.programmers.preload_values_revision(user_id);
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
            ActionError::new(ActionErrorKind::NotFound, "Preload values are unavailable")
        })?;
        if !mode.redirects_normal_values_to_preload() {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "pending Preload values can only change while Programmer capture is redirected to Preload",
            )
            .at_revision(values_revision)
            .at_related_revision(actual));
        }
        Ok(actual)
    }
}

fn preload_values_context(
    action: &ActionEnvelope<ProgrammingPreloadValuesRequest>,
) -> Result<(SessionId, UserId, String, u64), ActionError> {
    let session = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values actions require an operator session",
        )
    })?;
    let user_id = action.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values actions require an authenticated user",
        )
    })?;
    let request_id = action.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Preload values actions require a request_id",
        )
    })?;
    validate_request_id(request_id)?;
    let expected_revision = action.context.expected_revision.ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Preload values actions require an expected revision",
        )
    })?;
    Ok((session, user_id, request_id.to_owned(), expected_revision))
}

fn domain_mutation(mutation: &ProgrammingPreloadValueMutation) -> PreloadProgrammerValueMutation {
    match mutation {
        ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => PreloadProgrammerValueMutation::SetFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => PreloadProgrammerValueMutation::ReleaseFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
        },
        ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => PreloadProgrammerValueMutation::SetGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => PreloadProgrammerValueMutation::ReleaseGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
        },
    }
}

fn domain_timing(timing: ProgrammingPreloadValueTiming) -> PreloadProgrammerValueTiming {
    PreloadProgrammerValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
}
