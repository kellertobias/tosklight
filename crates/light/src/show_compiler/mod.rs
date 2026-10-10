mod cue_presets;
mod dynamic_presets;
mod migrations;
mod native_sources;
mod objects;
mod patch;
mod prepare;
mod replacement_projection;

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
    pub(crate) native_sources: bool,
    pub(crate) cue_lists: bool,
    pub(crate) dynamics: bool,
    pub(crate) presets: bool,
    pub(crate) stage_layouts: bool,
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
    replacement_projection::validate(candidate)?;
    let fixtures = patch::compile_patch(candidate)?;
    let cue_lists = objects::decode_cue_lists(candidate)?;
    let groups = objects::decode_groups(candidate)?;
    let (dynamics, required_programming_contract) = objects::decode_dynamics(candidate, &groups)?;
    let dynamic_stage_positions = objects::decode_dynamic_stage_positions(candidate)?;
    let mut playbacks = objects::decode(candidate, "playback")?;
    let mut playback_pages = objects::decode(candidate, "playback_page")?;
    let routes = objects::decode(candidate, "route")?;
    let control_mappings = objects::decode(candidate, "control_mapping")?;
    validate_cuelist_namespace(&cue_lists)?;
    light_playback::CueListPoolCatalog::build(&cue_lists, &playbacks, playback_pages.is_empty())
        .map_err(|error| crate::ActionError::new(crate::ActionErrorKind::Invalid, error))?;
    objects::supply_playback_defaults(&cue_lists, &mut playbacks, &mut playback_pages);
    Ok(EngineSnapshot {
        required_programming_contract,
        native_color_sources: native_sources::compile(candidate, None)?,
        fixtures: fixtures.into(),
        cue_lists: cue_lists.into(),
        dynamics: dynamics.into(),
        dynamic_stage_positions: dynamic_stage_positions.into(),
        playbacks: playbacks.into(),
        playback_pages: playback_pages.into(),
        routes: routes.into(),
        control_mappings: control_mappings.into(),
        groups: groups.into(),
        revision: candidate.revision().value(),
    })
}

fn validate_cuelist_namespace(lists: &[light_playback::CueList]) -> Result<(), ActionError> {
    if lists.iter().any(|list| list.pool_number.is_some())
        && lists.iter().any(|list| list.pool_number.is_none())
    {
        return Err(invalid_candidate(
            "Cuelist address metadata must migrate atomically for every Cuelist",
        ));
    }
    Ok(())
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
    if dirty.cue_lists || dirty.presets || dirty.groups {
        replacement_projection::validate(candidate)?;
    }
    let mut snapshot = previous.clone();
    snapshot.revision = candidate.revision().value();

    if dirty.native_sources || !snapshot.native_color_sources.is_prepared() {
        snapshot.native_color_sources =
            native_sources::compile(candidate, Some(&previous.native_color_sources))?;
    }

    if dirty.cue_lists
        || ((dirty.presets
            || dirty.native_sources
            || (dirty.fixtures
                && candidate.objects_of_kind("preset").any(|object| {
                    object
                        .body()
                        .get("aim_at_fixture_number")
                        .is_some_and(|value| value.is_number())
                })))
            && previous.cue_lists.iter().any(|list| {
                list.required_programming_contract()
                    >= light_core::programming::LIVE_PRESET_REFERENCE_CONTRACT
            }))
    {
        snapshot.cue_lists = objects::decode_cue_lists(candidate)?.into();
    }
    if dirty.dynamics || dirty.presets || dirty.groups || dirty.cue_lists {
        let groups = if dirty.groups {
            objects::decode_groups(candidate)?
        } else {
            snapshot.groups.as_ref().clone()
        };
        let (dynamics, required) = objects::decode_dynamics(candidate, &groups)?;
        snapshot.dynamics = dynamics.into();
        snapshot.required_programming_contract = required;
    }
    if dirty.stage_layouts {
        snapshot.dynamic_stage_positions =
            objects::decode_dynamic_stage_positions(candidate)?.into();
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
        validate_cuelist_namespace(snapshot.cue_lists.as_slice())?;
        light_playback::CueListPoolCatalog::build(
            snapshot.cue_lists.as_slice(),
            &playbacks,
            playback_pages.is_empty(),
        )
        .map_err(|error| crate::ActionError::new(crate::ActionErrorKind::Invalid, error))?;
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
