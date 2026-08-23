use super::events::{persist_with_warning, publish_osc_result};
use super::programming_ports::ServerProgrammingPorts;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ExecutionPolicy,
    ProgrammingCommand, ProgrammingLiveSnapshot, ProgrammingResult,
};
use light_programmer::command_line::{CommandGesture, CommandKey, CommandKeyPhase};

use super::super::{ApiError, AppState, Session, capability_resources::ProgrammingResource};

pub(crate) enum ExistingCommandOutcome {
    Accepted {
        applied: usize,
        persistence_warning: Option<String>,
        replayed: bool,
    },
    ChoiceRequired {
        pending_choice: light_application::PendingCommandChoice,
    },
    Rejected {
        error: String,
    },
}

pub(crate) fn prevalidate_typed_command(
    state: &AppState,
    _session: &Session,
    command: &str,
    context: &ActionContext,
) -> Result<bool, String> {
    let show_id = state.active_show.current().map(|entry| entry.id);
    if super::speed_group_binding_command::parse(command)?.is_some() {
        if show_id.is_none() {
            return Err("no show is open".into());
        }
        return Ok(true);
    }
    let object = object_command(command)?;
    if object.is_some_and(|command| {
        matches!(
            command,
            ObjectCommand::RunMacro(_) | ObjectCommand::EditMacro(_)
        )
    }) && matches!(context.source, ActionSource::Macro)
    {
        return Err("Macros cannot run or edit another Macro".into());
    }
    let parsed = object.is_some()
        || super::programming_ports::is_fixture_freeze_command(command)?
        || group_record_command(command)?.is_some()
        || preset_record_address(command)?.is_some()
        || super::cue_recording_command::parse(command)?.is_some()
        || super::cue_link_command::parse(command)?.is_some()
        || super::cue_deletion_command::parse(command)?.is_some()
        || if super::cue_transfer_command::is_cue_transfer(command) {
            super::cue_transfer_command::parse(
                command,
                show_id.ok_or_else(|| "no show is open".to_owned())?,
            )?
            .is_some()
        } else {
            false
        }
        || super::cue_navigation_command::parse(command)?.is_some()
        || super::playback_selection_command::parse(command)?.is_some()
        || super::speed_group_command::parse(command)?.is_some();
    if parsed && state.active_show.current().is_none() {
        return Err("no show is open".into());
    }
    Ok(parsed)
}

pub(crate) fn prevalidate_external_command(
    state: &AppState,
    _session: &Session,
    command: &str,
    context: &ActionContext,
) -> Result<(), String> {
    let (tokens, _) = super::super::tokenize_programmer_command(command)?;
    if state.active_show.current().is_none() {
        return Err("no show is open".into());
    }
    if super::speed_group_binding_command::parse(command)?.is_some() {
        return Ok(());
    }
    if let Some(command) = object_command(command)? {
        if matches!(
            command,
            ObjectCommand::RunMacro(_) | ObjectCommand::EditMacro(_)
        ) && matches!(context.source, ActionSource::Macro)
        {
            return Err("Macros cannot run or edit another Macro".into());
        }
        return Ok(());
    }
    match tokens.first().map(String::as_str) {
        Some("GO" | "LOAD") => super::cue_navigation_command::parse(command)?
            .is_some()
            .then_some(())
            .ok_or_else(|| "GO TO or LOAD command is invalid".into()),
        Some("PBK" | "VPBK" | "CUELIST") => super::playback_selection_command::parse(command)?
            .is_some()
            .then_some(())
            .ok_or_else(|| "playback selection command is invalid".into()),
        Some("SPD") => super::speed_group_command::parse(command)?
            .is_some()
            .then_some(())
            .ok_or_else(|| "SPD GRP command is invalid".into()),
        Some(
            "RECORD" | "REC" | "UPDATE" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY"
            | "SET" | "ASSIGN",
        ) => {
            if tokens.len() < 2 {
                Err(format!("{} requires a target", tokens[0]))
            } else {
                compatibility_only_family(command).map(|_| ())
            }
        }
        _ => Err("command family is invalid".into()),
    }
}

