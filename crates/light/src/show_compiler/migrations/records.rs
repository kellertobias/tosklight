use super::invalid_object;
use crate::ActionError;
use light_fixture::PortablePatchedFixtureRecord;
use light_show::{PortableShowCandidate, PortableShowCandidateObject};
use std::collections::HashSet;

pub(super) struct DecodedFixtureRecord<'a> {
    pub(super) object: PortableShowCandidateObject<'a>,
    pub(super) record: PortablePatchedFixtureRecord,
}

pub(super) fn decode_unique_records(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<DecodedFixtureRecord<'_>>, ActionError> {
    let mut fixture_ids = HashSet::new();
    candidate
        .objects_of_kind("patched_fixture")
        .map(|object| decode_unique_record(object, &mut fixture_ids))
        .collect()
}

fn decode_unique_record<'a>(
    object: PortableShowCandidateObject<'a>,
    fixture_ids: &mut HashSet<light_core::FixtureId>,
) -> Result<DecodedFixtureRecord<'a>, ActionError> {
    let record = PortablePatchedFixtureRecord::decode(object.body().clone())
        .map_err(|error| invalid_object(object, error))?;
    let fixture_id = record
        .fixture_id()
        .map_err(|error| invalid_object(object, error))?;
    if !fixture_ids.insert(fixture_id) {
        return Err(invalid_object(
            object,
            format!("duplicate fixture identity {}", fixture_id.0),
        ));
    }
    Ok(DecodedFixtureRecord { object, record })
}
