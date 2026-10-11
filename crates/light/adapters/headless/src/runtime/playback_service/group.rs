use super::*;

pub(super) fn execute(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
    group_id: PlaybackGroupId,
    _playback_address: Option<light_playback::PlaybackAddress>,
    action: PlaybackAction,
    control_source: &light_playback::PlaybackControlSource,
) -> Result<PlaybackExecution, ActionError> {
    // A direct Group operand controls the shared target, irrespective of which
    // assignment currently displays it. Pickup belongs only to addressed faders.
    let changed = match action {
        PlaybackAction::Master(level) => set_master(ports, context, group_id, level)?,
        PlaybackAction::MasterTransition {
            level,
            duration_millis,
        } => {
            let changed = ports
                .state
                .output
                .set_group_master_transition(
                    group_id.as_str(),
                    level.value(),
                    u64::from(duration_millis),
                )
                .map_err(|error| invalid(error.to_string()))?;
            if changed && let Err(error) = persist_output_runtime(ports.state) {
                ports.mark_persistence_pending(context, "output_runtime", error);
            }
            changed
        }
        PlaybackAction::Flash { pressed } => set_flash(
            ports,
            group_id,
            &control_source
                .scoped_to_desk(context.desk_id)
                .map_err(invalid)?,
            pressed,
        ),
        _ => return Err(invalid("action is incompatible with Group runtime")),
    };
    Ok(PlaybackExecution::Target { changed })
}

fn set_master(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
    group_id: PlaybackGroupId,
    level: light_application::PlaybackLevel,
) -> Result<bool, ActionError> {
    let changed = set_group_playback_master(ports.state, group_id.as_str(), level.value())
        .map_err(api_action_error)?;
    if changed && let Err(error) = persist_output_runtime(ports.state) {
        ports.mark_persistence_pending(context, "output_runtime", error);
    }
    Ok(changed)
}

fn set_flash(
    ports: &ServerPlaybackPorts<'_>,
    group_id: PlaybackGroupId,
    source: &light_playback::PlaybackControlSource,
    pressed: bool,
) -> bool {
    let before = ports.state.output.group_master_flash(group_id.as_str());
    ports
        .state
        .output
        .set_group_master_flash_source(group_id.as_str().to_owned(), source, pressed);
    ports.state.output.group_master_flash(group_id.as_str()) != before
}
