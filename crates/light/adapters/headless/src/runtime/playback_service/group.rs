use super::*;

pub(super) fn execute(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
    group_id: PlaybackGroupId,
    playback_number: Option<u16>,
    action: PlaybackAction,
    surface: PlaybackSurface,
) -> Result<PlaybackExecution, ActionError> {
    if let Some(number) = playback_number {
        return ports.execute_pool(
            context,
            ResolvedPlaybackAddress::Pool {
                number,
                page: None,
                slot: None,
            },
            action,
            surface,
        );
    }
    let changed = match action {
        PlaybackAction::Master(level) => set_master(ports, context, group_id, level)?,
        PlaybackAction::Flash { pressed } => set_flash(ports, group_id, pressed),
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

fn set_flash(ports: &ServerPlaybackPorts<'_>, group_id: PlaybackGroupId, pressed: bool) -> bool {
    let value = if pressed { 1.0 } else { 0.0 };
    if ports.state.engine.group_master_flash(group_id.as_str()) == value {
        return false;
    }
    ports
        .state
        .engine
        .set_group_master_flash(group_id.as_str().to_owned(), value);
    true
}
