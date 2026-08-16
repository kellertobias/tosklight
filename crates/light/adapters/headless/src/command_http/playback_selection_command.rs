use super::playback_address_command::{self, CommandPlaybackTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaybackSelectionTarget {
    Playback(CommandPlaybackTarget),
    Cuelist { number: u16 },
}

pub(super) fn parse(command: &str) -> Result<Option<PlaybackSelectionTarget>, String> {
    let (tokens, _timing) = super::super::tokenize_programmer_command(command)?;
    match tokens.first().map(String::as_str) {
        Some("PBK" | "VPBK") => {
            let (target, rest) = playback_address_command::parse(&tokens)?;
            if !rest.is_empty() {
                return Err("unexpected tokens after playback address".into());
            }
            Ok(Some(PlaybackSelectionTarget::Playback(target)))
        }
        Some("CUELIST") => {
            if tokens.len() != 2 {
                return Err("CUELIST requires exactly one Cuelist number".into());
            }
            let number = tokens[1]
                .parse::<u16>()
                .map_err(|_| "Cuelist number is invalid")?;
            if !(1..=light_playback::MAX_PLAYBACKS).contains(&number) {
                return Err("Cuelist number must be within 1-1000".into());
            }
            Ok(Some(PlaybackSelectionTarget::Cuelist { number }))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_manual_selection_roots() {
        assert_eq!(
            parse("PBK 6").unwrap().unwrap(),
            PlaybackSelectionTarget::Playback(CommandPlaybackTarget::CurrentPage { slot: 6 })
        );
        assert_eq!(
            parse("CUELIST 4").unwrap().unwrap(),
            PlaybackSelectionTarget::Cuelist { number: 4 }
        );
        assert!(parse("VPBK 1001").unwrap().is_some());
        assert!(parse("PLAYBACK 6").unwrap().is_none());
        assert!(parse("CUE 2").unwrap().is_none());
        assert!(parse("PBK 6 CUE 2").is_err());
    }
}
