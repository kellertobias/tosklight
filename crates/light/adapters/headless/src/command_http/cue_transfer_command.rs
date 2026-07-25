use light_application::{
    CueNumber, CueTransferOperation, ProgrammingCueTransferAddress,
    ProgrammingCueTransferChoiceRequest, ProgrammingCueTransferEndpoint,
    ProgrammingCueTransferMode,
};
use light_core::ShowId;

pub(crate) struct ParsedCueTransferCommand {
    pub request: ProgrammingCueTransferChoiceRequest,
    pub mode: Option<ProgrammingCueTransferMode>,
}

pub(crate) fn is_cue_transfer(command: &str) -> bool {
    let tokens = tokens(command);
    if operation(tokens.first().map(String::as_str)).is_none() {
        return false;
    }
    let body = match tokens.get(1).map(String::as_str) {
        Some("PLAIN" | "STATUS") => &tokens[2..],
        _ => &tokens[1..],
    };
    body.first().is_some_and(|token| token == "SET")
}

pub(crate) fn parse(
    command: &str,
    show_id: ShowId,
) -> Result<Option<ParsedCueTransferCommand>, String> {
    let tokens = tokens(command);
    let Some((operation, operation_token)) = operation(tokens.first().map(String::as_str)) else {
        return Ok(None);
    };
    let (mode, body) = transfer_mode(&tokens[1..]);
    let at = body
        .iter()
        .position(|token| token == "AT")
        .ok_or("MOVE and COPY require AT and a destination")?;
    let source = endpoint(&body[..at], "source")?;
    let destination = endpoint(&body[at + 1..], "destination")?;
    let suffix = body.join(" ");
    Ok(Some(ParsedCueTransferCommand {
        request: ProgrammingCueTransferChoiceRequest {
            show_id,
            operation,
            source,
            destination,
            command: command.to_owned(),
            plain_command: format!("{operation_token} PLAIN {suffix}"),
            status_command: format!("{operation_token} STATUS {suffix}"),
        },
        mode,
    }))
}

fn endpoint(tokens: &[String], label: &str) -> Result<ProgrammingCueTransferEndpoint, String> {
    if tokens.first().is_none_or(|token| token != "SET") {
        return Err(format!("Cue transfer {label} must start with SET"));
    }
    let first = number::<u16>(tokens.get(1), "playback number")?;
    let (address, cue_index) = if tokens.get(2).is_some_and(|token| token == ".") {
        let page = u8::try_from(first).map_err(|_| "page number is invalid")?;
        let slot = number::<u8>(tokens.get(3), "page playback number")?;
        (ProgrammingCueTransferAddress::PageSlot { page, slot }, 4)
    } else {
        (
            ProgrammingCueTransferAddress::Pool {
                playback_number: first,
            },
            2,
        )
    };
    if tokens.get(cue_index).is_none_or(|token| token != "CUE") {
        return Err(format!("Cue transfer {label} requires CUE <cue-number>"));
    }
    let cue_number = decimal(&tokens[cue_index + 1..])?;
    Ok(ProgrammingCueTransferEndpoint {
        address,
        cue_number: CueNumber::new(cue_number),
    })
}

fn decimal(tokens: &[String]) -> Result<f64, String> {
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
        return Err("unexpected tokens after Cue transfer address".into());
    }
    value
        .parse::<f64>()
        .map_err(|_| "cue number is invalid".into())
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
        .map(|token| token.to_ascii_uppercase())
        .collect()
}

fn operation(value: Option<&str>) -> Option<(CueTransferOperation, &'static str)> {
    match value? {
        "COPY" | "CPY" => Some((CueTransferOperation::Copy, "COPY")),
        "MOVE" | "MOV" => Some((CueTransferOperation::Move, "MOVE")),
        _ => None,
    }
}

fn transfer_mode(tokens: &[String]) -> (Option<ProgrammingCueTransferMode>, &[String]) {
    match tokens.first().map(String::as_str) {
        Some("PLAIN") => (Some(ProgrammingCueTransferMode::Plain), &tokens[1..]),
        Some("STATUS") => (Some(ProgrammingCueTransferMode::Status), &tokens[1..]),
        _ => (None, tokens),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pool_and_explicit_page_addresses_without_resolving_them() {
        let parsed = parse("CPY SET 7 CUE 1.2 AT SET 2.3 CUE 4", ShowId::new())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.request.operation, CueTransferOperation::Copy);
        assert_eq!(parsed.request.source.cue_number.value(), 1.2);
        assert!(matches!(
            parsed.request.destination.address,
            ProgrammingCueTransferAddress::PageSlot { page: 2, slot: 3 }
        ));
        assert_eq!(parsed.mode, None);
    }

    #[test]
    fn parses_explicit_transfer_mode() {
        let parsed = parse("MOVE STATUS SET 1 CUE 2 AT SET 2 CUE 3", ShowId::new())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.mode, Some(ProgrammingCueTransferMode::Status));
    }

    #[test]
    fn leaves_unrelated_programmer_commands_to_their_owner() {
        assert!(!is_cue_transfer("FIXTURE 1"));
        assert!(!is_cue_transfer("COPY PRESET 2.1 AT 2"));
        assert!(!is_cue_transfer("MOVE 2.1 AT 2"));
        assert!(is_cue_transfer("COPY SET 1 CUE 1 AT SET 2 CUE 2"));
        assert!(is_cue_transfer("MOVE STATUS SET 1 CUE 1 AT SET 2 CUE 2"));
    }
}
