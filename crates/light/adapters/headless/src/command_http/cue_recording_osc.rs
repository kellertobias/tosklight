use light_application::{
    ActionContext, ActionError, ActionErrorKind, ActionSource, PlaybackAddress,
    ProgrammingCueRecordTarget,
};

use super::super::{AppState, Session, emit};
use super::adapter::{ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command};
use super::programming_ports::ServerProgrammingPorts;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaybackTargetOperation {
    Record,
    Set,
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlaybackTargetInterception {
    NotArmed,
    Consumed,
    Off,
}

pub(crate) fn intercept_armed_playback(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
    is_record_target: bool,
    activated: bool,
) -> PlaybackTargetInterception {
    let Some(operation) = active_playback_target_operation(state, session) else {
        return PlaybackTargetInterception::NotArmed;
    };
    if operation == PlaybackTargetOperation::Record && !is_record_target {
        return PlaybackTargetInterception::NotArmed;
    }
    if !activated {
        return PlaybackTargetInterception::Consumed;
    }
    if operation == PlaybackTargetOperation::Off {
        return PlaybackTargetInterception::Off;
    }
    let selected_address = address.clone();
    let result = match operation {
        PlaybackTargetOperation::Record => record_target(state, session, address),
        PlaybackTargetOperation::Set => set_target(state, session, address),
        PlaybackTargetOperation::Off => unreachable!("Off returns before target mutation"),
    };
    match result {
        Ok(()) => emit_result(state, session, operation, &selected_address, None),
        Err(error) => emit_result(
            state,
            session,
            operation,
            &selected_address,
            Some(error.message),
        ),
    }
    PlaybackTargetInterception::Consumed
}

pub(crate) fn complete_off_target(state: &AppState, session: &Session, address: &PlaybackAddress) {
    state
        .programming
        .set_command_line(session.id, String::new());
    let _ = super::super::persist_programmer(state, session);
    emit_result(state, session, PlaybackTargetOperation::Off, address, None);
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"source":"osc_target"}),
    );
}

fn active_playback_target_operation(
    state: &AppState,
    session: &Session,
) -> Option<PlaybackTargetOperation> {
    let command = state.programming.get(session.id)?.command_line;
    playback_target_operation(&command)
}

fn playback_target_operation(command: &str) -> Option<PlaybackTargetOperation> {
    let normalized = command.trim().to_ascii_uppercase();
    if super::record_update_option::armed_record_option(&normalized).is_some() {
        return Some(PlaybackTargetOperation::Record);
    }
    match normalized.as_str() {
        "OFF" => Some(PlaybackTargetOperation::Off),
        _ if supported_pending_set(&normalized) => Some(PlaybackTargetOperation::Set),
        // COPY, MOVE, and DELETE have no whole-Playback mutation in the authoritative command
        // grammar. Their Cue and Preset forms require a complete source/destination address, so a
        // Playback surface must not guess one from a button or fader touch.
        _ => None,
    }
}

fn supported_pending_set(command: &str) -> bool {
    match command.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["ASSIGN", source, identifier, "AT"] => match *source {
            "GROUP" => !identifier.is_empty(),
            "CUELIST" | "DYNAMIC" | "MACRO" | "TIMECODE" => identifier.parse::<u16>().is_ok(),
            _ => false,
        },
        _ => false,
    }
}

fn record_target(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
) -> Result<(), ActionError> {
    let target = target(state, session, address)?;
    let ports = ServerProgrammingPorts::new(state, session, "osc_cue_record", true);
    ports
        .record_armed_cue(target)
        .map(|_| ())
        .map_err(|message| ActionError::new(ActionErrorKind::Invalid, message))
}

