use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommandTiming {
    pub(super) fade_millis: Option<u64>,
    pub(super) delay_millis: Option<u64>,
}

pub(super) fn programmer_value_timing(state: &AppState, timing: CommandTiming) -> CommandTiming {
    CommandTiming {
        fade_millis: Some(
            timing
                .fade_millis
                .unwrap_or_else(|| state.configuration.read().programmer_fade_millis),
        ),
        ..timing
    }
}

pub(super) fn set_command_fixture_intensities(
    state: &AppState,
    session: &Session,
    values: impl IntoIterator<Item = (light_core::FixtureId, f32)>,
    timing: CommandTiming,
) {
    state.programmers.set_many_faded_with_timing(
        session.id,
        values.into_iter().map(|(fixture_id, value)| {
            (
                fixture_id,
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(value),
            )
        }),
        timing.fade_millis,
        timing.delay_millis,
    );
}

pub(super) fn command_time_millis(token: &str) -> Result<u64, String> {
    let seconds = token
        .parse::<f64>()
        .map_err(|_| "TIME and DELAY require seconds")?;
    if !seconds.is_finite() || !(0.0..=86_400.0).contains(&seconds) {
        return Err("TIME and DELAY must be within 0-86400 seconds".into());
    }
    Ok((seconds * 1_000.0).round() as u64)
}

pub(super) fn command_time_at(tokens: &[String], index: usize) -> Result<(u64, usize), String> {
    let whole = tokens.get(index).ok_or("TIME and DELAY require seconds")?;
    if tokens.get(index + 1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(index + 2)
            .ok_or("time decimal requires digits after the dot")?;
        return Ok((command_time_millis(&format!("{whole}.{fraction}"))?, 3));
    }
    Ok((command_time_millis(whole)?, 1))
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
            "TIME" => {
                let (value, used) = command_time_at(tokens, index + 1)?;
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
    let spaced = command_line
        .replace(',', ".")
        .replace('.', " . ")
        .replace('+', " + ")
        .replace('-', " - ");
    let mut raw_tokens = Vec::new();
    for token in spaced
        .split_whitespace()
        .map(|token| token.to_ascii_uppercase())
    {
        if token == "DEGRP" {
            raw_tokens.extend(["GROUP".to_owned(), "GROUP".to_owned()]);
            continue;
        }
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
