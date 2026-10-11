use super::*;
#[derive(Clone)]
pub(super) struct CommandPlaybackAddress {
    pub(super) playback: light_playback::PlaybackAddress,
    pub(super) cue: Option<light_playback::CueNumber>,
    application: light_application::PlaybackAddress,
}
impl CommandPlaybackAddress {
    #[cfg(test)]
    pub(super) fn application_address(self) -> light_application::PlaybackAddress {
        self.application
    }
}
pub(super) fn page_playback(
    snapshot: &EngineSnapshot,
    page: u8,
    position: u16,
) -> Result<light_playback::PlaybackAddress, String> {
    let address = light_playback::PlaybackAddress::new(page, position)?;
    if snapshot
        .playback_pages
        .iter()
        .any(|item| item.number == page && item.assignments.contains_key(&position))
    {
        Ok(address)
    } else {
        Err(format!("Playback {address} is not assigned"))
    }
}
pub(super) fn parse_playback_address(
    tokens: &[String],
    require_playback_token: bool,
    snapshot: &EngineSnapshot,
    current_page: u8,
) -> Result<(CommandPlaybackAddress, usize), String> {
    let prefixed;
    let input = if require_playback_token {
        tokens
    } else {
        prefixed = std::iter::once("PBK".to_owned())
            .chain(tokens.iter().cloned())
            .collect::<Vec<_>>();
        prefixed.as_slice()
    };
    let (target, rest) = command_http::playback_address_command::parse(input)?;
    let used = input.len() - rest.len() - usize::from(!require_playback_token);
    let (page, position) = match target {
        command_http::playback_address_command::CommandPlaybackTarget::CurrentPage { position } => {
            (current_page, position)
        }
        command_http::playback_address_command::CommandPlaybackTarget::ExplicitPage {
            page,
            position,
        } => (page, position),
    };
    let playback = page_playback(snapshot, page, position)?;
    let application = match target {
        command_http::playback_address_command::CommandPlaybackTarget::CurrentPage { position } => {
            light_application::PlaybackAddress::CurrentPage { position }
        }
        command_http::playback_address_command::CommandPlaybackTarget::ExplicitPage {
            page,
            position,
        } => light_application::PlaybackAddress::Playback(light_playback::PlaybackAddress::new(
            page, position,
        )?),
    };
    let mut index = used;
    let cue = if tokens.get(index).is_some_and(|token| token == "CUE") {
        index += 1;
        let mut value = tokens
            .get(index)
            .ok_or("CUE requires a Cue number")?
            .clone();
        index += 1;
        while tokens.get(index).is_some_and(|token| token == ".") {
            value.push('.');
            index += 1;
            value.push_str(tokens.get(index).ok_or("DOT requires another Cue part")?);
            index += 1;
        }
        Some(value.parse::<light_playback::CueNumber>()?)
    } else {
        None
    };
    Ok((
        CommandPlaybackAddress {
            playback,
            cue,
            application,
        },
        index,
    ))
}
pub(super) fn parse_update_playback_address(
    tokens: &[String],
    current_page: u8,
    snapshot: &EngineSnapshot,
) -> Result<CommandPlaybackAddress, String> {
    let (address, used) = parse_playback_address(tokens, true, snapshot, current_page)?;
    if used != tokens.len() {
        return Err("Unexpected tokens after Update Playback target".into());
    }
    Ok(address)
}

pub(super) fn programmer_preset(
    programmer: &light_programmer::ProgrammerState,
    name: String,
    address: light_programmer::PresetAddress,
) -> light_programmer::Preset {
    let mut preset = light_programmer::Preset {
        name,
        family: address.family,
        number: address.number,
        aim_at_fixture_number: None,
        ..Default::default()
    };
    for value in programmer.values.iter() {
        preset
            .values
            .entry(value.fixture_id)
            .or_default()
            .insert(value.attribute.clone(), value.value.clone());
    }
    for (group, attributes) in programmer.group_values.iter() {
        for (attribute, value) in attributes {
            preset
                .group_values
                .entry(group.clone())
                .or_default()
                .insert(attribute.clone(), value.value.clone());
        }
    }
    carry_preset_replacement_metadata(
        &mut preset,
        programmer,
        &programmer.values,
        &programmer.group_values,
    );
    preset.retain_family_attributes();
    preset
}

