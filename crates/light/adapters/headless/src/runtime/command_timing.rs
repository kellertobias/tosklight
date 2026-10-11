use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommandTiming {
    pub(super) fade: bool,
    pub(super) fade_millis: Option<u64>,
    pub(super) delay_millis: Option<u64>,
}

pub(super) fn programmer_value_timing(state: &AppState, timing: CommandTiming) -> CommandTiming {
    if timing.fade {
        return timing;
    }
    let configuration = state.installation.configuration();
    if !configuration.command_line_at_uses_programmer_fade {
        return timing;
    }
    CommandTiming {
        fade: true,
        fade_millis: Some(configuration.programmer_fade_millis),
        ..timing
    }
}

pub(super) fn set_command_fixture_intensities(
    state: &AppState,
    session: &Session,
    values: impl IntoIterator<Item = (light_core::FixtureId, f32)>,
    timing: CommandTiming,
) {
    let assignments = values.into_iter().map(|(fixture_id, value)| {
        (
            fixture_id,
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(value),
        )
    });
    if timing.fade {
        state.programming.set_many_faded_with_timing(
            session.id,
            assignments,
            timing.fade_millis,
            timing.delay_millis,
        );
    } else {
        state.programming.set_many_immediate_with_delay(
            session.id,
            assignments,
            timing.delay_millis,
        );
    }
}

/// Apply arbitrary attribute values with the command's own timing.
///
/// The intensity path above is the common case; this is for commands that write something else,
/// such as the pan and tilt that aim a selection at an object.
pub(super) fn set_command_fixture_values(
    state: &AppState,
    session: &Session,
    assignments: impl IntoIterator<
        Item = (
            light_core::FixtureId,
            light_core::AttributeKey,
            light_core::AttributeValue,
        ),
    >,
    timing: CommandTiming,
) {
    if timing.fade {
        state.programming.set_many_faded_with_timing(
            session.id,
            assignments,
            timing.fade_millis,
            timing.delay_millis,
        );
    } else {
        state.programming.set_many_immediate_with_delay(
            session.id,
            assignments,
            timing.delay_millis,
        );
    }
}

pub(super) fn set_command_group_intensity(
    state: &AppState,
    session: &Session,
    group_id: String,
    value: light_core::AttributeValue,
    timing: CommandTiming,
) {
    if timing.fade {
        state.programming.set_group_faded_with_timing(
            session.id,
            group_id,
            light_core::AttributeKey::intensity(),
            value,
            timing.fade_millis,
            timing.delay_millis,
        );
    } else {
        state.programming.set_group_immediate_with_delay(
            session.id,
            group_id,
            light_core::AttributeKey::intensity(),
            value,
            timing.delay_millis,
        );
    }
}

pub(super) fn command_level_percent(token: &str, allow_full: bool) -> Result<f32, String> {
    let percent = if token == "FULL" && allow_full {
        100.0
    } else {
        token
            .strip_suffix('%')
            .unwrap_or(token)
            .parse::<f32>()
            .map_err(|_| "level must be a percentage or FULL")?
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err("level must be within 0-100".into());
    }
    Ok(percent)
}

pub(super) fn command_time_millis(token: &str) -> Result<u64, String> {
    let seconds = token
        .strip_suffix('S')
        .or_else(|| token.strip_suffix('s'))
        .unwrap_or(token)
        .parse::<f64>()
        .map_err(|_| "TIME and DELAY require seconds")?;
    if !seconds.is_finite() || !(0.0..=86_400.0).contains(&seconds) {
        return Err("TIME and DELAY must be within 0-86400 seconds".into());
    }
    Ok((seconds * 1_000.0).round() as u64)
}

pub(super) fn command_time_at(tokens: &[String], index: usize) -> Result<(u64, usize), String> {
    let whole = tokens.get(index).ok_or("TIME and DELAY require seconds")?;
    let (number, used) = if tokens.get(index + 1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(index + 2)
            .ok_or("time decimal requires digits after the dot")?;
        (format!("{whole}.{fraction}"), 3)
    } else {
        (whole.clone(), 1)
    };
    let value = command_time_millis(&number)?;
    if tokens
        .get(index + used)
        .is_some_and(|token| token == "THRU")
    {
        return Err("Timing ranges are not supported".into());
    }
    Ok((value, used))
}

