use super::invalid_candidate;
use crate::ActionError;
use light_fixture::{
    PatchedFixture, PatchedFixtureCompiler, PortablePatchedFixtureRecord,
    ResolvedFixtureProfileRevision,
};
use light_show::PortableShowCandidate;
use std::collections::HashSet;

pub(super) fn compile_patch(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<PatchedFixture>, ActionError> {
    let records = decode_records(candidate)?;
    let resolver = |reference: light_fixture::PatchedFixtureProfileReference| {
        candidate
            .fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .map(|profile| {
                ResolvedFixtureProfileRevision::new(
                    profile.id().profile_id(),
                    profile.id().revision(),
                    profile.digest().as_str(),
                    profile.profile().clone(),
                )
            })
    };
    PatchedFixtureCompiler::new(resolver)
        .compile_all(&records)
        .map_err(|error| invalid_candidate(format!("invalid patched fixture: {error}")))
}

fn decode_records(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<PortablePatchedFixtureRecord>, ActionError> {
    let mut fixture_ids = HashSet::new();
    candidate
        .objects_of_kind("patched_fixture")
        .map(|object| {
            let record = PortablePatchedFixtureRecord::decode(object.body().clone())
                .map_err(|error| invalid_record(object.key().id(), error))?;
            let fixture_id = record
                .fixture_id()
                .map_err(|error| invalid_record(object.key().id(), error))?;
            if !fixture_ids.insert(fixture_id) {
                return Err(invalid_candidate(format!(
                    "patched fixture object {} duplicates body identity {}",
                    object.key().id(),
                    fixture_id.0
                )));
            }
            Ok(record)
        })
        .collect()
}

fn invalid_record(id: &str, error: light_fixture::PortablePatchError) -> ActionError {
    invalid_candidate(format!("invalid patched fixture {id}: {error}"))
}
