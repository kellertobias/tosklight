use light_playback::CueNumber;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum CueLinkAddress {
    Selected,
    Pool { playback_number: u16 },
    PageSlot { page: u8, slot: u8 },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CueLinkCommand {
    pub address: CueLinkAddress,
    pub source_number: CueNumber,
    pub destination_number: CueNumber,
    pub delay_millis: u64,
}

pub(super) fn parse(command: &str) -> Result<Option<CueLinkCommand>, String> {
    let (tokens, timing) = super::super::tokenize_programmer_command(command)?;
    if tokens.first().is_none_or(|token| token != "LINK") {
        return Ok(None);
    }
    let at = tokens
        .iter()
        .position(|token| token == "AT")
        .ok_or("LINK requires AT CUE <destination>")?;
    let (address, source_number) = source(&tokens[1..at])?;
    let destination_number = cue_number(&tokens[at + 1..], "Link destination")?;
    Ok(Some(CueLinkCommand {
        address,
        source_number,
        destination_number,
        delay_millis: timing.delay_millis.unwrap_or(0),
    }))
}

fn source(tokens: &[String]) -> Result<(CueLinkAddress, CueNumber), String> {
    if tokens.first().is_some_and(|token| token == "CUE") {
        return Ok((CueLinkAddress::Selected, cue_number(tokens, "Link source")?));
    }
    if tokens.first().is_none_or(|token| token != "SET") {
        return Err("LINK requires CUE or SET playback CUE source".into());
    }
    let first = number::<u16>(tokens.get(1), "playback number")?;
    let (address, cue_index) = if tokens.get(2).is_some_and(|token| token == ".") {
        (
            CueLinkAddress::PageSlot {
                page: u8::try_from(first).map_err(|_| "page number is invalid")?,
                slot: number::<u8>(tokens.get(3), "page playback number")?,
            },
            4,
        )
    } else {
        (
            CueLinkAddress::Pool {
                playback_number: first,
            },
            2,
        )
    };
    Ok((address, cue_number(&tokens[cue_index..], "Link source")?))
}

fn cue_number(tokens: &[String], label: &str) -> Result<CueNumber, String> {
    if tokens.first().is_none_or(|token| token != "CUE") {
        return Err(format!("{label} requires CUE <cue-number>"));
    }
    let number = tokens
        .get(1)
        .ok_or_else(|| format!("{label} is required"))?;
    let mut text = number.clone();
    let mut index = 2;
    while tokens.get(index).is_some_and(|token| token == ".") {
        text.push('.');
        text.push_str(
            tokens
                .get(index + 1)
                .ok_or_else(|| format!("{label} decimal is incomplete"))?,
        );
        index += 2;
    }
    if index != tokens.len() {
        return Err(format!("unexpected tokens after {label}"));
    }
    text.parse()
        .map_err(|error: String| format!("{label} is invalid: {error}"))
}

fn number<T: std::str::FromStr>(value: Option<&String>, label: &str) -> Result<T, String> {
    value
        .ok_or_else(|| format!("{label} is required"))?
        .parse()
        .map_err(|_| format!("{label} is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selected_pool_page_and_delay() {
        let selected = parse("LINK CUE 1 AT CUE 3").unwrap().unwrap();
        assert_eq!(selected.address, CueLinkAddress::Selected);
        let pool = parse("LINK SET 7 CUE 1.5 AT CUE 8 DELAY 2")
            .unwrap()
            .unwrap();
        assert_eq!(pool.delay_millis, 2_000);
        assert_eq!(pool.source_number.to_string(), "1.5");
        let page = parse("LINK SET 2.3 CUE 1 AT CUE 4").unwrap().unwrap();
        assert_eq!(page.address, CueLinkAddress::PageSlot { page: 2, slot: 3 });
    }

    #[test]
    fn preserves_deep_paths_and_rejects_leading_zeroes() {
        let parsed = parse("LINK CUE 2.0.15 AT CUE 7.3.1").unwrap().unwrap();
        assert_eq!(parsed.source_number.to_string(), "2.0.15");
        assert_eq!(parsed.destination_number.to_string(), "7.3.1");
        assert!(parse("LINK CUE 02 AT CUE 3").is_err());
    }
}
