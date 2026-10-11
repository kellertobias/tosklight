use super::*;

pub(super) struct OscPlaybackFeedback<'a> {
    pub(super) state: &'a AppState,
    pub(super) subscriber: &'a OscSubscriber,
    pub(super) desk: &'a ControlDesk,
    pub(super) page: u8,
    pub(super) selected_playback: Option<light_playback::PlaybackAddress>,
    pub(super) snapshot: &'a EngineSnapshot,
    pub(super) runtime: &'a [light_playback::PlaybackRuntimeStatus],
    pub(super) speed_groups: &'a [SpeedSnapshot; 5],
}

fn playback_level(
    state: &AppState,
    address: light_playback::PlaybackAddress,
    definition: &light_playback::PlaybackDefinition,
    running: Option<&light_playback::PlaybackRuntimeStatus>,
    speed_groups: &[SpeedSnapshot; 5],
) -> f32 {
    let level = match &definition.target {
        light_playback::PlaybackTarget::CueList { .. } => running
            .map(|status| status.playback.fader_position)
            .unwrap_or(0.0),
        light_playback::PlaybackTarget::Dynamic { .. } => state
            .output
            .active_dynamic_playback(address)
            .map(|playback| playback.fader_value)
            .unwrap_or(0.0),
        // Macros are one-shot actions and intentionally have no latched fader level.
        light_playback::PlaybackTarget::Macro { .. } => 0.0,
        light_playback::PlaybackTarget::Timecode { timecode_id } => state
            .timecodes
            .snapshot(*timecode_id)
            .map(|snapshot| {
                f32::from(snapshot.transport == light_playback::TimecodeTransportState::Playing)
            })
            .unwrap_or(0.0),
        light_playback::PlaybackTarget::Group { group_id, .. } => {
            state.output.group_master(group_id).unwrap_or(0.0)
        }
        light_playback::PlaybackTarget::SpeedGroup { group } => {
            let index = speed_group_index(group).unwrap_or(0);
            match definition.fader {
                light_playback::PlaybackFaderMode::DirectBpm => {
                    (speed_groups[index].effective_bpm / 300.0) as f32
                }
                light_playback::PlaybackFaderMode::CenteredRelative => {
                    ((speed_groups[index].speed_master_scale.log(4.0) / 2.0) + 0.5) as f32
                }
                _ => speed_groups[index].speed_master_scale as f32,
            }
        }
        light_playback::PlaybackTarget::ProgrammerFade => {
            state.installation.configuration().programmer_fade_millis as f32 / 20_000.0
        }
        light_playback::PlaybackTarget::CueFade => {
            state
                .installation
                .configuration()
                .sequence_master_fade_millis as f32
                / 60_000.0
        }
        light_playback::PlaybackTarget::GrandMaster => {
            state.output.control_projection().grand_master
        }
    };
    level.clamp(0.0, 1.0)
}

fn send_cue_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    prefix: &str,
    selected: bool,
    running: Option<&light_playback::PlaybackRuntimeStatus>,
) {
    let current = running.and_then(|item| item.playback.current_cue_number.as_ref());
    let normal_next = running.and_then(|item| item.normal_next_cue_number.as_ref());
    let effective_next = running.and_then(|item| item.effective_next_cue_number.as_ref());
    let values = [
        ("selected", OscArgument::Bool(selected)),
        (
            "current-cue",
            OscArgument::Float(legacy_cue_number(current)),
        ),
        (
            "normal-next-cue",
            OscArgument::Float(legacy_cue_number(normal_next)),
        ),
        (
            "effective-next-cue",
            OscArgument::Float(legacy_cue_number(effective_next)),
        ),
        (
            "loaded-next",
            OscArgument::Bool(running.is_some_and(|item| item.effective_next_is_loaded)),
        ),
    ];
    for (suffix, argument) in values {
        send_osc(
            state,
            subscriber.target,
            format!("{prefix}/{suffix}"),
            vec![argument],
        );
    }
    for (suffix, number) in [
        ("current-cue-path", current),
        ("normal-next-cue-path", normal_next),
        ("effective-next-cue-path", effective_next),
    ] {
        send_osc(
            state,
            subscriber.target,
            format!("{prefix}/{suffix}"),
            vec![OscArgument::String(
                number.map_or_else(|| "-1".into(), ToString::to_string),
            )],
        );
    }
}

fn legacy_cue_number(number: Option<&light_playback::CueNumber>) -> f32 {
    number
        .and_then(|number| number.to_string().parse::<f32>().ok())
        .unwrap_or(-1.0)
}

fn send_button_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    prefix: &str,
    definition: Option<&light_playback::PlaybackDefinition>,
    running: Option<&light_playback::PlaybackRuntimeStatus>,
) {
    let active = running
        .is_some_and(|item| item.playback.enabled || item.temporary_active || item.swap_active);
    for button in 1..=definition.map_or(3, |definition| definition.button_count) {
        let (r, g, b) = definition
            .map(|definition| playback_color_rgb(&definition.color, active))
            .unwrap_or((0.18, 0.20, 0.23));
        let prefix = format!("{prefix}/button/{button}");
        send_osc(
            state,
            subscriber.target,
            prefix.clone(),
            vec![
                OscArgument::Float(r),
                OscArgument::Float(g),
                OscArgument::Float(b),
                OscArgument::String(if active { "on" } else { "off" }.into()),
            ],
        );
        if let Some(action) = definition
            .and_then(|definition| definition.buttons.get(usize::from(button - 1)))
            .and_then(|action| serde_json::to_value(action).ok())
            .and_then(|action| action.as_str().map(str::to_owned))
        {
            send_osc(
                state,
                subscriber.target,
                format!("{prefix}/action"),
                vec![OscArgument::String(action)],
            );
        }
    }
}

