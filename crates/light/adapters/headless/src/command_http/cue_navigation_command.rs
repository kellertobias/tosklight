//! Manual GO TO and LOAD command grammar.

use super::playback_address_command::{self, CommandPlaybackTarget};
use light_playback::CueNumber;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CueNavigationCommand {
    pub load: bool,
    pub target: CommandPlaybackTarget,
    pub cue_number: CueNumber,
}

pub(super) fn parse(command: &str) -> Result<Option<CueNavigationCommand>, String> {
    let Ok((tokens, _timing)) = super::super::tokenize_programmer_command(command) else {
        return Ok(None);
    };
    let (load, body) = match tokens.as_slice() {
        [first, second, rest @ ..] if first == "GO" && second == "TO" => (false, rest),
        [first, rest @ ..] if first == "LOAD" => (true, rest),
        _ => return Ok(None),
    };
    let (target, rest) = playback_address_command::parse(body)?;
    if rest.first().is_none_or(|token| token != "CUE") {
        return Err(format!(
            "{} requires PBK or VPBK followed by CUE and a Cue number",
            if load { "LOAD" } else { "GO TO" }
        ));
    }
    let cue_number = super::super::parse_command_cue_number(&rest[1..])?;
    Ok(Some(CueNavigationCommand {
        load,
        target,
        cue_number,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_playback::VirtualPlaybackAddress;

    #[test]
    fn parses_manual_go_to_and_load_grammar() {
        assert_eq!(
            parse("GO TO PBK 6 CUE 2.1").unwrap().unwrap(),
            CueNavigationCommand {
                load: false,
                target: CommandPlaybackTarget::CurrentPage { slot: 6 },
                cue_number: "2.1".parse().unwrap(),
            }
        );
        assert_eq!(
            parse("LOAD PBK 2.6 CUE 2.1.0").unwrap().unwrap(),
            CueNavigationCommand {
                load: true,
                target: CommandPlaybackTarget::ExplicitPage { page: 2, slot: 6 },
                cue_number: "2.1.0".parse().unwrap(),
            }
        );
        assert_eq!(
            parse("LOAD VPBK 1001 CUE 2").unwrap().unwrap().target,
            CommandPlaybackTarget::Virtual(VirtualPlaybackAddress::from_number(1001).unwrap())
        );
    }

    #[test]
    fn rejects_cuelist_only_and_superseded_navigation_grammar() {
        assert!(parse("GO TO CUELIST 4 CUE 2").is_err());
        assert!(parse("GO TO PBK 6").is_err());
        assert!(parse("LOAD VPLBK 1001 CUE 2").is_err());
        assert!(parse("CUE 2").unwrap().is_none());
        assert!(parse("CUE CUE SET 2 . 6 CUE 2").unwrap().is_none());
    }

    #[test]
    fn leaves_other_families_to_their_owner() {
        assert!(parse("RECORD CUELIST 4").unwrap().is_none());
        assert!(parse("PBK 6").unwrap().is_none());
    }

    #[test]
    fn preserves_arbitrary_integer_component_cue_paths() {
        assert_eq!(
            parse("GO TO PBK 6 CUE 2.0.15")
                .unwrap()
                .unwrap()
                .cue_number
                .to_string(),
            "2.0.15"
        );
        assert!(parse("GO TO PBK 6 CUE 02").is_err());
    }
}
