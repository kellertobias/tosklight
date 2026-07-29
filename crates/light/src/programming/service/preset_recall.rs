use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPresetRecallDisposition,
    ProgrammingPresetRecallEnvironment, ProgrammingPresetRecallOutcome,
    ProgrammingPresetRecallPorts, ProgrammingPresetRecallRequest, ProgrammingPresetRecallResult,
    ProgrammingPresetRecallRevisionExpectation, ProgrammingRecalledPresetProjection,
};
use light_core::{SessionId, UserId};
use light_programmer::SelectionExpression;
use std::sync::Arc;

struct RecallIdentity {
    session_id: SessionId,
    user_id: UserId,
}

impl ProgrammingService {
    pub fn handle_preset_recall(
        &self,
        action: ActionEnvelope<ProgrammingPresetRecallRequest>,
        ports: &dyn ProgrammingPresetRecallPorts,
    ) -> Result<ProgrammingPresetRecallResult, ActionError> {
        let identity = recall_identity(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, identity.user_id, || {
            self.apply_preset_recall(action, ports, identity)
        })
    }

    fn apply_preset_recall(
        &self,
        action: ActionEnvelope<ProgrammingPresetRecallRequest>,
        ports: &dyn ProgrammingPresetRecallPorts,
        identity: RecallIdentity,
    ) -> Result<ProgrammingPresetRecallResult, ActionError> {
        ports.authorize_preset_recall(&action.context)?;
        self.assert_recall_owner(identity.session_id, identity.user_id)?;
        validate_request(&action.command)?;
        let values_revision = self.assert_recall_values_revision(
            identity.user_id,
            action.command.expected_values_revision,
        )?;
        let capture_mode_revision = self.assert_recall_capture_revision(
            identity.session_id,
            identity.user_id,
            action.command.expected_capture_mode_revision,
        )?;
        let selection = self
            .programmers
            .selection(identity.session_id)
            .ok_or_else(recall_unavailable)?;
        assert_expected(
            action.command.expected_selection_revision,
            selection.revision,
            "Programmer selection",
            values_revision,
        )?;
        let before = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        let environment = ports.preset_recall_environment(&action.context, &action.command)?;
        validate_environment(&action.command, &environment, values_revision)?;
        if selection.selected.is_empty() {
            return self.select_preset_targets(
                action,
                ports,
                identity,
                selection,
                before,
                environment,
                values_revision,
                capture_mode_revision,
            );
        }
        let mutations = super::super::preset_recall_plan::plan(
            &selection,
            &environment.preset,
            &environment.groups,
            environment.programmer_fade_millis,
        )?;
        let active_context = format!("preset:{}", action.command.address.storage_key());
        let transition = self
            .programmers
            .apply_normal_preset_recall(identity.session_id, &mutations, active_context.clone())
            .ok_or_else(recall_unavailable)?;
        let after = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            &before,
            &after,
        );
        let values_change = self.values_change(
            identity.user_id,
            &before.values_content,
            &after.values_content,
        )?;
        let changed = transition.changed() || interaction.is_some();
        let warning = changed
            .then(|| ports.persist_preset_recall(&action.context, "preset.apply"))
            .flatten();
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let (projection, values_event_sequence, resulting_revision) =
            self.complete_recall_values(&action, values_change, values_revision);
        let outcome = if changed {
            ProgrammingPresetRecallOutcome::Changed {
                values_revision: resulting_revision,
                projection,
                values_event_sequence,
            }
        } else {
            ProgrammingPresetRecallOutcome::NoChange {
                values_revision: resulting_revision,
            }
        };
        let result = ProgrammingPresetRecallResult {
            context: action.context.clone(),
            disposition: ProgrammingPresetRecallDisposition::Recalled,
            applied_fixtures: selection.selected.len(),
            selected_targets: 0,
            selection_revision: after.selection_revision,
            interaction_event_sequence,
            capture_mode_revision,
            active_context: Some(active_context),
            preset: recalled_projection(environment),
            outcome,
            warning,
        };
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn select_preset_targets(
        &self,
        action: ActionEnvelope<ProgrammingPresetRecallRequest>,
        ports: &dyn ProgrammingPresetRecallPorts,
        identity: RecallIdentity,
        selection: light_programmer::ProgrammerSelection,
        before: Snapshot,
        environment: ProgrammingPresetRecallEnvironment,
        values_revision: u64,
        capture_mode_revision: u64,
    ) -> Result<ProgrammingPresetRecallResult, ActionError> {
        let target_plan = super::super::preset_recall_plan::target_selection(
            &environment.preset,
            &environment.groups,
            &environment.selectable_targets,
            &environment.target_expansions,
        );
        if !target_plan.selected.is_empty() {
            self.programmers
                .replace_selection_if_revision(
                    identity.session_id,
                    selection.revision,
                    target_plan.selected.iter().copied(),
                    SelectionExpression::Static,
                )
                .map_err(super::support::selection_replace_error)?;
        }
        let after = Snapshot::read(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            identity.user_id,
        )?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            identity.session_id,
            &before,
            &after,
        );
        let changed = interaction.is_some();
        let persistence_warning = changed
            .then(|| ports.persist_preset_recall(&action.context, "preset.select_targets"))
            .flatten();
        let warning = combine_warnings(target_plan.warning, persistence_warning);
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let active_context = self
            .programmers
            .get(identity.session_id)
            .and_then(|programmer| programmer.active_context);
        let outcome = if changed {
            ProgrammingPresetRecallOutcome::Changed {
                values_revision,
                projection: None,
                values_event_sequence: None,
            }
        } else {
            ProgrammingPresetRecallOutcome::NoChange { values_revision }
        };
        let result = ProgrammingPresetRecallResult {
            context: action.context.clone(),
            disposition: ProgrammingPresetRecallDisposition::TargetsSelected,
            applied_fixtures: 0,
            selected_targets: target_plan.selected.len(),
            selection_revision: after.selection_revision,
            interaction_event_sequence,
            capture_mode_revision,
            active_context,
            preset: recalled_projection(environment),
            outcome,
            warning,
        };
        Ok(result)
    }

    fn complete_recall_values(
        &self,
        action: &ActionEnvelope<ProgrammingPresetRecallRequest>,
        change: Option<crate::ProgrammingValuesChange>,
        revision_before: u64,
    ) -> (
        Option<Arc<crate::ProgrammingValuesProjection>>,
        Option<u64>,
        u64,
    ) {
        let Some(change) = change else {
            return (None, None, revision_before);
        };
        let projection = Arc::clone(&change.projection);
        let revision = projection.revision;
        let event_sequence = self.publish_values(&action.context, Some(change));
        (Some(projection), event_sequence, revision)
    }

    fn assert_recall_owner(&self, session: SessionId, user_id: UserId) -> Result<(), ActionError> {
        match self.programmers.user_id(session) {
            Some(owner) if owner == user_id => Ok(()),
            Some(_) => Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            )),
            None => Err(recall_unavailable()),
        }
    }

    fn assert_recall_values_revision(
        &self,
        user_id: UserId,
        expected: ProgrammingPresetRecallRevisionExpectation,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.normal_values_revision(user_id);
        assert_expected(expected, actual, "Programmer values", actual)?;
        Ok(actual)
    }

    fn assert_recall_capture_revision(
        &self,
        session: SessionId,
        user_id: UserId,
        expected: ProgrammingPresetRecallRevisionExpectation,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.capture_mode_revision(user_id);
        assert_expected(expected, actual, "Programmer capture-mode", actual)?;
        let mode = self
            .programmers
            .capture_mode(session)
            .ok_or_else(recall_unavailable)?;
        if mode.redirects_normal_values_to_preload() {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Preset recall is unavailable while Programmer capture is redirected to Preload",
            )
            .at_revision(self.programmers.normal_values_revision(user_id))
            .at_related_revision(actual));
        }
        Ok(actual)
    }
}

