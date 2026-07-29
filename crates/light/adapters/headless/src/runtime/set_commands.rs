use super::*;

pub(super) fn execute_set_command(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if tokens.first().is_some_and(|token| token == "DYNAMIC") {
        return assign_dynamic_playback(state, session, tokens, context);
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
    let at = tokens.iter().position(|token| token == "AT");
    if let Some(at) = at {
        if tokens.first().is_some_and(|token| token == "GROUP") {
            return Err(
                "playback pages accept Cuelists only; store the group in a Cuelist first".into(),
            );
        } else {
            let playback = tokens
                .first()
                .ok_or("playback number is required")?
                .parse::<u16>()
                .map_err(|_| "playback number is invalid")?;
            let (page, slot) = parse_page_slot(&tokens[at + 1..])?;
            assign_page_slot(state, context, page, slot, playback)?;
        }
        return Ok(1);
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

fn assign_dynamic_playback(
    state: &AppState,
    session: &Session,
    tokens: &[String],
    context: &light_application::ActionContext,
) -> Result<usize, String> {
    if tokens.len() != 4 || tokens[2] != "PLAYBACK" {
        return Err("expected SET DYNAMIC <number> PLAYBACK <number>".into());
    }
    let dynamic_number = tokens[1]
        .parse::<u16>()
        .ok()
        .filter(|number| (1..=9_999).contains(number))
        .ok_or("Dynamic number must be within 1-9999")?;
    let playback_number = tokens[3]
        .parse::<u16>()
        .ok()
        .filter(|number| (1..=light_playback::MAX_PLAYBACKS).contains(number))
        .ok_or("Playback number must be within 1-1000")?;
    let snapshot = state.output.snapshot();
    let dynamic = snapshot
        .dynamics
        .iter()
        .find(|dynamic| dynamic.pool_number == dynamic_number)
        .cloned()
        .ok_or_else(|| format!("Dynamic {dynamic_number} does not exist"))?;
    let target_scope = if matches!(
        dynamic.target_binding,
        light_dynamics::DynamicTargetBinding::Targetless
    ) {
        let selected = state
            .programming
            .get(session.id)
            .ok_or("programmer does not exist")?
            .selected;
        if selected.is_empty() {
            return Err(
                "targetless Dynamic Playback assignment requires a current ordered selection"
                    .into(),
            );
        }
        Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets: selected })
    } else {
        None
    };
    let (entry, store) = active_show_store(state)?;
    let object = store
        .objects("playback")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == playback_number.to_string());
    let existing = object
        .as_ref()
        .map(|object| {
            serde_json::from_value::<light_playback::PlaybackDefinition>(object.body.clone())
                .map_err(|error| format!("invalid stored Playback: {error}"))
        })
        .transpose()?
        .or_else(|| {
            snapshot
                .playbacks
                .iter()
                .find(|playback| playback.number == playback_number)
                .cloned()
        });
    let assignment_revision = existing
        .as_ref()
        .and_then(|playback| match &playback.target {
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
                    definition: Box::new(dynamic.clone()),
                },
            },
            revision: assignment_revision,
            target_scope,
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
    let mut playback = existing.unwrap_or_else(|| light_playback::PlaybackDefinition {
        number: playback_number,
        name: dynamic.name.clone(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: dynamic.color.clone().unwrap_or_else(|| "#20c997".into()),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: dynamic.icon.clone(),
        presentation_image: None,
        target: target.clone(),
    });
    playback.target = target;
    playback.reset_incompatible_layout();
    let mutation = put_active_show_object(
        light_application::ActiveShowObjectKind::Playback,
        playback_number.to_string(),
        object.as_ref().map_or(0, |stored| stored.revision),
        serde_json::to_value(playback).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.message)?;
    let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    let change = result
        .changes
        .first()
        .expect("the committed Dynamic Playback assignment returns its change");
    emit_command_object_changed(
        state,
        &entry,
        change.kind.as_str(),
        &change.object_id,
        change.object_revision,
    );
    Ok(1)
}

pub(super) fn parse_page_slot(tokens: &[String]) -> Result<(u8, u8), String> {
    if tokens.len() != 3 || tokens[1] != "." {
        return Err("expected <page> . <page-playback>".into());
    }
    Ok((
        tokens[0].parse().map_err(|_| "page number is invalid")?,
        tokens[2]
            .parse()
            .map_err(|_| "page playback number is invalid")?,
    ))
}

pub(super) fn assign_page_slot(
    state: &AppState,
    context: &light_application::ActionContext,
    page: u8,
    slot: u8,
    playback: u16,
) -> Result<(), String> {
    let (entry, store) = active_show_store(state)?;
    let snapshot = state.output.snapshot();
    validate_cuelist_assignment(&snapshot, playback)?;
    let object = store
        .objects("playback_page")
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|object| object.id == page.to_string());
    let mut definition = if let Some(object) = &object {
        serde_json::from_value::<light_playback::PlaybackPage>(object.body.clone())
            .map_err(|error| error.to_string())?
    } else {
        snapshot
            .playback_pages
            .iter()
            .find(|item| item.number == page)
            .cloned()
            .unwrap_or(light_playback::PlaybackPage {
                number: page,
                name: format!("Page {page}"),
                slots: HashMap::new(),
                virtual_playbacks: HashMap::new(),
            })
    };
    definition.slots.insert(slot, playback);
    let mutation = playback_layout_mutations::put_page(
        definition,
        object.as_ref().map_or(0, |stored| stored.revision),
    )
    .map_err(|error| error.message)?;
    let action = active_show_object_action(context.clone(), entry.id, vec![mutation]);
    let result = run_active_show_object_action_in_programming_interaction(state, action)
        .map_err(|error| error.message)?;
    let change = result
        .changes
        .first()
        .expect("the committed page assignment returns its page change");
    emit_command_object_changed(
        state,
        &entry,
        change.kind.as_str(),
        &change.object_id,
        change.object_revision,
    );
    Ok(())
}

fn validate_cuelist_assignment(snapshot: &EngineSnapshot, playback: u16) -> Result<(), String> {
    let Some(definition) = snapshot
        .playbacks
        .iter()
        .find(|item| item.number == playback)
    else {
        return Err(format!("Cuelist {playback} does not exist"));
    };
    if matches!(
        definition.target,
        light_playback::PlaybackTarget::CueList { .. }
    ) {
        Ok(())
    } else {
        Err(format!(
            "Cuelist {playback} cannot be assigned to a playback"
        ))
    }
}
