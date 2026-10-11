use crate::{ActionError, ActionErrorKind};
use light_playback::{PlaybackPage, PlaybackTarget};
use light_show::{PortableShowCandidate, PortableShowTransaction};

/// Only explicit target deletions are guarded. Existing orphaned references remain loadable,
/// and clearing or retargeting assignments in this same transaction removes the dependency.
pub(super) fn validate(
    candidate: PortableShowCandidate<'_>,
    transaction: &PortableShowTransaction,
) -> Result<(), ActionError> {
    let deleted = transaction.object_deletes().collect::<Vec<_>>();
    if !deleted.iter().any(|key| {
        matches!(
            key.kind(),
            "group" | "cue_list" | "macro" | "timecode" | "dynamic"
        )
    }) {
        return Ok(());
    }
    for object in candidate.objects_of_kind("playback_page") {
        let page: PlaybackPage = serde_json::from_value(object.body().clone())
            .map_err(|error| super::invalid_candidate(format!("invalid Playback page: {error}")))?;
        let mut assignments = page.assignments.iter().collect::<Vec<_>>();
        assignments.sort_unstable_by_key(|(position, _)| **position);
        for (position, playback) in assignments {
            let Some((kind, id)) = referenced_target(&playback.target) else {
                continue;
            };
            if deleted
                .iter()
                .any(|key| key.kind() == kind && key.id() == id)
            {
                return Err(ActionError::new(
                    ActionErrorKind::Conflict,
                    format!(
                        "Cannot delete {kind} {id}: Playback {}.{position} uses it. Clear or retarget that Playback first.",
                        page.number,
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn referenced_target(target: &PlaybackTarget) -> Option<(&'static str, String)> {
    match target {
        PlaybackTarget::Group { group_id, .. } => Some(("group", group_id.clone())),
        PlaybackTarget::CueList { cue_list_id } => Some(("cue_list", cue_list_id.0.to_string())),
        PlaybackTarget::Macro { macro_id } => Some(("macro", macro_id.to_string())),
        PlaybackTarget::Timecode { timecode_id } => Some(("timecode", timecode_id.0.to_string())),
        // A detached Dynamic continues from its valid embedded definition after pool deletion.
        PlaybackTarget::Dynamic { assignment } => assignment
            .dynamic
            .dynamic_id
            .map(|id| ("dynamic", id.to_string())),
        _ => None,
    }
}
