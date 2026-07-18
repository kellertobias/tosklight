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
    let snapshot = state.engine.snapshot();
    let (mut fixtures, sources) = if tokens[start..at_index].iter().any(|token| token == "GROUP") {
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
            .programmers
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

fn fixture_level_values(
    state: &AppState,
    fixtures: &[light_core::FixtureId],
    value: &[String],
) -> Result<Vec<(light_core::FixtureId, f32)>, String> {
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
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
    let resolved = relative.then(|| state.engine.resolved_values());
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
    state
        .programmers
        .select_expression(session.id, fixtures.clone(), expression);
    if at_index == tokens.len() {
        state
            .programmers
            .set_command_line(session.id, command_line.to_owned());
        return Ok(fixtures.len());
    }
    let value = &tokens[at_index + 1..];
    if value.len() == 3 && value[1] == "." {
        apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            &fixtures,
        )?;
    } else if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
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
        state
            .programmers
            .set_command_line(session.id, command_line.to_owned());
        let values = fixture_level_values(state, &fixtures, value)?;
        set_command_fixture_intensities(state, session, values, timing);
    }
    Ok(fixtures.len())
}
