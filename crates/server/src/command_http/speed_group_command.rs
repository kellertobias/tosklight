//! Pure syntax for `SPD GRP` retained/manual Speed Group commands.

use light_application::{
    SpeedBpm, SpeedBpmDelta, SpeedGroupAction, SpeedGroupCommand, SpeedGroupId,
};

pub(super) fn parse(command: &str) -> Result<Option<SpeedGroupCommand>, String> {
    let Ok((tokens, _timing)) = super::super::tokenize_programmer_command(command) else {
        return Ok(None);
    };
    if tokens.first().is_none_or(|token| token != "SPD") {
        return Ok(None);
    }
    if tokens.len() < 5 || tokens[1] != "GRP" || tokens[3] != "AT" {
        return Err("expected SPD GRP <1-5> AT <BPM | +/- BPM | SPD GRP <1-5>>".into());
    }
    let source = parse_group(&tokens[2])?;
    let right = &tokens[4..];
    let action = if right.first().is_some_and(|token| token == "SPD") {
        parse_synchronization(source, right)?
    } else {
        parse_bpm_action(source, right)?
    };
    Ok(Some(SpeedGroupCommand::current(action)))
}

pub(super) fn normalized(command: &str) -> Result<String, String> {
    let (tokens, _) = super::super::tokenize_programmer_command(command)?;
    Ok(tokens.join(" "))
}

pub(super) fn addressed(command: SpeedGroupCommand) -> Vec<SpeedGroupId> {
    match command.action {
        SpeedGroupAction::SetBpm { group, .. } | SpeedGroupAction::AdjustBpm { group, .. } => {
            vec![group]
        }
        SpeedGroupAction::Synchronize { source, target } => vec![source, target],
    }
}

fn parse_synchronization(
    source: SpeedGroupId,
    right: &[String],
) -> Result<SpeedGroupAction, String> {
    if right.len() != 3 || right[1] != "GRP" {
        return Err("synchronization target must be SPD GRP <1-5>".into());
    }
    Ok(SpeedGroupAction::Synchronize {
        source,
        target: parse_group(&right[2])?,
    })
}

fn parse_bpm_action(group: SpeedGroupId, right: &[String]) -> Result<SpeedGroupAction, String> {
    let (sign, value_tokens) = match right.first().map(String::as_str) {
        Some("+") => (Some(1.0), &right[1..]),
        Some("-") => (Some(-1.0), &right[1..]),
        _ => (None, right),
    };
    let (entered, consumed) = parse_decimal(value_tokens)?;
    if consumed != value_tokens.len() {
        return Err("unexpected tokens after BPM value".into());
    }
    if let Some(sign) = sign {
        if entered <= 0.0 {
            return Err("relative BPM must be positive".into());
        }
        let delta =
            SpeedBpmDelta::new(sign * entered).ok_or("relative BPM must be finite and non-zero")?;
        return Ok(SpeedGroupAction::AdjustBpm { group, delta });
    }
    let bpm = SpeedBpm::new(entered).ok_or("BPM must be finite and within 0.1-999")?;
    Ok(SpeedGroupAction::SetBpm { group, bpm })
}

fn parse_group(token: &str) -> Result<SpeedGroupId, String> {
    let one_based = token
        .parse::<u8>()
        .map_err(|_| "Speed Group number is invalid")?;
    SpeedGroupId::new(one_based).ok_or_else(|| "Speed Group number must be within 1-5".into())
}

fn parse_decimal(tokens: &[String]) -> Result<(f64, usize), String> {
    let whole = tokens.first().ok_or("AT requires a BPM value")?;
    let (value, consumed) = if tokens.get(1).is_some_and(|token| token == ".") {
        let fraction = tokens
            .get(2)
            .ok_or("BPM decimal requires digits after the separator")?;
        (format!("{whole}.{fraction}"), 3)
    } else {
        (whole.clone(), 1)
    };
    let value = value.parse::<f64>().map_err(|_| "BPM value is invalid")?;
    if !value.is_finite() {
        return Err("BPM value must be finite".into());
    }
    Ok((value, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_comma_relative_and_synchronization_without_runtime_state() {
        let decimal = parse("SPD GRP 1 AT 128,5").unwrap().unwrap();
        let relative = parse("SPD GRP 2 AT - 8.25").unwrap().unwrap();
        let synchronized = parse("SPD GRP 3 AT SPD GRP 5").unwrap().unwrap();

        assert!(matches!(
            decimal.action,
            SpeedGroupAction::SetBpm { group, bpm }
                if group.one_based() == 1 && bpm.value() == 128.5
        ));
        assert!(matches!(
            relative.action,
            SpeedGroupAction::AdjustBpm { group, delta }
                if group.one_based() == 2 && delta.value() == -8.25
        ));
        assert!(matches!(
            synchronized.action,
            SpeedGroupAction::Synchronize { source, target }
                if source.one_based() == 3 && target.one_based() == 5
        ));
    }

    #[test]
    fn rejects_out_of_range_and_trailing_tokens() {
        assert!(parse("SPD GRP 0 AT 120").unwrap_err().contains("1-5"));
        assert!(parse("SPD GRP 1 AT 1000").unwrap_err().contains("0.1-999"));
        assert!(
            parse("SPD GRP 1 AT + 4 EXTRA")
                .unwrap_err()
                .contains("unexpected")
        );
        assert!(parse("GROUP 1").unwrap().is_none());
    }
}
