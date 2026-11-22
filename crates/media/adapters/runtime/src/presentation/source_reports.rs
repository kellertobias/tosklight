//! Source lifecycle publication guarded by the state used for rendering.

use media_domain::{MediaState, Timestamp};

/// Applies the renderer's source reports to the authoritative state.
///
/// Returns the next state when anything changed, so a frame in which nothing loaded or failed
/// publishes nothing at all rather than churning a snapshot every sixtieth of a second.
#[cfg(test)]
pub(crate) fn with_reports(
    state: &MediaState,
    reports: &[(media_domain::OutputId, usize, media_domain::SourceStatus)],
    now: Timestamp,
) -> Option<MediaState> {
    with_reports_from(state, reports, now, state)
}

/// Rejects a renderer report if the selected source or its transport changed while rendering.
pub(crate) fn with_reports_from(
    state: &MediaState,
    reports: &[(media_domain::OutputId, usize, media_domain::SourceStatus)],
    now: Timestamp,
    rendered: &MediaState,
) -> Option<MediaState> {
    let mut next = MediaState::clone(state);
    let mut changed = false;
    for (output, layer, status) in reports {
        let current = state
            .output(*output)
            .and_then(|output| output.layer(*layer));
        let origin = rendered
            .output(*output)
            .and_then(|output| output.layer(*layer));
        if current.zip(origin).is_none_or(|(current, origin)| {
            current.address != origin.address
                || current.reset_trigger_id != origin.reset_trigger_id
                || current.play_mode != origin.play_mode
        }) {
            continue;
        }
        let command = media_domain::Command::new(
            media_domain::CommandKind::ReportSourceStatus {
                output: *output,
                layer: *layer,
                status: *status,
            },
            media_domain::CommandSource::Internal,
            now,
        );
        if media_domain::apply(&mut next, &command) == media_domain::Applied::Changed {
            changed = true;
        }
    }
    changed.then_some(next)
}
