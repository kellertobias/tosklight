use super::*;

fn percent_level(tokens: &[String]) -> Result<f32, String> {
    let level = tokens.first().ok_or("AT requires a level")?;
    let percent = if level == "FULL" {
        100.0
    } else {
        level
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
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
    let snapshot = state.engine.snapshot();
    let parsed = parse_group_mixed_selection(&snapshot, address, true)?;
    let percent = percent_level(value)?;
    state.programmers.select_expression(
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
                state.programmers.set_group_faded_with_timing(
                    session.id,
                    group_id.clone(),
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(percent / 100.0),
                    timing.fade_millis,
                    timing.delay_millis,
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
    if value.len() == 3 && value[1] == "." {
        return apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            fixtures,
        );
    }
    if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
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
            state.programmers.set_group_faded_with_timing(
                session.id,
                group_id.to_owned(),
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Spread(points),
                timing.fade_millis,
                timing.delay_millis,
            );
        }
        return Ok(());
    }
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    if relative && !frozen {
        return Err(
            "relative group values require GROUP GROUP so each fixture keeps its own offset".into(),
        );
    }
    if relative && value[1] == "FULL" {
        return Err("level must be a percentage or FULL".into());
    }
    let percent = percent_level(&value[usize::from(relative)..])?;
    if frozen {
        let resolved = state.engine.resolved_values();
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
        state.programmers.set_group_faded_with_timing(
            session.id,
            group_id.to_owned(),
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(percent / 100.0),
            timing.fade_millis,
            timing.delay_millis,
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
    let frozen = tokens.get(1).is_some_and(|token| token == "GROUP");
    let id_index = if frozen { 2 } else { 1 };
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
            .windows(2)
            .any(|pair| pair[0] == "GROUP" && pair[1] == "GROUP");
    if at_index < tokens.len() && mixed {
        return execute_mixed_group_value(
            state,
            session,
            &tokens[1..at_index],
            &tokens[at_index + 1..],
            timing,
        );
    }
    if at_index == tokens.len() && mixed && !address.iter().any(|token| token == "DIV") {
        let parsed =
            parse_group_mixed_selection(&state.engine.snapshot(), &tokens[1..at_index], true)?;
        state.programmers.select_expression(
            session.id,
            parsed.fixtures.clone(),
            light_programmer::SelectionExpression::Sources {
                items: parsed.sources,
            },
        );
        state
            .programmers
            .set_command_line(session.id, command_line.to_owned());
        return Ok(parsed.fixtures.len());
    }
    let group_id = tokens
        .get(id_index)
        .ok_or("GROUP requires a group number")?
        .clone();
    let snapshot = state.engine.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let base = light_programmer::resolve_group(&group_id, &groups)?;
    let rule = parse_subset_rule(&tokens[id_index + 1..at_index])?;
    let fixtures = light_programmer::apply_selection_rule(&base, &rule);
    let expression = if frozen {
        light_programmer::SelectionExpression::FrozenGroup {
            group_id: group_id.clone(),
            source_revision: snapshot.revision,
        }
    } else {
        light_programmer::SelectionExpression::LiveGroup {
            group_id: group_id.clone(),
            rule,
        }
    };
    state
        .programmers
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
