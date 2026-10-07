use super::{ProgrammingService, context_session};
use crate::{ActionContext, ActionError, ActionErrorKind, ProgrammingPorts};
use light_core::{AttributeKey, AttributeValue, SessionId};
use light_programmer::{
    ProgrammerAlignmentBase, ProgrammerAlignmentError, ProgrammerAlignmentMode,
    ProgrammerAlignmentState,
};

impl ProgrammingService {
    /// Already inside a Programmer action boundary; do not reacquire its desk gate.
    pub(super) fn finish_alignment(&self, context: &ActionContext, session: SessionId) {
        if self.programmers.deactivate_alignment(session) {
            self.publish_interaction(
                context,
                crate::ProgrammingInteractionChange::with_alignment(
                    context.desk_id,
                    None,
                    None,
                    Some(self.programmers.alignment_projection()),
                ),
            );
        }
    }
    /// Change the desk-local Align modifier without changing Programmer values or Undo history.
    ///
    /// `Ok(None)` means Align is Off afterwards. Activating Align while nothing is selected
    /// returns `Ok(None)` without changing any state.
    pub fn set_alignment(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        mode: Option<ProgrammerAlignmentMode>,
    ) -> Result<Option<ProgrammerAlignmentState>, ActionError> {
        self.change_alignment(context, ports, |_| mode)
    }

    pub fn cycle_alignment(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<Option<ProgrammerAlignmentState>, ActionError> {
        self.change_alignment(context, ports, |current| match current {
            None => Some(ProgrammerAlignmentMode::Left),
            Some(ProgrammerAlignmentMode::Left) => Some(ProgrammerAlignmentMode::Right),
            Some(ProgrammerAlignmentMode::Right) => Some(ProgrammerAlignmentMode::Out),
            Some(ProgrammerAlignmentMode::Out) => Some(ProgrammerAlignmentMode::In),
            Some(ProgrammerAlignmentMode::In) => None,
        })
    }

    fn change_alignment(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        requested: impl FnOnce(Option<ProgrammerAlignmentMode>) -> Option<ProgrammerAlignmentMode>,
    ) -> Result<Option<ProgrammerAlignmentState>, ActionError> {
        let session = context_session(context)?;
        self.with_programmer_and_desk_gate(context.desk_id, || {
            ports.authorize_programming_change(context)?;
            if !self.programmers.knows_session(session) {
                return Err(ActionError::new(
                    ActionErrorKind::NotFound,
                    "Programmer Align is unavailable",
                ));
            }
            let before = self.programmers.alignment_projection();
            let mode = requested(before.mode);
            let result = match mode {
                None => {
                    self.programmers.deactivate_alignment(session);
                    Ok(None)
                }
                Some(mode) => self.activate_or_reanchor_alignment(context, ports, session, mode),
            }?;
            let after = self.programmers.alignment_projection();
            if before != after {
                self.publish_interaction(
                    context,
                    crate::ProgrammingInteractionChange::with_alignment(
                        context.desk_id,
                        None,
                        None,
                        Some(after),
                    ),
                );
            }
            Ok(result)
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
            return match self.programmers.activate_alignment(session, mode) {
                Ok(state) => Ok(Some(state)),
                // Align with nothing selected is a harmless no-op: Align stays Off and nothing
                // else changes. Expected no-ops do not produce a notice.
                Err(ProgrammerAlignmentError::EmptySelection) => Ok(None),
                Err(error) => Err(alignment_error(error)),
            };
        };
        if current.mode == mode {
            return Ok(Some(current));
        }
        if let Some(binding) = &current.family_binding {
            return self
                .reanchor_aligned_family(context, ports, session, mode, binding)
                .map(Some);
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
        ProgrammerAlignmentError::InvalidFamily(message) => (ActionErrorKind::Invalid, message),
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