/// Whether a command line only *operates* the desk, leaving the Programmer untouched.
///
/// Deliberately an allow-list. A guest surface runs a command only when it is one of the families
/// that are known not to reach the Programmer — going to a Cue, selecting a playback, setting a
/// speed-group speed, running a Macro. Anything unrecognised counts as programming, so a command
/// added later is refused from a guest until somebody decides it is safe.
pub(crate) fn command_only_operates_the_desk(command: &str) -> bool {
    let Ok((tokens, _)) = super::super::tokenize_programmer_command(command) else {
        return false;
    };
    if super::speed_group_binding_command::parse(command)
        .ok()
        .flatten()
        .is_some()
    {
        return true;
    }
    // Running a stored Macro is operating; editing one is programming.
    if let Ok(Some(object)) = object_command(command) {
        return matches!(object, ObjectCommand::RunMacro(_));
    }
    matches!(
        tokens.first().map(String::as_str),
        Some("GO" | "LOAD" | "PBK" | "VPBK" | "CUELIST" | "SPD")
    )
}

#[derive(Clone, Copy)]
pub(crate) enum ExistingCommandPolicy {
    /// Temporary adapter for the legacy WebSocket and OSC grammar while owning services migrate.
    Compatibility,
    /// Public v2 guarantee: only commands whose complete mutation is isolated in Programmer.
    AtomicProgrammer,
}

pub(crate) fn ordered_ui_command_policy(command: &str) -> ExistingCommandPolicy {
    match compatibility_only_family(command) {
        Ok(Some(_)) => ExistingCommandPolicy::Compatibility,
        Ok(None) | Err(_) => ExistingCommandPolicy::AtomicProgrammer,
    }
}

