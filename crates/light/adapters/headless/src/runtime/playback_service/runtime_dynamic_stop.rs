//! Exact Running-row owner validation. The caller holds PlaybackService's operation lock.
use super::*;
use light_dynamics::DynamicControllerSource;
use light_playback::PlaybackTarget;
use uuid::Uuid;

pub(super) fn validate(
    state: &AppState,
    address: &ResolvedPlaybackAddress,
    dynamic_id: Uuid,
    instance_id: Uuid,
    controller_id: Uuid,
) -> Result<(), ActionError> {
    let conflict = || {
        ActionError::new(
            ActionErrorKind::Conflict,
            "The running Dynamic owner changed. Refresh Running and retry.",
        )
    };
    let ResolvedPlaybackAddress::Playback(identity) = address else {
        return Err(conflict());
    };
    let identity = *identity;
    let definition = playback_definition(state, identity)?;
    let source = DynamicControllerSource::Playback {
        page: identity.page(),
        position: identity.position(),
    };
    let PlaybackTarget::Dynamic { assignment } = &definition.target else {
        return Err(conflict());
    };
    if assignment.target_id() != dynamic_id {
        return Err(conflict());
    }
    let active = state
        .output
        .active_dynamic_playback(identity)
        .ok_or_else(conflict)?;
    if !active.enabled || active.dynamic_id != Some(dynamic_id) {
        return Err(conflict());
    }
    let runtime = state.output.dynamic_runtime_snapshot();
    let instance = runtime
        .instances
        .iter()
        .find(|instance| instance.id == instance_id && instance.definition.id == dynamic_id)
        .ok_or_else(conflict)?;
    if !instance
        .controllers
        .iter()
        .any(|controller| controller.id == controller_id && controller.source == source)
    {
        return Err(conflict());
    }
    Ok(())
}
