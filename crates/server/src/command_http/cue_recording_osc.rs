use light_application::{
    ActionError, ActionErrorKind, PlaybackAddress, ProgrammingCueRecordTarget,
};

use super::super::{AppState, Session, emit};
use super::programming_ports::ServerProgrammingPorts;

pub(crate) fn intercept_armed_playback(
    state: &AppState,
    session: &Session,
    address: PlaybackAddress,
    touched: bool,
) -> bool {
    if !record_is_armed(state, session) {
        return false;
    }
    if !touched {
        return true;
    }
    let result = target(state, session, address).and_then(|target| {
        let ports = ServerProgrammingPorts::new(state, session, "osc_cue_record", true);
        ports
            .record_armed_cue(target)
            .map(|_| ())
            .map_err(|message| ActionError::new(ActionErrorKind::Invalid, message))
    });
    match result {
        Ok(()) => emit_result(state, session, "cue_recorded", None),
        Err(error) => emit_result(state, session, "cue_record_rejected", Some(error.message)),
    }
    true
}

fn record_is_armed(state: &AppState, session: &Session) -> bool {
    state.programmers.get(session.id).is_some_and(|programmer| {
        matches!(
            programmer.command_line.trim().to_ascii_uppercase().as_str(),
            "RECORD" | "REC"
        )
    })
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
    })
}

fn current_page(state: &AppState, session: &Session) -> Result<u8, ActionError> {
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "no show is open"))?;
    state
        .desk
        .lock()
        .desk_page(session.desk.id, show.id)
        .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))
}

fn emit_result(state: &AppState, session: &Session, kind: &str, error: Option<String>) {
    emit(
        state,
        kind,
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "source":"osc",
            "error":error,
        }),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_bare_record_arms_playback_targeting() {
        for armed in ["RECORD", "record ", " REC "] {
            assert!(matches!(
                armed.trim().to_ascii_uppercase().as_str(),
                "RECORD" | "REC"
            ));
        }
        assert!(!matches!(
            "RECORD CUE 2".trim().to_ascii_uppercase().as_str(),
            "RECORD" | "REC"
        ));
    }
}
