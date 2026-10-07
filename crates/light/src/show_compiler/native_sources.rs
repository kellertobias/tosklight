//! All original profiles retained by the show, independent of its current patch or library.
use super::invalid_candidate;
use crate::ActionError;
use light_engine::{NativeColorSourceCatalog, NativeColorSourceRevisionKey};
use light_show::PortableShowCandidate;
use std::sync::Arc;

pub(super) fn compile(
    candidate: PortableShowCandidate<'_>,
    previous: Option<&NativeColorSourceCatalog>,
) -> Result<Arc<NativeColorSourceCatalog>, ActionError> {
    let revisions = candidate.fixture_profile_revisions().map(|revision| {
        let key = NativeColorSourceRevisionKey {
            profile_id: revision.id().profile_id(),
            revision: revision.id().revision(),
            // The portable digest includes unknown fields. It is distinct from the typed
            // native identity and must participate in cold cache invalidation.
            raw_store_digest: revision.digest().as_str().to_owned(),
        };
        NativeColorSourceCatalog::compile_revision(key, previous, || {
            serde_json::from_value(revision.profile().clone()).map_err(|error| error.to_string())
        })
    });
    NativeColorSourceCatalog::from_revisions(revisions)
        .map(Arc::new)
        .map_err(|error| invalid_candidate(format!("invalid native source catalogue: {error}")))
}
