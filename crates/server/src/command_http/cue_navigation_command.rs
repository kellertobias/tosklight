//! The CUE navigation grammar.
//!
//! Parsing is deliberately syntactic: it never reads the engine snapshot, the desk selection, or
//! the show. Resolving a page slot, a selected Playback, or a Cue belongs to the typed Playback
//! application service, so the same parsed command describes HTTP, WebSocket, and OSC input.

/// The addressed Playback, before the desk or page topology resolves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum CueNavigationTarget {
    /// `CUE <cue>` and `CUE CUE <cue>` address this desk's selected Playback.
    SelectedPlayback,
    /// `SET <playback> CUE <cue>` addresses one pool Playback.
    Pool { playback_number: u16 },
    /// `SET <page> . <slot> CUE <cue>` pins an explicit page position.
    ExplicitPage { page: u8, slot: u8 },
}

/// One `CUE`/`CUE CUE` navigation command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CueNavigationCommand {
    /// Two leading Cue keys mean Load; one means Go To. A later `CUE` is only an address separator.
    pub load: bool,
    pub target: CueNavigationTarget,
    pub cue_number: f64,
}

/// Claims the family by its leading key after timing clauses are lifted out, so `DELAY 0.5 CUE 2`
/// is owned by the same parser as `CUE 2`. A command this returns `None` for belongs to another
/// owner, which then reports its own tokenization error.
pub(super) fn parse(command: &str) -> Result<Option<CueNavigationCommand>, String> {
    let Ok((tokens, _timing)) = super::super::tokenize_programmer_command(command) else {
        return Ok(None);
    };
    if tokens.first().is_none_or(|token| token != "CUE") {
        return Ok(None);
    }
    // The second consecutive Cue key is the Load operation, not an address separator.
    let load = tokens.get(1).is_some_and(|token| token == "CUE");
    let body = &tokens[if load { 2 } else { 1 }..];
    if body.first().is_some_and(|token| token == "SET") {
        return explicit(body, load).map(Some);
    }
    Ok(Some(CueNavigationCommand {
        load,
        target: CueNavigationTarget::SelectedPlayback,
        cue_number: super::super::parse_command_cue_number(body)?,
    }))
}

fn explicit(body: &[String], load: bool) -> Result<CueNavigationCommand, String> {
    let first = body
        .get(1)
        .ok_or("playback number is required")?
        .parse::<u16>()
        .map_err(|_| "playback number is invalid")?;
    let (target, index) = if body.get(2).is_some_and(|token| token == ".") {
        let page = u8::try_from(first).map_err(|_| "page number is invalid")?;
        let slot = body
            .get(3)
            .ok_or("page playback number is required")?
            .parse::<u8>()
            .map_err(|_| "page playback number is invalid")?;
        (CueNavigationTarget::ExplicitPage { page, slot }, 4)
    } else {
        (
            CueNavigationTarget::Pool {
                playback_number: first,
            },
            2,
        )
    };
    if body.get(index).is_none_or(|token| token != "CUE") {
        return Err("explicit Cue address requires CUE and a Cue number".into());
    }
    let (cue_tokens, rest) = decimal_cue(&body[index + 1..]);
    if !rest.is_empty() {
        return Err("unexpected tokens after Cue address".into());
    }
    Ok(CueNavigationCommand {
        load,
        target,
        cue_number: super::super::parse_command_cue_number(cue_tokens)?,
    })
}

/// Splits `2 . 5` from any trailing tokens so decimal Cue numbers keep their dotted parts.
fn decimal_cue(tokens: &[String]) -> (&[String], &[String]) {
    let mut index = usize::from(!tokens.is_empty());
    while tokens.get(index).is_some_and(|token| token == ".") {
        index += 2;
    }
    let end = index.min(tokens.len());
    (&tokens[..end], &tokens[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selected_playback_go_to_and_load() {
        assert_eq!(
            parse("CUE 3").unwrap().unwrap(),
            CueNavigationCommand {
                load: false,
                target: CueNavigationTarget::SelectedPlayback,
                cue_number: 3.0,
            }
        );
        assert_eq!(
            parse("CUE CUE 2").unwrap().unwrap(),
            CueNavigationCommand {
                load: true,
                target: CueNavigationTarget::SelectedPlayback,
                cue_number: 2.0,
            }
        );
    }

    #[test]
    fn parses_explicit_pool_and_page_addresses_without_resolving_them() {
        assert_eq!(
            parse("CUE SET 1 CUE 3").unwrap().unwrap().target,
            CueNavigationTarget::Pool { playback_number: 1 }
        );
        let page = parse("CUE CUE SET 1 . 2 CUE 2").unwrap().unwrap();
        assert!(page.load);
        assert_eq!(
            page.target,
            CueNavigationTarget::ExplicitPage { page: 1, slot: 2 }
        );
        assert_eq!(page.cue_number, 2.0);
    }

    #[test]
    fn parses_decimal_cue_numbers_on_both_addressing_forms() {
        assert_eq!(parse("CUE 2.5").unwrap().unwrap().cue_number, 2.5);
        assert_eq!(
            parse("CUE CUE SET 2 . 3 CUE 10.25")
                .unwrap()
                .unwrap()
                .cue_number,
            10.25
        );
    }

    #[test]
    fn leaves_other_families_to_their_owner() {
        assert!(parse("RECORD CUE 2").unwrap().is_none());
        assert!(parse("DELETE SET 1 CUE 2").unwrap().is_none());
        assert!(parse("COPY SET 1 CUE 1 AT SET 2 CUE 2").unwrap().is_none());
        assert!(parse("GROUP 1 AT 50").unwrap().is_none());
    }

    #[test]
    fn owns_the_family_after_lifted_timing_clauses_and_lowercase_input() {
        assert_eq!(parse("cue 3").unwrap().unwrap().cue_number, 3.0);
        assert_eq!(parse("DELAY 0.5 CUE 2").unwrap().unwrap().cue_number, 2.0);
    }

    #[test]
    fn rejects_incomplete_and_trailing_navigation_grammar() {
        assert_eq!(parse("CUE").unwrap_err(), "CUE requires a cue number");
        assert_eq!(parse("CUE CUE").unwrap_err(), "CUE requires a cue number");
        assert_eq!(
            parse("CUE SET 1").unwrap_err(),
            "explicit Cue address requires CUE and a Cue number"
        );
        assert_eq!(
            parse("CUE SET 1 CUE 2 EXTRA").unwrap_err(),
            "unexpected tokens after Cue address"
        );
        assert_eq!(parse("CUE 0").unwrap_err(), "cue number must be positive");
        assert_eq!(parse("CUE ABC").unwrap_err(), "cue number is invalid");
    }
}
