use super::*;

struct ParsedShowOperation<'a> {
    operation: &'a str,
    body: &'a [String],
}

fn parse_show_operation(tokens: &[String]) -> ParsedShowOperation<'_> {
    let operation = match tokens[0].as_str() {
        "REC" => "RECORD",
        "DEL" => "DELETE",
        "MOV" => "MOVE",
        "CPY" => "COPY",
        value => value,
    };
    ParsedShowOperation {
        operation,
        body: &tokens[1..],
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
        "SET" => execute_set_command(state, session, parsed.body, context),
        operation => {
            if operation == "DELETE" && parsed.body.first().is_some_and(|token| token == "GROUP") {
                delete_group_command(state, parsed.body, context)
            } else if operation == "DELETE"
                && parsed.body.first().is_some_and(|token| token == "SET")
            {
                Err("Cue DELETE must use the typed Programming deletion action".into())
            } else if matches!(operation, "MOVE" | "COPY")
                && parsed.body.first().is_some_and(|token| token == "SET")
            {
                Err("Cue MOVE/COPY must use the typed Programming transfer action".into())
            } else {
                execute_preset_mutation(state, operation, parsed.body, context)
            }
        }
    }
}