/// Executes the existing grammar while keeping transport envelopes out of the domain path.
pub(crate) fn execute_existing_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    context: &ActionContext,
    policy: ExistingCommandPolicy,
) -> ExistingCommandOutcome {
    let request_id = context.request_id.as_deref();
    match object_command(command) {
        Ok(Some(object_command)) => {
            return match execute_object_command(state, session, object_command, context) {
                Ok((applied, feedback)) => {
                    super::super::record_command_history(
                        state, session, command, "accepted", &feedback, source, request_id,
                    );
                    ExistingCommandOutcome::Accepted {
                        applied,
                        persistence_warning: None,
                        replayed: false,
                    }
                }
                Err(error) => {
                    super::super::record_command_history(
                        state,
                        session,
                        command,
                        "rejected",
                        &error.to_string(),
                        source,
                        request_id,
                    );
                    ExistingCommandOutcome::Rejected {
                        error: error.to_string(),
                    }
                }
            };
        }
        Err(error) => {
            super::super::record_command_history(
                state, session, command, "rejected", &error, source, request_id,
            );
            return ExistingCommandOutcome::Rejected { error };
        }
        Ok(None) => {}
    }
    if let Some(error) = atomic_policy_error(command, policy) {
        super::super::record_command_history(
            state, session, command, "rejected", &error, source, request_id,
        );
        return ExistingCommandOutcome::Rejected { error };
    }
    let result = execute_with_policy(state, session, command, context, policy);
    finish_existing_command(state, session, command, source, request_id, result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectCommand {
    RunMacro(u16),
    EditMacro(u16),
    RunTimecode(u32),
    ArmTimecode(u32),
    DisarmTimecode(u32),
    EditTimecode(u32),
}

fn object_command(command: &str) -> Result<Option<ObjectCommand>, String> {
    let tokens = command
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
        .collect::<Vec<_>>();
    let (family, number, suffix, edit) = match tokens.as_slice() {
        [family, number] => (family.as_str(), number.as_str(), None, false),
        [family, number, suffix] if family == "TIMECODE" => (
            family.as_str(),
            number.as_str(),
            Some(suffix.as_str()),
            false,
        ),
        [set, family, number] if set == "SET" => (family.as_str(), number.as_str(), None, true),
        _ if matches!(
            tokens.first().map(String::as_str),
            Some("MACRO" | "TIMECODE")
        ) || matches!(tokens.as_slice(), [set, family, ..] if set == "SET" && matches!(family.as_str(), "MACRO" | "TIMECODE")) =>
        {
            return Err("expected [SET] MACRO|TIMECODE <positive pool number> [+|-]".into());
        }
        _ => return Ok(None),
    };
    if !matches!(family, "MACRO" | "TIMECODE") {
        return Ok(None);
    }
    let number = number
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("{family} pool number must be a positive whole number"))?;
    let macro_number =
        || u16::try_from(number).map_err(|_| "MACRO pool number cannot exceed 65535".to_owned());
    match (family, edit, suffix) {
        ("MACRO", false, None) => Ok(Some(ObjectCommand::RunMacro(macro_number()?))),
        ("MACRO", true, None) => Ok(Some(ObjectCommand::EditMacro(macro_number()?))),
        ("TIMECODE", false, None) => Ok(Some(ObjectCommand::RunTimecode(number))),
        ("TIMECODE", false, Some("+")) => Ok(Some(ObjectCommand::ArmTimecode(number))),
        ("TIMECODE", false, Some("-")) => Ok(Some(ObjectCommand::DisarmTimecode(number))),
        ("TIMECODE", true, None) => Ok(Some(ObjectCommand::EditTimecode(number))),
        ("MACRO", _, Some(_)) => Err("MACRO does not accept + or -".into()),
        ("TIMECODE", true, Some(_)) => Err("SET TIMECODE does not accept + or -".into()),
        ("TIMECODE", false, Some(_)) => Err("TIMECODE accepts only + or - after its number".into()),
        _ => Err("object command is invalid".into()),
    }
}

fn execute_object_command(
    state: &AppState,
    session: &Session,
    command: ObjectCommand,
    context: &ActionContext,
) -> Result<(usize, String), ApiError> {
    use crate::runtime::timecode_v2::CommandLineTimecodeAction as Timecode;
    match command {
        ObjectCommand::RunMacro(number) => {
            let execution_id =
                crate::runtime::macros_v2::start_macro_from_command_line(state, session, number)?;
            Ok((
                1,
                format!("Started Macro {number} as execution {execution_id}"),
            ))
        }
        ObjectCommand::EditMacro(number) => {
            crate::runtime::macros_v2::request_macro_editor_from_command_line(
                state, session, number,
            )?;
            Ok((0, format!("Opened Macro {number} editor")))
        }
        ObjectCommand::RunTimecode(number) => crate::runtime::timecode_v2::timecode_command(
            state,
            session,
            number,
            Timecode::Run,
            context,
        )
        .map(|feedback| (1, feedback)),
        ObjectCommand::ArmTimecode(number) => crate::runtime::timecode_v2::timecode_command(
            state,
            session,
            number,
            Timecode::Arm,
            context,
        )
        .map(|feedback| (1, feedback)),
        ObjectCommand::DisarmTimecode(number) => crate::runtime::timecode_v2::timecode_command(
            state,
            session,
            number,
            Timecode::Disarm,
            context,
        )
        .map(|feedback| (1, feedback)),
        ObjectCommand::EditTimecode(number) => crate::runtime::timecode_v2::timecode_command(
            state,
            session,
            number,
            Timecode::Edit,
            context,
        )
        .map(|feedback| (0, feedback)),
    }
}

fn atomic_policy_error(command: &str, policy: ExistingCommandPolicy) -> Option<String> {
    if !matches!(policy, ExistingCommandPolicy::AtomicProgrammer) {
        return None;
    }
    match compatibility_only_family(command) {
        Err(error) => Some(error),
        Ok(Some("UPDATE" | "ASSIGN" | "Speed Group binding")) => None,
        Ok(Some(family)) => Some(format!(
            "{family} commands are not yet available through the atomic command-line HTTP API"
        )),
        Ok(None) => None,
    }
}

pub(super) fn preset_record_address(
    command: &str,
) -> Result<Option<light_programmer::PresetAddress>, String> {
    let (tokens, timing) = super::super::tokenize_programmer_command(command)?;
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "RECORD" | "REC"))
        || timing.fade_millis.is_some()
        || timing.delay_millis.is_some()
        || tokens.len() != 4
    {
        return Ok(None);
    }
    Ok(super::super::command_preset_address(&tokens[1..]).ok())
}

