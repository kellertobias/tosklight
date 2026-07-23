use super::*;

pub(super) fn parse_spread_points(tokens: &[String]) -> Result<Vec<f32>, String> {
    if tokens.len() < 3 || tokens.len().is_multiple_of(2) {
        return Err("a spread requires levels separated by THRU".into());
    }
    let mut points = Vec::with_capacity(tokens.len().div_ceil(2));
    for (index, token) in tokens.iter().enumerate() {
        if index % 2 == 1 {
            if token != "THRU" {
                return Err("spread control points must be separated by THRU".into());
            }
            continue;
        }
        let percent = if token == "FULL" {
            100.0
        } else {
            token
                .parse::<f32>()
                .map_err(|_| "spread levels must be percentages or FULL")?
        };
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err("spread levels must be within 0-100".into());
        }
        points.push(percent / 100.0);
    }
    Ok(points)
}

pub(super) use light_core::spread_position;

/// Rejects multi-point spreads that cannot place every explicit control point (docs/plans/Next/50).
/// One- and two-point inputs keep their established single-item/endpoint behavior.
pub(super) fn ensure_spread_fits(points: &[f32], count: usize) -> Result<(), String> {
    if points.len() > 2 && points.len() > count {
        return Err(format!(
            "spread has {} control points but only {count} selected items",
            points.len()
        ));
    }
    Ok(())
}

pub(super) fn parse_command_cue_number(tokens: &[String]) -> Result<f64, String> {
    if tokens.is_empty() {
        return Err("CUE requires a cue number".into());
    }
    if tokens.last().is_some_and(|token| token == ".") {
        return Err("cue number is invalid".into());
    }
    let value = tokens.join("");
    let number = value.parse::<f64>().map_err(|_| "cue number is invalid")?;
    if !number.is_finite() || number <= 0.0 {
        return Err("cue number must be positive".into());
    }
    Ok(number)
}

/// Direct compatibility entry point for the CUE navigation family.
///
/// The grammar, address resolution, typed Playback action, command-line reset, and the temporary
/// v1 `playback_changed` notification are owned by the feature module under `command_http`. This
/// wrapper only keeps the generic legacy executor's dispatch table working for callers that have
/// not moved to the Programming interaction boundary yet.
pub(super) fn execute_cue_operation(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    command: &str,
) -> Result<usize, String> {
    command_http::execute_compatibility_cue_navigation(state, session, context, command)
}

/// Compatibility dispatch entry. Grammar, ordering, persistence, and events are owned by the
/// typed Speed Group feature under `command_http`.
pub(super) fn execute_speed_group_operation(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    command: &str,
) -> Result<usize, String> {
    command_http::execute_compatibility_speed_group(state, session, context, command)
}
