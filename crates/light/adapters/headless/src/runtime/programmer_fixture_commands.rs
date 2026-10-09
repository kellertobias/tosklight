use super::programmer_family_values::typed_family_command as typed_family;
use super::*;

fn fixture_selection(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    start: usize,
    at_index: usize,
    continuing: bool,
) -> Result<
    (
        Vec<light_core::FixtureId>,
        light_programmer::SelectionExpression,
    ),
    String,
> {
    let snapshot = state.output.snapshot();
    let (mut fixtures, sources) = if tokens[start..at_index]
        .iter()
        .any(|token| matches!(token.as_str(), "GROUP" | "DEGROUP" | "DEGRP"))
    {
        let parsed = parse_group_mixed_selection(&snapshot, &tokens[start..at_index], false)?;
        (parsed.fixtures, parsed.sources)
    } else {
        let fixtures = parse_fixture_selection(&snapshot.fixtures, &tokens[start..at_index])?;
        let sources = fixtures
            .iter()
            .map(|fixture| light_programmer::SelectionReference::Fixture {
                fixture_id: *fixture,
            })
            .collect();
        (fixtures, sources)
    };
    let mut sources = sources;
    if continuing {
        let current = state
            .programming
            .get(session.id)
            .ok_or("programmer does not exist")?;
        let mut combined = match current.selection_expression {
            Some(light_programmer::SelectionExpression::Sources { items }) => items,
            Some(light_programmer::SelectionExpression::LiveGroup {
                group_id,
                rule: light_programmer::SelectionRule::All,
            }) => vec![light_programmer::SelectionReference::LiveGroup { group_id }],
            _ => current
                .selected
                .into_iter()
                .map(|fixture| light_programmer::SelectionReference::Fixture {
                    fixture_id: fixture,
                })
                .collect(),
        };
        combined.extend(sources);
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        fixtures = light_programmer::resolve_selection_references(&combined, &groups);
        sources = combined;
    }
    Ok((
        fixtures,
        light_programmer::SelectionExpression::Sources { items: sources },
    ))
}

pub(super) fn execute_dmx_selection_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
    tokens: &[String],
) -> Result<usize, String> {
    let (universe, address) = parse_dmx_address(tokens)?;
    let snapshot = state.output.snapshot();
    let fixtures = resolve_dmx_fixture_selection(&snapshot.fixtures, universe, address)?;
    if fixtures.is_empty() {
        return Err(format!("No fixture is patched at DMX {universe}.{address}"));
    }
    let expression = light_programmer::SelectionExpression::Sources {
        items: fixtures
            .iter()
            .map(|fixture_id| light_programmer::SelectionReference::Fixture {
                fixture_id: *fixture_id,
            })
            .collect(),
    };
    state
        .programming
        .select_expression(session.id, fixtures.clone(), expression);
    state
        .programming
        .set_command_line(session.id, command_line.to_owned());
    Ok(fixtures.len())
}

fn fixture_level_values(
    state: &AppState,
    fixtures: &[light_core::FixtureId],
    value: &[String],
) -> Result<Vec<(light_core::FixtureId, f32)>, String> {
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    if value.len() != if relative { 2 } else { 1 } {
        return Err("unexpected tokens after level".into());
    }
    let level = value
        .get(usize::from(relative))
        .ok_or("AT requires a level")?;
    let percent = if level == "FULL" && !relative {
        100.0
    } else {
        level
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
    let resolved = relative.then(|| state.output.resolved_values());
    Ok(fixtures
        .iter()
        .map(|fixture| {
            let target = resolved.as_ref().map_or(percent, |resolved| {
                let current = resolved
                    .get(&(*fixture, light_core::AttributeKey::intensity()))
                    .and_then(light_core::AttributeValue::normalized)
                    .unwrap_or(0.0)
                    * 100.0;
                (current + if value[0] == "+" { percent } else { -percent }).clamp(0.0, 100.0)
            });
            (*fixture, target / 100.0)
        })
        .collect())
}

/// `AT FIXTURE 5` names an object to aim at rather than a level to set.
fn aim_target(value: &[String]) -> Option<u32> {
    let [keyword, number] = value else {
        return None;
    };
    matches!(keyword.as_str(), "FIXTURE" | "FIXTURES" | "FIX")
        .then(|| number.parse::<u32>().ok())
        .flatten()
}

pub(super) fn execute_fixture_programmer_command(
    state: &AppState,
    session: &Session,
    command_line: &str,
    tokens: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    let continuing = tokens[0] == "+";
    let start = if continuing {
        1
    } else {
        usize::from(matches!(
            tokens[0].as_str(),
            "FIXTURE" | "FIXTURES" | "CHANNEL" | "CHANNELS"
        ))
    };
    if tokens.len() <= start {
        return Err("expected a fixture number".into());
    }
    let at_index = tokens
        .iter()
        .position(|token| token == "AT")
        .unwrap_or(tokens.len());
    let (fixtures, expression) =
        fixture_selection(state, session, tokens, start, at_index, continuing)?;
    if at_index == tokens.len() {
        state
            .programming
            .select_expression(session.id, fixtures.clone(), expression);

        state
            .programming
            .set_command_line(session.id, command_line.to_owned());
        return Ok(fixtures.len());
    }
    let value = &tokens[at_index + 1..];
    if let Some(n) = typed_family(state, session, value, &fixtures, Some(&expression), timing) {
        return n;
    }
    if let Some(target) = aim_target(value) {
        let assignments = super::programmer_aim_command::aim_selection(state, &fixtures, target)?;
        if assignments.is_empty() {
            let live_group = state.output.supported_programming_contract()
                >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
                && !expression.live_group_owners().is_empty();
            if !live_group
                || super::programmer_aim_command::aim_target_intent(state, target)?.is_none()
            {
                return Ok(0);
            }
        }
        state
            .programming
            .select_expression(session.id, fixtures.clone(), expression);
        state
            .programming
            .set_command_line(session.id, command_line.to_owned());
        if state.output.supported_programming_contract()
            >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
        {
            super::programmer_aim_command::apply_semantic_aim(
                state,
                session,
                assignments,
                timing,
                target,
            )?;
        } else {
            set_command_fixture_values(state, session, assignments, timing);
        }
    } else if (value.len() == 3 && value[1] == ".")
        || (value.len() == 3 && value[1] == "PRESET")
        || (value.len() == 2 && command_preset_address(value).is_ok())
    {
        // Validate the explicit preset before selection changes, even for an empty target list.
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
            !super::programmer_aim_command::aim_selection(state, &fixtures, target)?.is_empty()
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
                &fixtures,
                &expression.live_group_owners(),
                &groups,
                state.output.supported_programming_contract(),
            )?
        };
        if !applicable {
            return Ok(0);
        }
        state
            .programming
            .select_expression(session.id, fixtures.clone(), expression);
        apply_command_preset(state, session, &address.storage_key(), &fixtures)?;
    } else if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
        if fixtures.is_empty() {
            return Ok(0);
        }
        ensure_spread_fits(&points, fixtures.len())?;
        state
            .programming
            .select_expression(session.id, fixtures.clone(), expression);
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
        let values = fixture_level_values(state, &fixtures, value)?;
        if values.is_empty() {
            return Ok(0);
        }
        state
            .programming
            .select_expression(session.id, fixtures.clone(), expression);
        state
            .programming
            .set_command_line(session.id, command_line.to_owned());
        set_command_fixture_intensities(state, session, values, timing);
    }
    Ok(fixtures.len())
}