fn send_runtime_feedback(
    state: &AppState,
    subscriber: &OscSubscriber,
    prefix: &str,
    definition: Option<&light_playback::PlaybackDefinition>,
    running: Option<&light_playback::PlaybackRuntimeStatus>,
) {
    let (Some(definition), Some(running)) = (definition, running) else {
        return;
    };
    let fader_mode = serde_json::to_value(definition.fader)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let direction = serde_json::to_value(running.playback.manual_xfade_direction)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    for (suffix, argument) in [
        ("fader-mode", OscArgument::String(fader_mode)),
        (
            "fader-pickup",
            OscArgument::Bool(running.playback.fader_pickup_required),
        ),
        ("temporary", OscArgument::Bool(running.temporary_active)),
        ("xfade-direction", OscArgument::String(direction)),
    ] {
        send_osc(
            state,
            subscriber.target,
            format!("{prefix}/{suffix}"),
            vec![argument],
        );
    }
}

fn send_slot_feedback(feedback: &OscPlaybackFeedback<'_>, slot: u16) {
    let binding = expanded_osc_binding(
        feedback.state,
        feedback.snapshot,
        feedback.page,
        feedback.desk,
        slot,
    );
    let position = binding.map_or(slot, |binding| binding.anchor_slot);
    let address = light_playback::PlaybackAddress::new(feedback.page, position)
        .expect("validated Playback layout");
    let source_definition = feedback
        .snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == feedback.page)
        .and_then(|page| page.assignments.get(&position));
    let mut projected_definition = source_definition.cloned();
    if let (Some(binding), Some(definition)) = (binding, projected_definition.as_mut()) {
        match (binding.position, definition.footprint) {
            (
                ExpandedOscPosition::TallerUpper,
                light_playback::PlaybackFootprint::Taller { upper_button },
            ) => {
                definition.buttons = [
                    upper_button,
                    light_playback::PlaybackButtonAction::None,
                    light_playback::PlaybackButtonAction::None,
                ];
                definition.button_count = 1;
                definition.has_fader = false;
            }
            (
                ExpandedOscPosition::WiderRight,
                light_playback::PlaybackFootprint::Wider {
                    right_buttons,
                    right_fader,
                },
            ) => {
                definition.buttons = right_buttons;
                definition.fader = right_fader;
            }
            _ => {}
        }
    }
    let definition = if binding.is_some() {
        projected_definition.as_ref()
    } else {
        source_definition
    };
    let running = feedback
        .runtime
        .iter()
        .find(|status| status.playback.playback_address == Some(address));
    let prefix = format!(
        "/light/{}/feedback/playback/{}/{slot}",
        feedback.subscriber.path, feedback.page
    );
    send_osc(
        feedback.state,
        feedback.subscriber.target,
        format!("{prefix}/assigned"),
        vec![OscArgument::Bool(definition.is_some())],
    );
    if let Some(binding) = binding {
        send_osc(
            feedback.state,
            feedback.subscriber.target,
            format!("{prefix}/owner"),
            vec![
                OscArgument::Int(i32::from(binding.anchor_slot)),
                OscArgument::String(
                    match binding.position {
                        ExpandedOscPosition::TallerUpper => "taller_upper",
                        ExpandedOscPosition::WiderRight => "wider_right",
                    }
                    .into(),
                ),
            ],
        );
    }
    let level = definition
        .map(|definition| {
            playback_level(
                feedback.state,
                address,
                definition,
                running,
                feedback.speed_groups,
            )
        })
        .unwrap_or(0.0);
    send_osc(
        feedback.state,
        feedback.subscriber.target,
        format!("{prefix}/fader"),
        vec![OscArgument::Float(level)],
    );
    send_cue_feedback(
        feedback.state,
        feedback.subscriber,
        &prefix,
        source_definition.is_some() && Some(address) == feedback.selected_playback,
        running,
    );
    send_button_feedback(
        feedback.state,
        feedback.subscriber,
        &prefix,
        definition,
        running,
    );
    send_runtime_feedback(
        feedback.state,
        feedback.subscriber,
        &prefix,
        definition,
        running,
    );
}

pub(super) fn send_playback_osc_feedback(feedback: OscPlaybackFeedback<'_>) {
    let mut positions = feedback
        .state
        .installation
        .playback_surfaces(feedback.desk.id)
        .ok()
        .and_then(|surfaces| surfaces.get(&feedback.desk.surface_id).cloned())
        .map(|surface| {
            surface
                .layout
                .rows
                .into_iter()
                .flat_map(|row| (0..row.count).map(move |column| row.first_position + column))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    positions.extend(
        feedback
            .snapshot
            .playback_pages
            .iter()
            .flat_map(|page| page.assignments.keys().copied()),
    );
    positions.sort_unstable();
    positions.dedup();
    for position in positions {
        send_slot_feedback(&feedback, position);
    }
    for (index, speed_group) in feedback.speed_groups.iter().copied().enumerate() {
        send_osc(
            feedback.state,
            feedback.subscriber.target,
            format!(
                "/light/{}/feedback/speed-group/{}",
                feedback.subscriber.path,
                index + 1
            ),
            speed_group_osc_feedback(speed_group),
        );
    }
}
