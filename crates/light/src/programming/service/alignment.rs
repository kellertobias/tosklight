use super::{ProgrammingService, context_session, context_user};
use crate::{ActionContext, ActionError, ActionErrorKind, ProgrammingPorts};
use light_core::{AttributeKey, AttributeValue, SessionId};
use light_programmer::{
    ProgrammerAlignmentBase, ProgrammerAlignmentError, ProgrammerAlignmentMode,
    ProgrammerAlignmentState,
};

impl ProgrammingService {
    /// Change the desk-local Align modifier without changing Programmer values or Undo history.
    pub fn set_alignment(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        mode: Option<ProgrammerAlignmentMode>,
    ) -> Result<Option<ProgrammerAlignmentState>, ActionError> {
        let session = context_session(context)?;
        let user_id = context_user(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            match self.programmers.user_id(session) {
                Some(owner) if owner == user_id => {}
                Some(_) => {
                    return Err(ActionError::new(
                        ActionErrorKind::Forbidden,
                        "the Programmer session does not belong to the authenticated user",
                    ));
                }
                None => {
                    return Err(ActionError::new(
                        ActionErrorKind::NotFound,
                        "Programmer Align is unavailable",
                    ));
                }
            }
            match mode {
                None => {
                    self.programmers.deactivate_alignment(session);
                    Ok(None)
                }
                Some(mode) => self.activate_or_reanchor_alignment(context, ports, session, mode),
            }
        })
    }

    fn activate_or_reanchor_alignment(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
        mode: ProgrammerAlignmentMode,
    ) -> Result<Option<ProgrammerAlignmentState>, ActionError> {
        let Some(current) = self.programmers.alignment(session) else {
            return self
                .programmers
                .activate_alignment(session, mode)
                .map(Some)
                .map_err(alignment_error);
        };
        if current.mode == mode {
            return Ok(Some(current));
        }
        let bases = match current.binding.as_ref() {
            None => Vec::new(),
            Some(binding) => alignment_bases(
                &self.programmers,
                ports,
                context,
                session,
                &binding
                    .bases
                    .iter()
                    .map(|base| base.fixture_id)
                    .collect::<Vec<_>>(),
                &binding.attribute,
            )?,
        };
        self.programmers
            .reanchor_alignment(session, mode, &bases)
            .map(Some)
            .map_err(alignment_error)
    }
}

pub(super) fn alignment_bases(
    programmers: &light_programmer::ProgrammerRegistry,
    ports: &dyn ProgrammingPorts,
    context: &ActionContext,
    session: SessionId,
    fixtures: &[light_core::FixtureId],
    attribute: &AttributeKey,
) -> Result<Vec<ProgrammerAlignmentBase>, ActionError> {
    let content = programmers
        .get(session)
        .map(|state| state.update_content())
        .unwrap_or_default();
    let owned = content
        .fixture_values
        .into_iter()
        .map(|value| ((value.fixture_id, value.attribute), value.value))
        .collect::<std::collections::HashMap<_, _>>();
    let environment = ports.values_environment(context)?;
    Ok(fixtures
        .iter()
        .filter(|fixture_id| {
            environment
                .supported_attributes
                .get(fixture_id)
                .is_some_and(|attributes| attributes.contains(attribute))
        })
        .filter_map(|fixture_id| {
            let address = (*fixture_id, attribute.clone());
            owned
                .get(&address)
                .or_else(|| environment.current_values.get(&address))
                .or_else(|| environment.default_values.get(&address))
                .and_then(AttributeValue::normalized)
                .map(|value| ProgrammerAlignmentBase {
                    fixture_id: *fixture_id,
                    value,
                    wraps: ports.programmer_attribute_wraps(context, *fixture_id, attribute),
                })
        })
        .collect())
}

pub(super) fn alignment_error(error: ProgrammerAlignmentError) -> ActionError {
    let (kind, message) = match error {
        ProgrammerAlignmentError::UnknownSession => (
            ActionErrorKind::NotFound,
            "Programmer Align is unavailable".to_owned(),
        ),
        ProgrammerAlignmentError::EmptySelection => (
            ActionErrorKind::Invalid,
            "Programmer Align requires a non-empty selection".to_owned(),
        ),
        ProgrammerAlignmentError::NotActive => (
            ActionErrorKind::Conflict,
            "Programmer Align is not active".to_owned(),
        ),
        ProgrammerAlignmentError::NonFiniteDelta => (
            ActionErrorKind::Invalid,
            "Programmer Align requires a finite encoder delta".to_owned(),
        ),
        ProgrammerAlignmentError::MissingBases => (
            ActionErrorKind::Invalid,
            "none of the aligned fixtures support the selected attribute".to_owned(),
        ),
        ProgrammerAlignmentError::UnexpectedBases
        | ProgrammerAlignmentError::BaseFixtureNotInFrozenOrder { .. }
        | ProgrammerAlignmentError::InvalidBaseValue { .. }
        | ProgrammerAlignmentError::DifferentAttribute { .. } => (
            ActionErrorKind::Invalid,
            format!("invalid Programmer Align transition: {error:?}"),
        ),
        ProgrammerAlignmentError::RevisionConflict { actual, .. } => {
            return ActionError::new(
                ActionErrorKind::Conflict,
                "Programmer Align changed during the encoder action",
            )
            .at_revision(actual);
        }
    };
    ActionError::new(kind, message)
}
