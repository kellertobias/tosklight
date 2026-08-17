use super::*;

pub(super) fn execute_set_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    _context: &light_application::ActionContext,
) -> Result<usize, String> {
    if tokens.iter().any(|token| token == "AT")
        || tokens.first().is_some_and(|token| token == "DYNAMIC")
    {
        return Err(
            "SET does not assign objects; use ASSIGN <source> AT PBK <address> or ASSIGN <source> AT VPBK <number>"
                .into(),
        );
    }
    if tokens.first().is_some_and(|token| token == "GROUP") {
        if tokens.len() != 2 {
            return Err("expected SET GROUP <group-number>".into());
        }
        let group_id = &tokens[1];
        if !state
            .output
            .snapshot()
            .groups
            .iter()
            .any(|group| &group.id == group_id)
        {
            return Err(format!("group {group_id} does not exist"));
        }
        emit(
            state,
            "group_configuration_requested",
            serde_json::json!({"group_id":group_id,"desk_id":session.desk.id}),
        );
        return Ok(0);
    }
    if tokens.is_empty() {
        return Err("SET requires an editable value or object".into());
    }
    let snapshot = state.output.snapshot();
    let (address, used) = parse_playback_address(tokens, false, &snapshot)?;
    if used != tokens.len() {
        return Err("unexpected tokens after playback selection".into());
    }
    emit(
        state,
        "playback_configuration_requested",
        serde_json::json!({"playback":address.playback,"cue":address.cue}),
    );
    Ok(0)
}

pub(super) fn execute_assign_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    use crate::runtime::command_http::playback_address_command as address;

    let at = tokens.iter().position(|token| token == "AT");
    let (source, target_tokens) = match at {
        Some(index) if index > 0 => (&tokens[..index], &tokens[index + 1..]),
        Some(_) => return Err("ASSIGN requires a source before AT".into()),
        None => {
            let (target, rest) = address::parse(tokens)?;
            if !rest.is_empty() {
                return Err("unexpected tokens after playback address".into());
            }
            emit_assignment_configuration_request(state, session, target);
            return Ok(0);
        }
    };
    let (target, rest) = address::parse(target_tokens)?;
    if !rest.is_empty() {
        return Err("unexpected tokens after playback address".into());
    }
    if source.first().is_some_and(|token| token == "GROUP") {
        if source.len() != 2 {
            return Err("expected ASSIGN GROUP <number> AT PBK or VPBK <address>".into());
        }
        return assign_group_target(state, session, context, &source[1], target);
    }
    let playback = assignment_playback(state, context, source, target)?;
    configure_assignment_target(state, session, context, target, playback)?;
    Ok(1)
}

fn emit_assignment_configuration_request(
    state: &AppState,
    session: &Session,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) {
    use crate::runtime::command_http::playback_address_command::CommandPlaybackTarget;
    let body = match target {
        CommandPlaybackTarget::CurrentPage { slot } => serde_json::json!({
            "desk_id": session.desk.id,
            "addressing": "current_page",
            "slot": slot,
        }),
        CommandPlaybackTarget::ExplicitPage { page, slot } => serde_json::json!({
            "desk_id": session.desk.id,
            "addressing": "explicit_page",
            "page": page,
            "slot": slot,
        }),
        CommandPlaybackTarget::Virtual(address) => serde_json::json!({
            "desk_id": session.desk.id,
            "addressing": "virtual",
            "page": address.page(),
            "playback": address.number().get(),
        }),
    };
    emit(state, "playback_configuration_requested", body);
}

