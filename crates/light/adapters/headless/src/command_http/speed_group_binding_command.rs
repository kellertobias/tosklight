//! Pure syntax for binding show objects to an authoritative Speed Group.

use light_application::SpeedGroupId;

use super::playback_address_command::{self, CommandPlaybackTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpeedGroupBindingSource {
    CueList(u16),
    Playback(CommandPlaybackTarget),
    Dynamic(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SpeedGroupBindingCommand {
    pub source: SpeedGroupBindingSource,
    pub group: SpeedGroupId,
}

pub(crate) fn parse(command: &str) -> Result<Option<SpeedGroupBindingCommand>, String> {
    let Ok((tokens, _timing)) = super::super::tokenize_programmer_command(command) else {
        return Ok(None);
    };
    if !matches!(
        tokens.first().map(String::as_str),
        Some("CUELIST" | "PBK" | "VPBK" | "DYNAMIC")
    ) {
        return Ok(None);
    }
    let Some(at) = tokens.iter().position(|token| token == "AT") else {
        return Ok(None);
    };
    if at == 0 {
        return Err("Speed Group binding requires a source before AT".into());
    }
    if tokens.get(at + 1).map(String::as_str) != Some("SPD") {
        return Ok(None);
    }
    if tokens[at + 1..].len() != 3 || tokens[at + 1] != "SPD" || tokens[at + 2] != "GRP" {
        return Err("Speed Group binding target must be SPD GRP <1-5>".into());
    }
    let group = parse_group(&tokens[at + 3])?;
    let source = match tokens[0].as_str() {
        "CUELIST" => SpeedGroupBindingSource::CueList(parse_number(&tokens[..at], "Cuelist")?),
        "DYNAMIC" => SpeedGroupBindingSource::Dynamic(parse_number(&tokens[..at], "Dynamic")?),
        "PBK" | "VPBK" => {
            let (target, rest) = playback_address_command::parse(&tokens[..at])?;
            if !rest.is_empty() {
                return Err("unexpected tokens after playback address".into());
            }
            SpeedGroupBindingSource::Playback(target)
        }
        _ => unreachable!("the command root was checked above"),
    };
    Ok(Some(SpeedGroupBindingCommand { source, group }))
}

fn parse_number(tokens: &[String], label: &str) -> Result<u16, String> {
    if tokens.len() != 2 {
        return Err(format!(
            "expected {} <number> AT SPD GRP <1-5>",
            label.to_ascii_uppercase()
        ));
    }
    tokens[1]
        .parse::<u16>()
        .ok()
        .filter(|number| *number > 0)
        .ok_or_else(|| format!("{label} number must be positive"))
}

fn parse_group(token: &str) -> Result<SpeedGroupId, String> {
    let one_based = token
        .parse::<u8>()
        .map_err(|_| "Speed Group number is invalid")?;
    SpeedGroupId::new(one_based).ok_or_else(|| "Speed Group number must be within 1-5".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_documented_binding_sources() {
        assert!(matches!(
            parse("CUELIST 4 AT SPD GRP 2").unwrap().unwrap(),
            SpeedGroupBindingCommand {
                source: SpeedGroupBindingSource::CueList(4),
                group,
            } if group.one_based() == 2
        ));
        assert!(matches!(
            parse("PBK 6 AT SPD GRP 3").unwrap().unwrap().source,
            SpeedGroupBindingSource::Playback(CommandPlaybackTarget::CurrentPage { slot: 6 })
        ));
        assert!(matches!(
            parse("PBK 2 . 6 AT SPD GRP 4").unwrap().unwrap().source,
            SpeedGroupBindingSource::Playback(CommandPlaybackTarget::ExplicitPage {
                page: 2,
                slot: 6
            })
        ));
        assert!(matches!(
            parse("VPBK 1001 AT SPD GRP 5").unwrap().unwrap().source,
            SpeedGroupBindingSource::Playback(CommandPlaybackTarget::Virtual(_))
        ));
        assert!(matches!(
            parse("DYNAMIC 29 AT SPD GRP 1").unwrap().unwrap().source,
            SpeedGroupBindingSource::Dynamic(29)
        ));
    }

    #[test]
    fn rejects_invalid_binding_without_stealing_plain_selection() {
        assert!(parse("PBK 6").unwrap().is_none());
        assert!(parse("SPD GRP 1 AT 120").unwrap().is_none());
        assert!(parse("DYNAMIC 29 SIZE AT 50").unwrap().is_none());
        assert!(parse("CUELIST 4 AT SPD GRP 0").unwrap_err().contains("1-5"));
        assert!(
            parse("DYNAMIC 29 AT SPD GRP 6")
                .unwrap_err()
                .contains("1-5")
        );
        assert!(
            parse("PBK 6 AT SPD GRP 2 EXTRA")
                .unwrap_err()
                .contains("target")
        );
    }
}