pub(super) fn carry_preset_replacement_metadata(
    preset: &mut light_programmer::Preset,
    programmer: &light_programmer::ProgrammerState,
    fixtures: &[light_core::TimedValue],
    groups: &HashMap<
        String,
        HashMap<light_core::AttributeKey, light_programmer::GroupProgrammerValue>,
    >,
) {
    for value in fixtures {
        if let Some(projection) = programmer
            .replacement_provenance
            .get(&value.programmer_order)
            .and_then(|map| map.get(&value.fixture_id))
        {
            preset
                .fixture_replacement_projections
                .entry(value.fixture_id)
                .or_default()
                .insert(value.attribute.clone(), projection.clone());
        }
    }
    for (group, attributes) in groups {
        for (attribute, value) in attributes {
            if let Some(map) = programmer
                .replacement_provenance
                .get(&value.programmer_order)
            {
                preset
                    .group_replacement_projections
                    .entry(group.clone())
                    .or_default()
                    .insert(attribute.clone(), map.clone());
            }
        }
    }
}

pub(super) fn programmer_cue(
    programmer: &light_programmer::ProgrammerState,
    number: light_playback::CueNumber,
    timing: CommandTiming,
) -> light_playback::Cue {
    let mut cue = light_playback::Cue::new(number);
    cue.fade_millis = timing.fade_millis.unwrap_or(0);
    cue.trigger = match timing.delay_millis {
        Some(0) => light_playback::CueTrigger::Follow { delay_millis: 0 },
        Some(delay_millis) => light_playback::CueTrigger::Wait { delay_millis },
        None => light_playback::CueTrigger::Manual,
    };
    cue.changes = programmer
        .values
        .iter()
        .map(|value| {
            let mut change = light_playback::CueChange::set(
                value.fixture_id,
                value.attribute.clone(),
                value.value.clone(),
            );
            change.replacement_projection = programmer
                .replacement_provenance
                .get(&value.programmer_order)
                .and_then(|map| map.get(&value.fixture_id))
                .cloned();
            change.preset_reference = programmer
                .preset_provenance
                .get(&value.programmer_order)
                .cloned();
            change.fade_millis = value.fade_millis;
            change.delay_millis = value.delay_millis;
            change
        })
        .collect();
    cue.group_changes = programmer
        .group_values
        .iter()
        .flat_map(|(group, attributes)| {
            attributes
                .iter()
                .map(|(attribute, value)| light_playback::GroupCueChange {
                    replacement_projections: programmer
                        .replacement_provenance
                        .get(&value.programmer_order)
                        .cloned()
                        .unwrap_or_default(),
                    preset_reference: programmer
                        .preset_provenance
                        .get(&value.programmer_order)
                        .cloned(),
                    group_id: group.clone(),
                    attribute: attribute.clone(),
                    value: Some(value.value.clone()),
                    automatic_restore: false,
                    fade_millis: value.fade_millis,
                    delay_millis: value.delay_millis,
                })
        })
        .collect();
    cue
}

pub(super) fn cue_list_for_playback(
    store: &ActiveShowRepository,
    snapshot: &EngineSnapshot,
    address: light_playback::PlaybackAddress,
) -> Result<
    (
        light_playback::PlaybackDefinition,
        light_show::VersionedObject,
        light_playback::CueList,
    ),
    String,
> {
    let definition = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())
        .and_then(|page| page.assignments.get(&address.position()))
        .cloned()
        .ok_or_else(|| format!("Playback {address} does not exist"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("Playback {address} does not contain Cues"));
    };
    let object = store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| {
            object.body.get("id").and_then(serde_json::Value::as_str)
                == Some(&cue_list_id.0.to_string())
        })
        .ok_or("Cuelist does not exist")?;
    let cue_list: light_playback::CueList =
        serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())?;
    Ok((definition, object, cue_list))
}
