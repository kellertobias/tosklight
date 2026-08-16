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
    touched: bool,
) -> PlaybackTargetInterception {
    let Some(operation) = active_playback_target_operation(state, session) else {
        return PlaybackTargetInterception::NotArmed;
    };
    if !touched {
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
    match normalized.as_str() {
        "RECORD" | "REC" => Some(PlaybackTargetOperation::Record),
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
        ["SET"] => true,
        ["SET", number] => number.parse::<u16>().is_ok(),
        ["SET", "GROUP", group_id] => !group_id.is_empty(),
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
        .ok_or_else(|| ActionError::new(ActionErrorKind::Invalid, "SET target is not pending"))?;
    let (page, slot) = match address {
        PlaybackAddress::ExplicitPage { page, slot } => (page, slot),
        PlaybackAddress::CurrentPage { slot } => (current_page(state, session)?, slot),
        PlaybackAddress::Pool(_) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "a SET source requires a physical page Playback destination",
            ));
        }
        PlaybackAddress::Virtual(address) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                format!(
                    "Virtual {}.{} has no authoritative typed SET Playback target",
                    address.page(),
                    address.number().get()
                ),
            ));
        }
        PlaybackAddress::CueList(_) | PlaybackAddress::Group(_) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "this OSC address does not identify a configurable Playback",
            ));
        }
    };
    let command = if pending == "SET" {
        format!("SET {page} . {slot}")
    } else {
        format!("{pending} AT {page} . {slot}")
    };
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Osc,
    );
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
            "SET Playback targeting cannot require a command choice",
        )),
    }
}

fn target(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
) -> Result<ProgrammingCueRecordTarget, ActionError> {
    Ok(match address {
        PlaybackAddress::Pool(playback_number) => {
            ProgrammingCueRecordTarget::Pool { playback_number }
        }
        PlaybackAddress::ExplicitPage { page, slot } => {
            ProgrammingCueRecordTarget::PageSlot { page, slot }
        }
        PlaybackAddress::CurrentPage { slot } => ProgrammingCueRecordTarget::PageSlot {
            page: current_page(state, session)?,
            slot,
        },
        PlaybackAddress::CueList(cue_list_id) => {
            ProgrammingCueRecordTarget::CueList { cue_list_id }
        }
        PlaybackAddress::Group(_) => {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "Group runtime cannot be a cue record target",
            ));
        }
        PlaybackAddress::Virtual(address) => virtual_cue_target(state, address)?,
    })
}

fn virtual_cue_target(
    state: &AppState,
    address: light_playback::VirtualPlaybackAddress,
) -> Result<ProgrammingCueRecordTarget, ActionError> {
    let snapshot = state.output.snapshot();
    let definition = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.virtual_playbacks.get(&address.number().get()))
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                format!(
                    "Virtual {}.{} is not assigned",
                    address.page(),
                    address.number().get()
                ),
            )
        })?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = &definition.target else {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            format!(
                "Virtual {}.{} is not assigned to a Cuelist",
                address.page(),
                address.number().get()
            ),
        ));
    };
    Ok(ProgrammingCueRecordTarget::CueList {
        cue_list_id: *cue_list_id,
    })
}

fn current_page(state: &AppState, session: &Session) -> Result<u8, ActionError> {
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "no show is open"))?;
    state
        .installation
        .desk_page(session.desk.id, show.id)
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
            "user_id":session.user.id,
            "source":"osc",
            "target":playback_target_identity(address),
            "error":error,
        }),
    );
}

fn playback_target_identity(address: &PlaybackAddress) -> serde_json::Value {
    match address {
        PlaybackAddress::CurrentPage { slot } => {
            serde_json::json!({"addressing":"current_page","slot":slot})
        }
        PlaybackAddress::ExplicitPage { page, slot } => {
            serde_json::json!({"addressing":"explicit_page","page":page,"slot":slot})
        }
        PlaybackAddress::Pool(playback) => {
            serde_json::json!({"addressing":"pool","playback":playback})
        }
        PlaybackAddress::Virtual(address) => serde_json::json!({
            "addressing":"virtual",
            "page":address.page(),
            "playback":address.number().get(),
        }),
        PlaybackAddress::CueList(cue_list) => {
            serde_json::json!({"addressing":"cuelist","cuelist":cue_list})
        }
        PlaybackAddress::Group(group) => {
            serde_json::json!({"addressing":"group","group":group.as_str()})
        }
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
        for command in ["SET", "set 41", " SET GROUP front "] {
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
            "SET GROUP",
            "SET DYNAMIC 1",
            "COPY",
            "MOVE",
            "DELETE",
        ] {
            assert_eq!(playback_target_operation(unsupported), None);
        }
    }
}
