use super::playback_address_command::{self, CommandPlaybackTarget};
use light_application::{CueNumber, ProgrammingCueRecordOperation, ProgrammingCueRecordTiming};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum CueRecordCommandTarget {
    SelectedCuelist,
    Cuelist { number: u16 },
    Playback(CommandPlaybackTarget),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CueRecordCommand {
    pub target: CueRecordCommandTarget,
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
    let Some(root) = body.first().map(String::as_str) else {
        return Ok(None);
    };
    if !matches!(root, "CUE" | "CUELIST" | "PBK" | "VPBK") {
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

fn target(body: &[String]) -> Result<(CueRecordCommandTarget, Option<CueNumber>), String> {
    match body.first().map(String::as_str) {
        Some("CUE") => Ok((
            CueRecordCommandTarget::SelectedCuelist,
            optional_number(&body[1..])?,
        )),
        Some("CUELIST") => {
            let number = body
                .get(1)
                .ok_or("Cuelist number is required")?
                .parse::<u16>()
                .map_err(|_| "Cuelist number is invalid")?;
            if !(1..=light_playback::MAX_PLAYBACKS).contains(&number) {
                return Err("Cuelist number must be within 1-1000".into());
            }
            Ok((
                CueRecordCommandTarget::Cuelist { number },
                trailing_cue(&body[2..])?,
            ))
        }
        Some("PBK" | "VPBK") => {
            let (target, rest) = playback_address_command::parse(body)?;
            Ok((
                CueRecordCommandTarget::Playback(target),
                trailing_cue(rest)?,
            ))
        }
        _ => Err("expected CUE, CUELIST, PBK, or VPBK recording target".into()),
    }
}

fn optional_number(tokens: &[String]) -> Result<Option<CueNumber>, String> {
    if tokens.is_empty() {
        Ok(None)
    } else {
        super::super::parse_command_cue_number(tokens).map(Some)
    }
}

fn trailing_cue(tokens: &[String]) -> Result<Option<CueNumber>, String> {
    if tokens.is_empty() {
        return Ok(None);
    }
    if tokens.first().is_none_or(|token| token != "CUE") {
        return Err("unexpected tokens after recording target".into());
    }
    super::super::parse_command_cue_number(&tokens[1..]).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selected_and_explicit_cuelists() {
        let selected = parse("RECORD CUE").unwrap().unwrap();
        assert_eq!(selected.target, CueRecordCommandTarget::SelectedCuelist);
        assert!(selected.cue_number.is_none());

        let selected_cue = parse("RECORD CUE 2.0.15").unwrap().unwrap();
        assert_eq!(selected_cue.cue_number.unwrap().to_string(), "2.0.15");

        let list = parse("RECORD CUELIST 4 CUE 2.1").unwrap().unwrap();
        assert_eq!(list.target, CueRecordCommandTarget::Cuelist { number: 4 });
        assert_eq!(list.cue_number.unwrap().to_string(), "2.1");
    }

    #[test]
    fn parses_current_explicit_and_virtual_playbacks() {
        assert_eq!(
            parse("RECORD PBK 6").unwrap().unwrap().target,
            CueRecordCommandTarget::Playback(CommandPlaybackTarget::CurrentPage { slot: 6 })
        );
        assert_eq!(
            parse("RECORD PBK 2.6 CUE 3").unwrap().unwrap().target,
            CueRecordCommandTarget::Playback(CommandPlaybackTarget::ExplicitPage {
                page: 2,
                slot: 6,
            })
        );
        assert!(matches!(
            parse("RECORD VPBK 1001").unwrap().unwrap().target,
            CueRecordCommandTarget::Playback(CommandPlaybackTarget::Virtual(_))
        ));
    }

    #[test]
    fn parses_operation_and_timing() {
        let parsed = parse("RECORD + CUELIST 4 CUE 2.5 TIME 3 DELAY 1.25")
            .unwrap()
            .unwrap();
        assert_eq!(parsed.operation, ProgrammingCueRecordOperation::Merge);
        assert_eq!(parsed.timing.fade_millis, Some(3_000));
        assert_eq!(parsed.timing.delay_millis, Some(1_250));
    }

    #[test]
    fn rejects_superseded_set_grammar_and_invalid_paths() {
        assert!(parse("RECORD SET 27").unwrap().is_none());
        assert!(parse("RECORD CUELIST").is_err());
        assert!(parse("RECORD PBK 0").is_err());
        assert!(parse("RECORD CUE 02").is_err());
    }
}
