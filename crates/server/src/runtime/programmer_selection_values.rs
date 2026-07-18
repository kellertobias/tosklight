use super::*;

pub(super) fn apply_current_selection_value(
    state: &AppState,
    session: &Session,
    value: &[String],
    timing: CommandTiming,
) -> Result<usize, String> {
    let current = state
        .programmers
        .get(session.id)
        .ok_or("programmer does not exist")?;
    if current.selected.is_empty() {
        return Err("AT requires a current selection".into());
    }
    if value.len() == 3 && value[1] == "." {
        apply_command_preset(
            state,
            session,
            &format!("{}.{}", value[0], value[2]),
            &current.selected,
        )?;
        return Ok(current.selected.len());
    }
    if value.iter().any(|token| token == "THRU") {
        let points = parse_spread_points(value)?;
        if let Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) =
            current.selection_expression.clone()
        {
            state.programmers.set_group_faded_with_timing(
                session.id,
                group_id,
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Spread(points),
                timing.fade_millis,
                timing.delay_millis,
            );
            return Ok(current.selected.len());
        }
        let count = current.selected.len();
        set_command_fixture_intensities(
            state,
            session,
            current
                .selected
                .iter()
                .enumerate()
                .map(|(index, fixture_id)| (*fixture_id, spread_position(&points, index, count))),
            timing,
        );
        return Ok(current.selected.len());
    }
    let relative = value.len() == 2 && matches!(value[0].as_str(), "+" | "-");
    if value.len() != if relative { 2 } else { 1 } {
        return Err("unexpected tokens after level".into());
    }
    let level_token = value
        .get(usize::from(relative))
        .ok_or("AT requires a level")?;
    let percent = if level_token == "FULL" && !relative {
        100.0
    } else {
        level_token
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
    if let Some(light_programmer::SelectionExpression::LiveGroup { group_id, .. }) =
        current.selection_expression.clone()
    {
        if relative {
            return Err(
                "relative group values require GROUP GROUP so each fixture keeps its own offset"
                    .into(),
            );
        }
        state.programmers.set_group_faded_with_timing(
            session.id,
            group_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(percent / 100.0),
            timing.fade_millis,
            timing.delay_millis,
        );
        return Ok(current.selected.len());
    }
    let resolved = relative.then(|| state.engine.resolved_values());
    let values = current.selected.iter().map(|fixture_id| {
        let target = if let Some(resolved) = &resolved {
            let current = resolved
                .get(&(*fixture_id, light_core::AttributeKey::intensity()))
                .and_then(light_core::AttributeValue::normalized)
                .unwrap_or(0.0)
                * 100.0;
            (current + if value[0] == "+" { percent } else { -percent }).clamp(0.0, 100.0)
        } else {
            percent
        };
        (*fixture_id, target / 100.0)
    });
    set_command_fixture_intensities(state, session, values, timing);
    Ok(current.selected.len())
}
