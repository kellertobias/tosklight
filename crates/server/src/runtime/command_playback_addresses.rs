use super::*;

#[derive(Clone, Copy)]
pub(super) struct CommandPlaybackAddress {
    pub(super) playback: u16,
    pub(super) cue: Option<f64>,
    application: light_application::PlaybackAddress,
}

impl CommandPlaybackAddress {
    /// CUE navigation was the last production reader; it now parses its own syntactic address and
    /// lets the Playback application service resolve it. The remaining production callers use only
    /// `playback` and `cue`, so this stays as the characterization of the current-page versus
    /// explicit-page mapping that `parse_update_playback_address` still depends on.
    #[cfg(test)]
    pub(super) fn application_address(self) -> light_application::PlaybackAddress {
        self.application
    }
}

pub(super) fn page_playback(snapshot: &EngineSnapshot, page: u8, slot: u8) -> Result<u16, String> {
    snapshot
        .playback_pages
        .iter()
        .find(|item| item.number == page)
        .and_then(|item| item.slots.get(&slot))
        .copied()
        .ok_or_else(|| format!("page {page} slot {slot} is not assigned"))
}

pub(super) fn parse_playback_address(
    tokens: &[String],
    require_set: bool,
    snapshot: &EngineSnapshot,
) -> Result<(CommandPlaybackAddress, usize), String> {
    let mut index = 0;
    if require_set {
        if tokens.get(index).is_none_or(|token| token != "SET") {
            return Err("playback address must start with SET".into());
        }
        index += 1;
    }
    let first = tokens
        .get(index)
        .ok_or("playback number is required")?
        .parse::<u16>()
        .map_err(|_| "playback number is invalid")?;
    index += 1;
    let (playback, application) = if tokens.get(index).is_some_and(|token| token == ".") {
        index += 1;
        let slot = tokens
            .get(index)
            .ok_or("page playback number is required")?
            .parse::<u8>()
            .map_err(|_| "page playback number is invalid")?;
        index += 1;
        let page = first.try_into().map_err(|_| "page number is invalid")?;
        (
            page_playback(snapshot, page, slot)?,
            light_application::PlaybackAddress::ExplicitPage { page, slot },
        )
    } else {
        (first, light_application::PlaybackAddress::Pool(first))
    };
    let cue = if tokens.get(index).is_some_and(|token| token == "CUE") {
        index += 1;
        let mut cue = tokens
            .get(index)
            .ok_or("CUE requires a cue number")?
            .clone();
        index += 1;
        while tokens.get(index).is_some_and(|token| token == ".") {
            cue.push('.');
            index += 1;
            cue.push_str(tokens.get(index).ok_or("DOT requires another cue part")?);
            index += 1;
        }
        let cue = cue.parse::<f64>().map_err(|_| "cue number is invalid")?;
        if !cue.is_finite() || cue <= 0.0 {
            return Err("cue number must be positive".into());
        }
        Some(cue)
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
    if tokens.first().is_none_or(|token| token != "SET") {
        return Err("playback address must start with SET".into());
    }

    // Update follows the control-surface playback model: SET <slot> addresses
    // that slot on this desk's current page, while SET <page> . <slot> keeps an
    // explicit page stable when the operator changes pages.
    let (explicit, current_slot) = if tokens.get(2).is_some_and(|token| token == ".") {
        (tokens.to_vec(), None)
    } else {
        let slot = tokens
            .get(1)
            .ok_or("playback number is required")?
            .parse::<u8>()
            .map_err(|_| "playback number is invalid")?;
        if slot == 0 || slot > 127 {
            return Err("playback number must be within 1-127".into());
        }
        let mut explicit = vec![
            "SET".to_string(),
            current_page.to_string(),
            ".".to_string(),
            slot.to_string(),
        ];
        explicit.extend(tokens.iter().skip(2).cloned());
        (explicit, Some(slot))
    };
    let (mut address, used) = parse_playback_address(&explicit, true, snapshot)?;
    if used != explicit.len() {
        return Err("unexpected tokens after Update playback target".into());
    }
    if let Some(slot) = current_slot {
        address.application = light_application::PlaybackAddress::CurrentPage { slot };
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
        ..Default::default()
    };
    for value in &programmer.values {
        preset
            .values
            .entry(value.fixture_id)
            .or_default()
            .insert(value.attribute.clone(), value.value.clone());
    }
    for (group, attributes) in &programmer.group_values {
        for (attribute, value) in attributes {
            preset
                .group_values
                .entry(group.clone())
                .or_default()
                .insert(attribute.clone(), value.value.clone());
        }
    }
    preset.retain_family_attributes();
    preset
}

pub(super) fn programmer_cue(
    programmer: &light_programmer::ProgrammerState,
    number: f64,
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
    store: &ShowStore,
    snapshot: &EngineSnapshot,
    playback: u16,
) -> Result<
    (
        light_playback::PlaybackDefinition,
        light_show::VersionedObject,
        light_playback::CueList,
    ),
    String,
> {
    let definition = snapshot
        .playbacks
        .iter()
        .find(|item| item.number == playback)
        .cloned()
        .ok_or_else(|| format!("Cuelist {playback} does not exist"))?;
    let light_playback::PlaybackTarget::CueList { cue_list_id } = definition.target else {
        return Err(format!("Cuelist {playback} does not contain Cues"));
    };
    let id = cue_list_id.0.to_string();
    let object = store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == id)
        .ok_or("Cuelist does not exist")?;
    let cue_list =
        serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())?;
    Ok((definition, object, cue_list))
}
