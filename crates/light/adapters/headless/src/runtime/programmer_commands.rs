use super::*;

fn show_command(token: &str) -> bool {
    matches!(
        token,
        "RECORD" | "REC" | "UPDATE" | "DELETE" | "DEL" | "MOVE" | "MOV" | "COPY" | "CPY" | "SET"
    )
}

#[cfg(test)]
pub(super) fn execute_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
) -> Result<usize, String> {
    let context = operator_action_context(session, light_application::ActionSource::Http);
    execute_programmer_command_from(state, session, command_line, &context)
}

pub(super) enum ProgrammerCommandExecution {
    Applied(usize),
    ChoiceRequired(light_application::DynamicInstanceChoice),
}

struct CommandPrevalidationFailure {
    index: usize,
    message: String,
}

impl From<String> for CommandPrevalidationFailure {
    fn from(message: String) -> Self {
        Self { index: 0, message }
    }
}

/// Validates one entered command with the same tokenizer and dispatch table used by execution.
/// Programmer-only families run against a detached operator state, so fixture/group resolution,
/// levels, spreads, presets, and current-selection requirements are checked without publishing or
/// committing a mutation. Families that own external runtime/show mutations expose their parser
/// through `command_http` and are checked there instead of being executed speculatively.
pub(super) fn prevalidate_programmer_commands_from(
    state: &AppState,
    session: &Session,
    command_lines: &[&str],
    context: &light_application::ActionContext,
) -> Result<(), (usize, String)> {
    state
        .programming
        .with_detached_command(session.id, |detached_programming| {
            let mut detached_state = state.clone();
            detached_state.programming = detached_programming.clone();
            for (index, command_line) in command_lines.iter().enumerate() {
                prevalidate_programmer_command_in_state(
                    &detached_state,
                    session,
                    command_line,
                    context,
                )
                .map_err(|message| CommandPrevalidationFailure { index, message })?;
            }
            Ok(())
        })
        .map_err(|error: CommandPrevalidationFailure| (error.index, error.message))
}

fn prevalidate_programmer_command_in_state(
    state: &AppState,
    session: &Session,
    command_line: &str,
    context: &light_application::ActionContext,
) -> Result<(), String> {
    if command_http::prevalidate_typed_command(state, session, command_line, context)? {
        return Ok(());
    }
    let (tokens, _timing) = tokenize_programmer_command(command_line)?;
    let first = tokens.first().ok_or("the command line is empty")?;
    if tokens
        .iter()
        .any(|token| matches!(token.as_str(), "FIXAT" | "DYNAMIC"))
    {
        // These families own typed Dynamics mutations outside Programmer. Their authoritative
        // grammar rejects incomplete commands before execution; retain the current-context checks
        // that can be performed without invoking that mutation authority.
        if tokens.last().is_some_and(|token| {
            matches!(
                token.as_str(),
                "FIXAT" | "DYNAMIC" | "AT" | "ATTRIBUTE" | "PARAMETER"
            )
        }) {
            return Err(format!(
                "{} requires a value or target",
                tokens.last().unwrap()
            ));
        }
        if tokens.iter().any(|token| token == "FIXAT")
            && state.programming.get(session.id).is_none()
        {
            return Err("programmer does not exist".into());
        }
        return Ok(());
    }
    if matches!(first.as_str(), "CUE" | "SPD") || show_command(first) {
        return command_http::prevalidate_external_command(state, session, command_line, context);
    }
    execute_programmer_command_effect_from(state, session, command_line, context).map(|_| ())
}

#[cfg(test)]
pub(super) fn execute_programmer_command_from(
    state: &AppState,
    session: &Session,
    command_line: &str,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    match execute_programmer_command_effect_from(state, session, command_line, context)? {
        ProgrammerCommandExecution::Applied(applied) => Ok(applied),
        ProgrammerCommandExecution::ChoiceRequired(_) => {
            Err("the Dynamic command requires an exact running-instance choice".into())
        }
    }
}

