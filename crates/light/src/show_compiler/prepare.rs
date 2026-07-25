use super::{
    ShowCompileDirty, compile_show_candidate, compile_show_candidate_incremental,
    invalid_candidate,
    migrations::{stage_candidate_migrations, stage_candidate_migrations_preserving_object},
};
use crate::ActionError;
use light_engine::EngineSnapshot;
use light_show::{PortableShowDocument, PortableShowTransaction};

/// A transaction and the runtime snapshot compiled from its migration-compatible candidate.
///
/// The backing show remains unchanged until an application adapter commits `transaction`. Some
/// capability-owned actions deliberately retain only their explicit transaction while applying
/// compatibility migrations to the compiled snapshot in memory.
pub struct PreparedShowCandidate {
    transaction: PortableShowTransaction,
    snapshot: EngineSnapshot,
}

impl PreparedShowCandidate {
    pub(crate) const fn transaction(&self) -> &PortableShowTransaction {
        &self.transaction
    }

    pub fn into_parts(self) -> (PortableShowTransaction, EngineSnapshot) {
        (self.transaction, self.snapshot)
    }
}

/// Prepares a mutation against the already-normalized active document and structurally shares
/// every unaffected runtime projection.
///
/// The show-open path remains the full compatibility oracle. This path rechecks only objects
/// touched by the transaction, so legacy bodies accepted through an API mutation are normalized
/// without sweeping the rest of an already-normalized document.
pub fn prepare_normalized_show_candidate_incremental(
    document: &PortableShowDocument,
    mut transaction: PortableShowTransaction,
    previous: &EngineSnapshot,
) -> Result<PreparedShowCandidate, ActionError> {
    super::migrations::stage_touched_object_migrations(document, &mut transaction)?;
    let mut dirty = ShowCompileDirty {
        fixtures: transaction.patch_changed() || transaction.fixture_profile_revisions_changed(),
        ..ShowCompileDirty::default()
    };
    for kind in transaction.changed_object_kinds() {
        match kind {
            "patched_fixture" => dirty.fixtures = true,
            "cue_list" => dirty.cue_lists = true,
            "playback" => dirty.playbacks = true,
            "playback_page" => dirty.playback_pages = true,
            "route" => dirty.routes = true,
            "control_mapping" => dirty.control_mappings = true,
            "group" => dirty.groups = true,
            _ => {}
        }
    }
    let candidate = document
        .candidate(&transaction)
        .map_err(|error| invalid_candidate(format!("invalid portable show candidate: {error}")))?;
    let snapshot = compile_show_candidate_incremental(candidate, previous, dirty)?;
    Ok(PreparedShowCandidate {
        transaction,
        snapshot,
    })
}

// @tour portable-show:30 Prepare a side-effect-free runtime candidate
// Compatibility migrations, candidate validation, and compilation finish before persistence or
// live runtime installation can begin.

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

/// Compiles one migration-compatible runtime snapshot while retaining the caller's exact
/// transaction for persistence.
///
/// Capability-owned actions use this when their projections and events promise that only their
/// explicit object changes are retained. Legacy migrations still participate in runtime
/// compilation, but they must not silently join that capability's persisted transaction.
pub(crate) fn prepare_show_candidate_exact_transaction(
    document: &PortableShowDocument,
    transaction: PortableShowTransaction,
) -> Result<PreparedShowCandidate, ActionError> {
    let mut compilation = transaction.clone();
    stage_candidate_migrations(document, &mut compilation)?;
    let candidate = document
        .candidate(&compilation)
        .map_err(|error| invalid_candidate(format!("invalid portable show candidate: {error}")))?;
    let snapshot = compile_show_candidate(candidate)?;
    Ok(PreparedShowCandidate {
        transaction,
        snapshot,
    })
}

/// Compiles and returns the exact candidate that will be committed while retaining one staged
/// Undo body byte-for-byte. Other pending compatibility migrations still join the transaction.
pub(crate) fn prepare_show_candidate_preserving_object(
    document: &PortableShowDocument,
    mut transaction: PortableShowTransaction,
    kind: &str,
    object_id: &str,
) -> Result<PreparedShowCandidate, ActionError> {
    stage_candidate_migrations_preserving_object(document, &mut transaction, kind, object_id)?;
    let candidate = document
        .candidate(&transaction)
        .map_err(|error| invalid_candidate(format!("invalid portable show candidate: {error}")))?;
    let snapshot = compile_show_candidate(candidate)?;
    Ok(PreparedShowCandidate {
        transaction,
        snapshot,
    })
}
