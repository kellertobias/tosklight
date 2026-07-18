use super::*;

fn show_command(token: &str) -> bool {
    matches!(
        token,
        "RECORD" | "REC" | "UPDATE" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY" | "SET"
    )
}

pub(super) fn execute_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
) -> Result<usize, String> {
    let (tokens, timing) = tokenize_programmer_command(command_line)?;
    let first = tokens.first().ok_or("the command line is empty")?;
    match first.as_str() {
        "CUE" => execute_cue_operation(state, session, &tokens),
        "AT" => apply_current_selection_value(
            state,
            session,
            &tokens[1..],
            programmer_value_timing(state, timing),
        ),
        "SPD" => execute_speed_group_command(state, &tokens),
        command if show_command(command) => execute_show_command(state, session, &tokens, timing),
        "GROUP" => execute_group_programmer_command(
            state,
            session,
            command_line,
            &tokens,
            programmer_value_timing(state, timing),
        ),
        _ => execute_fixture_programmer_command(
            state,
            session,
            command_line,
            &tokens,
            programmer_value_timing(state, timing),
        ),
    }
}