pub(super) fn group_record_command(
    command: &str,
) -> Result<Option<(String, light_application::ProgrammingGroupRecordOperation)>, String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let parsed = match tokens.as_slice() {
        [record, group, id] if is_record(record) && group.eq_ignore_ascii_case("GROUP") => Some((
            (*id).to_owned(),
            light_application::ProgrammingGroupRecordOperation::Overwrite,
        )),
        [record, operation, group, id]
            if is_record(record) && group.eq_ignore_ascii_case("GROUP") =>
        {
            let operation = match *operation {
                "+" => light_application::ProgrammingGroupRecordOperation::Merge,
                "-" => light_application::ProgrammingGroupRecordOperation::Subtract,
                _ => return Ok(None),
            };
            Some(((*id).to_owned(), operation))
        }
        [delete, group, id] if is_delete(delete) && group.eq_ignore_ascii_case("GROUP") => Some((
            (*id).to_owned(),
            light_application::ProgrammingGroupRecordOperation::Delete,
        )),
        _ => None,
    };
    Ok(parsed)
}

fn is_record(token: &str) -> bool {
    token.eq_ignore_ascii_case("RECORD") || token.eq_ignore_ascii_case("REC")
}

fn is_delete(token: &str) -> bool {
    token.eq_ignore_ascii_case("DELETE") || token.eq_ignore_ascii_case("DEL")
}

fn execute_with_policy(
    state: &AppState,
    session: &Session,
    command: &str,
    context: &ActionContext,
    policy: ExistingCommandPolicy,
) -> Result<super::super::ProgrammerCommandExecution, String> {
    let policy = if matches!(policy, ExistingCommandPolicy::AtomicProgrammer)
        && matches!(
            compatibility_only_family(command)?,
            Some("UPDATE" | "ASSIGN" | "Speed Group binding")
        ) {
        // These commands own atomic show-object transactions and must read authoritative live
        // state. A staged Programmer clone would make the show mutation external to the staging
        // commit and could replace newer live Programmer state afterwards.
        ExistingCommandPolicy::Compatibility
    } else {
        policy
    };
    match policy {
        ExistingCommandPolicy::Compatibility => {
            // Cross-user reconciliation must not run while one user's mutation gate is held.
            let outcome = super::super::execute_programmer_command_effect_from(
                state, session, command, context,
            )?;
            if matches!(
                &outcome,
                super::super::ProgrammerCommandExecution::Applied(_)
            ) {
                super::programming_ports::clear_command_line(&state.programming, session)?;
            }
            Ok(outcome)
        }
        ExistingCommandPolicy::AtomicProgrammer => {
            state
                .programming
                .with_staged_command(session.id, |staged_programmers| {
                    execute_staged(state, session, command, context, staged_programmers)
                })
        }
    }
}

fn execute_staged(
    state: &AppState,
    session: &Session,
    command: &str,
    context: &ActionContext,
    staged_programmers: &ProgrammingResource,
) -> Result<super::super::ProgrammerCommandExecution, String> {
    let mut staged_state = state.clone();
    staged_state.programming = staged_programmers.clone();
    staged_state.dynamics =
        light_application::DynamicsService::new(staged_programmers.programmers());
    let outcome = super::super::execute_programmer_command_effect_from(
        &staged_state,
        session,
        command,
        context,
    )?;
    if matches!(
        &outcome,
        super::super::ProgrammerCommandExecution::Applied(_)
    ) {
        staged_programmers
            .update_command_line(session.id, |current| (String::new(), current.target, true))
            .ok_or_else(|| "programmer command line does not exist".to_owned())?;
    }
    Ok(outcome)
}

fn finish_existing_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    request_id: Option<&str>,
    result: Result<super::super::ProgrammerCommandExecution, String>,
) -> ExistingCommandOutcome {
    match result {
        Ok(super::super::ProgrammerCommandExecution::Applied(applied)) => {
            accepted_command(state, session, command, source, request_id, applied)
        }
        Ok(super::super::ProgrammerCommandExecution::ChoiceRequired(choice)) => {
            ExistingCommandOutcome::ChoiceRequired {
                pending_choice: light_application::PendingCommandChoice::DynamicInstance(choice),
            }
        }
        Err(error) => {
            super::super::record_command_history(
                state, session, command, "rejected", &error, source, request_id,
            );
            ExistingCommandOutcome::Rejected { error }
        }
    }
}

