//! Shared command-line playback addresses from the operator manual.

use light_playback::VirtualPlaybackAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandPlaybackTarget {
    CurrentPage { slot: u8 },
    ExplicitPage { page: u8, slot: u8 },
    Virtual(VirtualPlaybackAddress),
}

pub(crate) fn parse(tokens: &[String]) -> Result<(CommandPlaybackTarget, &[String]), String> {
    match tokens.first().map(String::as_str) {
        Some("PBK") => physical(&tokens[1..]),
        Some("VPBK") => virtual_playback(&tokens[1..]),
        _ => Err("expected PBK or VPBK playback address".into()),
    }
}

fn physical(tokens: &[String]) -> Result<(CommandPlaybackTarget, &[String]), String> {
    let first = bounded_u8(
        tokens.first().ok_or("playback number is required")?,
        "playback number",
    )?;
    if tokens.get(1).is_some_and(|token| token == ".") {
        let slot = bounded_u8(
            tokens.get(2).ok_or("page playback number is required")?,
            "page playback number",
        )?;
        Ok((
            CommandPlaybackTarget::ExplicitPage { page: first, slot },
            &tokens[3..],
        ))
    } else {
        Ok((
            CommandPlaybackTarget::CurrentPage { slot: first },
            &tokens[1..],
        ))
    }
}

fn virtual_playback(tokens: &[String]) -> Result<(CommandPlaybackTarget, &[String]), String> {
    let number = tokens
        .first()
        .ok_or("virtual playback number is required")?
        .parse::<u16>()
        .map_err(|_| "virtual playback number is invalid")?;
    let address = VirtualPlaybackAddress::from_number(number)?;
    Ok((CommandPlaybackTarget::Virtual(address), &tokens[1..]))
}

fn bounded_u8(value: &str, label: &str) -> Result<u8, String> {
    let value = value
        .parse::<u8>()
        .map_err(|_| format!("{label} is invalid"))?;
    if !(1..=127).contains(&value) {
        return Err(format!("{label} must be within 1-127"));
    }
    Ok(value)
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
    fn parses_current_explicit_and_virtual_playbacks() {
        let current = tokens("PBK 6 CUE 2.1");
        let (target, rest) = parse(&current).unwrap();
        assert_eq!(target, CommandPlaybackTarget::CurrentPage { slot: 6 });
        assert_eq!(rest, ["CUE", "2", ".", "1"]);

        let explicit = tokens("PBK 2.6");
        assert_eq!(
            parse(&explicit).unwrap(),
            (
                CommandPlaybackTarget::ExplicitPage { page: 2, slot: 6 },
                &[][..]
            )
        );

        let virtual_tokens = tokens("VPBK 1001");
        let (target, rest) = parse(&virtual_tokens).unwrap();
        assert_eq!(
            target,
            CommandPlaybackTarget::Virtual(VirtualPlaybackAddress::from_number(1001).unwrap())
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn rejects_incomplete_and_out_of_range_addresses() {
        for command in ["PBK", "PBK 0", "PBK 1 .", "PBK 1 . 128", "VPBK 1000"] {
            assert!(parse(&tokens(command)).is_err(), "{command}");
        }
    }
}
