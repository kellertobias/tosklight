use super::*;

pub(super) fn cuelist_for_page_playback(
    snapshot: &EngineSnapshot,
    page_number: u8,
    slot: u8,
) -> Option<u16> {
    let number = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_number)?
        .slots
        .get(&slot)
        .copied()?;
    snapshot
        .playbacks
        .iter()
        .any(|definition| definition.number == number)
        .then_some(number)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExpandedOscBinding {
    pub(super) anchor_slot: u8,
    pub(super) position: ExpandedOscPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExpandedOscPosition {
    TallerUpper,
    WiderRight,
}

pub(super) fn expanded_osc_binding(
    snapshot: &EngineSnapshot,
    page_number: u8,
    desk: &ControlDesk,
    claimed_slot: u8,
) -> Option<ExpandedOscBinding> {
    let page = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_number)?;
    if page.slots.contains_key(&claimed_slot) {
        return None;
    }
    let rows = desk.playback_layout.clone().unwrap_or_else(|| {
        let columns = desk.columns.max(1);
        light_show::PlaybackSurfaceLayout {
            playbacks_per_row: columns,
            rows: (0..desk.rows)
                .map(|row| light_show::PlaybackSurfaceRow {
                    first_playback_slot: 1 + row.saturating_mul(columns),
                    has_fader: true,
                    button_count: desk.buttons,
                })
                .collect(),
        }
    });
    let mut claimants = Vec::new();
    for (row_index, row) in rows.rows.iter().enumerate() {
        for column in 0..rows.playbacks_per_row {
            let anchor_slot = row.first_playback_slot.saturating_add(column);
            let Some(number) = page.slots.get(&anchor_slot) else {
                continue;
            };
            let Some(definition) = snapshot
                .playbacks
                .iter()
                .find(|definition| definition.number == *number)
            else {
                continue;
            };
            let target = match definition.footprint {
                light_playback::PlaybackFootprint::Normal => None,
                light_playback::PlaybackFootprint::Taller { .. } if row_index > 0 => {
                    let upper = &rows.rows[row_index - 1];
                    (upper.button_count > 0).then_some((
                        upper.first_playback_slot.saturating_add(column),
                        ExpandedOscPosition::TallerUpper,
                    ))
                }
                light_playback::PlaybackFootprint::Taller { .. } => None,
                light_playback::PlaybackFootprint::Wider { .. }
                    if column + 1 < rows.playbacks_per_row =>
                {
                    Some((
                        anchor_slot.saturating_add(1),
                        ExpandedOscPosition::WiderRight,
                    ))
                }
                light_playback::PlaybackFootprint::Wider { .. } => None,
            };
            if let Some((target_slot, position)) = target
                && target_slot == claimed_slot
                && !page.slots.contains_key(&target_slot)
            {
                claimants.push(ExpandedOscBinding {
                    anchor_slot,
                    position,
                });
            }
        }
    }
    (claimants.len() == 1).then(|| claimants[0])
}

pub(super) fn update_target_for_playback(
    state: &AppState,
    definition: &light_playback::PlaybackDefinition,
) -> Result<UpdateApiTarget, String> {
    match &definition.target {
        light_playback::PlaybackTarget::CueList { cue_list_id } => {
            let context = active_update_cue_contexts(state)
                .into_iter()
                .find(|context| context.playback_number == definition.number);
            Ok(UpdateApiTarget {
                family: UpdateApiTargetFamily::Cue,
                object_id: Some(cue_list_id.0.to_string()),
                playback_number: Some(definition.number),
                cue_id: context.as_ref().map(|context| context.cue_id),
                cue_number: context.map(|context| context.cue_number),
                validate_active_context: true,
            })
        }
        light_playback::PlaybackTarget::Group { group_id, .. } => Ok(UpdateApiTarget {
            family: UpdateApiTargetFamily::Group,
            object_id: Some(group_id.clone()),
            playback_number: Some(definition.number),
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        }),
        _ => Err(format!(
            "Playback {} is not assigned to a recordable Update target",
            definition.number
        )),
    }
}

