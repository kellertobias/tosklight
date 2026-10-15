//! Prepare immutable Timecode assets before any Live-control serialization gate.
use super::super::{AppState, Session, preload};
use super::programming_ports::ServerProgrammingPorts;
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ProgrammingCommand, ProgrammingPorts,
};
use light_programmer::command_line::{
    CommandGestureIntent, CommandKeyIntent, command_gesture_intent, command_key_intent,
};

pub(in crate::runtime) fn prepare(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
) -> Result<Option<preload::PreloadResources>, ActionError> {
    let ports = ServerProgrammingPorts::new(state, session, "preload_resource_preflight", true);
    ports.authorize_programming_change(context)?;
    // A resource refusal is still a normal service-owned rejected GO, including command history
    // and its existing wire outcome. Authorization failures above never read assets.
    Ok(
        match preload::preflight_timecode_resources(state, session) {
            Ok(resources) => resources,
            Err(message) => Some(preload::PreloadResources::Refused(message)),
        },
    )
}

pub(in crate::runtime) fn for_command(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    command: &ProgrammingCommand,
) -> Result<Option<preload::PreloadResources>, ActionError> {
    if !state
        .programming
        .capture_mode(session.id)
        .is_some_and(|mode| mode.blind)
    {
        return Ok(None);
    }
    let commits = match command {
        ProgrammingCommand::Preload { .. } => true,
        ProgrammingCommand::ApplyKey {
            key,
            phase,
            gesture,
            ..
        } => {
            let current = state
                .programming
                .command_line_state(session.id)
                .ok_or_else(|| {
                    ActionError::new(
                        ActionErrorKind::Forbidden,
                        "Programmer session is unavailable",
                    )
                })?;
            match gesture {
                Some(gesture) => matches!(
                    command_gesture_intent(&current, *key, *gesture),
                    CommandGestureIntent::Key(CommandKeyIntent::Preload)
                ),
                None => matches!(
                    command_key_intent(&current, *key, *phase),
                    CommandKeyIntent::Preload
                ),
            }
        }
        _ => false,
    };
    if commits {
        prepare(state, session, context)
    } else {
        Ok(None)
    }
}

pub(super) async fn for_command_async(
    state: &AppState,
    session: &Session,
    context: &ActionContext,
    command: &ProgrammingCommand,
) -> Result<Option<preload::PreloadResources>, ActionError> {
    let (state, session, context, command) = (
        state.clone(),
        session.clone(),
        context.clone(),
        command.clone(),
    );
    tokio::task::spawn_blocking(move || for_command(&state, &session, &context, &command))
        .await
        .map_err(|error| {
            ActionError::new(
                ActionErrorKind::Internal,
                format!("Preload resource task failed: {error}"),
            )
        })?
}
