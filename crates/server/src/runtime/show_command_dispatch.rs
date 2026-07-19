use super::*;

struct ParsedShowOperation<'a> {
    operation: &'a str,
    body: &'a [String],
    transfer_mode: Option<CueTransferMode>,
}

fn parse_show_operation(tokens: &[String]) -> ParsedShowOperation<'_> {
    let operation = match tokens[0].as_str() {
        "REC" => "RECORD",
        "DEL" => "DELETE",
        "MOV" => "MOVE",
        "CPY" => "COPY",
        value => value,
    };
    let mut body = &tokens[1..];
    let transfer_mode = match body.first().map(String::as_str) {
        Some("PLAIN") => {
            body = &body[1..];
            Some(CueTransferMode::Plain)
        }
        Some("STATUS") => {
            body = &body[1..];
            Some(CueTransferMode::Status)
        }
        _ => None,
    };
    ParsedShowOperation {
        operation,
        body,
        transfer_mode,
    }
}

pub(super) fn execute_show_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    timing: CommandTiming,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let parsed = parse_show_operation(tokens);
    let snapshot = state.engine.snapshot();
    match parsed.operation {
        "UPDATE" => execute_update_show_command(state, session, parsed.body, &snapshot, context),
        "RECORD" => {
            execute_record_show_command(state, session, parsed.body, timing, &snapshot, context)
        }
        "SET" => execute_set_command(state, session, parsed.body),
        operation => {
            if operation == "DELETE" && parsed.body.first().is_some_and(|token| token == "GROUP") {
                delete_group_command(state, parsed.body, context)
            } else if parsed.body.first().is_some_and(|token| token == "SET") {
                let (entry, store) = active_show_store(state)?;
                execute_cue_mutation(
                    state,
                    operation,
                    parsed.transfer_mode,
                    parsed.body,
                    &entry,
                    &store,
                    &snapshot,
                )
            } else {
                execute_preset_mutation(state, operation, parsed.body, context)
            }
        }
    }
}