pub(super) fn intercept_update_playback_target(
    state: &AppState,
    session: &Session,
    definition: &light_playback::PlaybackDefinition,
    touched: bool,
) -> bool {
    if !touched
        || !state
            .programming
            .get(session.id)
            .is_some_and(|programmer| command_line_arms_update(&programmer.command_line))
    {
        return false;
    }
    let target = match update_target_for_playback(state, definition) {
        Ok(target) => target,
        Err(error) => {
            emit(
                state,
                "update_target_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "playback_number":definition.number,
                    "source":"osc",
                    "error":error,
                }),
            );
            return true;
        }
    };
    state
        .programming
        .set_command_line(session.id, String::new());
    let _ = persist_programmer(state, session);
    emit(
        state,
        "update_target_requested",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "source":"osc",
            "target":target,
        }),
    );
    emit_update_armed_transition(state, session, true, false, "osc_target");
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"desk_id":session.desk.id,"source":"osc_target"}),
    );
    true
}

fn osc_playback_values(arguments: &[OscArgument]) -> (bool, Option<f32>) {
    let pressed = arguments
        .first()
        .map(|argument| match argument {
            OscArgument::Bool(value) => *value,
            OscArgument::Int(value) => *value != 0,
            OscArgument::Float(value) => *value > 0.0,
            OscArgument::String(value) => value != "0" && value != "false",
        })
        .unwrap_or(true);
    let value = arguments.first().and_then(|argument| match argument {
        OscArgument::Float(value) => Some(*value),
        OscArgument::Int(value) => Some(*value as f32 / 127.0),
        _ => None,
    });
    (pressed, value)
}

fn handle_osc_page(state: &AppState, parts: &[&str], arguments: &[OscArgument]) -> bool {
    if parts.len() != 3 || parts.first() != Some(&"light") || parts.get(2) != Some(&"page") {
        return false;
    }
    let page = arguments.first().and_then(|argument| match argument {
        OscArgument::Int(value) => u8::try_from(*value).ok(),
        OscArgument::Float(value) if value.is_finite() => Some(*value as u8),
        _ => None,
    });
    let Some(page) = page else {
        return true;
    };
    let Ok(_activation) = state.active_show.try_acquire() else {
        return true;
    };
    let Some((show, desk)) = state
        .active_show
        .current()
        .clone()
        .and_then(|show| osc_control_desk(state, parts[1]).map(|desk| (show, desk)))
    else {
        return true;
    };
    let context =
        light_application::ActionContext::system(desk.id, light_application::ActionSource::Osc);
    let completed = state
        .playback
        .run_unit_of_work(playback_service::ChangePage {
            state,
            show: &show,
            context,
            desk_id: desk.id,
            page,
        });
    if !completed
        .output
        .is_ok_and(|availability| availability.available())
    {
        return true;
    }
    emit(
        state,
        "playback_page_changed",
        serde_json::json!({"desk_id":desk.id,"page":page}),
    );
    true
}

pub(super) fn osc_playback_address(parts: &[&str]) -> Option<(PlaybackAddress, usize)> {
    if parts.len() >= 6
        && parts.first() == Some(&"light")
        && parts.get(2) == Some(&"virtual-playback")
    {
        let page = parts[3].parse::<u8>().ok()?;
        let number = parts[4].parse::<u16>().ok()?;
        let address = light_playback::VirtualPlaybackAddress::new(page, number).ok()?;
        Some((PlaybackAddress::Virtual(address), 5))
    } else if parts.len() >= 5
        && parts.first() == Some(&"light")
        && parts.get(1) == Some(&"playback")
    {
        let page = parts[2].parse::<u8>().ok()?;
        let slot = parts[3].parse::<u8>().ok()?;
        Some((PlaybackAddress::ExplicitPage { page, slot }, 4))
    } else if parts.len() >= 4
        && parts.first() == Some(&"light")
        && parts
            .get(1)
            .is_some_and(|name| *name == "cuelist" || *name == "qlist" || *name == "playback")
    {
        Some((PlaybackAddress::Pool(parts[2].parse::<u16>().ok()?), 3))
    } else if parts.len() >= 5
        && parts.first() == Some(&"light")
        && parts
            .get(2)
            .is_some_and(|name| *name == "page-playback" || *name == "paged-playback")
    {
        Some((
            PlaybackAddress::CurrentPage {
                slot: parts[3].parse::<u8>().ok()?,
            },
            4,
        ))
    } else {
        None
    }
}

