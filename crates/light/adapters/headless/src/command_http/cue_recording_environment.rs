use light_application::{
    ActionContext, ActionError, ActionErrorKind, PlaybackCueReference, ProgrammingCueRecordRequest,
    ProgrammingCueRecordTarget, ProgrammingCueRecordingEnvironment, ProgrammingCueResolvedTarget,
};

use super::super::AppState;

pub(super) fn environment(
    state: &AppState,
    context: &ActionContext,
    request: &ProgrammingCueRecordRequest,
) -> Result<ProgrammingCueRecordingEnvironment, ActionError> {
    let snapshot = state.output.snapshot();
    let resolved = resolve_target(state, context, request.target, &snapshot)?;
    let configuration = state.installation.configuration();
    Ok(ProgrammingCueRecordingEnvironment {
        target: resolved,
        active_cue: if needs_active_cue(request) {
            active_cue(state, resolved)?
        } else {
            None
        },
        cuelist_auto_off_at_zero_default: configuration.cuelist_auto_off_at_zero_default,
        cuelist_auto_off_flash_release_default: configuration
            .cuelist_auto_off_flash_release_default,
        start_after_first_recording: configuration.start_after_first_recording,
    })
}

fn needs_active_cue(request: &ProgrammingCueRecordRequest) -> bool {
    request.operation.targets_active_cue() && request.cue_number.is_none()
}

fn resolve_target(
    state: &AppState,
    context: &ActionContext,
    target: ProgrammingCueRecordTarget,
    _snapshot: &light_engine::EngineSnapshot,
) -> Result<ProgrammingCueResolvedTarget, ActionError> {
    Ok(match target {
        ProgrammingCueRecordTarget::CuelistPool { number } => {
            ProgrammingCueResolvedTarget::CuelistPool { number }
        }
        ProgrammingCueRecordTarget::Playback { address } => {
            ProgrammingCueResolvedTarget::Playback { address }
        }
        ProgrammingCueRecordTarget::CueList { cue_list_id } => {
            ProgrammingCueResolvedTarget::CueList { cue_list_id }
        }
        ProgrammingCueRecordTarget::SelectedPlayback => return selected_playback(state, context),
    })
}
fn selected_playback(
    state: &AppState,
    context: &ActionContext,
) -> Result<ProgrammingCueResolvedTarget, ActionError> {
    let show = state
        .active_show
        .current()
        .clone()
        .ok_or_else(|| not_found("no show is open"))?;
    if let Some(cue_list_id) = state
        .installation
        .selected_cue_list(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?
    {
        return Ok(ProgrammingCueResolvedTarget::CueList { cue_list_id });
    }
    let selection = state
        .installation
        .selected_playback(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?
        .ok_or_else(|| not_found("No Playback selected"))?;
    Ok(ProgrammingCueResolvedTarget::Playback {
        address: light_playback::PlaybackAddress::new(selection.page, selection.position)
            .map_err(invalid)?,
    })
}

fn active_cue(
    state: &AppState,
    target: ProgrammingCueResolvedTarget,
) -> Result<Option<PlaybackCueReference>, ActionError> {
    let pool_id = if let ProgrammingCueResolvedTarget::CuelistPool { number } = target {
        let show = state
            .active_show
            .current()
            .clone()
            .ok_or_else(|| not_found("no show is open"))?;
        let document = crate::runtime::ActiveShowRepository::open(&show.path)
            .and_then(|store| store.portable_document())
            .map_err(|error| invalid(error.to_string()))?;
        light_application::cuelist_pool_catalog(&document)?.resolve(number)
    } else {
        None
    };
    let runtime = state.output.playback_runtime_status();
    let mut candidates = runtime.iter().filter(|status| match target {
        ProgrammingCueResolvedTarget::Playback { address } => {
            status.playback.playback_address == Some(address)
        }
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => {
            status.playback.cue_list_id == cue_list_id
        }
        ProgrammingCueResolvedTarget::CuelistPool { .. } => {
            pool_id == Some(status.playback.cue_list_id)
        }
    });
    let first = candidates.find_map(runtime_current_cue);
    if candidates
        .filter_map(runtime_current_cue)
        .any(|candidate| Some(candidate) != first)
    {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            "the Cuelist is active on multiple different Cues; supply an explicit Cue number",
        ));
    }
    Ok(first)
}

fn runtime_current_cue(
    status: &light_playback::PlaybackRuntimeStatus,
) -> Option<PlaybackCueReference> {
    status
        .playback
        .current_cue_id
        .zip(status.playback.current_cue_number.clone())
        .map(|(id, number)| PlaybackCueReference { id, number })
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