fn assignment_playback(
    state: &AppState,
    context: &light_application::ActionContext,
    source: &[String],
    target_address: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<light_playback::PlaybackDefinition, String> {
    if source.len() != 2 {
        return Err(
            "expected ASSIGN <CUELIST|DYNAMIC|MACRO|TIMECODE> <number> AT PBK or VPBK <address>"
                .into(),
        );
    }
    let number = source[1]
        .parse::<u16>()
        .map_err(|_| format!("{} number is invalid", source[0]))?;
    let snapshot = state.output.snapshot();
    let (name, color, icon, target) = match source[0].as_str() {
        "CUELIST" => {
            let playback = snapshot
                .playbacks
                .iter()
                .find(|playback| playback.number == number)
                .filter(|playback| {
                    matches!(
                        playback.target,
                        light_playback::PlaybackTarget::CueList { .. }
                    )
                })
                .ok_or_else(|| format!("Cuelist {number} does not exist"))?;
            (
                playback.name.clone(),
                playback.color.clone(),
                playback.presentation_icon.clone(),
                playback.target.clone(),
            )
        }
        "DYNAMIC" => {
            let dynamic = snapshot
                .dynamics
                .iter()
                .find(|dynamic| dynamic.pool_number == number)
                .cloned()
                .ok_or_else(|| format!("Dynamic {number} does not exist"))?;
            if matches!(
                dynamic.target_binding,
                light_dynamics::DynamicTargetBinding::Targetless
            ) {
                return Err("targetless Dynamics cannot be assigned directly to a Playback".into());
            }
            let revision = existing_assignment(state, context, target_address)?
                .and_then(|playback| match playback.target {
                    light_playback::PlaybackTarget::Dynamic { assignment } => {
                        assignment.revision.checked_add(1)
                    }
                    _ => Some(1),
                })
                .ok_or("Dynamic Playback assignment revision is exhausted")?;
            let target = light_playback::PlaybackTarget::Dynamic {
                assignment: light_playback::DynamicPlaybackAssignment {
                    dynamic: light_dynamics::DynamicReference {
                        dynamic_id: Some(dynamic.id),
                        last_known_pool_number: dynamic.pool_number,
                        embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                            definition: Arc::new(dynamic.clone()),
                        },
                    },
                    revision,
                    target_scope: None,
                    fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
                    priority: 0,
                    activation_override: None,
                    resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
                    local_speed_multiplier: light_dynamics::Rational::ONE,
                    learned_duration_millis: None,
                    crossfade_non_intensity: false,
                    auto_off_at_zero: false,
                    auto_off_flash_release: false,
                    auto_off_full_control: true,
                },
            };
            (
                dynamic.name.clone(),
                dynamic.color.clone().unwrap_or_else(|| "#20c997".into()),
                dynamic.icon.clone(),
                target,
            )
        }
        "MACRO" => {
            let (_, store) = active_show_store(state)?;
            let definition = store
                .objects("macro")
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter_map(|object| {
                    serde_json::from_value::<light_application::CommandMacroDefinition>(object.body)
                        .ok()
                })
                .find(|definition| definition.number == number)
                .ok_or_else(|| format!("Macro {number} does not exist"))?;
            (
                definition.name,
                definition
                    .presentation
                    .color
                    .unwrap_or_else(|| "#20c997".into()),
                definition.presentation.icon,
                light_playback::PlaybackTarget::Macro {
                    macro_id: definition.id,
                },
            )
        }
        "TIMECODE" => {
            let (_, store) = active_show_store(state)?;
            let definition = store
                .objects("timecode")
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter_map(|object| {
                    serde_json::from_value::<light_playback::TimecodeDefinition>(object.body).ok()
                })
                .find(|definition| definition.number == u32::from(number))
                .ok_or_else(|| format!("Timecode {number} does not exist"))?;
            (
                definition.name,
                "#20c997".into(),
                None,
                light_playback::PlaybackTarget::Timecode {
                    timecode_id: definition.id,
                },
            )
        }
        _ => {
            return Err(format!(
                "{} cannot be assigned to a Playback; use CUELIST, GROUP, DYNAMIC, MACRO, or TIMECODE",
                source[0]
            ));
        }
    };
    let playback_number = match target_address {
        crate::runtime::command_http::playback_address_command::CommandPlaybackTarget::Virtual(
            address,
        ) => address.number().get(),
        _ => existing_assignment(state, context, target_address)?
            .map(|playback| playback.number)
            .unwrap_or_else(|| next_playback_number(&snapshot)),
    };
    let mut playback = existing_assignment(state, context, target_address)?.unwrap_or_else(|| {
        light_playback::PlaybackDefinition {
            number: playback_number,
            name: name.clone(),
            buttons: light_playback::PlaybackDefinition::default_buttons(&target),
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            footprint: light_playback::PlaybackFootprint::Normal,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: color.clone(),
            flash_release: light_playback::FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: icon.clone(),
            presentation_image: None,
            target: target.clone(),
        }
    });
    playback.number = playback_number;
    playback.name = name;
    playback.color = color;
    playback.presentation_icon = icon;
    playback.target = target;
    playback.reset_incompatible_layout();
    Ok(playback)
}

