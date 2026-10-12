use super::*;
pub(in crate::runtime) mod holds;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExpandedOscBinding {
    pub(super) anchor_slot: u16,
    pub(super) position: ExpandedOscPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExpandedOscPosition {
    TallerUpper,
    WiderRight,
}

pub(super) fn expanded_osc_binding(
    state: &AppState,
    snapshot: &EngineSnapshot,
    page_number: u8,
    desk: &ControlDesk,
    claimed_slot: u16,
) -> Option<ExpandedOscBinding> {
    let page = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_number)?;
    if page.assignments.contains_key(&claimed_slot) {
        return None;
    }
    let registry = state.installation.playback_surfaces(desk.id).ok()?;
    let layout = &registry.get(&desk.surface_id)?.layout;
    let mut claimants = Vec::new();
    for (row_index, row) in layout.rows.iter().enumerate() {
        for column in 0..row.count {
            let anchor_slot = row.first_position.checked_add(column)?;
            let Some(definition) = page.assignments.get(&anchor_slot) else {
                continue;
            };
            let target = match definition.footprint {
                light_playback::PlaybackFootprint::Normal => None,
                light_playback::PlaybackFootprint::Taller { .. } if row_index > 0 => {
                    let upper = &layout.rows[row_index - 1];
                    (upper.button_count > 0 && column < upper.count).then_some((
                        upper.first_position + column,
                        ExpandedOscPosition::TallerUpper,
                    ))
                }
                light_playback::PlaybackFootprint::Wider { .. } if column + 1 < row.count => {
                    Some((anchor_slot + 1, ExpandedOscPosition::WiderRight))
                }
                _ => None,
            };
            if let Some((position, kind)) = target {
                if position == claimed_slot && !page.assignments.contains_key(&position) {
                    claimants.push(ExpandedOscBinding {
                        anchor_slot,
                        position: kind,
                    });
                }
            }
        }
    }
    (claimants.len() == 1).then(|| claimants[0])
}

pub(super) fn update_target_for_playback(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
) -> Result<UpdateApiTarget, String> {
    match &definition.target {
        light_playback::PlaybackTarget::CueList { cue_list_id } => {
            let context = active_update_cue_contexts(state)
                .into_iter()
                .find(|context| context.playback_address == address);
            Ok(UpdateApiTarget {
                family: UpdateApiTargetFamily::Cue,
                object_id: Some(cue_list_id.0.to_string()),
                playback_address: Some(address),
                cue_id: context.as_ref().map(|context| context.cue_id),
                cue_number: context.map(|context| context.cue_number.to_string()),
                validate_active_context: true,
            })
        }
        light_playback::PlaybackTarget::Group { group_id, .. } => Ok(UpdateApiTarget {
            family: UpdateApiTargetFamily::Group,
            object_id: Some(group_id.clone()),
            playback_address: Some(address),
            cue_id: None,
            cue_number: None,
            validate_active_context: false,
        }),
        _ => Err(format!(
            "Playback {} is not assigned to a recordable Update target",
            format!("{}.{}", address.page(), address.position())
        )),
    }
}

