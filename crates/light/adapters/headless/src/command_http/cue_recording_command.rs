use light_application::{
    CueNumber, ProgrammingCueRecordOperation, ProgrammingCueRecordTarget,
    ProgrammingCueRecordTiming,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CueRecordCommand {
    pub target: ProgrammingCueRecordTarget,
    pub operation: ProgrammingCueRecordOperation,
    pub cue_number: Option<CueNumber>,
    pub timing: ProgrammingCueRecordTiming,
}

pub(super) fn parse(command: &str) -> Result<Option<CueRecordCommand>, String> {
    let (tokens, timing) = super::super::tokenize_programmer_command(command)?;
    let Some((first, body)) = tokens.split_first() else {
        return Ok(None);
    };
    if !matches!(first.as_str(), "RECORD" | "REC") {
        return Ok(None);
    }
    let (operation, body) = operation(body);
    if body
        .first()
        .is_none_or(|token| !matches!(token.as_str(), "CUE" | "SET"))
    {
        return Ok(None);
    }
    let (target, cue_number) = target(body)?;
    Ok(Some(CueRecordCommand {
        target,
        operation,
        cue_number,
        timing: ProgrammingCueRecordTiming {
            fade_millis: timing.fade_millis,
            delay_millis: timing.delay_millis,
        },
    }))
}

fn operation(body: &[String]) -> (ProgrammingCueRecordOperation, &[String]) {
    match body.first().map(String::as_str) {
        Some("+") => (ProgrammingCueRecordOperation::Merge, &body[1..]),
        Some("-") => (ProgrammingCueRecordOperation::Subtract, &body[1..]),
        _ => (ProgrammingCueRecordOperation::Overwrite, body),
    }
}

fn target(body: &[String]) -> Result<(ProgrammingCueRecordTarget, Option<CueNumber>), String> {
    match body.first().map(String::as_str) {
        Some("CUE") => Ok((
            ProgrammingCueRecordTarget::SelectedPlayback,
            Some(cue_number(&body[1..])?),
        )),
        Some("SET") => set_target(&body[1..]),
        _ => Err("expected a CUE or SET recording target".into()),
    }
}

fn set_target(body: &[String]) -> Result<(ProgrammingCueRecordTarget, Option<CueNumber>), String> {
    let first = body.first().ok_or("playback or page number is required")?;
    if body.get(1).is_some_and(|token| token == ".") {
        let page = bounded_u8(first, "page number")?;
        let slot = bounded_u8(
            body.get(2).ok_or("page playback number is required")?,
            "page playback number",
        )?;
        return Ok((
            ProgrammingCueRecordTarget::PageSlot { page, slot },
            optional_cue(&body[3..])?,
        ));
    }
    let playback_number = first
        .parse::<u16>()
        .map_err(|_| "playback number is invalid")?;
    if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) {
        return Err("Cuelist number must be within 1-1000".into());
    }
    Ok((
        ProgrammingCueRecordTarget::Pool { playback_number },
        optional_cue(&body[1..])?,
    ))
}

fn optional_cue(tokens: &[String]) -> Result<Option<CueNumber>, String> {
    if tokens.is_empty() {
        return Ok(None);
    }
    if tokens.first().is_none_or(|token| token != "CUE") {
        return Err("unexpected tokens after cue target".into());
    }
    cue_number(&tokens[1..]).map(Some)
}

fn cue_number(tokens: &[String]) -> Result<CueNumber, String> {
    let value = super::super::parse_command_cue_number(tokens)?;
    Ok(CueNumber::new(value))
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

    #[test]
    fn parses_selected_pool_and_empty_page_targets() {
        let selected = parse("RECORD CUE 2.5").unwrap().unwrap();
        assert_eq!(
            selected.target,
            ProgrammingCueRecordTarget::SelectedPlayback
        );
        assert_eq!(selected.cue_number.unwrap().value(), 2.5);

        let pool = parse("REC SET 27").unwrap().unwrap();
        assert_eq!(
            pool.target,
            ProgrammingCueRecordTarget::Pool {
                playback_number: 27
            }
        );
        assert!(pool.cue_number.is_none());

        let page = parse("RECORD SET 2.7 CUE 3").unwrap().unwrap();
        assert_eq!(
            page.target,
            ProgrammingCueRecordTarget::PageSlot { page: 2, slot: 7 }
        );
    }

    #[test]
    fn parses_operation_and_timing_without_leaking_them_into_target() {
        let parsed = parse("RECORD + SET 27 CUE 2.5 TIME 3 DELAY 1.25")
            .unwrap()
            .unwrap();
        assert_eq!(parsed.operation, ProgrammingCueRecordOperation::Merge);
        assert_eq!(parsed.timing.fade_millis, Some(3_000));
        assert_eq!(parsed.timing.delay_millis, Some(1_250));
    }

    #[test]
    fn leaves_non_cue_recording_grammar_for_other_adapters() {
        assert!(parse("RECORD GROUP 7").unwrap().is_none());
        assert!(parse("RECORD 2.7").unwrap().is_none());
        assert!(parse("GROUP 1 AT 50").unwrap().is_none());
    }

    #[test]
    fn rejects_trailing_or_incomplete_cue_grammar() {
        assert!(parse("RECORD CUE").is_err());
        assert!(parse("RECORD SET 2.7 EXTRA").is_err());
        assert!(parse("RECORD SET 0").is_err());
    }
}
