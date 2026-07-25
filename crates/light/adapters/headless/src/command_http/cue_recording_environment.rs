use light_application::{
    ActionContext, ActionError, ActionErrorKind, PlaybackCueReference, ProgrammingCuePageSlot,
    ProgrammingCueRecordOperation, ProgrammingCueRecordRequest, ProgrammingCueRecordTarget,
    ProgrammingCueRecordingEnvironment, ProgrammingCueResolvedTarget,
};

use super::super::AppState;

pub(super) fn environment(
    state: &AppState,
    context: &ActionContext,
    request: &ProgrammingCueRecordRequest,
) -> Result<ProgrammingCueRecordingEnvironment, ActionError> {
    let snapshot = state.engine.snapshot();
    let resolved = resolve_target(state, context, request.target, &snapshot)?;
    Ok(ProgrammingCueRecordingEnvironment {
        target: resolved,
        active_cue: if needs_active_cue(request) {
            active_cue(state, resolved)?
        } else {
            None
        },
    })
}

fn needs_active_cue(request: &ProgrammingCueRecordRequest) -> bool {
    request.operation == ProgrammingCueRecordOperation::Merge && request.cue_number.is_none()
}

fn resolve_target(
    state: &AppState,
    context: &ActionContext,
    target: ProgrammingCueRecordTarget,
    snapshot: &light_engine::EngineSnapshot,
) -> Result<ProgrammingCueResolvedTarget, ActionError> {
    match target {
        ProgrammingCueRecordTarget::Pool { playback_number } => {
            Ok(ProgrammingCueResolvedTarget::Playback {
                playback_number,
                page_slot: None,
            })
        }
        ProgrammingCueRecordTarget::SelectedPlayback => selected_playback(state, context),
        ProgrammingCueRecordTarget::PageSlot { page, slot } => {
            page_slot(snapshot, ProgrammingCuePageSlot { page, slot })
        }
        ProgrammingCueRecordTarget::CueList { cue_list_id } => {
            Ok(ProgrammingCueResolvedTarget::CueList { cue_list_id })
        }
    }
}

fn selected_playback(
    state: &AppState,
    context: &ActionContext,
) -> Result<ProgrammingCueResolvedTarget, ActionError> {
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| not_found("no show is open"))?;
    let playback_number = state
        .desk
        .lock()
        .selected_playback(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?
        .ok_or_else(|| not_found("no playback is selected"))?;
    Ok(ProgrammingCueResolvedTarget::Playback {
        playback_number,
        page_slot: None,
    })
}

fn page_slot(
    snapshot: &light_engine::EngineSnapshot,
    page_slot: ProgrammingCuePageSlot,
) -> Result<ProgrammingCueResolvedTarget, ActionError> {
    let playback = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == page_slot.page)
        .and_then(|page| page.slots.get(&page_slot.slot))
        .copied();
    let Some(playback_number) = playback else {
        return Ok(ProgrammingCueResolvedTarget::EmptyPageSlot(page_slot));
    };
    if !snapshot
        .playbacks
        .iter()
        .any(|playback| playback.number == playback_number)
    {
        return Err(invalid(format!(
            "Playback page {} slot {} references missing playback {playback_number}",
            page_slot.page, page_slot.slot
        )));
    }
    Ok(ProgrammingCueResolvedTarget::Playback {
        playback_number,
        page_slot: Some(page_slot),
    })
}

fn active_cue(
    state: &AppState,
    target: ProgrammingCueResolvedTarget,
) -> Result<Option<PlaybackCueReference>, ActionError> {
    let runtime = state.engine.playback_runtime_status();
    let mut candidates = runtime.iter().filter(|status| match target {
        ProgrammingCueResolvedTarget::Playback {
            playback_number, ..
        } => status.playback.playback_number == Some(playback_number),
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => {
            status.playback.cue_list_id == cue_list_id
        }
        ProgrammingCueResolvedTarget::EmptyPageSlot(_) => false,
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
        .zip(status.playback.current_cue_number)
        .map(|(id, number)| PlaybackCueReference { id, number })
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_page_or_slot_resolves_to_exact_empty_slot() {
        let snapshot = light_engine::EngineSnapshot::default();
        let requested = ProgrammingCuePageSlot { page: 7, slot: 3 };
        assert_eq!(
            page_slot(&snapshot, requested).unwrap(),
            ProgrammingCueResolvedTarget::EmptyPageSlot(requested)
        );
    }
}
