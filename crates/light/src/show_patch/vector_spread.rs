use super::{PatchFixtureCandidate, PatchVectorAxis, PatchVectorKind, PatchVectorSpreadIntent};
use crate::{ActionError, ActionErrorKind};
use std::collections::HashMap;

pub(super) fn apply_vector_spreads(
    mut fixtures: Vec<PatchFixtureCandidate>,
    spreads: &[PatchVectorSpreadIntent],
) -> Result<Vec<PatchFixtureCandidate>, ActionError> {
    let indexes: HashMap<_, _> = fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| (fixture.patch.fixture_id, index))
        .collect();
    for spread in spreads {
        let count = spread.fixture_ids.len();
        for (selection_index, fixture_id) in spread.fixture_ids.iter().enumerate() {
            let index = indexes.get(fixture_id).copied().ok_or_else(|| {
                invalid("patch vector spread fixture identities must belong to the upsert batch")
            })?;
            let value = light_core::spread_position(&spread.points, selection_index, count);
            let fixture = &mut fixtures[index].patch;
            match (spread.kind, spread.axis) {
                (PatchVectorKind::Location, PatchVectorAxis::X) => {
                    fixture.location.x = location_coordinate(value)?
                }
                (PatchVectorKind::Location, PatchVectorAxis::Y) => {
                    fixture.location.y = location_coordinate(value)?
                }
                (PatchVectorKind::Location, PatchVectorAxis::Z) => {
                    fixture.location.z = location_coordinate(value)?
                }
                (PatchVectorKind::Rotation, PatchVectorAxis::X) => fixture.rotation.x = value,
                (PatchVectorKind::Rotation, PatchVectorAxis::Y) => fixture.rotation.y = value,
                (PatchVectorKind::Rotation, PatchVectorAxis::Z) => fixture.rotation.z = value,
            }
        }
    }
    Ok(fixtures)
}

fn location_coordinate(value: f32) -> Result<i32, ActionError> {
    if value < i32::MIN as f32 || value > i32::MAX as f32 {
        return Err(invalid(
            "patch location spread exceeds the supported coordinate range",
        ));
    }
    Ok(value.round() as i32)
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