pub(super) fn osc_playback_session(
    state: &AppState,
    source: Option<&str>,
    action_alias: &str,
    action_desk: Option<&ControlDesk>,
) -> Result<Option<Session>, ()> {
    let source = source.and_then(|source| source.parse::<SocketAddr>().ok());
    let subscribed = source.and_then(|source| state.integrations.osc_subscriber_for_source(source));
    if let Some(subscriber) = subscribed {
        let desk = action_desk.ok_or(())?;
        if !subscriber.desk_alias.eq_ignore_ascii_case(action_alias) {
            return Err(());
        }
        let session = state
            .sessions
            .session(subscriber.session_id)
            .filter(|session| session.connected && session.desk.id == desk.id)
            .ok_or(())?;
        if !subscriber
            .desk_alias
            .eq_ignore_ascii_case(&session.desk.osc_alias)
        {
            return Err(());
        }
        return Ok(Some(session));
    }
    Ok(action_desk.and_then(|desk| {
        state
            .sessions
            .sessions()
            .into_iter()
            .find(|session| session.connected && session.desk.id == desk.id)
    }))
}

fn osc_playback_context(
    state: &AppState,
    parts: &[&str],
    source: Option<&str>,
) -> Result<(Option<SocketAddr>, Option<ControlDesk>, Option<Session>), ()> {
    let source_socket = source.and_then(|source| source.parse::<SocketAddr>().ok());
    let path_alias = if parts.get(2).is_some_and(|part| {
        *part == "page-playback" || *part == "paged-playback" || *part == "virtual-playback"
    }) {
        Some(parts[1])
    } else {
        None
    };
    let subscribed =
        source_socket.and_then(|source| state.integrations.osc_subscriber_for_source(source));
    let action_alias = path_alias
        .map(str::to_owned)
        .or_else(|| {
            subscribed
                .as_ref()
                .map(|subscriber| subscriber.desk_alias.clone())
        })
        .unwrap_or_else(|| "main".into());
    let action_desk = subscribed
        .as_ref()
        .and_then(|subscriber| {
            state
                .sessions
                .session(subscriber.session_id)
                .map(|session| session.desk.clone())
        })
        .or_else(|| osc_control_desk(state, &action_alias));
    let session = osc_playback_session(state, source, &action_alias, action_desk.as_ref())?;
    Ok((source_socket, action_desk, session))
}

