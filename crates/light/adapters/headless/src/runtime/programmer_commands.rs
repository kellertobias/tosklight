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
    if command_http::speed_group_binding_command::parse(command_line)?.is_some() {
        return command_http::prevalidate_external_command(state, session, command_line, context);
    }
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
    if let Some(binding) = command_http::speed_group_binding_command::parse(command_line)? {
        return super::speed_group_bindings::execute_speed_group_binding(state, context, binding)
            .map(ProgrammerCommandExecution::Applied);
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
    if !show_command(first) && tokens.iter().any(|token| token == "DYNAMIC") {
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
        "DMX" => execute_dmx_selection_command(state, session, command_line, &tokens),
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
                Some(light_core::AttributeKey(attribute.to_lowercase().into())),
            )
        }
        _ => (before_fix_at, None),
    };
    let programmer = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let (targets, expression) = command_targets(state, address, &programmer)?;
    let ports = super::dynamics_adapter::ServerDynamicsPorts { state, session };
    if let Some(preset_address) = parse_fix_at_preset(&tokens[fix_at + 1..])? {
        let preset = load_command_preset(state, preset_address)?;
        let values = fix_at_preset_values(state, &targets, &preset, explicit_attribute.as_ref())?;
        if values.is_empty() {
            return Ok(0);
        }
        light_application::DynamicsService::validate_fix_at_contract(
            &values,
            state.output.supported_programming_contract(),
        )
        .map_err(|error| error.message)?;
        let applied = state
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
            .map_err(|error| error.message)?;
        if let Some(expression) = expression {
            state
                .programming
                .select_expression(session.id, targets.clone(), expression);
        }
        return Ok(applied);
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
    if targets.is_empty() {
        return Ok(0);
    }
    if let Some(attribute) = explicit_attribute.as_ref()
        && state.attributes.color_model() == light_core::ColorProgrammingModel::Intent
        && super::attribute_configuration::is_native_color_attribute(&attribute.0)
    {
        return Err(format!(
            "this show programs Color Intent: set the whole colour instead of the fixture-native \
             colour channel `{}`",
            attribute.0
        ));
    }
    let attribute = explicit_attribute
        .or_else(|| active_programmer_attribute(&programmer))
        .ok_or("FixAT requires an active parameter context or explicit ATTRIBUTE <name>")?;
    let applied = state
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
    if let Some(expression) = expression {
        state
            .programming
            .select_expression(session.id, targets.clone(), expression);
    }
    Ok(applied)
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
    let snapshot = state.output.snapshot();
    let fixture_values = release_fixture_values(
        &snapshot,
        &programmer,
        &state.output.resolved_values(),
        &targets,
        family,
    );
    let group_values = release_group_values(&programmer, expression.as_ref(), family);
    if fixture_values.is_empty() && group_values.is_empty() {
        return Ok(0);
    }
    if let Some(expression) = expression.clone() {
        state
            .programming
            .select_expression(session.id, targets.clone(), expression);
    }
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
    programmer: &light_programmer::ProgrammerState,
    current: &light_engine::ResolvedValues,
    targets: &[light_core::FixtureId],
    family: ReleaseFamily,
) -> Vec<light_programmer::ReleaseProgrammerFixtureValue> {
    let target_set = targets.iter().copied().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    let mut push =
        |fixture_id: light_core::FixtureId,
         heads: &mut dyn Iterator<Item = &light_fixture::LogicalHead>| {
            for attribute in heads
                .flat_map(|head| &head.parameters)
                .map(|parameter| &parameter.attribute)
                .filter(|attribute| release_accepts(family, attribute))
            {
                if seen.insert((fixture_id, attribute.clone())) {
                    values.push(light_programmer::ReleaseProgrammerFixtureValue {
                        fixture_id,
                        attribute: attribute.clone(),
                    });
                }
            }
        };
    for fixture in snapshot.fixtures.iter() {
        if target_set.contains(&fixture.fixture_id) {
            push(fixture.fixture_id, &mut fixture.definition.heads.iter());
        }
        // A multi-head fixture is selected head by head: each selected head releases what that
        // head carries, under the head's own identity, as the Programmer holds it.
        for head in fixture
            .logical_heads
            .iter()
            .filter(|head| head.fixture_id != fixture.fixture_id)
            .filter(|head| target_set.contains(&head.fixture_id))
        {
            push(
                head.fixture_id,
                &mut fixture
                    .definition
                    .heads
                    .iter()
                    .filter(|candidate| candidate.index == head.head_index),
            );
        }
    }
    // Whole semantic owners may have no corresponding native parameter. Include active and
    // underlying owners explicitly; unpatched fixtures and logical heads retain these identities.
    let (stored, dynamics) = if programmer.blind && programmer.preload_capture_programmer {
        (
            programmer.preload_pending.as_slice(),
            programmer.preload_dynamic_pending.as_slice(),
        )
    } else {
        (
            programmer.values.as_slice(),
            programmer.dynamic_values.as_slice(),
        )
    };
    let mut semantic = stored
        .iter()
        .map(|value| (value.fixture_id, value.attribute.clone()))
        .chain(
            dynamics
                .iter()
                .map(|value| (value.fixture_id, value.attribute.clone())),
        )
        .chain(
            current
                .iter()
                .filter(|(_, value)| {
                    matches!(
                        value,
                        light_core::AttributeValue::ColorXyz(_)
                            | light_core::AttributeValue::ColorProgram(_)
                            | light_core::AttributeValue::Position(_)
                            | light_core::AttributeValue::Zoom(_)
                    )
                })
                .map(|((fixture, attribute), _)| (*fixture, attribute.clone())),
        )
        .filter(|(fixture, attribute)| {
            target_set.contains(fixture) && release_accepts(family, attribute)
        })
        .collect::<Vec<_>>();
    semantic.sort_by(|(left_id, left_attribute), (right_id, right_attribute)| {
        left_id
            .0
            .cmp(&right_id.0)
            .then_with(|| left_attribute.cmp(right_attribute))
    });
    for (fixture_id, attribute) in semantic {
        if seen.insert((fixture_id, attribute.clone())) {
            values.push(light_programmer::ReleaseProgrammerFixtureValue {
                fixture_id,
                attribute,
            });
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
            (if programmer.blind && programmer.preload_capture_programmer {
                &programmer.preload_group_pending
            } else {
                &programmer.group_values
            })
            .get(&group_id)
            .into_iter()
            .flatten()
            .filter(|(attribute, _)| release_accepts(family, attribute))
            .map(move |(attribute, _)| {
                light_programmer::ReleaseProgrammerGroupValue {
                    group_id: group_id.clone(),
                    attribute: attribute.clone(),
                }
            })
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
    let snapshot = state.output.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(id, point)| {
            (
                *id,
                light_dynamics::Position3d {
                    x: f64::from(point.x),
                    y: f64::from(point.y),
                    z: f64::from(point.z),
                },
            )
        })
        .collect();
    let mutations =
        light_application::materialize_preset_fixture_values(preset, targets, &groups, &positions)
            .map_err(|error| error.message)?;
    let mut values = Vec::new();
    for mutation in mutations {
        let light_programmer::NormalProgrammerValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            ..
        } = mutation
        else {
            continue;
        };
        let typed = matches!(
            value,
            light_core::AttributeValue::Position(_)
                | light_core::AttributeValue::ColorProgram(_)
                | light_core::AttributeValue::Zoom(_)
        );
        let component = explicit_attribute.and_then(|explicit| {
            light_core::programming::programming_component(
                &explicit.0,
                light_core::programming::ProgrammingTargetRole::LightHead,
            )
        });
        if explicit_attribute.is_some_and(|explicit| explicit != &attribute)
            && !(typed && component.is_some_and(|component| component.owner().key() == attribute))
        {
            continue;
        }
        if typed && let Some(component) = component {
            let mask = light_dynamics::ProgrammingFamilyFixAt::from_family(
                component.owner(),
                Some(component),
                value,
            )
            .map_err(|error| error.to_string())?;
            values.push(light_application::DynamicFixAtValue::programming(
                fixture_id, mask,
            ));
        } else {
            if let Some(explicit) = explicit_attribute
                && state.attributes.color_model() == light_core::ColorProgrammingModel::Intent
                && super::attribute_configuration::is_native_color_attribute(&explicit.0)
            {
                return Err(format!(
                    "this show programs Color Intent: set the whole colour instead of the fixture-native colour channel `{}`",
                    explicit.0
                ));
            }
            values.push(light_application::DynamicFixAtValue {
                fixture_id,
                attribute,
                value,
                programming_mask: None,
            });
        }
    }
    Ok(values)
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

#[cfg(test)]
mod semantic_release_tests {
    use super::*;
    use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, programming::*};
    use std::sync::Arc;

    #[test]
    fn release_includes_complete_owners_without_native_parameters() {
        let registry = light_programmer::ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        let logical_head = FixtureId::new();
        registry.start(session);
        registry.apply_normal_values(
            session,
            &[
                light_programmer::NormalProgrammerValueMutation::SetFixture {
                    fixture_id: fixture,
                    attribute: AttributeKey::color(),
                    value: AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                        intent: ColorIntent {
                            uv: UvIntent { amount: 0.8 },
                            ..Default::default()
                        },
                    })),
                    timing: Default::default(),
                },
                light_programmer::NormalProgrammerValueMutation::SetFixture {
                    fixture_id: logical_head,
                    attribute: ProgrammingOwner::Position.key(),
                    value: AttributeValue::Position(Arc::new(PositionIntent::angles(540.0, 20.0))),
                    timing: Default::default(),
                },
            ],
        );
        let programmer = registry.get(session).unwrap();
        let snapshot = light_engine::EngineSnapshot::default();
        let current = light_engine::ResolvedValues::default();
        let color = release_fixture_values(
            &snapshot,
            &programmer,
            &current,
            &[fixture, logical_head],
            ReleaseFamily::Class(light_core::AttributeClass::Color),
        );
        assert_eq!(color.len(), 1);
        assert_eq!(color[0].fixture_id, fixture);
        assert_eq!(color[0].attribute, AttributeKey::color());
        let position = release_fixture_values(
            &snapshot,
            &programmer,
            &current,
            &[fixture, logical_head],
            ReleaseFamily::Class(light_core::AttributeClass::Position),
        );
        assert_eq!(position.len(), 1);
        assert_eq!(position[0].fixture_id, logical_head);
        assert_eq!(position[0].attribute, ProgrammingOwner::Position.key());
        assert!(
            release_fixture_values(&snapshot, &programmer, &current, &[], ReleaseFamily::All)
                .is_empty()
        );
    }

    #[test]
    fn release_records_an_underlying_semantic_owner_even_without_programmer_capture() {
        let registry = light_programmer::ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session);
        let current = light_engine::ResolvedValues::from_iter([(
            (fixture, ProgrammingOwner::Position.key()),
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [1.0, 2.0, 3.0],
            ))),
        )]);
        let result = release_fixture_values(
            &light_engine::EngineSnapshot::default(),
            &registry.get(session).unwrap(),
            &current,
            &[fixture],
            ReleaseFamily::Class(light_core::AttributeClass::Position),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].attribute, ProgrammingOwner::Position.key());
    }
}
