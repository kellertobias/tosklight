mod migrations;
mod objects;
mod patch;
mod prepare;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use migrations::stage_candidate_migrations;
pub(crate) use prepare::prepare_show_candidate_preserving_object;
pub use prepare::{PreparedShowCandidate, prepare_show_candidate};

use crate::{ActionError, ActionErrorKind};
use light_engine::EngineSnapshot;
use light_show::PortableShowCandidate;

/// Compiles one already-migrated portable candidate into the immutable runtime snapshot.
pub(crate) fn compile_show_candidate(
    candidate: PortableShowCandidate<'_>,
) -> Result<EngineSnapshot, ActionError> {
    let fixtures = patch::compile_patch(candidate)?;
    let cue_lists = objects::decode(candidate, "cue_list")?;
    let mut playbacks = objects::decode(candidate, "playback")?;
    let mut playback_pages = objects::decode(candidate, "playback_page")?;
    let routes = objects::decode(candidate, "route")?;
    let control_mappings = objects::decode(candidate, "control_mapping")?;
    let groups = objects::decode_groups(candidate)?;
    objects::supply_playback_defaults(&cue_lists, &mut playbacks, &mut playback_pages);
    Ok(EngineSnapshot {
        fixtures,
        cue_lists,
        playbacks,
        playback_pages,
        routes,
        control_mappings,
        groups,
        revision: candidate.revision().value(),
    })
}

fn invalid_candidate(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
