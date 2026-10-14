use super::programmer_family_values::typed_family_command as typed_family;
use super::*;

fn percent_level(tokens: &[String]) -> Result<f32, String> {
    let level = tokens.first().ok_or("AT requires a level")?;
    let percent = command_level_percent(level, true)?;
    if tokens.len() != 1 {
        return Err("unexpected tokens after level".into());
    }
    Ok(percent)
}

fn execute_mixed_group_value(
    state: &AppState,
    session: &Session,
    address: &[String],
    value: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    let snapshot = state.output.snapshot();
    let parsed = parse_group_mixed_selection(&snapshot, address, true)?;
    let sources = light_programmer::SelectionExpression::Sources {
        items: parsed.sources.clone(),
    };
    if let Some(n) = typed_family(
        state,
        session,
        value,
        &parsed.fixtures,
        Some(&sources),
        timing,
    ) {
        return n;
    }
    if explicit_command_preset_value(value) {
        let Some(address) = applicable_command_preset_address(
            state,
            value,
            &parsed.fixtures,
            &sources.live_group_owners(),
        )?
        else {
            return Ok(0);
        };
        state
            .programming
            .select_expression(session.id, parsed.fixtures.clone(), sources);
        apply_command_preset(state, session, &address.storage_key(), &parsed.fixtures)?;
        return Ok(parsed.fixtures.len());
    }
    let percent = percent_level(value)?;
    state.programming.select_expression(
        session.id,
        parsed.fixtures.clone(),
        light_programmer::SelectionExpression::Sources {
            items: parsed.sources.clone(),
        },
    );
    if parsed.sources.iter().all(|source| {
        matches!(
            source,
            light_programmer::SelectionReference::LiveGroup { .. }
        )
    }) {
        let mut programmed = HashSet::new();
        for source in &parsed.sources {
            let light_programmer::SelectionReference::LiveGroup { group_id } = source else {
                unreachable!("all mixed sources were checked as live Groups")
            };
            if programmed.insert(group_id.clone()) {
                set_command_group_intensity(
                    state,
                    session,
                    group_id.clone(),
                    light_core::AttributeValue::Normalized(percent / 100.0),
                    timing,
                );
            }
        }
    } else {
        set_command_fixture_intensities(
            state,
            session,
            parsed
                .fixtures
                .iter()
                .copied()
                .map(|fixture| (fixture, percent / 100.0)),
            timing,
        );
    }
    Ok(parsed.fixtures.len())
}

fn apply_group_value(
    state: &AppState,
    session: &Session,
    group_id: &str,
    fixtures: &[light_core::FixtureId],
    frozen: bool,
    value: &[String],
    timing: CommandTiming,
) -> Result<(), String> {
    if explicit_command_preset_value(value) {
        return apply_command_preset(
            state,
            session,
            &command_preset_address(value)?.storage_key(),
            fixtures,
        );
    }
    if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
        ensure_spread_fits(&points, fixtures.len())?;
        if frozen {
            let count = fixtures.len();
            set_command_fixture_intensities(
                state,
                session,
                fixtures
                    .iter()
                    .enumerate()
                    .map(|(index, fixture)| (*fixture, spread_position(&points, index, count))),
                timing,
            );
        } else {
            set_command_group_intensity(
                state,
                session,
                group_id.to_owned(),
                light_core::AttributeValue::Spread(points),
                timing,
            );
        }
        return Ok(());
    }
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    if relative && !frozen {
        return Err(
            "relative group values require DEGROUP so each fixture keeps its own offset".into(),
        );
    }
    if relative && value[1] == "FULL" {
        return Err("level must be a percentage or FULL".into());
    }
    let percent = percent_level(&value[usize::from(relative)..])?;
    if frozen {
        let resolved = state.output.resolved_values();
        let values = fixtures.iter().map(|fixture| {
            let target = if relative {
                let current = resolved
                    .get(&(*fixture, light_core::AttributeKey::intensity()))
                    .and_then(light_core::AttributeValue::normalized)
                    .unwrap_or(0.0)
                    * 100.0;
                (current + if value[0] == "+" { percent } else { -percent }).clamp(0.0, 100.0)
            } else {
                percent
            };
            (*fixture, target / 100.0)
        });
        set_command_fixture_intensities(state, session, values, timing);
    } else {
        set_command_group_intensity(
            state,
            session,
            group_id.to_owned(),
            light_core::AttributeValue::Normalized(percent / 100.0),
            timing,
        );
    }
    Ok(())
}

