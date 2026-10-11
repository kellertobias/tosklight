use super::*;

pub(super) fn request_cuelist_editor_from_command_line(
    state: &AppState,
    session: &Session,
    number: u16,
) -> Result<(), String> {
    if state.active_show.current().is_none() {
        return Err("no show is open".into());
    }
    let id = super::show_command_update::cuelist_pool_id(&state.output.snapshot(), number)?;
    emit(
        state,
        "desk_action",
        serde_json::json!({
            "action": "open-object-editor",
            "control": "cuelist",
            "value": id.0.to_string(),
            "desk_id": session.desk.id,
        }),
    );
    Ok(())
}

pub(super) fn execute_set_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    _context: &light_application::ActionContext,
) -> Result<usize, String> {
    if tokens.iter().any(|token| token == "AT") {
        return Err("SET does not assign objects; use ASSIGN <source> AT PBK <address>".into());
    }
    if tokens.first().is_some_and(|token| token == "DYNAMIC") {
        let [_, number] = tokens else {
            return Err("expected SET DYNAMIC <pool number>".into());
        };
        let number = number
            .parse::<u16>()
            .ok()
            .filter(|number| (1..=9999).contains(number))
            .ok_or("Dynamic pool number must be between 1 and 9999")?;
        let snapshot = state.output.snapshot();
        let dynamic = snapshot
            .dynamics
            .iter()
            .find(|dynamic| dynamic.pool_number == number)
            .ok_or_else(|| format!("Dynamic {number} does not exist"))?;
        emit(
            state,
            "desk_action",
            serde_json::json!({
                "action": "open-object-editor", "control": "dynamic", "value": dynamic.id.to_string(), "desk_id": session.desk.id,
            }),
        );
        return Ok(0);
    }
    if tokens.first().is_some_and(|token| token == "CUELIST") {
        if tokens.len() != 2 {
            return Err("expected SET CUELIST <positive pool number>".into());
        }
        let number = tokens[1]
            .parse::<u16>()
            .ok()
            .filter(|number| *number > 0)
            .ok_or_else(|| "CUELIST pool number must be between 1 and 65535".to_owned())?;
        request_cuelist_editor_from_command_line(state, session, number)?;
        return Ok(0);
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
    let (target, rest) = command_http::playback_address_command::parse(tokens)?;
    if !rest.is_empty() {
        return Err("unexpected tokens after playback selection".into());
    }
    emit_assignment_configuration_request(state, session, target)?;
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
            emit_assignment_configuration_request(state, session, target)?;
            return Ok(0);
        }
    };
    let (target, rest) = address::parse(target_tokens)?;
    if !rest.is_empty() {
        return Err("unexpected tokens after playback address".into());
    }
    if source.first().is_some_and(|token| token == "GROUP") {
        if source.len() != 2 {
            return Err("expected ASSIGN GROUP <number> AT PBK <address>".into());
        }
        return assign_group_target(state, session, context, &source[1], target);
    }
    let playback = assignment_playback(state, context, source, target)?;
    configure_assignment_target(state, session, context, target, playback)?;
    Ok(1)
}

