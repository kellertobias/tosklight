use super::model::PlannedFixture;
use crate::{ActionError, ActionErrorKind, PatchModeProjection, PatchProfileRevisionProjection};
use light_fixture::{PatchedFixture, PortablePatchedFixtureRecord, migrate_patched_fixture_to_v2};
use light_show::FixtureProfileRevision;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct ProjectionCache(BTreeMap<String, Arc<PatchProfileRevisionProjection>>);

pub(super) fn project_fixture(
    mut fixture: PatchedFixture,
    cache: &mut ProjectionCache,
) -> Result<PlannedFixture, ActionError> {
    // The archive is immutable and shared; normalization/patch extraction needs no copy of it.
    let source = fixture
        .definition
        .profile_snapshot
        .as_mut()
        .and_then(|profile| profile.source_gdtf.take());
    migrate_patched_fixture_to_v2(&mut fixture).map_err(invalid)?;
    let record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).map_err(invalid)?;
    if let Some(profile) = fixture.definition.profile_snapshot.as_mut() {
        profile.source_gdtf = source;
    }
    let profile = record
        .selected_profile_reference()
        .map_err(invalid)?
        .ok_or_else(|| invalid("imported fixture has no portable profile identity"))?;
    let patch = record.patch().map_err(invalid)?;
    let snapshot = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or_else(|| invalid("imported fixture has no portable profile snapshot"))?;
    let source_key = snapshot.source_gdtf.as_ref().map(|source| {
        (
            // Pointer identity is a per-import cache key only, never persisted. A different archive
            // allocation is validated separately, even when it claims the same content hash.
            Arc::as_ptr(&source.archive_asset) as *const u8 as usize,
            source.version,
            &source.archive_sha256,
            &source.profile_fingerprint,
        )
    });
    let key = format!(
        "{}:{}:{}:{source_key:?}",
        light_fixture::fixture_profile_source_fingerprint(snapshot).map_err(invalid)?,
        snapshot.revision,
        profile.mode_id
    );
    if let Some(projection) = cache.0.get(&key) {
        return Ok(PlannedFixture {
            profile,
            patch,
            profile_projection: Arc::clone(projection),
            record: record.into_body(),
        });
    }
    snapshot.validate().map_err(invalid)?;
    let stored =
        FixtureProfileRevision::from_profile(serde_json::to_value(snapshot).map_err(invalid)?)
            .map_err(invalid)?;
    let mode = snapshot
        .mode(profile.mode_id)
        .ok_or_else(|| invalid("imported fixture profile does not contain its selected mode"))?;
    let projection = PatchProfileRevisionProjection {
        profile_id: stored.id().profile_id(),
        profile_revision: stored.id().revision(),
        content_digest: stored.digest().as_str().to_owned(),
        manufacturer: snapshot.manufacturer.clone(),
        name: snapshot.name.clone(),
        fixture_type: snapshot.fixture_type.clone(),
        patch_policy: snapshot.patch_policy,
        referenced_modes: vec![PatchModeProjection {
            position_calibration_identity: snapshot
                .position_calibration_identity(mode.id)
                .map_err(invalid)?,
            mode_id: mode.id,
            name: mode.name.clone(),
            splits: mode.splits.clone(),
            native_color_identities: if mode.color_physical.is_some() {
                snapshot.native_color_identities(mode.id).map_err(invalid)?
            } else {
                vec![]
            },
        }],
        profile_snapshot: stored.profile().clone(),
    };
    let projection = Arc::new(projection);
    cache.0.insert(key, Arc::clone(&projection));
    Ok(PlannedFixture {
        profile,
        patch,
        profile_projection: projection,
        record: record.into_body(),
    })
}

pub(super) fn profile_projections(
    fixtures: &[PlannedFixture],
) -> Result<Vec<PatchProfileRevisionProjection>, ActionError> {
    let mut profiles = BTreeMap::new();
    for fixture in fixtures {
        let projection = fixture.profile_projection.as_ref();
        let key = (projection.profile_id.0, projection.profile_revision);
        if let Some(existing) = profiles.get(&key) {
            let existing: &PatchProfileRevisionProjection = existing;
            if existing.content_digest != projection.content_digest {
                return Err(invalid(format!(
                    "MVR contains conflicting contents for profile {} revision {}",
                    key.0, key.1
                )));
            }
        }
        profiles
            .entry(key)
            .and_modify(|existing: &mut PatchProfileRevisionProjection| {
                for mode in &projection.referenced_modes {
                    if !existing
                        .referenced_modes
                        .iter()
                        .any(|item| item.mode_id == mode.mode_id)
                    {
                        existing.referenced_modes.push(mode.clone());
                    }
                }
            })
            .or_insert_with(|| projection.clone());
    }
    Ok(profiles.into_values().collect())
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}