pub(super) fn execute_group_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
    tokens: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    // The keypad normalizes the double Group gesture to DEGROUP. Keep DEGRP as a
    // backwards-compatible string-input alias for shows and integrations using the old spelling.
    if tokens
        .windows(2)
        .any(|pair| pair[0] == "GROUP" && pair[1] == "GROUP")
    {
        return Err("GROUP GROUP is not a command; use DEGROUP to dereference a group".into());
    }
    let frozen = tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "DEGROUP" | "DEGRP"));
    let id_index = 1;
    let at_index = tokens
        .iter()
        .position(|token| token == "AT")
        .unwrap_or(tokens.len());
    let address = &tokens[id_index..at_index];
    let mixed = address
        .iter()
        .any(|token| matches!(token.as_str(), "THRU" | "+" | "-"))
        || tokens[..at_index].iter().any(|token| {
            matches!(
                token.as_str(),
                "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
            )
        })
        || tokens[1..at_index]
            .iter()
            .any(|token| matches!(token.as_str(), "DEGROUP" | "DEGRP"));
    if at_index < tokens.len() && mixed {
        return execute_mixed_group_value(
            state,
            session,
            &tokens[..at_index],
            &tokens[at_index + 1..],
            timing,
        );
    }
    if at_index == tokens.len() && mixed && !address.iter().any(|token| token == "DIV") {
        let parsed =
            parse_group_mixed_selection(&state.output.snapshot(), &tokens[..at_index], true)?;
        state.programming.select_expression(
            session.id,
            parsed.fixtures.clone(),
            light_programmer::SelectionExpression::Sources {
                items: parsed.sources,
            },
        );
        state
            .programming
            .set_command_line(session.id, command_line.to_owned());
        return Ok(parsed.fixtures.len());
    }
    let group_id = tokens
        .get(id_index)
        .ok_or("GROUP requires a group number")?
        .clone();
    let snapshot = state.output.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let base = light_programmer::resolve_group(&group_id, &groups)?;
    let rule = parse_subset_rule(&tokens[id_index + 1..at_index])?;
    let fixtures = light_programmer::apply_selection_rule(&base, &rule);
    let expression = if frozen {
        // DEGROUP (GROUP GROUP) dereferences to individual fixtures with no group reference,
        // identical to the double-click dereference gesture.
        light_programmer::SelectionExpression::Sources {
            items: fixtures
                .iter()
                .map(|fixture_id| light_programmer::SelectionReference::Fixture {
                    fixture_id: *fixture_id,
                })
                .collect(),
        }
    } else {
        light_programmer::SelectionExpression::LiveGroup {
            group_id: group_id.clone(),
            rule,
        }
    };
    let value = &tokens[(at_index + 1).min(tokens.len())..];
    if let Some(n) = typed_family(state, session, value, &fixtures, Some(&expression), timing) {
        return n;
    }
    if at_index < tokens.len()
        && explicit_command_preset_value(value)
        && applicable_command_preset_address(
            state,
            value,
            &fixtures,
            &expression.live_group_owners(),
        )?
        .is_none()
    {
        return Ok(0);
    }
    state
        .programming
        .select_expression(session.id, fixtures.clone(), expression);
    if at_index < tokens.len() {
        apply_group_value(
            state,
            session,
            &group_id,
            &fixtures,
            frozen,
            &tokens[at_index + 1..],
            timing,
        )?;
    }
    Ok(fixtures.len())
}

/// Validate against the intended selection before publishing it; a valid but inapplicable
/// Preset is a no-op. Keep semantic Aim and programming-contract checks identical to Fixture AT.
pub(super) fn applicable_command_preset_address(
    state: &AppState,
    value: &[String],
    fixtures: &[light_core::FixtureId],
    live_groups: &[String],
) -> Result<Option<light_programmer::PresetAddress>, String> {
    let address = command_preset_address(value)?;
    let preset = load_command_preset(state, address)?;
    let semantic_aim = preset.aim_at_fixture_number.is_some()
        && state.output.supported_programming_contract()
            >= light_core::programming::PROGRAMMING_CONTRACT_VERSION;
    let preset = if semantic_aim {
        super::programmer_aim_command::resolve_aim_preset(state, &preset)?
    } else {
        preset
    };
    let applicable = if let Some(target) = preset.aim_at_fixture_number {
        !super::programmer_aim_command::aim_selection(state, fixtures, target)?.is_empty()
    } else {
        let groups = state
            .output
            .snapshot()
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect();
        super::command_presets::command_preset_has_values(
            &preset,
            fixtures,
            live_groups,
            &groups,
            state.output.supported_programming_contract(),
        )?
    };
    Ok(applicable.then_some(address))
}
