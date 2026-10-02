//! Load adapter-owned revision slots, then use the shared immutable reservation algorithm.
use super::*;
use light_application::mvr_import::{MvrProfileSlots, mvr_profile_identity, reserve_mvr_profiles};
use std::collections::BTreeSet;
/// Keep previewed contents immutable. Only allocate revision numbers against the current target.
/// A subsequent destination change is rejected by the existing atomic show commit boundary.
pub(in crate::runtime) fn prepare_mvr_profiles(
    state: &AppState,
    staged: &mut MvrDefinitions,
    destination_id: Option<Uuid>,
    resolutions: &HashMap<Uuid, MvrResolution>,
) -> Result<(), ApiError> {
    let ids = staged
        .definitions
        .values()
        .filter_map(|definition| definition.profile_id.map(|id| id.0))
        .collect::<BTreeSet<_>>();
    let mut slots = MvrProfileSlots::new();
    for id in ids {
        for (revision, document) in state
            .installation
            .fixture_profile_revision_documents(light_core::FixtureId(id))
            .map_err(ApiError::fixture)?
        {
            slots
                .entry((id, u64::from(revision)))
                .or_default()
                .insert(mvr_profile_identity(document).map_err(super::mvr_api_error)?);
        }
    }
    if let Some(id) = destination_id {
        let entry = state
            .installation
            .show(light_core::ShowId(id))
            .map_err(ApiError::store)?
            .ok_or_else(|| ApiError::not_found("show"))?;
        let document = ActiveShowRepository::open(entry.path)
            .map_err(ApiError::store)?
            .portable_document()
            .map_err(ApiError::store)?;
        let legacy = document
            .canonical_legacy_fixture_profile_revisions()
            .map_err(ApiError::store)?;
        for profile in document
            .fixture_profile_revisions()
            .iter()
            .chain(legacy.iter())
        {
            slots
                .entry((profile.id().profile_id().0, profile.id().revision()))
                .or_default()
                .insert(
                    mvr_profile_identity(profile.profile().clone())
                        .map_err(super::mvr_api_error)?,
                );
        }
    }
    let resolutions = super::super::mvr_apply::application_mvr_resolutions(resolutions.clone());
    reserve_mvr_profiles(staged, &mut slots, &resolutions).map_err(super::mvr_api_error)
}