fn next_playback_number(snapshot: &EngineSnapshot) -> u16 {
    (1..=light_playback::MAX_PLAYBACKS)
        .find(|number| {
            snapshot
                .playbacks
                .iter()
                .all(|playback| playback.number != *number)
        })
        .unwrap_or(light_playback::MAX_PLAYBACKS)
}

fn resolved_target(
    state: &AppState,
    context: &light_application::ActionContext,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<(u8, Option<u8>, Option<u16>), String> {
    use crate::runtime::command_http::playback_address_command::CommandPlaybackTarget;
    Ok(match target {
        CommandPlaybackTarget::CurrentPage { slot } => (
            state
                .installation
                .desk_page(context.desk_id, active_show_store(state)?.0.id)
                .map_err(|error| error.to_string())?,
            Some(slot),
            None,
        ),
        CommandPlaybackTarget::ExplicitPage { page, slot } => (page, Some(slot), None),
        CommandPlaybackTarget::Virtual(address) => {
            (address.page(), None, Some(address.number().get()))
        }
    })
}

fn page_and_playback_objects(
    state: &AppState,
    page: u8,
    slot: Option<u8>,
) -> Result<
    (
        Option<light_show::VersionedObject>,
        Option<light_show::VersionedObject>,
        Option<light_playback::PlaybackPage>,
    ),
    String,
> {
    let (_, store) = active_show_store(state)?;
    let page_object = store
        .objects("playback_page")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == page.to_string());
    let definition = page_object
        .as_ref()
        .map(|object| {
            serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
                .map_err(|error| format!("invalid stored Playback page: {error}"))
        })
        .transpose()?;
    let playback_number = slot.and_then(|slot| {
        definition
            .as_ref()
            .and_then(|definition| definition.slots.get(&slot).copied())
    });
    let playback_object = match playback_number {
        Some(number) => store
            .objects("playback")
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|object| object.id == number.to_string()),
        None => None,
    };
    Ok((page_object, playback_object, definition))
}

