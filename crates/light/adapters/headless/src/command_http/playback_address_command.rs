//! Shared canonical PBK page.position grammar from the operator manual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandPlaybackTarget {
    CurrentPage { position: u16 },
    ExplicitPage { page: u8, position: u16 },
}
pub(crate) fn parse(tokens: &[String]) -> Result<(CommandPlaybackTarget, &[String]), String> {
    if tokens.first().is_none_or(|token| token != "PBK") {
        return Err("expected PBK playback address".into());
    }
    let first = tokens
        .get(1)
        .ok_or("Playback position is required")?
        .parse::<u16>()
        .map_err(|_| "Playback position is invalid")?;
    if tokens.get(2).is_some_and(|token| token == ".") {
        let page = u8::try_from(first).map_err(|_| "Playback page must be within 1-99")?;
        let position = tokens
            .get(3)
            .ok_or("Playback position is required")?
            .parse::<u16>()
            .map_err(|_| "Playback position is invalid")?;
        light_playback::PlaybackAddress::new(page, position)?;
        Ok((
            CommandPlaybackTarget::ExplicitPage { page, position },
            &tokens[4..],
        ))
    } else {
        light_playback::PlaybackAddress::new(1, first)?;
        Ok((
            CommandPlaybackTarget::CurrentPage { position: first },
            &tokens[2..],
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tokens(value: &str) -> Vec<String> {
        super::super::super::tokenize_programmer_command(value)
            .unwrap()
            .0
    }
    #[test]
    fn current_and_explicit_positions_are_canonical() {
        let input = tokens("PBK 1000 CUE 2.1");
        assert_eq!(
            parse(&input).unwrap(),
            (
                CommandPlaybackTarget::CurrentPage { position: 1000 },
                &input[2..]
            )
        );
        let input = tokens("PBK 99.1000");
        assert_eq!(
            parse(&input).unwrap().0,
            CommandPlaybackTarget::ExplicitPage {
                page: 99,
                position: 1000
            }
        );
    }
    #[test]
    fn invalid_and_legacy_addresses_are_rejected() {
        for input in [
            "PBK",
            "PBK 0",
            "PBK 1001",
            "PBK 0.1",
            "PBK 100.1",
            "PBK 1.1001",
            "PBK 1.",
            "VPBK 1001",
        ] {
            assert!(parse(&tokens(input)).is_err(), "{input}");
        }
    }
}
