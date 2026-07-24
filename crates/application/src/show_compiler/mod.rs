mod migrations;
mod objects;
mod patch;
mod prepare;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use migrations::stage_candidate_migrations;
pub use prepare::{
    PreparedShowCandidate, prepare_normalized_show_candidate_incremental, prepare_show_candidate,
};
pub(crate) use prepare::{
    prepare_show_candidate_exact_transaction, prepare_show_candidate_preserving_object,
};

use crate::{ActionError, ActionErrorKind};
use light_engine::EngineSnapshot;
use light_show::PortableShowCandidate;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ShowCompileDirty {
    pub(crate) fixtures: bool,
    pub(crate) cue_lists: bool,
    pub(crate) playbacks: bool,
    pub(crate) playback_pages: bool,
    pub(crate) routes: bool,
    pub(crate) control_mappings: bool,
    pub(crate) groups: bool,
}

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
        fixtures: fixtures.into(),
        cue_lists: cue_lists.into(),
        playbacks: playbacks.into(),
        playback_pages: playback_pages.into(),
        routes: routes.into(),
        control_mappings: control_mappings.into(),
        groups: groups.into(),
        revision: candidate.revision().value(),
    })
}

fn invalid_candidate(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

/// Compiles only the portable projections affected by one normalized active-show transaction.
///
/// The initial show-open path remains the compatibility oracle and always performs the full
/// migration and compile pass. Once that normalized document is live, ordinary typed mutations
/// can structurally share every unaffected projection with the previous runtime generation.
fn compile_show_candidate_incremental(
    candidate: PortableShowCandidate<'_>,
    previous: &EngineSnapshot,
    dirty: ShowCompileDirty,
) -> Result<EngineSnapshot, ActionError> {
    let mut snapshot = previous.clone();
    snapshot.revision = candidate.revision().value();

    if dirty.cue_lists {
        snapshot.cue_lists = objects::decode(candidate, "cue_list")?.into();
    }
    if dirty.groups {
        snapshot.groups = objects::decode_groups(candidate)?.into();
    }
    if dirty.routes {
        snapshot.routes = objects::decode(candidate, "route")?.into();
    }
    if dirty.control_mappings {
        snapshot.control_mappings = objects::decode(candidate, "control_mapping")?.into();
    }
    if dirty.fixtures {
        snapshot.fixtures = patch::compile_patch(candidate)?.into();
    }

    // Defaults couple these three small topology projections. Recompile them together whenever
    // any member changes, while leaving patch, geometry, routes, mappings, and groups shared.
    if dirty.cue_lists || dirty.playbacks || dirty.playback_pages {
        let mut playbacks = objects::decode(candidate, "playback")?;
        let mut playback_pages = objects::decode(candidate, "playback_page")?;
        objects::supply_playback_defaults(
            snapshot.cue_lists.as_slice(),
            &mut playbacks,
            &mut playback_pages,
        );
        snapshot.playbacks = playbacks.into();
        snapshot.playback_pages = playback_pages.into();
    }

    Ok(snapshot)
}
