use super::*;

pub(super) fn execute_dynamic_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    timing: CommandTiming,
    context: &light_application::ActionContext,
) -> Result<super::ProgrammerCommandExecution, String> {
    let dynamic_index = tokens
        .iter()
        .position(|token| token == "DYNAMIC")
        .ok_or("DYNAMIC is missing")?;
    let number = tokens
        .get(dynamic_index + 1)
        .ok_or("DYNAMIC requires a pool number")?
        .parse::<u16>()
        .map_err(|_| "Dynamic pool number must be an integer")?;
    let snapshot = state.output.snapshot();
    let definition = snapshot
        .dynamics
        .iter()
        .find(|definition| definition.pool_number == number)
        .ok_or_else(|| format!("Dynamic {number} does not exist"))?;
    let targets = command_dynamic_targets(state, session, &snapshot, &tokens[..dynamic_index])?;
    let (explicit_controller, tail) = parse_dynamic_command_tail(&tokens[dynamic_index + 2..])?;
    let ports = dynamics_adapter::ServerDynamicsPorts { state, session };
    let command = light_application::DynamicStartCommand {
        dynamic_id: definition.id,
        targets: targets.clone(),
        overrides: light_dynamics::DynamicInstanceOverrides {
            size: 1.0,
            speed_multiplier: light_dynamics::Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: light_dynamics::DynamicValueTiming {
            fade_millis: timing.fade_millis,
            delay_millis: timing.delay_millis,
        },
        undo_group: None,
    };
    match tail {
        [] => {
            if explicit_controller.is_some() {
                return Err("Dynamic INSTANCE requires OFF, SIZE, SPEED, or PHASE".into());
            }
            if command_controller_candidates(state, session, definition.id, &targets)?.is_empty() {
                state
                    .dynamics
                    .start(context, command, &ports)
                    .map_err(|error| error.message)?;
            }
        }
        [off] if off == "OFF" => {
            let candidates =
                command_controller_candidates(state, session, definition.id, &targets)?;
            match resolve_command_controller(
                state,
                definition,
                explicit_controller,
                tokens,
                dynamic_index,
                timing,
                candidates,
            )? {
                ControllerResolution::Exact(controller_id) => {
                    state
                        .dynamics
                        .off(
                            context,
                            light_application::DynamicOffCommand {
                                controller_id,
                                timing: command.timing,
                            },
                            &ports,
                        )
                        .map_err(|error| error.message)?;
                }
                ControllerResolution::Choice(choice) => {
                    return Ok(super::ProgrammerCommandExecution::ChoiceRequired(choice));
                }
                ControllerResolution::Absent => {
                    state
                        .dynamics
                        .off_matching(context, command, &ports)
                        .map_err(|error| error.message)?;
                }
            }
        }
        [field, at, values @ ..] if at == "AT" => {
            if matches!(field.as_str(), "BLOCKS" | "REPEATS" | "WINGS")
                || (field == "PHASE" && values.iter().any(|token| token == "THRU"))
            {
                update_dynamic_phase_definition(state, context, definition.id, field, values)?;
                persist_output_runtime(state).map_err(|error| error.message)?;
                return Ok(super::ProgrammerCommandExecution::Applied(targets.len()));
            }
            let candidates =
                command_controller_candidates(state, session, definition.id, &targets)?;
            let controller_id = match resolve_command_controller(
                state,
                definition,
                explicit_controller,
                tokens,
                dynamic_index,
                timing,
                candidates,
            )? {
                ControllerResolution::Exact(controller_id) => controller_id,
                ControllerResolution::Choice(choice) => {
                    return Ok(super::ProgrammerCommandExecution::ChoiceRequired(choice));
                }
                ControllerResolution::Absent => {
                    return Err("no matching Dynamic instance is active for this selection".into());
                }
            };
            let value = parse_controller_value(field, values)?;
            let (size, speed_multiplier, phase_offset_degrees) = match field.as_str() {
                "SIZE" => (Some(value / 100.0), None, None),
                "SPEED" => (None, Some(value), None),
                "PHASE" => (None, None, Some(value)),
                _ => {
                    return Err("Dynamic instance parameter must be SIZE, SPEED, or PHASE".into());
                }
            };
            state
                .dynamics
                .update_controller(
                    context,
                    light_application::DynamicControllerUpdate {
                        controller_id,
                        size,
                        speed_multiplier,
                        phase_offset_degrees,
                        undo_group: context.request_id.clone(),
                    },
                    &ports,
                )
                .map_err(|error| error.message)?;
        }
        _ => {
            return Err(
                "expected DYNAMIC <number>, OFF, SIZE/SPEED/PHASE AT, or PHASE helpers".into(),
            );
        }
    }
    persist_output_runtime(state).map_err(|error| error.message)?;
    Ok(super::ProgrammerCommandExecution::Applied(targets.len()))
}

fn parse_dynamic_command_tail(tokens: &[String]) -> Result<(Option<Uuid>, &[String]), String> {
    match tokens {
        [instance, controller, rest @ ..] if instance == "INSTANCE" => Ok((
            Some(
                controller
                    .parse::<Uuid>()
                    .map_err(|_| "Dynamic INSTANCE requires a controller UUID")?,
            ),
            rest,
        )),
        _ => Ok((None, tokens)),
    }
}

fn command_dynamic_targets(
    state: &AppState,
    session: &Session,
    snapshot: &EngineSnapshot,
    address: &[String],
) -> Result<Vec<light_core::FixtureId>, String> {
    if address.is_empty() {
        return Ok(state
            .programming
            .get(session.id)
            .ok_or("programmer does not exist")?
            .selected);
    }
    let (targets, expression) = if address
        .iter()
        .any(|token| matches!(token.as_str(), "GROUP" | "DEGRP"))
    {
        let parsed = parse_group_mixed_selection(snapshot, address, true)?;
        (
            parsed.fixtures,
            light_programmer::SelectionExpression::Sources {
                items: parsed.sources,
            },
        )
    } else {
        let start = usize::from(matches!(
            address.first().map(String::as_str),
            Some("FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS")
        ));
        let targets = parse_fixture_selection(&snapshot.fixtures, &address[start..])?;
        let expression = light_programmer::SelectionExpression::Sources {
            items: targets
                .iter()
                .map(|fixture_id| light_programmer::SelectionReference::Fixture {
                    fixture_id: *fixture_id,
                })
                .collect(),
        };
        (targets, expression)
    };
    state
        .programming
        .select_expression(session.id, targets.clone(), expression);
    Ok(targets)
}

fn command_controller_candidates(
    state: &AppState,
    session: &Session,
    dynamic_id: Uuid,
    targets: &[light_core::FixtureId],
) -> Result<Vec<(Uuid, usize)>, String> {
    let programmer = state
        .programming
        .get(session.id)
        .ok_or("programmer does not exist")?;
    let expected = targets.iter().copied().collect::<HashSet<_>>();
    let mut by_controller = HashMap::<Uuid, HashSet<light_core::FixtureId>>::new();
    for stored in programmer
        .dynamic_values
        .iter()
        .chain(programmer.preload_dynamic_pending.iter())
    {
        if let light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            ..
        } = &stored.value
            && dynamic.dynamic_id == Some(dynamic_id)
        {
            by_controller
                .entry(*instance_link)
                .or_default()
                .insert(stored.fixture_id);
        }
    }
    let mut matches = by_controller
        .into_iter()
        .filter_map(|(controller, found)| {
            (expected.is_empty() || found == expected).then_some((controller, found.len()))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(controller, _)| *controller);
    Ok(matches)
}

enum ControllerResolution {
    Exact(Uuid),
    Choice(light_application::DynamicInstanceChoice),
    Absent,
}

fn resolve_command_controller(
    state: &AppState,
    definition: &light_dynamics::DynamicDefinition,
    explicit_controller: Option<Uuid>,
    tokens: &[String],
    dynamic_index: usize,
    timing: CommandTiming,
    candidates: Vec<(Uuid, usize)>,
) -> Result<ControllerResolution, String> {
    if let Some(controller_id) = explicit_controller {
        return candidates
            .iter()
            .any(|(candidate, _)| *candidate == controller_id)
            .then_some(ControllerResolution::Exact(controller_id))
            .ok_or_else(|| {
                "the selected Dynamic instance is no longer active for this scope".to_owned()
            });
    }
    match candidates.as_slice() {
        [(controller, _)] => Ok(ControllerResolution::Exact(*controller)),
        [] => Ok(ControllerResolution::Absent),
        _ => {
            let show = state.active_show.current().clone();
            let prefix = &tokens[..dynamic_index + 2];
            let tail = &tokens[dynamic_index + 2..];
            let options = candidates
                .into_iter()
                .enumerate()
                .map(|(index, (controller_id, target_count))| {
                    let mut command = prefix.to_vec();
                    command.extend(["INSTANCE".to_owned(), controller_id.simple().to_string()]);
                    command.extend_from_slice(tail);
                    append_command_timing(&mut command, timing);
                    light_application::DynamicInstanceChoiceOption {
                        controller_id,
                        label: format!(
                            "Instance {} · {} target{} · {}",
                            index + 1,
                            target_count,
                            if target_count == 1 { "" } else { "s" },
                            &controller_id.to_string()[..8],
                        ),
                        command: command.join(" "),
                    }
                })
                .collect();
            Ok(ControllerResolution::Choice(
                light_application::DynamicInstanceChoice {
                    choice_id: Uuid::new_v4(),
                    show_id: show.as_ref().map_or(Uuid::nil(), |show| show.id.0),
                    show_revision: show
                        .as_ref()
                        .map_or_else(|| state.output.snapshot().revision, |show| show.revision),
                    dynamic_id: definition.id,
                    pool_number: definition.pool_number,
                    command: tokens.join(" "),
                    options,
                    cancel_label: "Cancel".into(),
                },
            ))
        }
    }
}

fn append_command_timing(tokens: &mut Vec<String>, timing: CommandTiming) {
    if let Some(fade) = timing.fade_millis {
        tokens.extend(["TIME".into(), format_command_seconds(fade)]);
    }
    if let Some(delay) = timing.delay_millis {
        tokens.extend(["DELAY".into(), format_command_seconds(delay)]);
    }
}

fn format_command_seconds(millis: u64) -> String {
    if millis.is_multiple_of(1_000) {
        return format!("{}", millis / 1_000);
    }
    format!("{:.3}", millis as f64 / 1_000.0)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn parse_controller_value(field: &str, values: &[String]) -> Result<f32, String> {
    let value = match values {
        [value] => value
            .parse::<f32>()
            .map_err(|_| format!("Dynamic {field} value must be numeric"))?,
        [numerator, div, denominator] if field == "SPEED" && div == "DIV" => {
            let numerator = numerator
                .parse::<f32>()
                .map_err(|_| "Dynamic SPEED ratio numerator must be numeric")?;
            let denominator = denominator
                .parse::<f32>()
                .map_err(|_| "Dynamic SPEED ratio denominator must be numeric")?;
            if !denominator.is_finite() || denominator <= 0.0 {
                return Err("Dynamic SPEED ratio denominator must be positive".into());
            }
            numerator / denominator
        }
        _ => {
            return Err(format!(
                "Dynamic {field} requires one value{}",
                if field == "SPEED" {
                    " or <numerator> DIV <denominator>"
                } else {
                    ""
                }
            ));
        }
    };
    let valid = match field {
        "SIZE" => (0.0..=100.0).contains(&value),
        "SPEED" => value > 0.0,
        "PHASE" => value.is_finite(),
        _ => false,
    };
    if !value.is_finite() || !valid {
        return Err(format!("Dynamic {field} value is outside its valid range"));
    }
    Ok(value)
}

fn update_dynamic_phase_definition(
    state: &AppState,
    context: &light_application::ActionContext,
    dynamic_id: Uuid,
    field: &str,
    values: &[String],
) -> Result<(), String> {
    let (entry, store) = active_show_store(state)?;
    let (_, objects) = store
        .objects_with_portable_revision("dynamic")
        .map_err(|error| error.to_string())?;
    let object = objects
        .into_iter()
        .find(|object| object.id == dynamic_id.to_string())
        .ok_or("Dynamic is not stored in the active show")?;
    let mut definition: light_dynamics::DynamicDefinition =
        serde_json::from_value(object.body).map_err(|error| error.to_string())?;
    match field {
        "PHASE" => {
            let anchors = parse_phase_anchors(values)?;
            if anchors.len() < 2 {
                return Err("Dynamic PHASE THRU requires at least two anchors".into());
            }
            definition.phase.anchors_degrees = anchors;
        }
        "BLOCKS" => {
            definition.phase.block_size = parse_positive_phase_count("BLOCKS", values)?;
        }
        "REPEATS" => {
            definition.phase.repeats = parse_positive_phase_count("REPEATS", values)?;
        }
        "WINGS" => {
            definition.phase.wings = match values {
                [value] if value == "ON" => true,
                [value] if value == "OFF" => false,
                _ => return Err("Dynamic WINGS requires ON or OFF".into()),
            };
        }
        _ => return Err("unsupported Dynamic phase helper".into()),
    }
    definition.revision = definition.revision.saturating_add(1);
    light_dynamics::validate_definition(&definition).map_err(|error| error.to_string())?;
    let mutation = put_active_show_object(
        light_application::ActiveShowObjectKind::Dynamic,
        dynamic_id.to_string(),
        object.revision,
        serde_json::to_value(definition).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.message)?;
    let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    let change = result
        .changes
        .first()
        .ok_or("Dynamic phase edit produced no show-object change")?;
    emit_command_object_changed(
        state,
        &entry,
        change.kind.as_str(),
        &change.object_id,
        change.object_revision,
    );
    Ok(())
}

fn parse_phase_anchors(values: &[String]) -> Result<Vec<f32>, String> {
    if values.len() < 3 || values.len().is_multiple_of(2) {
        return Err("Dynamic PHASE spread uses <degrees> THRU <degrees> [THRU <degrees>]".into());
    }
    let mut anchors = Vec::with_capacity(values.len().div_ceil(2));
    for (index, value) in values.iter().enumerate() {
        if index % 2 == 1 {
            if value != "THRU" {
                return Err("Dynamic PHASE spread anchors must be separated by THRU".into());
            }
            continue;
        }
        let value = value
            .parse::<f32>()
            .map_err(|_| "Dynamic PHASE anchors must be numeric")?;
        if !value.is_finite() {
            return Err("Dynamic PHASE anchors must be finite".into());
        }
        anchors.push(value);
    }
    Ok(anchors)
}

fn parse_positive_phase_count(field: &str, values: &[String]) -> Result<u16, String> {
    let [value] = values else {
        return Err(format!("Dynamic {field} requires one integer"));
    };
    let value = value
        .parse::<u16>()
        .map_err(|_| format!("Dynamic {field} must be a positive integer"))?;
    if value == 0 {
        return Err(format!("Dynamic {field} must be a positive integer"));
    }
    Ok(value)
}
