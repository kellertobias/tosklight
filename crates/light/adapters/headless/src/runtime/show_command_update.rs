use super::command_http::playback_address_command::{self, CommandPlaybackTarget};
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandUpdateMode {
    Update,
    Tracked,
    Known,
    All,
    /// Add Existing: Cues gain values for attributes the Cuelist already knows; Presets and
    /// Groups change only what they already store.
    AddExisting,
}

impl CommandUpdateMode {
    const fn cue(self) -> update::CueUpdateMode {
        match self {
            Self::Update => update::CueUpdateMode::ExistingInCurrentCue,
            Self::Tracked => update::CueUpdateMode::ExistingOnly,
            Self::Known => update::CueUpdateMode::AddToCurrentCue,
            Self::All => update::CueUpdateMode::AddNew,
            Self::AddExisting => update::CueUpdateMode::AddToCurrentCue,
        }
    }

    fn existing_content(self) -> Result<update::ExistingContentMode, String> {
        match self {
            Self::Update | Self::AddExisting => Ok(update::ExistingContentMode::UpdateExisting),
            Self::All => Ok(update::ExistingContentMode::AddNew),
            Self::Tracked | Self::Known => Err(
                "UPDATE TRACKED and UPDATE KNOWN apply only to Cues; use UPDATE or UPDATE ALL for this target"
                    .into(),
            ),
        }
    }
}

/// The mode a Record/Update option stands for. Add Cue is carried out as a Cue recording before
/// an Update is planned, so a target that still reaches here (a Preset or Group) is updated as
/// Smart would update it.
const fn option_mode(option: update::RecordUpdateOption) -> CommandUpdateMode {
    match option {
        update::RecordUpdateOption::Smart | update::RecordUpdateOption::AddCue => {
            CommandUpdateMode::Update
        }
        update::RecordUpdateOption::Merge => CommandUpdateMode::All,
        update::RecordUpdateOption::AddExisting => CommandUpdateMode::AddExisting,
    }
}

fn parse_mode(
    body: &[String],
    default: update::RecordUpdateOption,
) -> (CommandUpdateMode, &[String]) {
    match body.first().map(String::as_str) {
        Some("TRACKED") => (CommandUpdateMode::Tracked, &body[1..]),
        Some("KNOWN") => (CommandUpdateMode::Known, &body[1..]),
        Some("ALL")
            if body.get(1).is_none_or(|token| {
                token != "PRESET" && !token.chars().all(|character| character.is_ascii_digit())
            }) =>
        {
            (CommandUpdateMode::All, &body[1..])
        }
        _ => {
            let (option, body) = super::command_http::record_update_option::parse_option(body);
            (option_mode(option.unwrap_or(default)), body)
        }
    }
}

fn playback_definition(
    snapshot: &EngineSnapshot,
    address: light_playback::PlaybackAddress,
) -> Option<&light_playback::PlaybackDefinition> {
    snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.assignments.get(&address.position()))
}
pub(super) fn cuelist_pool_id(
    snapshot: &EngineSnapshot,
    number: u16,
) -> Result<light_core::CueListId, String> {
    let assignments: Vec<_> = snapshot
        .playback_pages
        .iter()
        .flat_map(|page| {
            page.assignments.iter().map(move |(position, definition)| {
                (
                    light_playback::PlaybackAddress::new(page.number, *position)
                        .expect("validated snapshot"),
                    definition.clone(),
                )
            })
        })
        .collect();
    light_playback::CueListPoolCatalog::build(&snapshot.cue_lists, &assignments, false)?
        .resolve(number)
        .ok_or_else(|| format!("Cuelist {number} does not exist"))
}
fn playback_cue_list_id(
    snapshot: &EngineSnapshot,
    address: light_playback::PlaybackAddress,
) -> Result<light_core::CueListId, String> {
    let definition = playback_definition(snapshot, address)
        .ok_or_else(|| format!("Playback {address} does not exist"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("Playback {address} is not assigned to a Cuelist"));
    };
    Ok(cue_list_id)
}
fn playback_address(
    state: &AppState,
    session: &Session,
    _snapshot: &EngineSnapshot,
    target: CommandPlaybackTarget,
) -> Result<light_playback::PlaybackAddress, String> {
    let (page, position) = match target {
        CommandPlaybackTarget::CurrentPage { position } => (
            state
                .installation
                .command_page(session.desk.id)
                .map_err(|error| error.to_string())?,
            position,
        ),
        CommandPlaybackTarget::ExplicitPage { page, position } => (page, position),
    };
    light_playback::PlaybackAddress::new(page, position)
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
    playback_address: Option<light_playback::PlaybackAddress>,
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
            playback_address,
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
    if let Some(id) = state
        .installation
        .selected_cue_list(session.desk.id, show.id)
        .map_err(|error| error.to_string())?
    {
        return explicit_cue_request(snapshot, id, None, parse_cue_number(tokens)?, mode);
    }
    let playback = state
        .installation
        .selected_playback(session.desk.id, show.id)
        .map_err(|error| error.to_string())?
        .ok_or("no Cuelist is selected; select one with CUELIST or PBK")?;
    let playback = light_playback::PlaybackAddress::new(playback.page, playback.position)?;
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
        cuelist_pool_id(snapshot, number)?,
        None,
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
        return Err("PBK Update the current Cue; remove the extra Cue address".into());
    }
    let playback = playback_address(state, session, snapshot, target)?;
    let cue_list_id = playback_cue_list_id(snapshot, playback)?;
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family: UpdateApiTargetFamily::Cue,
            object_id: Some(cue_list_id.0.to_string()),
            playback_address: Some(playback),
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
    if tokens.len() == 2 {
        return command_preset_address(tokens);
    }
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
    state: &AppState,
    tokens: &[String],
    mode: CommandUpdateMode,
) -> Result<UpdateApiRequest, String> {
    let (family, object_id, expected_show_revision) =
        if tokens.first().is_some_and(|token| token == "GROUP") {
            if tokens.len() != 2 {
                return Err("expected UPDATE GROUP <group-number>".into());
            }
            (UpdateApiTargetFamily::Group, tokens[1].clone(), None)
        } else {
            let (_, store) = active_show_store(state)?;
            let document = store
                .portable_document()
                .map_err(|error| error.to_string())?;
            let object_id =
                update::resolve_preset_update_address(&document, preset_address(tokens)?)
                    .map_err(|error| error.message)?;
            (
                UpdateApiTargetFamily::Preset,
                object_id,
                Some(document.revision().value()),
            )
        };
    Ok(UpdateApiRequest {
        target: UpdateApiTarget {
            family,
            object_id: Some(object_id),
            playback_address: None,
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        },
        mode: update::UpdateMode::ExistingContent(mode.existing_content()?),
        expected_revision: None,
        expected_programmer_revision: None,
        expected_show_revision,
    })
}

