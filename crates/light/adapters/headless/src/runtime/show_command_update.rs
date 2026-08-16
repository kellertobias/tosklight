use super::command_http::playback_address_command::{self, CommandPlaybackTarget};
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandUpdateMode {
    Update,
    Tracked,
    Known,
    All,
}

impl CommandUpdateMode {
    const fn cue(self) -> update::CueUpdateMode {
        match self {
            Self::Update => update::CueUpdateMode::ExistingInCurrentCue,
            Self::Tracked => update::CueUpdateMode::ExistingOnly,
            Self::Known => update::CueUpdateMode::AddToCurrentCue,
            Self::All => update::CueUpdateMode::AddNew,
        }
    }

    fn existing_content(self) -> Result<update::ExistingContentMode, String> {
        match self {
            Self::Update => Ok(update::ExistingContentMode::UpdateExisting),
            Self::All => Ok(update::ExistingContentMode::AddNew),
            Self::Tracked | Self::Known => Err(
                "UPDATE TRACKED and UPDATE KNOWN apply only to Cues; use UPDATE or UPDATE ALL for this target"
                    .into(),
            ),
        }
    }
}

fn parse_mode(body: &[String]) -> (CommandUpdateMode, &[String]) {
    match body.first().map(String::as_str) {
        Some("TRACKED") => (CommandUpdateMode::Tracked, &body[1..]),
        Some("KNOWN") => (CommandUpdateMode::Known, &body[1..]),
        Some("ALL") if body.get(1).is_none_or(|token| token != "PRESET") => {
            (CommandUpdateMode::All, &body[1..])
        }
        _ => (CommandUpdateMode::Update, body),
    }
}

fn playback_definition(
    snapshot: &EngineSnapshot,
    number: u16,
) -> Option<&light_playback::PlaybackDefinition> {
    snapshot
        .playbacks
        .iter()
        .find(|definition| definition.number == number)
        .or_else(|| {
            snapshot
                .playback_pages
                .iter()
                .find_map(|page| page.virtual_playbacks.get(&number))
        })
}

fn playback_cue_list_id(
    snapshot: &EngineSnapshot,
    number: u16,
) -> Result<light_core::CueListId, String> {
    let definition = playback_definition(snapshot, number)
        .ok_or_else(|| format!("playback {number} does not exist"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("playback {number} is not assigned to a Cuelist"));
    };
    Ok(cue_list_id)
}

fn playback_number(
    state: &AppState,
    session: &Session,
    snapshot: &EngineSnapshot,
    target: CommandPlaybackTarget,
) -> Result<u16, String> {
    match target {
        CommandPlaybackTarget::CurrentPage { slot } => {
            let show = state
                .active_show
                .current()
                .clone()
                .ok_or("no show is open")?;
            let page = state
                .installation
                .desk_page(session.desk.id, show.id)
                .unwrap_or(1);
            page_playback(snapshot, page, slot)
        }
        CommandPlaybackTarget::ExplicitPage { page, slot } => page_playback(snapshot, page, slot),
        CommandPlaybackTarget::Virtual(address) => Ok(address.number().get()),
    }
}

fn parse_cue_number(tokens: &[String]) -> Result<light_playback::CueNumber, String> {
    if tokens.is_empty() {
        return Err("CUE requires a Cue number".into());
    }
    let mut expected_number = true;
    let mut value = String::new();
    for token in tokens {
        if expected_number {
            if token.is_empty() || !token.chars().all(|character| character.is_ascii_digit()) {
                return Err("Cue number parts must be integers".into());
            }
            value.push_str(token);
        } else if token == "." {
            value.push('.');
        } else {
            return Err("unexpected tokens after Cue number".into());
        }
        expected_number = !expected_number;
    }
    if expected_number {
        return Err("DOT requires another Cue number part".into());
    }
    value
        .parse::<light_playback::CueNumber>()
        .map_err(|error| format!("Cue number is invalid: {error}"))
}

fn explicit_cue_request(
    snapshot: &EngineSnapshot,
    cue_list_id: light_core::CueListId,
    playback_number: Option<u16>,
    number: light_playback::CueNumber,
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let cue_list = snapshot
        .cue_lists
        .iter()
        .find(|cue_list| cue_list.id == cue_list_id)
        .ok_or("the Cuelist does not exist")?;
    let cue = cue_list
        .cues
        .iter()
        .find(|cue| cue.number == number)
        .ok_or_else(|| format!("Cue {number} does not exist in {}", cue_list.name))?;
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_number,
            cue_id: Some(cue.id),
            cue_number: Some(cue.number.to_string()),
            validate_active_context: false,
        },
        mode: update::UpdateMode::Cue(mode.cue()),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    })
}

fn selected_cue_request(
    state: &AppState,
    session: &Session,
    snapshot: &EngineSnapshot,
    tokens: &[String],
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or("no show is open")?;
    let playback = state
        .installation
        .selected_playback(session.desk.id, show.id)
        .map_err(|error| error.to_string())?
        .ok_or("no Cuelist is selected; select one with CUELIST or PBK")?;
    explicit_cue_request(
        snapshot,
        playback_cue_list_id(snapshot, playback)?,
        Some(playback),
        parse_cue_number(tokens)?,
        mode,
    )
}

