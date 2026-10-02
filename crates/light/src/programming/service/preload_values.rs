use super::values_legacy::semantic_intent;
use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPorts,
    ProgrammingPreloadValueMutation, ProgrammingPreloadValueTiming,
    ProgrammingPreloadValuesCommand, ProgrammingPreloadValuesOutcome,
    ProgrammingPreloadValuesRequest, ProgrammingPreloadValuesResult, ProgrammingValueMutation,
};
use light_core::SessionId;
use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};
use std::{borrow::Cow, sync::Arc};

use super::preload_values_replay::PreloadReplayIdentity;
use super::values::plan_value_intent;
use super::values_replay_fingerprint::preload_request_fingerprint;
use super::values_validation::{validate_preload_value_mutations, validate_request_id};

impl ProgrammingService {
    pub fn handle_preload_values(
        &self,
        action: ActionEnvelope<ProgrammingPreloadValuesRequest>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPreloadValuesResult, ActionError> {
        let (session, request_id, expected_revision) = preload_values_context(&action)?;
        self.with_programmer_and_desk_gate(action.context.desk_id, || {
            self.assert_value_action_boundary()?;
            ports.authorize_programming_change(&action.context)?;
            // A delayed Finish is harmless after disarm or a capture-mode change. Ordinary
            // edits retain their existing owner and revision admission checks.
            if !matches!(
                &action.command.command,
                ProgrammingPreloadValuesCommand::FinishGesture { .. }
            ) {
                self.assert_preload_values_owner(session)?;
            }
            let fingerprint = preload_request_fingerprint(expected_revision, &action.command);
            let replay_identity = PreloadReplayIdentity {
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
            let result = if let ProgrammingPreloadValuesCommand::FinishGesture {
                attribute,
                undo_group,
            } = &action.command.command
            {
                self.finish_captured_family_gesture(
                    &action.context,
                    true,
                    &super::values_legacy::semantic_gesture_attribute(attribute),
                    undo_group,
                )?;
                ProgrammingPreloadValuesResult {
                    context: action.context.clone(),
                    outcome: ProgrammingPreloadValuesOutcome::NoChange {
                        revision: self.programmers.preload_values_revision(),
                    },
                    capture_mode_revision: self.programmers.capture_mode_revision(),
                    interaction_event_sequence: None,
                    replayed: false,
                    warning: None,
                    hold: None,
                    color_adoption: None,
                }
            } else {
                self.assert_preload_values_revision(expected_revision)?;
                let capture_mode_revision = self.assert_preload_capture_precondition(
                    session,
                    action.command.expected_capture_mode_revision,
                )?;
                self.apply_preload_values_action(
                    &action,
                    ports,
                    session,
                    expected_revision,
                    capture_mode_revision,
                )?
            };
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
        revision_before: u64,
        capture_mode_revision: u64,
    ) -> Result<ProgrammingPreloadValuesResult, ActionError> {
        let before = Snapshot::read(&self.programmers, action.context.desk_id, session)?;
        let raw_mutations = action.command.command.mutations();
        let mut environment = (!raw_mutations.is_empty()
            || action.command.command.intent().is_some())
        .then(|| ports.values_environment(&action.context))
        .transpose()?;
        let mut family_gesture = None;
        let mut hold = None;
        let mut family_alignment_plan = None;
        let semantic = semantic_intent(action.command.command.intent(), environment.as_ref())?;
        let mutations = if let Some(intent) = semantic.as_deref() {
            let active = self
                .programmers
                .preload_pending_values(session)
                .ok_or_else(|| {
                    ActionError::new(ActionErrorKind::NotFound, "Preload values are unavailable")
                })?;
            let active_values = active
                .fixture_values
                .iter()
                .map(|value| {
                    (
                        (value.fixture_id, value.attribute.clone()),
                        value.value.clone(),
                    )
                })
                .collect();
            let environment = environment
                .as_mut()
                .expect("Preload intents load a values environment");
            super::values::validate_value_intent(intent, environment)?;
            family_gesture = self.prepare_family_gesture(
                &action.context,
                ports,
                true,
                intent,
                environment,
                &active_values,
            )?;
            // TL-594: an unresolvable displayed source holds the whole action quietly.
            hold = environment.displayed_source_hold;
            let groups = active
                .group_values
                .iter()
                .map(|value| {
                    (
                        (value.group_id.clone(), value.attribute.clone()),
                        value.value.clone(),
                    )
                })
                .collect();
            let normal_plan = if hold.is_some() {
                Vec::new()
            } else if let Some((mutations, plan)) = self.plan_aligned_family_intent(
                session,
                light_programmer::ProgrammerAlignmentLane::Preload,
                intent,
                environment,
                &active_values,
                &groups,
            )? {
                family_alignment_plan = Some(plan);
                mutations
            } else {
                plan_value_intent(intent, environment, active_values, groups)?
            };
            Cow::Owned(normal_plan.into_iter().map(preload_mutation).collect())
        } else {
            raw_mutations
        };
        if !mutations.is_empty() {
            validate_preload_value_mutations(
                mutations.as_ref(),
                environment
                    .as_ref()
                    .expect("Preload value mutations load a values environment"),
                // Intent validation already checks newly authored curves; untouched curves
                // remain legal if a live Group has since shrunk.
                action.command.command.intent().is_none(),
            )?;
        }
        let domain_mutations = mutations.iter().map(domain_mutation).collect::<Vec<_>>();
        let aligned = family_alignment_plan.is_some();
        let mutate = || {
            self.programmers.apply_preload_values_grouped(
                session,
                &domain_mutations,
                family_gesture
                    .as_ref()
                    .map(|gesture| gesture.undo_group.as_str())
                    .or_else(|| {
                        action
                            .command
                            .command
                            .intent()
                            .and_then(|intent| intent.undo_group.as_deref())
                    }),
            )
        };
        let changed =
            if let Some(plan) = family_alignment_plan.filter(|plan| !plan.values.is_empty()) {
                self.programmers
                    .apply_family_alignment_plan(session, plan, mutate)
                    .map_err(super::alignment::alignment_error)?
                    .0
            } else {
                mutate()
            };
        if changed && !aligned && action.command.command.intent().is_some() {
            self.programmers.deactivate_alignment(session);
        }
        let warning = changed
            .then(|| ports.persist(&action.context, "programmer.preload_values"))
            .flatten();
        let after = Snapshot::read(&self.programmers, action.context.desk_id, session)?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            session,
            &before,
            &after,
        );
        let values = self.preload_values_change(
            session,
            before.preload_values_generation,
            after.preload_values_generation,
        )?;
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let outcome = self.preload_values_outcome(&action.context, values, revision_before);
        // A held first sample retains no capture: the next sample adopts its own lease.
        self.finish_family_gesture(family_gesture.filter(|_| hold.is_none()), changed);
        Ok(ProgrammingPreloadValuesResult {
            context: action.context.clone(),
            outcome,
            capture_mode_revision,
            interaction_event_sequence,
            replayed: false,
            warning,
            hold,
            color_adoption: super::color_adoption::reported(environment, hold),
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

    fn assert_preload_values_owner(&self, session: SessionId) -> Result<(), ActionError> {
        if self.programmers.knows_session(session) {
            Ok(())
        } else {
            Err(ActionError::new(
                ActionErrorKind::NotFound,
                "Preload values are unavailable",
            ))
        }
    }

    fn assert_preload_values_revision(&self, expected: u64) -> Result<(), ActionError> {
        let actual = self.programmers.preload_values_revision();
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
        expected: u64,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.capture_mode_revision();
        let values_revision = self.programmers.preload_values_revision();
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

fn preload_mutation(mutation: ProgrammingValueMutation) -> ProgrammingPreloadValueMutation {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing: ProgrammingPreloadValueTiming {
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            },
        },
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        },
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing: ProgrammingPreloadValueTiming {
                fade: timing.fade,
                fade_millis: timing.fade_millis,
                delay_millis: timing.delay_millis,
            },
        },
        ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        },
    }
}

fn preload_values_context(
    action: &ActionEnvelope<ProgrammingPreloadValuesRequest>,
) -> Result<(SessionId, String, u64), ActionError> {
    let session = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values actions require an operator session",
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
    Ok((session, request_id.to_owned(), expected_revision))
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