pub(super) fn intercept_update_playback_target(
    state: &AppState,
    session: &Session,
    address: light_playback::PlaybackAddress,
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
    let target = match update_target_for_playback(state, address, definition) {
        Ok(target) => target,
        Err(error) => {
            emit(
                state,
                "update_target_rejected",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "playback_address":address,
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

    let Some((show, desk)) = state
        .active_show
        .current()
        .clone()
        .and_then(|show| osc_control_desk(state).map(|desk| (show, desk)))
    else {
        return true;
    };
    let context =
        light_application::ActionContext::system(desk.id, light_application::ActionSource::Osc);
    let completed = playback_service::ChangePage {
        state,
        show: &show,
        context,
        desk_id: desk.id,
        page,
    }
    .run();
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
    if parts.first() != Some(&"light") || parts.get(2) != Some(&"playback") {
        return None;
    }
    let position = parts.get(3)?.parse::<u16>().ok()?;
    if parts.get(4)?.parse::<u16>().is_ok() {
        let address = light_playback::PlaybackAddress::new(
            u8::try_from(position).ok()?,
            parts[4].parse().ok()?,
        )
        .ok()?;
        let action = *parts.get(5)?;
        if parts.len() != if action == "button" { 7 } else { 6 } {
            return None;
        }
        Some((PlaybackAddress::Playback(address), 5))
    } else {
        light_playback::PlaybackAddress::new(1, position).ok()?;
        let action = parts[4];
        if parts.len() != if action == "button" { 6 } else { 5 } {
            return None;
        }
        Some((PlaybackAddress::CurrentPage { position }, 4))
    }
}

pub(super) fn osc_playback_session(
    state: &AppState,
    source: Option<&str>,
    action_alias: &str,
    action_desk: Option<&ControlDesk>,
) -> Result<Option<Session>, ()> {
    if let Some(source) = source {
        let source = source.parse::<SocketAddr>().map_err(|_| ())?;
        let subscriber = state
            .integrations
            .osc_subscriber_for_source(source)
            .ok_or(())?;
        if subscriber.last_seen.elapsed() >= Duration::from_secs(20) {
            return Err(());
        }
        // The alias no longer names which desk this is — there is one. It names the path the
        // surface connected on, and therefore what it may do. A message must still arrive on the
        // path its sender subscribed to, so a guest cannot reach the desk-button routes.
        if !subscriber.path.eq_ignore_ascii_case(action_alias) {
            return Err(());
        }
        let session = state
            .sessions
            .session(subscriber.session_id)
            .filter(|session| session.connected)
            .ok_or(())?;
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
    let path_alias = parts.get(1).copied();
    let subscribed =
        source_socket.and_then(|source| state.integrations.osc_subscriber_for_source(source));
    let action_alias = path_alias
        .map(str::to_owned)
        .or_else(|| {
            subscribed
                .as_ref()
                .map(|subscriber| subscriber.path.clone())
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
        .or_else(|| osc_control_desk(state));
    let session = osc_playback_session(state, source, &action_alias, action_desk.as_ref())?;
    Ok((source_socket, action_desk, session))
}

pub(super) fn is_playback_osc_route(address: &str) -> bool {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    osc_playback_address(&parts).is_some()
        || (parts.len() == 3 && parts[0] == "light" && parts[2] == "page")
}

#[derive(Clone)]
pub(super) struct OscInputAdmission {
    epoch: u64,
    cancel: Option<CancellationToken>,
}

impl OscInputAdmission {
    pub(super) fn capture(state: &AppState, cancel: Option<CancellationToken>) -> Self {
        Self {
            epoch: state.integrations.osc_playback_epoch(),
            cancel,
        }
    }

    pub(super) fn accepts(&self, state: &AppState, address: &str) -> bool {
        let current = state.integrations.osc_playback_epoch();
        if self.epoch == current
            && !self
                .cancel
                .as_ref()
                .is_some_and(|cancel| cancel.is_cancelled())
        {
            return true;
        }
        tracing::warn!(
            address,
            captured_epoch = self.epoch,
            current_epoch = current,
            "OSC Playback rejected after runtime replacement or input shutdown"
        );
        emit(
            state,
            "osc_playback_rejected",
            serde_json::json!({
                "address":address,"reason":"runtime replaced or input stopped",
                "captured_epoch":self.epoch,"current_epoch":current
            }),
        );
        false
    }
}

pub(super) fn handle_playback_osc(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
) -> bool {
    handle_playback_osc_admitted(
        state,
        address,
        arguments,
        source,
        &OscInputAdmission::capture(state, None),
        None,
    )
}

fn admit_playback_route(
    state: &AppState,
    address: &str,
    parts: &[&str],
    arguments: &[OscArgument],
    admission: &OscInputAdmission,
    lease: &OscPlaybackLease,
    desk: Option<&ControlDesk>,
) -> Option<(
    PlaybackAddress,
    usize,
    capability_resources::ActiveShowPermit,
)> {
    let playback = osc_playback_address(parts);
    let page_route = parts.len() == 3 && parts[0] == "light" && parts[2] == "page";
    if playback.is_none() && !page_route {
        return None;
    }
    let activation = state.active_show.acquire_shared_blocking();
    if !admission.accepts(state, address) || !lease.accepts(state, desk) {
        return None;
    }
    if handle_osc_page(state, parts, arguments) {
        return None;
    }
    playback.map(|(address, index)| (address, index, activation))
}

pub(super) fn handle_playback_osc_admitted(
    state: &AppState,
    address: &str,
    arguments: &[OscArgument],
    source: Option<&str>,
    admission: &OscInputAdmission,
    captured_lease: Option<&OscPlaybackLease>,
) -> bool {
    // Explicit address and stable source are retained across page changes and release.
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    if !is_playback_osc_route(address) {
        return false;
    }
    let Ok((source_socket, action_desk, session)) = osc_playback_context(state, &parts, source)
    else {
        return false;
    };
    let may_program = osc_source_may_program(state, source_socket);
    let current_lease = osc_source_lease(source, &parts, session.as_ref(), may_program);
    let lease = captured_lease.unwrap_or(&current_lease);
    let (pressed, value) = osc_playback_values(arguments);
    let Some((mut playback_address, action_index, activation)) = admit_playback_route(
        state,
        address,
        &parts,
        arguments,
        admission,
        lease,
        action_desk.as_ref(),
    ) else {
        return false;
    };
    let mut input = osc_pool_input(&parts, action_index, pressed, value);
    let button = input.button;
    let Some(desk) = action_desk.as_ref() else {
        return false;
    };
    // Never hold show admission while waiting for the receipt lease. Dispatch takes
    // receipt → shared admission and retains it across interception and the live action.
    drop(activation);
    let Some(mut capture) = holds::CapturedInput::begin(
        state,
        &mut playback_address,
        &input,
        arguments,
        desk,
        (source_socket, source, parts[action_index]),
    ) else {
        return false;
    };
    let receipt = capture.receipt;
    if pressed && receipt.is_some() {
        let _activation = state.active_show.acquire_shared_blocking();
        return admission.accepts(state, address) && lease.accepts(state, action_desk.as_ref());
    }
    let record_target = may_program
        && is_physical_record_target(
            state,
            &playback_address,
            parts[action_index],
            button,
            action_desk.as_ref(),
        );
    let mut action = if parts[action_index] == "fader" {
        "master"
    } else {
        parts[action_index]
    };
    if receipt.is_none()
        && !apply_expanded_osc_binding(
            state,
            &mut playback_address,
            action_desk.as_ref(),
            &mut input,
            &mut action,
        )
    {
        return false;
    }
    if let Some(receipt) = receipt {
        playback_address = PlaybackAddress::Playback(receipt.address);
        action = receipt.action;
        input.button = receipt.button;
    }
    let PlaybackAddress::Playback(exact_address) = playback_address else {
        return false;
    };
    let held_action = receipt
        .map(|receipt| receipt.action)
        .or_else(|| holds::held_action(state, exact_address, action, &input));
    if !pressed && held_action.is_some() && receipt.is_none() {
        return false;
    }
    let suppression_input =
        osc_suppression_input(session.as_ref(), source_socket, address, action, pressed);
    if suppression_input.is_some_and(|input| {
        state
            .integrations
            .suppresses_osc_input(input, Instant::now())
    }) {
        return false;
    }
    let (result, off_target) = match dispatch_osc_input(OscDispatch {
        state,
        session: session.as_ref(),
        desk: action_desk.as_ref(),
        address: playback_address.clone(),
        action,
        input: &mut input,
        control_source: capture.source.clone(),
        suppression_input,
        record_target,
        may_program,
        captured_release: receipt.is_some(),
        lease: Some(lease),
        admission,
    }) {
        OscDispatchResult::Consumed => return true,
        OscDispatchResult::Rejected => return false,
        OscDispatchResult::Applied(result, off_target) => (result, off_target),
    };
    if off_target {
        action = "off";
    }
    let off_target_address = off_target.then(|| playback_address.clone());
    if !off_target {
        if pressed {
            if let Some(action) = held_action {
                capture.held.remember(
                    capture.key,
                    holds::HeldPlayback {
                        address: exact_address,
                        action,
                        button: input.button,
                    },
                );
            }
        } else {
            capture.held.forget(&capture.key);
        }
    }
    if let (Some(address), Some(session)) = (off_target_address.as_ref(), session.as_ref()) {
        command_http::complete_off_target(state, session, address);
    }
    publish_osc_result(state, &result, action, session.as_ref());
    true
}

fn osc_pool_input(
    parts: &[&str],
    action_index: usize,
    pressed: bool,
    value: Option<f32>,
) -> PoolPlaybackInput {
    PoolPlaybackInput {
        value: value.map(|value| value.clamp(0.0, 1.0)),
        pressed: Some(pressed),
        button: (parts[action_index] == "button")
            .then(|| parts.get(action_index + 1)?.parse::<u8>().ok())
            .flatten(),
        ..PoolPlaybackInput::default()
    }
}

fn apply_expanded_osc_binding(
    state: &AppState,
    playback_address: &mut PlaybackAddress,
    desk: Option<&ControlDesk>,
    input: &mut PoolPlaybackInput,
    action: &mut &str,
) -> bool {
    if let (PlaybackAddress::Playback(address), Some(desk)) = (playback_address.clone(), desk) {
        if let Some(binding) = expanded_osc_binding(
            state,
            &state.output.snapshot(),
            address.page(),
            desk,
            address.position(),
        ) {
            *playback_address = PlaybackAddress::Playback(
                light_playback::PlaybackAddress::new(address.page(), binding.anchor_slot)
                    .expect("validated layout address"),
            );
            match (binding.position, *action) {
                (ExpandedOscPosition::TallerUpper, "button") if input.button == Some(1) => {
                    input.button = Some(4)
                }
                (ExpandedOscPosition::WiderRight, "button") => {
                    input.button = input.button.and_then(|button| button.checked_add(3))
                }
                (ExpandedOscPosition::WiderRight, "master") => {
                    *action = "configured-fader";
                    input.button = Some(2);
                }
                _ => return false,
            }
        }
    }
    true
}

fn osc_playback_control_id(
    playback_address: &PlaybackAddress,
    input: &PoolPlaybackInput,
    arguments: &[OscArgument],
    control: &str,
) -> Option<u16> {
    let position = match playback_address {
        PlaybackAddress::Playback(address) => address.position(),
        PlaybackAddress::CurrentPage { position } => *position,
        _ => return None,
    };
    let token = match control {
        "button" => u16::from(input.button?),
        "fader" | "master" => 7,
        "flash" => 8,
        "swap" => 9,
        _ => 10,
    };
    if control == "button" && !(1..=6).contains(&token) {
        return None;
    }
    let fallback = position * 16 + token;
    match arguments.get(1) {
        Some(OscArgument::Int(value)) => u16::try_from(*value).ok(),
        None => Some(fallback),
        _ => None,
    }
}

/// The key an already-applied OSC action is remembered under, so its echo is ignored.
fn osc_suppression_input<'a>(
    session: Option<&Session>,
    source: Option<SocketAddr>,
    address: &'a str,
    action: &str,
    pressed: bool,
) -> Option<osc_cue_record_suppression::OscSuppressionInput<'a>> {
    session.map(|session| osc_cue_record_suppression::OscSuppressionInput {
        session_id: session.id,
        source,
        address,
        continuous: action == "master",
        pressed,
    })
}

/// Whether the surface that sent this message may change programming.
///
/// A remote-control-only route operates the playback and nothing else. It never captures a Record,
/// even while the operator has one armed on the desk's command line — that is what lets a guest
/// work a playback beside somebody who is programming.
fn osc_source_may_program(state: &AppState, source: Option<SocketAddr>) -> bool {
    match source {
        None => true, // Private in-process helper dispatch has no UDP surface.
        Some(source) => state
            .integrations
            .osc_subscriber_for_source(source)
            .is_some_and(|subscriber| {
                subscriber.last_seen.elapsed() < Duration::from_secs(20)
                    && subscriber.capability.may_program()
                    && state
                        .sessions
                        .session(subscriber.session_id)
                        .is_some_and(|session| session.connected)
            }),
    }
}

fn is_physical_record_target(
    state: &AppState,
    address: &PlaybackAddress,
    control: &str,
    button: Option<u8>,
    desk: Option<&ControlDesk>,
) -> bool {
    if control == "button" {
        return button == Some(1);
    }
    if control != "label" {
        return false;
    }
    let snapshot = state.output.snapshot();
    let concrete = match address {
        PlaybackAddress::Playback(address) => Some(*address),
        PlaybackAddress::CurrentPage { position } => desk
            .and_then(|desk| state.installation.command_page(desk.id).ok())
            .and_then(|page| light_playback::PlaybackAddress::new(page, *position).ok()),
        PlaybackAddress::CueList(_) | PlaybackAddress::Group(_) => None,
    };
    let playback = concrete.and_then(|address| {
        snapshot
            .playback_pages
            .iter()
            .find(|page| page.number == address.page())
            .and_then(|page| page.assignments.get(&address.position()))
    });
    playback.is_none_or(|playback| playback.button_count == 0)
}

#[cfg(test)]
mod playback_address_tests {
    use super::*;

    fn playback(
        footprint: light_playback::PlaybackFootprint,
    ) -> light_playback::PlaybackDefinition {
        let mut definition = light_playback::PlaybackDefinition::new_cue_list(
            "OSC test",
            light_core::CueListId::new(),
        );
        definition.footprint = footprint;
        definition
    }
    fn snapshot(assignments: Vec<(u16, light_playback::PlaybackDefinition)>) -> EngineSnapshot {
        EngineSnapshot {
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                assignments: assignments.into_iter().collect(),
            }]
            .into(),
            ..EngineSnapshot::default()
        }
    }
    #[test]
    fn unified_osc_address_is_page_qualified_and_range_checked() {
        for (page, position) in [(1, 1), (4, 401), (99, 1000)] {
            let page_text = page.to_string();
            let position_text = position.to_string();
            let parts = [
                "light",
                "main",
                "playback",
                &page_text,
                &position_text,
                "go",
            ];
            let (address, action_index) = osc_playback_address(&parts).unwrap();
            assert_eq!(
                address,
                PlaybackAddress::Playback(
                    light_playback::PlaybackAddress::new(page, position).unwrap()
                )
            );
            assert_eq!(action_index, 5);
        }
        for invalid in [
            ["light", "main", "playback", "0", "1", "go"],
            ["light", "main", "playback", "100", "1", "go"],
            ["light", "main", "playback", "4", "0", "go"],
            ["light", "main", "playback", "4", "1001", "go"],
            ["light", "main", "playback", "bad", "1", "go"],
            ["light", "main", "virtual-playback", "4", "1901", "go"],
        ] {
            assert!(osc_playback_address(&invalid).is_none());
        }
    }
    #[test]
    fn expanded_surface_positions_resolve_one_anchor_and_reject_conflicts() {
        let (state, data_dir) = crate::runtime::tests::test_state();
        let desk = state.installation.desk().unwrap();
        state
            .installation
            .update_playback_surface(desk.id, &desk.surface_id, |_| {
                Ok(light_show::PlaybackSurfaceConfiguration {
                    name: "OSC test".into(),
                    page_source: light_show::PlaybackPageSource::Own { page: 1 },
                    layout: light_show::PlaybackSurfaceLayout {
                        rows: vec![
                            light_show::PlaybackSurfaceRow {
                                first_position: 11,
                                count: 2,
                                presentation: light_show::PlaybackControlPresentation::Buttons,
                                button_count: 1,
                            },
                            light_show::PlaybackSurfaceRow {
                                first_position: 31,
                                count: 2,
                                presentation: light_show::PlaybackControlPresentation::Fader,
                                button_count: 3,
                            },
                        ],
                    },
                })
            })
            .unwrap();
        let wider = playback(light_playback::PlaybackFootprint::Wider {
            right_buttons: [light_playback::PlaybackButtonAction::Go; 3],
            right_fader: light_playback::PlaybackFaderMode::Master,
        });
        let taller = playback(light_playback::PlaybackFootprint::Taller {
            upper_button: light_playback::PlaybackButtonAction::Flash,
        });
        assert_eq!(
            expanded_osc_binding(&state, &snapshot(vec![(31, wider.clone())]), 1, &desk, 32),
            Some(ExpandedOscBinding {
                anchor_slot: 31,
                position: ExpandedOscPosition::WiderRight
            })
        );
        assert_eq!(
            expanded_osc_binding(&state, &snapshot(vec![(32, taller.clone())]), 1, &desk, 12),
            Some(ExpandedOscBinding {
                anchor_slot: 32,
                position: ExpandedOscPosition::TallerUpper
            })
        );
        assert_eq!(
            expanded_osc_binding(
                &state,
                &snapshot(vec![
                    (31, wider.clone()),
                    (32, taller.clone()),
                    (12, playback(light_playback::PlaybackFootprint::Normal))
                ]),
                1,
                &desk,
                12
            ),
            None,
            "occupied upper position remains independently addressed"
        );
        assert_eq!(
            expanded_osc_binding(
                &state,
                &snapshot(vec![(11, wider), (32, taller)]),
                1,
                &desk,
                12
            ),
            None,
            "two expanded claims are ambiguous"
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

pub(super) fn ingest_timecode(state: &AppState, timecode: SmpteTimecode) {
    let current = state.output.ingest_timecode(timecode);
    if let Some(timecode) = current {
        let fps = u64::from(timecode.rate.nominal_frames());
        let seconds = u64::from(timecode.hours) * 3600
            + u64::from(timecode.minutes) * 60
            + u64::from(timecode.seconds);
        let frame = seconds * fps + u64::from(timecode.frames);
        state.output.set_timecode_frame(Some(frame));
        state
            .timecodes
            .synchronize_external(light_playback::TimecodeFrame(frame));
    }
}

#[cfg(test)]
#[path = "osc_playback/holds_tests.rs"]
mod holds_tests;

pub(super) struct OscPlaybackLease {
    source: Option<String>,
    alias: String,
    session_id: Option<SessionId>,
    may_program: bool,
}
fn osc_source_lease(
    source: Option<&str>,
    parts: &[&str],
    session: Option<&Session>,
    may_program: bool,
) -> OscPlaybackLease {
    OscPlaybackLease {
        source: source.map(str::to_owned),
        alias: parts.get(1).copied().unwrap_or("main").to_owned(),
        session_id: session.map(|session| session.id),
        may_program,
    }
}
impl OscPlaybackLease {
    pub(super) fn accepts(&self, state: &AppState, desk: Option<&ControlDesk>) -> bool {
        let fallback_desk = desk.cloned().or_else(|| osc_control_desk(state));
        osc_playback_session(
            state,
            self.source.as_deref(),
            &self.alias,
            fallback_desk.as_ref(),
        )
        .is_ok_and(|session| {
            session.map(|session| session.id) == self.session_id
                && osc_source_may_program(
                    state,
                    self.source
                        .as_deref()
                        .and_then(|source| source.parse().ok()),
                ) == self.may_program
        })
    }
}
/// Capture the exact surface lease before any Playback or mapped-control admission wait.
pub(super) fn capture_playback_source(
    state: &AppState,
    address: &str,
    source: Option<&str>,
) -> Result<OscPlaybackLease, ()> {
    let parts = address.trim_matches('/').split('/').collect::<Vec<_>>();
    let (socket, _, session) = osc_playback_context(state, &parts, source)?;
    Ok(OscPlaybackLease {
        source: source.map(str::to_owned),
        alias: parts.get(1).copied().unwrap_or("main").to_owned(),
        session_id: session.map(|session| session.id),
        may_program: osc_source_may_program(state, socket),
    })
}

struct OscDispatch<'a> {
    state: &'a AppState,
    session: Option<&'a Session>,
    desk: Option<&'a ControlDesk>,
    address: PlaybackAddress,
    action: &'a str,
    input: &'a mut PoolPlaybackInput,
    control_source: light_playback::PlaybackControlSource,
    suppression_input: Option<osc_cue_record_suppression::OscSuppressionInput<'a>>,
    record_target: bool,
    may_program: bool,
    captured_release: bool,
    lease: Option<&'a OscPlaybackLease>,
    admission: &'a OscInputAdmission,
}
enum OscDispatchResult {
    Consumed,
    Rejected,
    Applied(light_application::PlaybackResult, bool),
}
fn dispatch_osc_input(dispatch: OscDispatch<'_>) -> OscDispatchResult {
    let OscDispatch {
        state,
        session,
        desk: action_desk,
        address: playback_address,
        mut action,
        input,
        control_source,
        suppression_input,
        record_target,
        may_program,
        captured_release,
        lease,
        admission,
    } = dispatch;
    let _activation = state.active_show.acquire_shared_blocking();
    if !admission.accepts(state, "captured Playback input")
        || lease.is_some_and(|lease| !lease.accepts(state, action_desk))
    {
        return OscDispatchResult::Rejected;
    }
    let pressed = input.pressed.unwrap_or(true);
    let mut off_target = false;
    if !captured_release && let Some(session) = session.filter(|_| may_program) {
        match command_http::intercept_armed_cue_playback(
            state,
            session,
            playback_address.clone(),
            record_target,
            pressed,
        ) {
            command_http::PlaybackTargetInterception::NotArmed => {}
            command_http::PlaybackTargetInterception::Consumed => {
                if let Some(input) = suppression_input {
                    state
                        .integrations
                        .remember_osc_intercept(input, Instant::now());
                }
                return OscDispatchResult::Consumed;
            }
            command_http::PlaybackTargetInterception::Off => {
                if let Some(input) = suppression_input {
                    state
                        .integrations
                        .remember_osc_intercept(input, Instant::now());
                }
                action = "off";
                input.pressed = Some(true);
                off_target = true;
            }
        }
    }
    let Ok(result) = playback_service::osc_action(
        state,
        session,
        action_desk,
        playback_address,
        action,
        &input,
        control_source,
    ) else {
        return OscDispatchResult::Rejected;
    };
    OscDispatchResult::Applied(result, off_target)
}

fn publish_osc_result(
    state: &AppState,
    result: &light_application::PlaybackResult,
    action: &str,
    session: Option<&Session>,
) {
    let changed = matches!(
        result.execution,
        PlaybackExecution::Playback { changed: true, .. }
    );
    if changed && let Some(number) = result.resolved.playback_address() {
        emit(
            state,
            "playback_changed",
            serde_json::json!({"playback_address":number,"action":action,"source":"osc","session_id":session.map(|session|session.id)}),
        );
    }
}

#[cfg(test)]
#[path = "osc_playback/subscription_tests.rs"]
mod subscription_tests;
