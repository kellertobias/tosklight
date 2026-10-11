use super::playback_address_command::CommandPlaybackTarget;
use light_application::CueNumber;
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CueDeletionCommandAddress {
    CuelistPool { number: u16 },
    Playback(CommandPlaybackTarget),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedCueDeletionCommand {
    pub address: CueDeletionCommandAddress,
    pub cue_number: CueNumber,
}

pub(crate) fn is_cue_deletion(command: &str) -> bool {
    let tokens = tokens(command);
    matches!(tokens.first().map(String::as_str), Some("DELETE" | "DEL"))
        && tokens
            .get(1)
            .is_some_and(|token| matches!(token.as_str(), "PBK" | "CUELIST"))
}

pub(crate) fn parse(command: &str) -> Result<Option<ParsedCueDeletionCommand>, String> {
    if !is_cue_deletion(command) {
        return Ok(None);
    }
    let tokens = tokens(command);
    let first = number::<u16>(tokens.get(2), "playback number")?;
    let (address, cue_index) = if tokens[1] == "CUELIST" {
        if !(1..=1000).contains(&first) {
            return Err("Cuelist number must be within 1-1000".into());
        }
        (CueDeletionCommandAddress::CuelistPool { number: first }, 3)
    } else {
        let (target, rest) = super::playback_address_command::parse(&tokens[1..])?;
        (
            CueDeletionCommandAddress::Playback(target),
            tokens.len() - rest.len(),
        )
    };
    if tokens.get(cue_index).is_none_or(|token| token != "CUE") {
        return Err("Cue deletion requires CUE <cue-number>".into());
    }
    let cue_number = cue_number(&tokens[cue_index + 1..])?;
    Ok(Some(ParsedCueDeletionCommand {
        address,
        cue_number,
    }))
}

fn cue_number(tokens: &[String]) -> Result<CueNumber, String> {
    let whole = tokens.first().ok_or("CUE requires a cue number")?;
    let mut value = whole.clone();
    let mut index = 1;
    while tokens.get(index).is_some_and(|token| token == ".") {
        value.push('.');
        value.push_str(
            tokens
                .get(index + 1)
                .ok_or("DOT requires another cue part")?,
        );
        index += 2;
    }
    if index != tokens.len() {
        return Err("unexpected tokens after Cue deletion address".into());
    }
    value
        .parse()
        .map_err(|error: String| format!("cue number is invalid: {error}"))
}

fn number<T: std::str::FromStr>(value: Option<&String>, label: &str) -> Result<T, String> {
    value
        .ok_or_else(|| format!("{label} is required"))?
        .parse::<T>()
        .map_err(|_| format!("{label} is invalid"))
}

fn tokens(command: &str) -> Vec<String> {
    command
        .replace(',', ".")
        .replace('.', " . ")
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_and_explicit_page_without_live_resolution() {
        let pool = parse("DEL PBK 7 CUE 2,5").unwrap().unwrap();
        assert!(matches!(
            pool.address,
            CueDeletionCommandAddress::Playback(CommandPlaybackTarget::CurrentPage { position: 7 })
        ));
        assert_eq!(pool.cue_number.to_string(), "2.5");
        let page = parse("DELETE PBK 2 . 3 CUE 4").unwrap().unwrap();
        assert!(matches!(
            page.address,
            CueDeletionCommandAddress::Playback(CommandPlaybackTarget::ExplicitPage {
                page: 2,
                position: 3
            })
        ));
    }

    #[test]
    fn claims_malformed_delete_playback_but_not_other_delete_families() {
        assert!(is_cue_deletion("DELETE PBK"));
        assert!(!is_cue_deletion("DELETE GROUP 4"));
        assert!(!is_cue_deletion("DELETE PRESET 2 . 1"));
        assert!(parse("DELETE PBK 1").is_err());
        assert!(parse("DELETE PBK 1 CUE 01").is_err());
    }

    #[test]
    fn cuelist_delete_uses_independent_address_without_reinterpreting_pbk() {
        assert_eq!(
            parse("DELETE CUELIST 101 CUE 2").unwrap().unwrap().address,
            CueDeletionCommandAddress::CuelistPool { number: 101 }
        );
        assert_eq!(
            parse("DELETE PBK 101 CUE 2").unwrap().unwrap().address,
            CueDeletionCommandAddress::Playback(CommandPlaybackTarget::CurrentPage {
                position: 101
            })
        );
    }

    #[test]
    fn preserves_deep_cue_paths() {
        let parsed = parse("DELETE PBK 1 CUE 2.0.15").unwrap().unwrap();
        assert_eq!(parsed.cue_number.to_string(), "2.0.15");
    }
}