fn accepted_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    request_id: Option<&str>,
    applied: usize,
) -> ExistingCommandOutcome {
    let warning = persist_with_warning(state, session, source, request_id, "programmer.execute");
    let feedback = warning.as_ref().map_or_else(
        || format!("Applied to {applied} target(s)"),
        |warning| format!("Applied to {applied} target(s); {warning}"),
    );
    super::super::record_command_history(
        state, session, command, "accepted", &feedback, source, request_id,
    );
    ExistingCommandOutcome::Accepted {
        applied,
        persistence_warning: warning,
        replayed: false,
    }
}

pub(super) fn compatibility_only_family(command: &str) -> Result<Option<&'static str>, String> {
    if super::speed_group_binding_command::parse(command)?.is_some() {
        return Ok(Some("Speed Group binding"));
    }
    if command
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("DYNAMIC"))
    {
        return Ok(Some("DYNAMIC"));
    }
    if preset_record_address(command)?.is_some()
        || group_record_command(command)?.is_some()
        || super::cue_recording_command::parse(command)?.is_some()
        || super::cue_link_command::parse(command)?.is_some()
        || super::cue_deletion_command::is_cue_deletion(command)
        || super::cue_navigation_command::parse(command)?.is_some()
        || super::playback_selection_command::parse(command)?.is_some()
        || super::speed_group_command::parse(command)?.is_some()
    {
        return Ok(None);
    }
    let Some(family) = super::super::normalized_programmer_command_family(command)? else {
        return Ok(None);
    };
    Ok(match family.as_str() {
        "SPD" => Some("SPD GRP"),
        "RECORD" | "REC" => Some("RECORD"),
        "UPDATE" => Some("UPDATE"),
        "DELETE" | "DEL" => Some("DELETE"),
        "MOVE" | "MOV" => Some("MOVE"),
        "COPY" | "CPY" => Some("COPY"),
        "SET" => Some("SET"),
        "ASSIGN" => Some("ASSIGN"),
        "DYNAMIC" => Some("DYNAMIC"),
        _ => None,
    })
}

pub(super) fn run_service(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    command: ProgrammingCommand,
) -> Result<ProgrammingResult, ApiError> {
    run_service_with_source(state, session, context, command, "http").map_err(action_error)
}

pub(crate) fn run_service_with_source(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    command: ProgrammingCommand,
    source: &'static str,
) -> Result<ProgrammingResult, ActionError> {
    let ports = ServerProgrammingPorts::new(state, session, source, true);
    state
        .programming
        .handle(ActionEnvelope { context, command }, &ports)
}

pub(super) fn run_snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
) -> Result<ProgrammingLiveSnapshot, ApiError> {
    let ports = ServerProgrammingPorts::new(state, session, "http", false);
    state
        .programming
        .snapshot(&context, &ports)
        .map_err(action_error)
}

pub(crate) fn route_osc_command_key_outcome(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
    request_id: Option<&str>,
) -> Option<bool> {
    route_osc_command_gesture_outcome(state, session, desk_alias, action, request_id, None)
}

pub(crate) fn route_osc_command_gesture_outcome(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
    request_id: Option<&str>,
    gesture: Option<CommandGesture>,
) -> Option<bool> {
    let Some(key) = osc_command_key(action) else {
        return None;
    };
    let Ok(_activation) = state.active_show.try_acquire() else {
        publish_osc_rejection(
            state,
            session,
            "the active show is changing; retry the Programmer action".into(),
        );
        return Some(false);
    };
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Osc,
    );
    let context = request_id.map_or(context.clone(), |id| context.with_request_id(id));
    let command = ProgrammingCommand::ApplyKey {
        key,
        phase: CommandKeyPhase::Press,
        gesture,
        execute_policy: ExecutionPolicy::Compatibility,
    };
    Some(
        match run_service_with_source(state, session, context, command, "osc") {
            Ok(result) => {
                publish_osc_result(state, session, desk_alias, &result);
                true
            }
            Err(error) => {
                publish_osc_rejection(state, session, error.message);
                false
            }
        },
    )
}

fn publish_osc_rejection(state: &AppState, session: &Session, error: String) {
    super::super::emit(
        state,
        "programmer_command_rejected",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "source":"osc",
            "error":error,
        }),
    );
}

