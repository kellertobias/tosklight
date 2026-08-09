use super::*;
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, HighlightActionPublication,
    HighlightChange, HighlightCommand, HighlightEnvironment, HighlightPorts,
};
use light_programmer::{HighlightSelectionWrite, ProgrammerSelection};

#[derive(Clone, Copy)]
pub(super) enum SelectionWriteFailure {
    Propagate,
    PreserveOscCompatibility,
}

pub(super) struct HeadlessHighlightPorts<'a> {
    state: &'a AppState,
    session: &'a Session,
    selection_write_failure: SelectionWriteFailure,
    environment: Option<HighlightEnvironment>,
}

impl<'a> HeadlessHighlightPorts<'a> {
    pub(super) fn new(state: &'a AppState, session: &'a Session) -> Self {
        Self {
            state,
            session,
            selection_write_failure: SelectionWriteFailure::Propagate,
            environment: None,
        }
    }

    pub(super) fn for_osc(state: &'a AppState, session: &'a Session) -> Self {
        Self {
            state,
            session,
            selection_write_failure: SelectionWriteFailure::PreserveOscCompatibility,
            environment: None,
        }
    }

    pub(super) fn with_environment(
        state: &'a AppState,
        session: &'a Session,
        environment: HighlightEnvironment,
    ) -> Self {
        Self {
            state,
            session,
            selection_write_failure: SelectionWriteFailure::Propagate,
            environment: Some(environment),
        }
    }
}

impl HighlightPorts for HeadlessHighlightPorts<'_> {
    fn environment(&self, _context: &ActionContext) -> Result<HighlightEnvironment, ActionError> {
        if let Some(environment) = &self.environment {
            return Ok(environment.clone());
        }
        let programmer = self
            .state
            .programming
            .get(self.session.id)
            .ok_or_else(|| not_found("programmer"))?;
        let selection = self
            .state
            .programming
            .selection(self.session.id)
            .ok_or_else(|| not_found("programmer selection"))?;
        let snapshot = self.state.output.snapshot();
        Ok(HighlightEnvironment {
            user_name: Some(self.session.user.name.clone()),
            selection,
            fixtures: highlight_fixture_summaries(&snapshot.fixtures),
            groups: highlight_groups(&snapshot),
            output_suppressed: programmer.blind || programmer.preview,
        })
    }

    fn apply_selection(
        &self,
        _context: &ActionContext,
        write: &HighlightSelectionWrite,
    ) -> Result<ProgrammerSelection, ActionError> {
        let result = apply_highlight_selection_write(self.state, self.session, Some(write));
        if let Err(error) = result
            && matches!(
                self.selection_write_failure,
                SelectionWriteFailure::Propagate
            )
        {
            return Err(action_error(error));
        }
        self.state
            .programming
            .selection(self.session.id)
            .ok_or_else(|| not_found("programmer selection"))
    }

    fn synchronize_output(
        &self,
        _context: &ActionContext,
        fixtures: &[light_core::FixtureId],
    ) -> Result<(), ActionError> {
        self.state.output.set_highlighted_fixtures(
            self.state
                .highlight
                .include_patch_previews(fixtures.iter().copied()),
        );
        Ok(())
    }

    fn synchronize_output_layers(
        &self,
        _context: &ActionContext,
        layers: &[light_programmer::HighlightOutputLayer],
    ) -> Result<(), ActionError> {
        self.state.output.set_highlight_layers(
            self.state
                .highlight
                .include_patch_preview_layers(layers.iter().cloned()),
        );
        Ok(())
    }

    fn publish_programmer_changed(&self, context: &ActionContext, command: &HighlightCommand) {
        let payload = match command {
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } if context.source == ActionSource::Osc => {
                serde_json::json!({"session_id":self.session.id,"source":"osc_highlight","action":action})
            }
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } if context.source == ActionSource::Extension => {
                serde_json::json!({"session_id":self.session.id,"source":"extension","action":action})
            }
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } => {
                serde_json::json!({"session_id":self.session.id,"source":"highlight","action":action})
            }
            HighlightCommand::Reconcile { source } => {
                serde_json::json!({"session_id":self.session.id,"source":source,"action":"highlight_selection_reconcile"})
            }
            HighlightCommand::Status => {
                serde_json::json!({"session_id":self.session.id,"source":"highlight_status_reconcile"})
            }
            HighlightCommand::Action {
                publication: HighlightActionPublication::Compatibility,
                ..
            } => return,
        };
        emit(self.state, "programmer_changed", payload);
    }

    fn publish_highlight_changed(
        &self,
        context: &ActionContext,
        command: &HighlightCommand,
        state: &HighlightState,
    ) {
        let payload = match command {
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } if context.source == ActionSource::Osc => serde_json::json!({
                "desk_id":self.session.desk.id,
                "user_id":self.session.user.id,
                "action":action,
                "source":"osc",
                "state":state,
            }),
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } if context.source == ActionSource::Extension => serde_json::json!({
                "desk_id":self.session.desk.id,
                "user_id":self.session.user.id,
                "action":action,
                "source":"extension",
                "state":state,
            }),
            HighlightCommand::Action {
                action,
                publication: HighlightActionPublication::Standard,
            } => serde_json::json!({
                "desk_id":self.session.desk.id,
                "user_id":self.session.user.id,
                "action":action,
                "state":state,
            }),
            HighlightCommand::Action {
                publication: HighlightActionPublication::Compatibility,
                ..
            } => serde_json::json!({
                "desk_id":self.session.desk.id,
                "user_id":self.session.user.id,
                "state":state,
            }),
            HighlightCommand::Reconcile { source } => serde_json::json!({
                "desk_id":self.session.desk.id,
                "user_id":self.session.user.id,
                "source":source,
                "state":state,
            }),
            HighlightCommand::Status => return,
        };
        let action = match command {
            HighlightCommand::Action { action, .. } => serde_json::to_value(action)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned)),
            HighlightCommand::Reconcile { .. } | HighlightCommand::Status => None,
        };
        let source = match command {
            HighlightCommand::Action { .. } if context.source == ActionSource::Osc => {
                Some("osc".to_owned())
            }
            HighlightCommand::Action { .. } if context.source == ActionSource::Extension => {
                Some("extension".to_owned())
            }
            HighlightCommand::Reconcile { source } => Some(source.clone()),
            HighlightCommand::Action { .. } | HighlightCommand::Status => None,
        };
        let revision = emit(self.state, "highlight_changed", payload);
        self.state
            .events
            .publish(light_application::EventDraft::highlight_changed(
                context,
                HighlightChange {
                    revision,
                    desk_id: self.session.desk.id,
                    user_id: self.session.user.id.0,
                    action,
                    source,
                    state: state.clone(),
                },
            ));
    }

    fn publish_feedback(&self, context: &ActionContext) {
        if context.source != ActionSource::Osc {
            send_osc_feedback(self.state, false);
        }
    }
}