pub(super) fn handle_playback_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    // Preserve the three established OSC address families as distinct typed intents:
    //
    // - `/light/playback/{page}/{slot}` always targets that explicit page.
    // - `/light/{desk}/virtual-playback/{page}/{number}` targets one dedicated Virtual
    //   Playback identity without aliasing a physical page slot.
    // - `/light/{desk}/page-playback/{slot}` resolves the desk's current page under the
    //   PlaybackService operation gate.
    // - `/light/playback/{number}` and its Cuelist aliases address the global pool directly.
    //
    // Keeping this distinction until the application boundary prevents page changes from
    // retargeting explicit hardware input while retaining current-page behavior for desk wings.
    // Parsing stops at typed intent here; address resolution and mutation ordering stay in the
    // application service, alongside the HTTP and compatibility WebSocket paths.
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let (pressed, value) = osc_playback_values(arguments);
    if handle_osc_page(state, &parts, arguments) {
        return false;
    }
    let Some((mut playback_address, action_index)) = osc_playback_address(&parts) else {
        return false;
    };
    let Ok(activation) = state.active_show.try_acquire() else {
        return false;
    };
    let button = (parts[action_index] == "button")
        .then(|| parts.get(action_index + 1)?.parse::<u8>().ok())
        .flatten();
    let mut input = PoolPlaybackInput {
        value: value.map(|value| value.clamp(0.0, 1.0)),
        pressed: Some(pressed),
        button,
        ..PoolPlaybackInput::default()
    };
    let Ok((source_socket, action_desk, session)) = osc_playback_context(state, &parts, source)
    else {
        return false;
    };
    let mut action = if parts[action_index] == "fader" {
        "master"
    } else {
        parts[action_index]
    };
    if let (PlaybackAddress::CurrentPage { slot }, Some(desk), Some(show)) = (
        playback_address.clone(),
        action_desk.as_ref(),
        state.active_show.current().clone(),
    ) && let Ok(page) = state.installation.desk_page(desk.id, show.id)
        && let Some(binding) = expanded_osc_binding(&state.output.snapshot(), page, desk, slot)
    {
        playback_address = PlaybackAddress::CurrentPage {
            slot: binding.anchor_slot,
        };
        match (binding.position, action) {
            (ExpandedOscPosition::TallerUpper, "button") if input.button == Some(1) => {
                input.button = Some(4);
            }
            (ExpandedOscPosition::WiderRight, "button") => {
                input.button = input.button.and_then(|button| button.checked_add(3));
            }
            (ExpandedOscPosition::WiderRight, "master") => {
                action = "configured-fader";
                input.button = Some(2);
            }
            _ => return false,
        }
    }
    let suppression_input =
        session
            .as_ref()
            .map(|session| osc_cue_record_suppression::OscSuppressionInput {
                session_id: session.id,
                source: source_socket,
                address,
                continuous: action == "master",
                pressed,
            });
    if suppression_input.is_some_and(|input| {
        state
            .integrations
            .suppresses_osc_input(input, Instant::now())
    }) {
        return false;
    }
    // Target-selection commands enter the same show mutation paths as typed commands. Release the
    // read-side OSC activation before interception so Group/Cuelist assignment can acquire its own
    // authoritative show transaction instead of deadlocking behind this ingress guard.
    drop(activation);
    if let Some(session) = session.as_ref()
        && command_http::intercept_armed_cue_playback(
            state,
            session,
            playback_address.clone(),
            action == "master" || pressed,
        )
    {
        if let Some(input) = suppression_input {
            state
                .integrations
                .remember_osc_intercept(input, Instant::now());
        }
        return true;
    }
    let Ok(_activation) = state.active_show.try_acquire() else {
        return false;
    };
    let Ok(result) = playback_service::osc_action(
        state,
        session.as_ref(),
        action_desk.as_ref(),
        playback_address,
        action,
        &input,
    ) else {
        return false;
    };
    let changed = matches!(
        result.execution,
        PlaybackExecution::Pool { changed: true, .. }
    );
    if changed && let Some(number) = result.resolved.playback_number() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"playback_number":number,"action":action,"source":"osc","session_id":session.map(|session|session.id)}),
        );
    }
    true
}

#[cfg(test)]
mod playback_address_tests {
    use super::*;

