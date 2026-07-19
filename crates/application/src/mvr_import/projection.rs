use super::model::PlannedFixture;
use crate::{ActionError, ActionErrorKind, PatchModeProjection, PatchProfileRevisionProjection};
use light_fixture::{
    FixtureLocation, FixtureVector, PatchedFixture, PortablePatchedFixtureRecord,
    migrate_patched_fixture_to_v2,
};
use light_show::FixtureProfileRevision;
use std::collections::BTreeMap;

pub(super) fn project_fixture(mut fixture: PatchedFixture) -> Result<PlannedFixture, ActionError> {
    migrate_patched_fixture_to_v2(&mut fixture).map_err(invalid)?;
    let body = serde_json::to_value(&fixture).map_err(invalid)?;
    let record = PortablePatchedFixtureRecord::decode(body).map_err(invalid)?;
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
            mode_id: mode.id,
            name: mode.name.clone(),
            splits: mode.splits.clone(),
        }],
    };
    Ok(PlannedFixture {
        profile,
        patch,
        profile_projection: projection,
    })
}

pub(super) fn profile_projections(
    fixtures: &[PlannedFixture],
) -> Vec<PatchProfileRevisionProjection> {
    let mut profiles = BTreeMap::new();
    for fixture in fixtures {
        let projection = fixture.profile_projection.clone();
        let key = (projection.profile_id.0, projection.profile_revision);
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
            .or_insert(projection);
    }
    profiles.into_values().collect()
}

pub(super) fn mvr_transform(matrix: [f64; 12]) -> (FixtureLocation, FixtureVector) {
    let location = FixtureLocation {
        x: matrix[9]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        y: matrix[10]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        z: matrix[11]
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
    };
    let rotation = FixtureVector {
        x: matrix[9].atan2(matrix[10]).to_degrees() as f32,
        y: (-matrix[8].asin().to_degrees()) as f32,
        z: matrix[4].atan2(matrix[0]).to_degrees() as f32,
    };
    (location, rotation)
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}