fn not_found(what: &str) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, format!("{what} not found"))
}

pub(super) fn action_error(error: ApiError) -> ActionError {
    let kind = match error.status {
        StatusCode::BAD_REQUEST => ActionErrorKind::Invalid,
        StatusCode::UNAUTHORIZED => ActionErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ActionErrorKind::Forbidden,
        StatusCode::NOT_FOUND => ActionErrorKind::NotFound,
        StatusCode::CONFLICT => ActionErrorKind::Conflict,
        StatusCode::SERVICE_UNAVAILABLE => ActionErrorKind::Unavailable,
        _ => ActionErrorKind::Internal,
    };
    ActionError::new(kind, error.message)
}

pub(super) fn api_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(
            error
                .message
                .strip_suffix(" not found")
                .unwrap_or(&error.message),
        ),
        ActionErrorKind::Conflict => ApiError::conflict(error.message),
        ActionErrorKind::Busy | ActionErrorKind::Unavailable => {
            ApiError::unavailable(error.message)
        }
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_highlight_actions_enter_the_application_service() {
        let adapters = [
            (
                "highlight_api.rs",
                include_str!("highlight_api.rs"),
                "highlight_service",
            ),
            (
                "osc_highlight.rs",
                include_str!("osc_highlight.rs"),
                "execute_highlight",
            ),
            (
                "ws_output_handlers.rs",
                include_str!("ws_output_handlers.rs"),
                "apply_highlight_action",
            ),
        ];
        for (name, source, service_call) in adapters {
            assert!(
                !source.contains(".action_guarded("),
                "{name} must not invoke the Highlight registry directly"
            );
            assert!(
                source.contains(service_call),
                "{name} must route Highlight actions through the application service"
            );
        }
    }
}