pub(super) fn update_request(
    state: &AppState,
    session: &Session,
    body: &[String],
    snapshot: &EngineSnapshot,
) -> Result<UpdateApiRequest, String> {
    let default = super::command_http::record_update_option::update_default(state);
    let (mode, target) = parse_mode(body, default);
    match target.first().map(String::as_str) {
        None => Err("UPDATE requires CUE, CUELIST, PBK, GROUP, or a Preset address".into()),
        Some("CUE") => selected_cue_request(state, session, snapshot, &target[1..], mode),
        Some("CUELIST") => cuelist_cue_request(snapshot, &target[1..], mode),
        Some("PBK") => playback_cue_request(state, session, snapshot, target, mode),
        _ => object_update_request(state, target, mode),
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

    fn tokens(value: &str) -> Vec<String> {
        value.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn plain_update_follows_the_desk_default_and_named_options_override_it() {
        use update::RecordUpdateOption as Option;
        let body = tokens("PBK 1");
        assert_eq!(
            parse_mode(&body, Option::Smart).0,
            CommandUpdateMode::Update
        );
        assert_eq!(parse_mode(&body, Option::Merge).0, CommandUpdateMode::All);
        assert_eq!(
            parse_mode(&body, Option::AddExisting).0,
            CommandUpdateMode::AddExisting
        );
        let smart = tokens("SMART PBK 1");
        assert_eq!(
            parse_mode(&smart, Option::Merge),
            (CommandUpdateMode::Update, &smart[1..])
        );
        let existing = tokens("ADD EXISTING GROUP 2");
        assert_eq!(
            parse_mode(&existing, Option::Smart),
            (CommandUpdateMode::AddExisting, &existing[2..])
        );
        assert_eq!(
            CommandUpdateMode::AddExisting.cue(),
            update::CueUpdateMode::AddToCurrentCue
        );
        assert_eq!(
            CommandUpdateMode::AddExisting.existing_content(),
            Ok(update::ExistingContentMode::UpdateExisting)
        );
        let tracked = tokens("TRACKED PBK 1");
        assert_eq!(
            parse_mode(&tracked, Option::Merge).0,
            CommandUpdateMode::Tracked
        );
    }

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
        let single_mixed = ["ALL".into(), "3".into()];
        assert_eq!(
            parse_mode(&single_mixed, update::RecordUpdateOption::Smart),
            (CommandUpdateMode::Update, &single_mixed[..])
        );
        assert_eq!(
            preset_address(&single_mixed).unwrap().family,
            light_programmer::PresetFamily::Mixed
        );
        assert_eq!(
            preset_address(&["INTENSITY".into(), "190".into()])
                .unwrap()
                .number,
            190
        );
        let mixed = ["ALL".into(), "PRESET".into(), "3".into()];
        assert_eq!(
            parse_mode(&mixed, update::RecordUpdateOption::Smart),
            (CommandUpdateMode::Update, &mixed[..])
        );
        let add_mixed = ["ALL".into(), "ALL".into(), "PRESET".into(), "3".into()];
        assert_eq!(
            parse_mode(&add_mixed, update::RecordUpdateOption::Smart),
            (CommandUpdateMode::All, &add_mixed[1..])
        );
        assert!(CommandUpdateMode::Tracked.existing_content().is_err());
        assert!(CommandUpdateMode::Known.existing_content().is_err());
    }
}
