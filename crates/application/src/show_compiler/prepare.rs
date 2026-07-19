use super::{compile_show_candidate, invalid_candidate, migrations::stage_candidate_migrations};
use crate::ActionError;
use light_engine::EngineSnapshot;
use light_show::{PortableShowDocument, PortableShowTransaction};

/// A losslessly migrated transaction and the runtime snapshot compiled from that exact candidate.
///
/// The backing show remains unchanged until an application adapter commits `transaction`.
pub struct PreparedShowCandidate {
    transaction: PortableShowTransaction,
    snapshot: EngineSnapshot,
}

impl PreparedShowCandidate {
    pub fn into_parts(self) -> (PortableShowTransaction, EngineSnapshot) {
        (self.transaction, self.snapshot)
    }
}

/// Stages compatibility migrations and compiles the resulting candidate without persistence or
/// live-runtime side effects. A failure leaves both the document and supplied transaction intact.
pub fn prepare_show_candidate(
    document: &PortableShowDocument,
    mut transaction: PortableShowTransaction,
) -> Result<PreparedShowCandidate, ActionError> {
    stage_candidate_migrations(document, &mut transaction)?;
    let candidate = document
        .candidate(&transaction)
        .map_err(|error| invalid_candidate(format!("invalid portable show candidate: {error}")))?;
    let snapshot = compile_show_candidate(candidate)?;
    Ok(PreparedShowCandidate {
        transaction,
        snapshot,
    })
}