fn cuelist_cue_request(
    snapshot: &EngineSnapshot,
    tokens: &[String],
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let number = tokens
        .first()
        .ok_or("CUELIST requires a Cuelist number")?
        .parse::<u16>()
        .map_err(|_| "Cuelist number is invalid")?;
    if tokens.get(1).is_none_or(|token| token != "CUE") {
        return Err("UPDATE CUELIST requires CUE <Cue-number>".into());
    }
    explicit_cue_request(
        snapshot,
        playback_cue_list_id(snapshot, number)?,
        Some(number),
        parse_cue_number(&tokens[2..])?,
        mode,
    )
}

fn playback_cue_request(
    state: &AppState,
    session: &Session,
    snapshot: &EngineSnapshot,
    tokens: &[String],
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let (target, rest) = playback_address_command::parse(tokens)?;
    if !rest.is_empty() {
        return Err("PBK and VPBK Update the current Cue; remove the extra Cue address".into());
    }
    let playback = playback_number(state, session, snapshot, target)?;
    let cue_list_id = playback_cue_list_id(snapshot, playback)?;
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_number: Some(playback),
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        },
        mode: update::UpdateMode::Cue(mode.cue()),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    })
}

fn preset_address(tokens: &[String]) -> Result<light_programmer::PresetAddress, String> {
    let [family, keyword, number] = tokens else {
        return Err("Preset Update must be <family> PRESET <number>".into());
    };
    if keyword != "PRESET" {
        return Err("Preset Update must include PRESET before its number".into());
    }
    let family = match family.as_str() {
        "ALL" => light_programmer::PresetFamily::Mixed,
        "INTENSITY" => light_programmer::PresetFamily::Intensity,
        "COLOR" => light_programmer::PresetFamily::Color,
        "POSITION" => light_programmer::PresetFamily::Position,
        "BEAM" => light_programmer::PresetFamily::Beam,
        _ => return Err(format!("unknown Preset family {family}")),
    };
    light_programmer::PresetAddress::new(
        family,
        number
            .parse::<u32>()
            .map_err(|_| "Preset number is invalid")?,
    )
}

fn object_update_request(
    tokens: &[String],
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let (family, object_id) = if tokens.first().is_some_and(|token| token == "GROUP") {
        if tokens.len() != 2 {
            return Err("expected UPDATE GROUP <group-number>".into());
        }
        (UpdateApiTargetFamily::Group, tokens[1].clone())
    } else {
        (
            UpdateApiTargetFamily::Preset,
            preset_address(tokens)?.storage_key(),
        )
    };
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family,
            object_id: Some(object_id),
            playback_number: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        },
        mode: update::UpdateMode::ExistingContent(mode.existing_content()?),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision: None,
    })
}

pub(super) fn update_request(
    state: &AppState,
    session: &Session,
    body: &[String],
    snapshot: &EngineSnapshot,
) -> Result<UpdateApiRequest, String> {
    let (mode, target) = parse_mode(body);
    match target.first().map(String::as_str) {
        None => Err("UPDATE requires CUE, CUELIST, PBK, VPBK, GROUP, or a Preset address".into()),
        Some("CUE") => selected_cue_request(state, session, snapshot, &target[1..], mode),
        Some("CUELIST") => cuelist_cue_request(snapshot, &target[1..], mode),
        Some("PBK" | "VPBK") => playback_cue_request(state, session, snapshot, target, mode),
        _ => object_update_request(target, mode),
    }
}

pub(super) fn execute_update_show_command(
    state: &AppState,
    session: &Session,
    body: &[String],
    snapshot: &EngineSnapshot,
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    let request = update_request(state, session, body, snapshot)?;
    perform_update_from(state, session, &request, context)
        .map(|result| result.changed_count)
        .map_err(|error| error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_manual_cue_modes_to_the_existing_typed_service() {
        assert_eq!(
            CommandUpdateMode::Update.cue(),
            update::CueUpdateMode::ExistingInCurrentCue
        );
        assert_eq!(
            CommandUpdateMode::Tracked.cue(),
            update::CueUpdateMode::ExistingOnly
        );
        assert_eq!(
            CommandUpdateMode::Known.cue(),
            update::CueUpdateMode::AddToCurrentCue
        );
        assert_eq!(CommandUpdateMode::All.cue(), update::CueUpdateMode::AddNew);
    }

    #[test]
    fn accepts_only_named_preset_grammar_and_rejects_cue_only_modes_for_presets() {
        assert_eq!(
            preset_address(&["COLOR".into(), "PRESET".into(), "22".into()]).unwrap(),
            light_programmer::PresetAddress::new(light_programmer::PresetFamily::Color, 22)
                .unwrap()
        );
        assert!(preset_address(&["2".into(), ".".into(), "22".into()]).is_err());
        let mixed = ["ALL".into(), "PRESET".into(), "3".into()];
        assert_eq!(parse_mode(&mixed), (CommandUpdateMode::Update, &mixed[..]));
        let add_mixed = ["ALL".into(), "ALL".into(), "PRESET".into(), "3".into()];
        assert_eq!(
            parse_mode(&add_mixed),
            (CommandUpdateMode::All, &add_mixed[1..])
        );
        assert!(CommandUpdateMode::Tracked.existing_content().is_err());
        assert!(CommandUpdateMode::Known.existing_content().is_err());
    }
}