pub(super) fn emit_assignment_configuration_request(
    state: &AppState,
    session: &Session,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<(), String> {
    use crate::runtime::command_http::playback_address_command::CommandPlaybackTarget;
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
    let address = light_playback::PlaybackAddress::new(page, position)?;
    emit(
        state,
        "playback_configuration_requested",
        serde_json::json!({"desk_id":session.desk.id,"address":address}),
    );
    Ok(())
}

fn assignment_playback(
    state: &AppState,
    context: &light_application::ActionContext,
    source: &[String],
    target_address: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<light_playback::PlaybackDefinition, String> {
    if source.len() != 2 {
        return Err(
            "expected ASSIGN <CUELIST|DYNAMIC|MACRO|TIMECODE> <number> AT PBK <address>".into(),
        );
    }
    let number = source[1]
        .parse::<u16>()
        .map_err(|_| format!("{} number is invalid", source[0]))?;
    let snapshot = state.output.snapshot();
    let (name, color, icon, target) = match source[0].as_str() {
        "CUELIST" => {
            let id = super::show_command_update::cuelist_pool_id(&snapshot, number)?;
            let list = snapshot
                .cue_lists
                .iter()
                .find(|list| list.id == id)
                .ok_or_else(|| format!("Cuelist {number} does not exist"))?;
            let presentation = snapshot.playback_pages.iter().flat_map(|page|page.assignments.values()).find(|playback| matches!(playback.target, light_playback::PlaybackTarget::CueList { cue_list_id } if cue_list_id == id));
            (
                list.name.clone(),
                presentation
                    .map(|definition| definition.color.clone())
                    .unwrap_or_else(|| "#20c997".into()),
                presentation.and_then(|definition| definition.presentation_icon.clone()),
                light_playback::PlaybackTarget::CueList { cue_list_id: id },
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
            let revision = match existing_assignment(state, context, target_address)? {
                Some(light_playback::PlaybackDefinition {
                    target: light_playback::PlaybackTarget::Dynamic { assignment },
                    ..
                }) => assignment
                    .revision
                    .checked_add(1)
                    .ok_or("Dynamic Playback assignment revision is exhausted")?,
                _ => 1,
            };
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
    let mut playback = existing_assignment(state, context, target_address)?.unwrap_or_else(|| {
        light_playback::PlaybackDefinition {
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
    playback.name = name;
    playback.color = color;
    playback.presentation_icon = icon;
    playback.target = target;
    playback.reset_incompatible_layout();
    Ok(playback)
}

fn resolved_target(
    state: &AppState,
    context: &light_application::ActionContext,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<light_playback::PlaybackAddress, String> {
    use crate::runtime::command_http::playback_address_command::CommandPlaybackTarget;
    let (page, position) = match target {
        CommandPlaybackTarget::CurrentPage { position } => (
            state
                .installation
                .command_page(context.desk_id)
                .map_err(|error| error.to_string())?,
            position,
        ),
        CommandPlaybackTarget::ExplicitPage { page, position } => (page, position),
    };
    light_playback::PlaybackAddress::new(page, position)
}
fn page_objects(
    state: &AppState,
    page: u8,
) -> Result<
    (
        Option<light_show::VersionedObject>,
        Option<light_playback::PlaybackPage>,
    ),
    String,
> {
    let (_, store) = active_show_store(state)?;
    let object = store
        .objects("playback_page")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.body["number"] == page);
    let definition = object
        .as_ref()
        .map(|object| {
            serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
                .map_err(|error| error.to_string())
        })
        .transpose()?;
    Ok((object, definition))
}
pub(super) fn existing_assignment(
    state: &AppState,
    context: &light_application::ActionContext,
    target: crate::runtime::command_http::playback_address_command::CommandPlaybackTarget,
) -> Result<Option<light_playback::PlaybackDefinition>, String> {
    let address = resolved_target(state, context, target)?;
    Ok(page_objects(state, address.page())?
        .1
        .and_then(|page| page.assignments.get(&address.position()).cloned()))
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
    let address = resolved_target(state, context, target)?;
    let (page_object, page) = page_objects(state, address.page())?;
    let expected_page_revision = page_object.as_ref().map_or(0, |object| object.revision);
    let expected_page_object_id = page_object.as_ref().map(|object| object.id.clone());
    let action = if page
        .as_ref()
        .is_some_and(|page| page.assignments.contains_key(&address.position()))
    {
        light_application::PlaybackTopologyAction::UpdatePlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
            playback,
        }
    } else {
        light_application::PlaybackTopologyAction::AssignPlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
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
    let group = store
        .objects("group")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == group_id)
        .ok_or_else(|| format!("Group {group_id} does not exist"))?;
    let address = resolved_target(state, context, target)?;
    let (page_object, _) = page_objects(state, address.page())?;
    let address = light_application::GroupMasterPlaybackAddress {
        address,
        expected_page_revision: page_object.as_ref().map_or(0, |object| object.revision),
        expected_page_object_id: page_object.map(|object| object.id),
    };
    let request_id = context
        .request_id
        .clone()
        .unwrap_or_else(|| format!("assign-group-master-{}", context.correlation_id));
    let action = light_application::ActionEnvelope {
        context: context
            .clone()
            .with_request_id(request_id)
            .with_expected_revision(show_revision),
        command: light_application::PlaybackTopologyCommand {
            show_id: entry.id,
            action: light_application::PlaybackTopologyAction::AssignGroupMaster {
                group_object_id: group.id,
                expected_group_revision: group.revision,
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