fn recall_identity(
    action: &ActionEnvelope<ProgrammingPresetRecallRequest>,
) -> Result<RecallIdentity, ActionError> {
    let session_id = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preset recall requires an operator session",
        )
    })?;
    let user_id = action.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preset recall requires an authenticated user",
        )
    })?;
    Ok(RecallIdentity {
        session_id,
        user_id,
    })
}

fn validate_request(request: &ProgrammingPresetRecallRequest) -> Result<(), ActionError> {
    if request.show_id.0.is_nil() {
        return Err(invalid("Preset recall requires a valid show_id"));
    }
    light_programmer::PresetAddress::new(request.address.family, request.address.number)
        .map_err(invalid)?;
    Ok(())
}

fn validate_environment(
    request: &ProgrammingPresetRecallRequest,
    environment: &ProgrammingPresetRecallEnvironment,
    values_revision: u64,
) -> Result<(), ActionError> {
    if environment.show_id != request.show_id || environment.address != request.address {
        return Err(invalid("Preset recall resolved a mismatched authority"));
    }
    assert_expected(
        request.expected_show_revision,
        environment.show_revision.value(),
        "active Show",
        values_revision,
    )?;
    assert_expected(
        request.expected_preset_revision,
        environment.object_revision,
        "Preset object",
        values_revision,
    )?;
    Ok(())
}

fn assert_expected(
    expected: ProgrammingPresetRecallRevisionExpectation,
    actual: u64,
    authority: &str,
    values_revision: u64,
) -> Result<(), ActionError> {
    match expected {
        ProgrammingPresetRecallRevisionExpectation::Current => Ok(()),
        ProgrammingPresetRecallRevisionExpectation::Exact(expected) if expected == actual => Ok(()),
        ProgrammingPresetRecallRevisionExpectation::Exact(expected) => Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!("{authority} revision conflict: expected {expected}, actual {actual}"),
        )
        .at_revision(values_revision)
        .at_related_revision(actual)),
    }
}

fn recalled_projection(
    environment: ProgrammingPresetRecallEnvironment,
) -> ProgrammingRecalledPresetProjection {
    ProgrammingRecalledPresetProjection {
        show_id: environment.show_id,
        show_revision: environment.show_revision,
        object_id: environment.object_id,
        object_revision: environment.object_revision,
        address: environment.address,
        raw_body: environment.raw_body,
    }
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn recall_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Preset recall authority is unavailable",
    )
}

fn combine_warnings(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first} {second}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}
