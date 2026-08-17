use super::*;

fn show_command(token: &str) -> bool {
    matches!(
        token,
        "RECORD"
            | "REC"
            | "UPDATE"
            | "DELETE"
            | "DEL"
            | "MOVE"
            | "MOV"
            | "COPY"
            | "CPY"
            | "SET"
            | "ASSIGN"
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
#[cfg(test)]
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

/// Detached Macro preflight including the Macro-only execution-local selection restoration.
/// The concrete initiating selection is injected into the detached programmer only; no live desk
/// state, Group, or show object is mutated during validation.
pub(super) fn prevalidate_macro_commands_from(
    state: &AppState,
    session: &Session,
    command_lines: &[&str],
    initial_selection: &[light_core::FixtureId],
    context: &light_application::ActionContext,
) -> Result<(), (usize, String)> {
    state
        .programming
        .with_detached_command(session.id, |detached_programming| {
            let mut detached_state = state.clone();
            detached_state.programming = detached_programming.clone();
            for (index, command_line) in command_lines.iter().enumerate() {
                if command_line
                    .trim()
                    .eq_ignore_ascii_case(light_application::RESTORE_SELECTION_COMMAND)
                {
                    detached_state
                        .programming
                        .programmers()
                        .select(session.id, initial_selection.iter().copied());
                    continue;
                }
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
        .any(|token| matches!(token.as_str(), "FIXAT" | "DYNAMIC" | "RELEASE"))
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
        if tokens.iter().any(|token| token == "RELEASE") {
            let release = tokens.iter().position(|token| token == "RELEASE").unwrap();
            match &tokens[release + 1..] {
                [] => {}
                [family] => {
                    release_family(family)?;
                }
                _ => return Err("RELEASE accepts at most one attribute family".into()),
            }
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
    let raw_tokens = command_line.split_whitespace().collect::<Vec<_>>();
    if raw_tokens
        .first()
        .is_some_and(|token| token.eq_ignore_ascii_case("PLAYBACK"))
    {
        return Err("PLAYBACK is not a command root; use PBK".into());
    }
    let manual_playback_command = raw_tokens.first().is_some_and(|root| {
        root.eq_ignore_ascii_case("GO")
            || root.eq_ignore_ascii_case("LOAD")
            || root.eq_ignore_ascii_case("PBK")
            || root.eq_ignore_ascii_case("VPBK")
            || root.eq_ignore_ascii_case("CUELIST")
            || (matches!(root.to_ascii_uppercase().as_str(), "RECORD" | "REC")
                && raw_tokens.iter().skip(1).any(|token| {
                    matches!(
                        token.to_ascii_uppercase().as_str(),
                        "CUE" | "CUELIST" | "PBK" | "VPBK"
                    )
                }))
    });
    if manual_playback_command {
        if let Some(execution) = command_http::execute_manual_command_without_line_cleanup(
            state,
            session,
            context,
            command_line,
        ) {
            return match execution {
                light_application::ProgrammingExecution::Accepted { applied, .. } => {
                    Ok(ProgrammerCommandExecution::Applied(applied))
                }
                light_application::ProgrammingExecution::Rejected { error } => Err(error),
                light_application::ProgrammingExecution::ChoiceRequired { pending_choice } => {
                    match pending_choice {
                        light_application::PendingCommandChoice::DynamicInstance(choice) => {
                            Ok(ProgrammerCommandExecution::ChoiceRequired(choice))
                        }
                        light_application::PendingCommandChoice::CueMoveCopy(_) => {
                            Err("the command requires an explicit Move/Copy choice".into())
                        }
                    }
                }
            };
        }
    }
    let (tokens, timing) = tokenize_programmer_command(command_line)?;
    let first = tokens.first().ok_or("the command line is empty")?;
    if first == "CUE" {
        return Err(
            "CUE selects a Cue; use GO TO PBK <playback> CUE <cue> or LOAD PBK <playback> CUE <cue> to navigate"
                .into(),
        );
    }
    if matches!(first.as_str(), "RECORD" | "REC")
        && tokens.iter().skip(1).any(|token| token == "SET")
    {
        return Err(
            "SET is not a recording address; use RECORD CUELIST, RECORD PBK, or RECORD VPBK".into(),
        );
    }
    if tokens.iter().any(|token| token == "RELEASE") {
        return execute_release_command(state, session, &tokens, context)
            .map(ProgrammerCommandExecution::Applied);
    }
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
        "GROUP" | "DEGROUP" | "DEGRP" => execute_group_programmer_command(
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
    let (targets, expression) = command_targets(state, address, &programmer)?;
    if targets.is_empty() {
        return Err("FixAT requires a current selection".into());
    }
    if let Some(expression) = expression {
        state
            .programming
            .select_expression(session.id, targets.clone(), expression);
    }
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    if let Some(preset_address) = parse_fix_at_preset(&tokens[fix_at + 1..])? {
        let preset = load_command_preset(state, preset_address)?;
        let values = fix_at_preset_values(state, &targets, &preset, explicit_attribute.as_ref())?;
        return state
            .dynamics
            .fix_at_batch(
                context,
                light_application::DynamicFixAtBatchCommand {
                    values,
                    timing: light_dynamics::DynamicValueTiming {
                        fade_millis: timing.fade_millis,
                        delay_millis: timing.delay_millis,
                    },
                },
                &ports,
            )
            .map_err(|error| error.message);
    }
    if tokens[fix_at + 1..].len() != 1 {
        return Err("FixAT requires one scalar value or Preset".into());
    }
    let value = if tokens[fix_at + 1] == "FULL" {
        100.0
    } else {
        tokens[fix_at + 1]
            .parse::<f32>()
            .map_err(|_| "FixAT value must be a percentage, FULL, or Preset")?
    };
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err("FixAT value must be within 0-100".into());
    }
    let attribute = explicit_attribute
        .or_else(|| active_programmer_attribute(&programmer))
        .ok_or("FixAT requires an active parameter context or explicit ATTRIBUTE <name>")?;
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

fn execute_release_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let release = tokens
        .iter()
        .position(|token| token == "RELEASE")
        .ok_or("RELEASE is missing")?;
    let family = match &tokens[release + 1..] {
        [] => ReleaseFamily::Class(light_core::AttributeClass::Intensity),
        [family] => release_family(family)?,
        _ => return Err("RELEASE accepts at most one attribute family".into()),
    };
    let mut address = &tokens[..release];
    if address.last().is_some_and(|token| token == "AT") {
        address = &address[..address.len() - 1];
    }
    let programmer = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let (targets, expression) = command_targets(state, address, &programmer)?;
    if targets.is_empty() {
        return Err("RELEASE requires a current selection".into());
    }
    if let Some(expression) = expression.clone() {
        state
            .programming
            .select_expression(session.id, targets.clone(), expression);
    }
    let snapshot = state.output.snapshot();
    let fixture_values = release_fixture_values(&snapshot, &targets, family);
    let group_values = release_group_values(&programmer, expression.as_ref(), family);
    state
        .dynamics
        .release_values(
            context,
            light_application::DynamicReleaseCommand {
                fixture_values,
                group_values,
            },
            &super::dynamics_adapter::ServerDynamicsPorts { state, session },
        )
        .map_err(|error| error.message)
}

fn command_targets(
    state: &AppState,
    address: &[String],
    programmer: &light_programmer::ProgrammerState,
) -> Result<
    (
        Vec<light_core::FixtureId>,
        Option<light_programmer::SelectionExpression>,
    ),
    String,
> {
    if address.is_empty() {
        return Ok((
            programmer.selected.clone(),
            programmer.selection_expression.clone(),
        ));
    }
    let snapshot = state.output.snapshot();
    if address
        .iter()
        .any(|token| matches!(token.as_str(), "GROUP" | "DEGROUP" | "DEGRP"))
    {
        let parsed = parse_group_mixed_selection(&snapshot, address, true)?;
        return Ok((
            parsed.fixtures,
            Some(light_programmer::SelectionExpression::Sources {
                items: parsed.sources,
            }),
        ));
    }
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
    Ok((fixtures, Some(expression)))
}

#[derive(Clone, Copy)]
enum ReleaseFamily {
    All,
    Class(light_core::AttributeClass),
}

fn release_family(token: &str) -> Result<ReleaseFamily, String> {
    let class = match token {
        "ALL" => return Ok(ReleaseFamily::All),
        "INTENSITY" => light_core::AttributeClass::Intensity,
        "COLOR" => light_core::AttributeClass::Color,
        "POSITION" => light_core::AttributeClass::Position,
        "BEAM" => light_core::AttributeClass::Beam,
        "SHAPERS" => light_core::AttributeClass::Shapers,
        "FOCUS" => light_core::AttributeClass::Focus,
        "CONTROL" => light_core::AttributeClass::Control,
        "MEDIA" => light_core::AttributeClass::Media,
        _ => return Err(format!("unknown RELEASE attribute family {token}")),
    };
    Ok(ReleaseFamily::Class(class))
}

fn release_accepts(family: ReleaseFamily, attribute: &light_core::AttributeKey) -> bool {
    let descriptor = light_core::attribute_descriptor(attribute);
    descriptor.recordable
        && match family {
            ReleaseFamily::All => true,
            ReleaseFamily::Class(class) => descriptor.family == class,
        }
}

fn release_fixture_values(
    snapshot: &light_engine::EngineSnapshot,
    targets: &[light_core::FixtureId],
    family: ReleaseFamily,
) -> Vec<light_programmer::ReleaseProgrammerFixtureValue> {
    let target_set = targets.iter().copied().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for fixture in snapshot
        .fixtures
        .iter()
        .filter(|fixture| target_set.contains(&fixture.fixture_id))
    {
        for attribute in fixture
            .definition
            .heads
            .iter()
            .flat_map(|head| &head.parameters)
            .map(|parameter| &parameter.attribute)
            .filter(|attribute| release_accepts(family, attribute))
        {
            if seen.insert((fixture.fixture_id, attribute.clone())) {
                values.push(light_programmer::ReleaseProgrammerFixtureValue {
                    fixture_id: fixture.fixture_id,
                    attribute: attribute.clone(),
                });
            }
        }
    }
    values
}

fn release_group_values(
    programmer: &light_programmer::ProgrammerState,
    expression: Option<&light_programmer::SelectionExpression>,
    family: ReleaseFamily,
) -> Vec<light_programmer::ReleaseProgrammerGroupValue> {
    let group_ids = match expression {
        Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) => {
            vec![group_id.clone()]
        }
        Some(light_programmer::SelectionExpression::Sources { items }) => items
            .iter()
            .filter_map(|item| match item {
                light_programmer::SelectionReference::LiveGroup { group_id } => {
                    Some(group_id.clone())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    group_ids
        .into_iter()
        .flat_map(|group_id| {
            programmer
                .group_values
                .get(&group_id)
                .into_iter()
                .flatten()
                .filter(|(attribute, _)| release_accepts(family, attribute))
                .map(
                    move |(attribute, _)| light_programmer::ReleaseProgrammerGroupValue {
                        group_id: group_id.clone(),
                        attribute: attribute.clone(),
                    },
                )
        })
        .collect()
}

fn parse_fix_at_preset(
    tokens: &[String],
) -> Result<Option<light_programmer::PresetAddress>, String> {
    if tokens.len() == 1 {
        return Ok(None);
    }
    let address = match tokens {
        [family, keyword, number] if keyword == "PRESET" => {
            let family = match family.as_str() {
                "ALL" => light_programmer::PresetFamily::Mixed,
                "INTENSITY" => light_programmer::PresetFamily::Intensity,
                "COLOR" => light_programmer::PresetFamily::Color,
                "POSITION" => light_programmer::PresetFamily::Position,
                "BEAM" => light_programmer::PresetFamily::Beam,
                _ => return Err(format!("unknown FixAT Preset family {family}")),
            };
            light_programmer::PresetAddress::new(
                family,
                number
                    .parse::<u32>()
                    .map_err(|_| "FixAT Preset number is invalid")?,
            )?
        }
        [_, separator, _] if separator == "." => command_preset_address(tokens)?,
        _ => return Err("FixAT Preset must be <family> PRESET <number>".into()),
    };
    Ok(Some(address))
}

fn fix_at_preset_values(
    state: &AppState,
    targets: &[light_core::FixtureId],
    preset: &light_programmer::Preset,
    explicit_attribute: Option<&light_core::AttributeKey>,
) -> Result<Vec<light_application::DynamicFixAtValue>, String> {
    let groups = state
        .output
        .snapshot()
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut planned = Vec::new();
    for fixture_id in targets {
        let direct = preset.values.get(fixture_id).into_iter().flatten();
        let expanded = preset
            .group_values
            .iter()
            .flat_map(|(group_id, attributes)| {
                light_programmer::resolve_group(group_id, &groups)
                    .is_ok_and(|members| members.contains(fixture_id))
                    .then_some(attributes)
                    .into_iter()
                    .flatten()
            });
        for (attribute, value) in direct.chain(expanded) {
            if explicit_attribute.is_some_and(|explicit| explicit != attribute) {
                continue;
            }
            if let Some(index) =
                planned
                    .iter()
                    .position(|existing: &light_application::DynamicFixAtValue| {
                        existing.fixture_id == *fixture_id && existing.attribute == *attribute
                    })
            {
                planned[index].value = value.clone();
            } else {
                planned.push(light_application::DynamicFixAtValue {
                    fixture_id: *fixture_id,
                    attribute: attribute.clone(),
                    value: value.clone(),
                });
            }
        }
    }
    if planned.is_empty() {
        return Err("FixAT Preset contains no applicable values for the selection".into());
    }
    Ok(planned)
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