fn existing_assignment(
    state: &AppState,
    context: &light_application::ActionContext,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<Option<light_playback::PlaybackDefinition>, String> {
    use crate::runtime::command_http::playback_address_command::CommandPlaybackTarget;
    let snapshot = state.output.snapshot();
    match target {
        CommandPlaybackTarget::CurrentPage { slot } => {
            let page = state
                .installation
                .desk_page(context.desk_id, active_show_store(state)?.0.id)
                .map_err(|error| error.to_string())?;
            let (_, _, definition) = page_and_playback_objects(state, page, Some(slot))?;
            Ok(definition
                .and_then(|page| page.slots.get(&slot).copied())
                .and_then(|number| snapshot.playbacks.iter().find(|item| item.number == number))
                .cloned())
        }
        CommandPlaybackTarget::ExplicitPage { page, slot } => {
            let (_, _, definition) = page_and_playback_objects(state, page, Some(slot))?;
            Ok(definition
                .and_then(|page| page.slots.get(&slot).copied())
                .and_then(|number| snapshot.playbacks.iter().find(|item| item.number == number))
                .cloned())
        }
        CommandPlaybackTarget::Virtual(address) => {
            let (_, _, definition) = page_and_playback_objects(state, address.page(), None)?;
            Ok(definition
                .and_then(|page| page.virtual_playbacks.get(&address.number().get()).cloned()))
        }
    }
}

fn configure_assignment_target(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
    playback: light_playback::PlaybackDefinition,
) -> Result<(), String> {
    let (entry, store) = active_show_store(state)?;
    let show_revision = store
        .portable_revision()
        .map_err(|error| error.to_string())?
        .value();
    let (page, slot, virtual_number) = resolved_target(state, context, target)?;
    let (page_object, playback_object, _) = page_and_playback_objects(state, page, slot)?;
    let action = if let Some(number) = virtual_number {
        light_application::PlaybackTopologyAction::ConfigureVirtual {
            page,
            number,
            expected_page_revision: page_object.as_ref().map_or(0, |object| object.revision),
            expected_page_object_id: page_object.as_ref().map(|object| object.id.clone()),
            playback,
        }
    } else {
        light_application::PlaybackTopologyAction::ConfigureSlot {
            page,
            slot: slot.expect("physical assignment has a slot"),
            expected_page_revision: page_object.as_ref().map_or(0, |object| object.revision),
            expected_page_object_id: page_object.as_ref().map(|object| object.id.clone()),
            expected_playback_revision: playback_object
                .as_ref()
                .map_or(0, |object| object.revision),
            expected_playback_object_id: playback_object.as_ref().map(|object| object.id.clone()),
            playback,
        }
    };
    let request_id = context
        .request_id
        .clone()
        .unwrap_or_else(|| format!("assign-{}", context.correlation_id));
    let envelope = light_application::ActionEnvelope {
        context: context
            .clone()
            .with_request_id(request_id)
            .with_expected_revision(show_revision),
        command: light_application::PlaybackTopologyCommand {
            show_id: entry.id,
            action,
        },
    };
    let ports =
        ServerPlaybackTopologyPorts::within_active_show(state.clone(), session.clone(), entry.id);
    state
        .playback
        .handle_topology(envelope, &ports)
        .map_err(|error| error.message)?;
    Ok(())
}

fn assign_group_target(
    state: &AppState,
    session: &Session,
    context: &light_application::ActionContext,
    group_id: &str,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<usize, String> {
    let (entry, store) = active_show_store(state)?;
    let show_revision = store
        .portable_revision()
        .map_err(|error| error.to_string())?
        .value();
    let group_object = store
        .objects("group")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == group_id)
        .ok_or_else(|| format!("group {group_id} does not exist"))?;
    let (page, slot, virtual_number) = resolved_target(state, context, target)?;
    let (page_object, playback_object, _) = page_and_playback_objects(state, page, slot)?;
    let request_id = context
        .request_id
        .clone()
        .unwrap_or_else(|| format!("assign-group-master-{}", context.correlation_id));
    let address = if let Some(playback_number) = virtual_number {
        light_application::GroupMasterPlaybackAddress::Virtual {
            page,
            playback_number,
            expected_page_revision: page_object.as_ref().map_or(0, |object| object.revision),
            expected_page_object_id: page_object.as_ref().map(|object| object.id.clone()),
        }
    } else {
        light_application::GroupMasterPlaybackAddress::Physical {
            page,
            slot: slot.expect("physical assignment has a slot"),
            expected_page_revision: page_object.as_ref().map_or(0, |object| object.revision),
            expected_page_object_id: page_object.as_ref().map(|object| object.id.clone()),
            expected_playback_revision: playback_object
                .as_ref()
                .map_or(0, |object| object.revision),
            expected_playback_object_id: playback_object.map(|object| object.id),
        }
    };
    let action = light_application::ActionEnvelope {
        context: context
            .clone()
            .with_request_id(request_id)
            .with_expected_revision(show_revision),
        command: light_application::PlaybackTopologyCommand {
            show_id: entry.id,
            action: light_application::PlaybackTopologyAction::AssignGroupMaster {
                group_object_id: group_object.id,
                expected_group_revision: group_object.revision,
                address,
            },
        },
    };
    let ports =
        ServerPlaybackTopologyPorts::within_active_show(state.clone(), session.clone(), entry.id);
    state
        .playback
        .handle_topology(action, &ports)
        .map_err(|error| error.message)?;
    Ok(1)
}