pub(super) fn execute_programmer_command_effect_from(
    state: &AppState,
    session: &Session,
    command_line: &str,
    context: &light_application::ActionContext,
) -> Result<ProgrammerCommandExecution, String> {
    let (tokens, timing) = tokenize_programmer_command(command_line)?;
    let first = tokens.first().ok_or("the command line is empty")?;
    if tokens.iter().any(|token| token == "FIXAT") {
        return execute_fix_at_command(
            state,
            session,
            &tokens,
            programmer_value_timing(state, timing),
            context,
        )
        .map(ProgrammerCommandExecution::Applied);
    }
    if tokens.iter().any(|token| token == "DYNAMIC") {
        return super::programmer_dynamic_commands::execute_dynamic_command(
            state,
            session,
            &tokens,
            programmer_value_timing(state, timing),
            context,
        );
    }
    let applied = match first.as_str() {
        "CUE" => execute_cue_operation(state, session, context, command_line),
        "AT" => apply_current_selection_value(
            state,
            session,
            &tokens[1..],
            programmer_value_timing(state, timing),
        ),
        "SPD" => execute_speed_group_operation(state, session, context, command_line),
        command if show_command(command) => {
            execute_show_command(state, session, &tokens, timing, context)
        }
        "GROUP" | "DEGRP" => execute_group_programmer_command(
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
    }?;
    Ok(ProgrammerCommandExecution::Applied(applied))
}

fn execute_fix_at_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    timing: CommandTiming,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let fix_at = tokens
        .iter()
        .position(|token| token == "FIXAT")
        .ok_or("FixAT is missing")?;
    if tokens[fix_at + 1..].len() != 1 {
        return Err("FixAT requires exactly one scalar value".into());
    }
    let value = if tokens[fix_at + 1] == "FULL" {
        100.0
    } else {
        tokens[fix_at + 1]
            .parse::<f32>()
            .map_err(|_| "FixAT value must be a percentage or FULL")?
    };
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err("FixAT value must be within 0-100".into());
    }
    let before_fix_at = &tokens[..fix_at];
    let (address, explicit_attribute) = match before_fix_at {
        [address @ .., keyword, attribute]
            if matches!(keyword.as_str(), "ATTRIBUTE" | "PARAMETER") =>
        {
            (
                address,
                Some(light_core::AttributeKey(attribute.to_lowercase())),
            )
        }
        _ => (before_fix_at, None),
    };
    let programmer = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let attribute = explicit_attribute
        .or_else(|| active_programmer_attribute(&programmer))
        .ok_or("FixAT requires an active parameter context or explicit ATTRIBUTE <name>")?;
    let (targets, expression) = if address.is_empty() {
        (programmer.selected, programmer.selection_expression)
    } else {
        let snapshot = state.output.snapshot();
        if address
            .iter()
            .any(|token| matches!(token.as_str(), "GROUP" | "DEGRP"))
        {
            let parsed = parse_group_mixed_selection(&snapshot, address, true)?;
            (
                parsed.fixtures,
                Some(light_programmer::SelectionExpression::Sources {
                    items: parsed.sources,
                }),
            )
        } else {
            let start = usize::from(matches!(
                address.first().map(String::as_str),
                Some("FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS")
            ));
            let fixtures = parse_fixture_selection(&snapshot.fixtures, &address[start..])?;
            let expression = light_programmer::SelectionExpression::Sources {
                items: fixtures
                    .iter()
                    .map(|fixture_id| light_programmer::SelectionReference::Fixture {
                        fixture_id: *fixture_id,
                    })
                    .collect(),
            };
            (fixtures, Some(expression))
        }
    };
    if targets.is_empty() {
        return Err("FixAT requires a current selection".into());
    }
    if let Some(expression) = expression {
        state
            .programming
            .select_expression(session.id, targets.clone(), expression);
    }
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    state
        .dynamics
        .fix_at(
            context,
            light_application::DynamicFixAtCommand {
                targets: targets.clone(),
                attribute,
                value: value / 100.0,
                timing: light_dynamics::DynamicValueTiming {
                    fade_millis: timing.fade_millis,
                    delay_millis: timing.delay_millis,
                },
            },
            &ports,
        )
        .map_err(|error| error.message)?;
    Ok(targets.len())
}

fn active_programmer_attribute(
    programmer: &light_programmer::ProgrammerState,
) -> Option<light_core::AttributeKey> {
    let fixture_values = programmer
        .values
        .iter()
        .map(|value| (value.programmer_order, value.attribute.clone()));
    let group_values = programmer.group_values.iter().flat_map(|(_, attributes)| {
        attributes
            .iter()
            .map(|(attribute, value)| (value.programmer_order, attribute.clone()))
    });
    let dynamic_values = programmer
        .dynamic_values
        .iter()
        .map(|value| (value.programmer_order, value.attribute.clone()));
    fixture_values
        .chain(group_values)
        .chain(dynamic_values)
        .max_by_key(|(order, _)| *order)
        .map(|(_, attribute)| attribute)
}
