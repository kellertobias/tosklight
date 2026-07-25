use super::super::*;

pub(super) struct MappedControlOrigin {
    context: light_application::ActionContext,
    desk: Option<ControlDesk>,
    surface: light_application::PlaybackSurface,
}

pub(super) fn mapped_control_origin(state: &AppState, event: &ControlEvent) -> MappedControlOrigin {
    let (source, desk, surface) = match event {
        ControlEvent::Osc { address, .. } => {
            let alias = address.trim_matches('/').split('/').nth(1);
            let desk = alias.and_then(|alias| osc_control_desk(state, alias));
            (
                light_application::ActionSource::Osc,
                desk,
                light_application::PlaybackSurface::Osc,
            )
        }
        ControlEvent::Midi { .. } => (
            light_application::ActionSource::Midi,
            None,
            light_application::PlaybackSurface::Physical,
        ),
        ControlEvent::Timecode(_) => (
            light_application::ActionSource::Timecode,
            None,
            light_application::PlaybackSurface::Physical,
        ),
    };
    let context = light_application::ActionContext::system(
        desk.as_ref().map(|desk| desk.id).unwrap_or_default(),
        source,
    );
    MappedControlOrigin {
        context,
        desk,
        surface,
    }
}

pub(super) fn apply_control_mappings<'a>(
    state: &AppState,
    origin: &MappedControlOrigin,
    actions: impl IntoIterator<Item = &'a ControlAction>,
) {
    let mut grand_master = None;
    let mut blackout = None;
    for action in actions {
        let result = match action {
            ControlAction::CueGo { cue_list_id } => apply_cue(
                state,
                origin,
                *cue_list_id,
                PlaybackAction::Go { pressed: true },
            ),
            ControlAction::CueBack { cue_list_id } => apply_cue(
                state,
                origin,
                *cue_list_id,
                PlaybackAction::Back { pressed: true },
            ),
            ControlAction::CuePause { cue_list_id } => apply_cue(
                state,
                origin,
                *cue_list_id,
                PlaybackAction::Pause { pressed: true },
            ),
            ControlAction::CueRelease { cue_list_id } => {
                apply_cue(state, origin, *cue_list_id, PlaybackAction::Release)
            }
            ControlAction::Blackout { enabled } => {
                blackout = Some(*enabled);
                Ok(())
            }
            ControlAction::GrandMaster { level } => {
                grand_master = Some(level.clamp(0.0, 1.0));
                Ok(())
            }
            ControlAction::DeskSet => {
                emit(state, "desk_action", serde_json::json!({"action":"set"}));
                Ok(())
            }
        };
        report_rejection(result);
    }
    if grand_master.is_some() || blackout.is_some() {
        let command = output_runtime_service::command(grand_master, blackout);
        report_rejection(apply_output(state, origin, command));
    }
}

fn apply_cue(
    state: &AppState,
    origin: &MappedControlOrigin,
    cue_list_id: light_core::CueListId,
    action: PlaybackAction,
) -> Result<(), ApiError> {
    playback_service::execute(
        state,
        None,
        origin.desk.as_ref(),
        origin.context.clone(),
        light_application::PlaybackCommand {
            address: PlaybackAddress::CueList(cue_list_id),
            action,
            surface: origin.surface,
        },
    )?;
    Ok(())
}

fn apply_output(
    state: &AppState,
    origin: &MappedControlOrigin,
    command: Result<light_application::OutputRuntimeCommand, ApiError>,
) -> Result<(), ApiError> {
    output_runtime_service::execute(state, None, origin.context.clone(), command?)?;
    Ok(())
}

fn report_rejection(result: Result<(), ApiError>) {
    if let Err(error) = result {
        tracing::warn!(error=%error.message, "mapped control action was rejected");
    }
}