    fn playback(
        number: u16,
        footprint: light_playback::PlaybackFootprint,
    ) -> light_playback::PlaybackDefinition {
        light_playback::PlaybackDefinition {
            number,
            name: format!("Playback {number}"),
            target: light_playback::PlaybackTarget::GrandMaster,
            buttons: [
                light_playback::PlaybackButtonAction::Blackout,
                light_playback::PlaybackButtonAction::PauseDynamics,
                light_playback::PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: light_playback::PlaybackFaderMode::Master,
            has_fader: true,
            footprint,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }

    fn desk() -> ControlDesk {
        ControlDesk {
            id: uuid::Uuid::nil(),
            name: "Desk".into(),
            osc_alias: "main".into(),
            columns: 2,
            rows: 2,
            buttons: 3,
            playback_layout: Some(light_show::PlaybackSurfaceLayout {
                playbacks_per_row: 2,
                rows: vec![
                    light_show::PlaybackSurfaceRow {
                        first_playback_slot: 11,
                        has_fader: false,
                        button_count: 1,
                    },
                    light_show::PlaybackSurfaceRow {
                        first_playback_slot: 31,
                        has_fader: true,
                        button_count: 3,
                    },
                ],
            }),
        }
    }

    fn snapshot(
        playbacks: Vec<light_playback::PlaybackDefinition>,
        slots: &[(u8, u16)],
    ) -> EngineSnapshot {
        EngineSnapshot {
            playbacks: playbacks.into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: slots.iter().copied().collect(),
                virtual_playbacks: std::collections::HashMap::new(),
            }]
            .into(),
            ..EngineSnapshot::default()
        }
    }

    #[test]
    fn dedicated_virtual_osc_address_is_page_qualified_and_range_checked() {
        let parts = ["light", "main", "virtual-playback", "4", "1901", "go"];
        let (address, action_index) = osc_playback_address(&parts).unwrap();
        assert_eq!(
            address,
            PlaybackAddress::Virtual(
                light_playback::VirtualPlaybackAddress::new(4, 1_901).unwrap()
            )
        );
        assert_eq!(action_index, 5);

        for invalid in [
            ["light", "main", "virtual-playback", "4", "1000", "go"],
            ["light", "main", "virtual-playback", "4", "1001", "go"],
            ["light", "main", "virtual-playback", "4", "9999", "go"],
        ] {
            assert!(osc_playback_address(&invalid).is_none());
        }
    }

    #[test]
    fn expanded_physical_slots_resolve_to_one_anchor_and_reject_conflicts() {
        let wider = playback(
            1,
            light_playback::PlaybackFootprint::Wider {
                right_buttons: [light_playback::PlaybackButtonAction::Go; 3],
                right_fader: light_playback::PlaybackFaderMode::Master,
            },
        );
        let taller = playback(
            2,
            light_playback::PlaybackFootprint::Taller {
                upper_button: light_playback::PlaybackButtonAction::Flash,
            },
        );
        assert_eq!(
            expanded_osc_binding(&snapshot(vec![wider.clone()], &[(31, 1)]), 1, &desk(), 32),
            Some(ExpandedOscBinding {
                anchor_slot: 31,
                position: ExpandedOscPosition::WiderRight
            })
        );
        assert_eq!(
            expanded_osc_binding(&snapshot(vec![taller.clone()], &[(32, 2)]), 1, &desk(), 12),
            Some(ExpandedOscBinding {
                anchor_slot: 32,
                position: ExpandedOscPosition::TallerUpper
            })
        );
        assert_eq!(
            expanded_osc_binding(
                &snapshot(
                    vec![
                        wider.clone(),
                        taller,
                        playback(3, light_playback::PlaybackFootprint::Normal),
                    ],
                    &[(31, 1), (32, 2), (12, 3)],
                ),
                1,
                &desk(),
                12,
            ),
            None,
            "the occupied upper position stays independently addressed"
        );
    }
}

pub(super) fn ingest_timecode(state: &AppState, timecode: SmpteTimecode) {
    let current = state.output.ingest_timecode(timecode);
    if let Some(timecode) = current {
        let fps = u64::from(timecode.rate.nominal_frames());
        let seconds = u64::from(timecode.hours) * 3600
            + u64::from(timecode.minutes) * 60
            + u64::from(timecode.seconds);
        state
            .output
            .set_timecode_frame(Some(seconds * fps + u64::from(timecode.frames)));
    }
}