pub(crate) fn osc_command_key(action: &str) -> Option<CommandKey> {
    if let Some(digit) = action.strip_prefix("digit-") {
        return digit
            .parse::<u8>()
            .ok()
            .filter(|digit| *digit <= 9)
            .map(CommandKey::Digit);
    }
    Some(match action {
        "set" => CommandKey::Set,
        "grp" | "group" => CommandKey::Group,
        "cue" => CommandKey::Cue,
        "playback" | "pbk" => CommandKey::Playback,
        "off" => CommandKey::Off,
        "record" => CommandKey::Record,
        "undo" => CommandKey::Undo,
        "clear" => CommandKey::Clear,
        "del" | "delete" => CommandKey::Delete,
        "mov" | "move" => CommandKey::Move,
        "cpy" | "copy" => CommandKey::Copy,
        "thru" => CommandKey::Thru,
        "div" => CommandKey::Divide,
        "backspace" => CommandKey::Backspace,
        "at" => CommandKey::At,
        "enter" => CommandKey::Enter,
        "preload" => CommandKey::Preload,
        "time" => CommandKey::Time,
        "delay" => CommandKey::Delay,
        "link" => CommandKey::Link,
        "select" => CommandKey::Select,
        "highlight" | "high" => CommandKey::Highlight,
        "previous" | "prev" => CommandKey::Previous,
        "next" => CommandKey::Next,
        "all" => CommandKey::All,
        "prog-playback" | "enc" => CommandKey::EncoderPlayback,
        "page-up" | "pgup" => CommandKey::PageUp,
        "page-down" | "pgdn" => CommandKey::PageDown,
        "align" => CommandKey::Align,
        "fade" => CommandKey::Fade,
        "plus" | "add" => CommandKey::Plus,
        "minus" | "subtract" => CommandKey::Minus,
        "dot" => CommandKey::Dot,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_ui_stages_programmer_grammar_without_disabling_compatibility_families() {
        assert!(matches!(
            ordered_ui_command_policy("FIXTURE 1 THRU 3 + 5 AT 50"),
            ExistingCommandPolicy::AtomicProgrammer
        ));
        assert!(matches!(
            ordered_ui_command_policy("FIXTURE 1 DYNAMIC SINE"),
            ExistingCommandPolicy::Compatibility
        ));
        assert!(
            atomic_policy_error(
                "UPDATE ALL GROUP 3",
                ExistingCommandPolicy::AtomicProgrammer
            )
            .is_none()
        );
        assert!(
            atomic_policy_error(
                "ASSIGN GROUP 3 AT PBK 6",
                ExistingCommandPolicy::AtomicProgrammer
            )
            .is_none()
        );
    }
}

fn action_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
        ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}

#[cfg(test)]
mod object_command_tests {
    use super::{ObjectCommand, object_command};

    #[test]
    fn object_commands_require_one_positive_pool_number() {
        assert_eq!(
            object_command("MACRO 17").unwrap(),
            Some(ObjectCommand::RunMacro(17))
        );
        assert_eq!(
            object_command("SET MACRO 17").unwrap(),
            Some(ObjectCommand::EditMacro(17))
        );
        assert_eq!(
            object_command("TIMECODE 9").unwrap(),
            Some(ObjectCommand::RunTimecode(9))
        );
        assert_eq!(
            object_command("TIMECODE 9 +").unwrap(),
            Some(ObjectCommand::ArmTimecode(9))
        );
        assert_eq!(
            object_command("TIMECODE 9 -").unwrap(),
            Some(ObjectCommand::DisarmTimecode(9))
        );
        assert_eq!(
            object_command("SET TIMECODE 9").unwrap(),
            Some(ObjectCommand::EditTimecode(9))
        );
        assert_eq!(
            object_command("TIMECODE 70000").unwrap(),
            Some(ObjectCommand::RunTimecode(70000))
        );
        assert_eq!(object_command("GROUP 17").unwrap(), None);
        assert!(object_command("MACRO").is_err());
        assert!(object_command("MACRO 0").is_err());
        assert!(object_command("MACRO 1 GO").is_err());
        assert!(object_command("MACRO 70000").is_err());
    }
}