fn set_target(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
) -> Result<(), ActionError> {
    let pending = state
        .programming
        .get(session.id)
        .map(|programmer| programmer.command_line.trim().to_ascii_uppercase())
        .filter(|command| supported_pending_set(command))
        .ok_or_else(|| {
            ActionError::new(ActionErrorKind::Invalid, "ASSIGN target is not pending")
        })?;
    let (page, position) = match address {
        PlaybackAddress::Playback(address) => (address.page(), address.position()),
        PlaybackAddress::CurrentPage { position } => (current_page(state, session)?, position),
        PlaybackAddress::CueList(_) | PlaybackAddress::Group(_) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "This OSC address does not identify a configurable Playback",
            ));
        }
    };
    let command = format!("{pending} PBK {page} . {position}");
    let context = ActionContext::operator(session.desk.id, session.id.0, ActionSource::Osc);
    match execute_existing_command(
        state,
        session,
        &command,
        "osc_target",
        &context,
        ExistingCommandPolicy::Compatibility,
    ) {
        ExistingCommandOutcome::Accepted { .. } => Ok(()),
        ExistingCommandOutcome::Rejected { error } => {
            Err(ActionError::new(ActionErrorKind::Invalid, error))
        }
        ExistingCommandOutcome::ChoiceRequired { .. } => Err(ActionError::new(
            ActionErrorKind::Invalid,
            "ASSIGN Playback targeting cannot require a command choice",
        )),
    }
}

fn target(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
) -> Result<ProgrammingCueRecordTarget, ActionError> {
    Ok(match address {
        PlaybackAddress::Playback(address) => ProgrammingCueRecordTarget::Playback { address },
        PlaybackAddress::CurrentPage { position } => ProgrammingCueRecordTarget::Playback {
            address: light_playback::PlaybackAddress::new(current_page(state, session)?, position)
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error))?,
        },
        PlaybackAddress::CueList(cue_list_id) => {
            ProgrammingCueRecordTarget::CueList { cue_list_id }
        }
        PlaybackAddress::Group(_) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "Group runtime cannot be a Cue record target",
            ));
        }
    })
}
fn current_page(state: &AppState, session: &Session) -> Result<u8, ActionError> {
    state
        .installation
        .command_page(session.desk.id)
        .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))
}

fn emit_result(
    state: &AppState,
    session: &Session,
    operation: PlaybackTargetOperation,
    address: &PlaybackAddress,
    error: Option<String>,
) {
    let kind = match (operation, error.is_some()) {
        (PlaybackTargetOperation::Record, false) => "cue_recorded",
        (PlaybackTargetOperation::Record, true) => "cue_record_rejected",
        (PlaybackTargetOperation::Set, false) => "playback_target_selected",
        (PlaybackTargetOperation::Set, true) => "playback_target_rejected",
        (PlaybackTargetOperation::Off, false) => "playback_off_targeted",
        (PlaybackTargetOperation::Off, true) => "playback_off_target_rejected",
    };
    emit(
        state,
        kind,
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "source":"osc",
            "target":playback_target_identity(address),
            "error":error,
        }),
    );
}

fn playback_target_identity(address: &PlaybackAddress) -> serde_json::Value {
    match address {
        PlaybackAddress::Playback(address) => {
            serde_json::json!({"kind":"playback","page":address.page(),"position":address.position()})
        }
        PlaybackAddress::CurrentPage { position } => {
            serde_json::json!({"kind":"current_page","position":position})
        }
        PlaybackAddress::CueList(id) => serde_json::json!({"kind":"cue_list","cue_list_id":id.0}),
        PlaybackAddress::Group(id) => serde_json::json!({"kind":"group","group_id":id.as_str()}),
    }
}

#[cfg(test)]
mod tests {
    use super::{PlaybackTargetOperation, playback_target_operation, supported_pending_set};

    #[test]
    fn only_bare_record_set_and_off_arm_supported_playback_targeting() {
        for command in ["RECORD", "record ", " REC "] {
            assert_eq!(
                playback_target_operation(command),
                Some(PlaybackTargetOperation::Record)
            );
        }
        for command in [
            "ASSIGN CUELIST 41 AT",
            "assign dynamic 2 at",
            " ASSIGN GROUP front AT ",
        ] {
            assert!(supported_pending_set(&command.trim().to_ascii_uppercase()));
            assert_eq!(
                playback_target_operation(command),
                Some(PlaybackTargetOperation::Set)
            );
        }
        for command in ["OFF", " off "] {
            assert_eq!(
                playback_target_operation(command),
                Some(PlaybackTargetOperation::Off)
            );
        }
        for unsupported in [
            "RECORD CUE 2",
            "SET",
            "SET GROUP front",
            "ASSIGN DYNAMIC 1",
            "COPY",
            "MOVE",
            "DELETE",
        ] {
            assert_eq!(playback_target_operation(unsupported), None);
        }
    }
}