pub(super) fn extract_command_timing(
    tokens: &[String],
) -> Result<(Vec<String>, CommandTiming), String> {
    let mut command = Vec::with_capacity(tokens.len());
    let mut timing = CommandTiming::default();
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "TIME" if tokens.get(index + 1).is_some_and(|token| token == "TIME") => {
                let (value, used) = command_time_at(tokens, index + 2)?;
                timing.delay_millis = Some(value);
                index += 2 + used;
            }
            "TIME" | "FADE" => {
                let (value, used) = command_time_at(tokens, index + 1)?;
                timing.fade = true;
                timing.fade_millis = Some(value);
                index += 1 + used;
            }
            "DELAY" => {
                let (value, used) = command_time_at(tokens, index + 1)?;
                timing.delay_millis = Some(value);
                index += 1 + used;
            }
            _ => {
                command.push(tokens[index].clone());
                index += 1;
            }
        }
    }
    Ok((command, timing))
}

pub(super) fn tokenize_programmer_command(
    command_line: &str,
) -> Result<(Vec<String>, CommandTiming), String> {
    let normalized = command_line.replace(',', ".");
    let spaced = normalized
        .split_whitespace()
        .map(|token| {
            let numeric_unit = token
                .strip_suffix('%')
                .or_else(|| token.strip_suffix('s'))
                .or_else(|| token.strip_suffix('S'));
            if numeric_unit.is_some_and(|number| {
                number.starts_with(|ch: char| ch.is_ascii_digit()) && number.parse::<f64>().is_ok()
            }) {
                token.to_owned()
            } else {
                token
                    .replace('.', " . ")
                    .replace('+', " + ")
                    .replace('-', " - ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut raw_tokens = Vec::new();
    for token in spaced
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
    {
        if token == "F" || token == "G" {
            raw_tokens.push(if token == "F" { "FIXTURE" } else { "GROUP" }.to_owned());
            continue;
        }
        if token.len() > 1 && matches!(token.as_bytes()[0], b'F' | b'G') {
            let (prefix, number) = token.split_at(1);
            if matches!(prefix, "F" | "G")
                && number.chars().all(|character| character.is_ascii_digit())
            {
                raw_tokens.push(if prefix == "F" { "FIXTURE" } else { "GROUP" }.to_owned());
                raw_tokens.push(number.to_owned());
                continue;
            }
        }
        raw_tokens.push(token);
    }
    extract_command_timing(&raw_tokens)
}

#[cfg(test)]
mod documented_timing_tests {
    use super::*;

    #[test]
    fn documented_simple_percentage_and_fade_timing_match_keypad_time() {
        for command in [
            "G91 AT 25 FADE 0 DELAY 0",
            "G91 AT 25% FADE 0s DELAY 0s",
            "GROUP 91 AT 25% TIME 0 DELAY 0",
        ] {
            let (tokens, timing) = tokenize_programmer_command(command).unwrap();
            assert_eq!(
                tokens,
                [
                    "GROUP",
                    "91",
                    "AT",
                    if command.contains('%') { "25%" } else { "25" }
                ]
            );
            assert!(timing.fade);
            assert_eq!(timing.fade_millis, Some(0));
            assert_eq!(timing.delay_millis, Some(0));
        }
        let (tokens, timing) =
            tokenize_programmer_command("F1 AT 25.5% FADE 12.5s DELAY 0.25s").unwrap();
        assert_eq!(tokens, ["FIXTURE", "1", "AT", "25.5%"]);
        assert_eq!(timing.fade_millis, Some(12_500));
        assert_eq!(timing.delay_millis, Some(250));
    }

    #[test]
    fn timing_ranges_refuse_instead_of_becoming_intensity_spreads() {
        for command in [
            "G91 AT 25 FADE 0 THRU 5",
            "F1 AT 25 TIME 0 THRU 5",
            "G91 AT 25 DELAY 0 THRU 5",
        ] {
            assert_eq!(
                tokenize_programmer_command(command).unwrap_err(),
                "Timing ranges are not supported"
            );
        }
        let (tokens, timing) = tokenize_programmer_command("G91 AT 0 THRU 100 TIME 0").unwrap();
        assert_eq!(tokens, ["GROUP", "91", "AT", "0", "THRU", "100"]);
        assert_eq!(timing.fade_millis, Some(0));
    }

    #[test]
    fn keypad_bare_decimal_and_double_time_preserve_seconds_and_delay() {
        let tokens = [
            "GROUP", "91", "AT", "25", "TIME", "12", ".", "5", "TIME", "TIME", "0",
        ]
        .map(String::from);
        let (body, timing) = extract_command_timing(&tokens).unwrap();
        assert_eq!(body, ["GROUP", "91", "AT", "25"]);
        assert!(timing.fade);
        assert_eq!(timing.fade_millis, Some(12_500));
        assert_eq!(timing.delay_millis, Some(0));
    }
}
