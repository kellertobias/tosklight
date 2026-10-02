//! TL-560/TL-552 programming-contract gate: show load/activation, Programmer restore, Playback
//! and Output runtime restore, object Undo/Redo history and the writer marker. Every check is
//! dormant at contract 0; it engages when the runtime supports contract 1 or later (production
//! since the TL-552 cutover). See `light_show::programming_contract` for the marker and the
//! validator.
use super::{ApiError, AppState, ShowEntry};
use light_show::PortableShowTransaction;

/// Startup and activation reader. `Ok` at contract 0 without touching the file. At contract 1 it
/// inspects the file read-only before anything opens it for writing, so a rejected show keeps
/// its original bytes (no schema migration, no compatibility commit, no backup rename). A file
/// the inspection cannot read (missing, not SQLite, malformed JSON) is left to the ordinary
/// loader, which reports it through the existing "corrupted or incompatible" recovery path.
pub(super) fn check_show_file(path: &std::path::Path, supported: u16) -> Result<(), String> {
    match light_show::validate_show_programming_contract(path, supported) {
        Err(light_show::StoreError::Invalid(message)) => Err(message),
        Ok(()) | Err(_) => Ok(()),
    }
}

/// Activation, show-open and revision-restore gate (HTTP callers).
pub(super) fn require_for_show(state: &AppState, entry: &ShowEntry) -> Result<(), ApiError> {
    require_for_path(state, std::path::Path::new(&entry.path))
}

/// Uses the contract engaged on the active-show resource (`ActiveShowResource::
/// legacy_programming_gate`): the runtime's own contract after `build_app_state`, and since TL-552
/// the engine contract of synthetic test states as well.
pub(super) fn require_for_path(state: &AppState, path: &std::path::Path) -> Result<(), ApiError> {
    check_show_file(path, state.active_show.legacy_programming_gate())
        .map_err(ApiError::bad_request)
}

/// Programmer/session restore gate: legacy normalized Position or Color component values in the
/// stored Programmer (Normal, Preload, Undo/Redo history) are rejected at contract ≥ 1; startup
/// then preserves the original JSON through `runtime_recovery`.
pub(super) fn check_programmer(value: &serde_json::Value, supported: u16) -> anyhow::Result<()> {
    if supported == 0 {
        return Ok(());
    }
    let legacy = light_show::legacy_programming_attributes(value);
    anyhow::ensure!(
        legacy.is_empty(),
        "stored Programmer holds programming from before semantic programming contract \
         {supported} ({}); it cannot be converted safely",
        legacy.into_iter().collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

/// Unit-of-work write gate (TL-552 follow-up): a contract ≥ 1 commit whose object writes hold
/// legacy programming is refused as `Invalid` (HTTP 400) before the store changes, so an accepted
/// write never makes the show fail the load-time validator on the next open.
pub(super) fn check_transaction(
    transaction: &PortableShowTransaction,
    supported: u16,
) -> Result<(), light_application::ActionError> {
    transaction
        .check_programming_contract(supported)
        .map_err(|rejection| {
            light_application::ActionError::new(
                light_application::ActionErrorKind::Invalid,
                rejection.message,
            )
        })
}

/// Writer marker: a contract ≥ 1 commit that writes authored programming stamps the show.
pub(super) fn stamp(transaction: &mut PortableShowTransaction, supported: u16) {
    transaction.stamp_programming_contract(supported);
}

/// Playback and Output runtime restore gate (TL-552): legacy normalized Position, Color component
/// or percentage Zoom values anywhere in a stored runtime payload (Cue holds, transition sources,
/// Dynamic checkpoints) are rejected at contract ≥ 1; startup then preserves the original JSON
/// through `runtime_recovery`, exactly like the Programmer.
/// Unparseable JSON passes here so the ordinary decoder reports it with its existing message.
pub(super) fn check_runtime_payload(
    label: &str,
    serialized: &str,
    supported: u16,
) -> anyhow::Result<()> {
    if supported == 0 {
        return Ok(());
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(serialized) else {
        return Ok(());
    };
    let legacy = light_show::legacy_programming_attributes(&value);
    anyhow::ensure!(
        legacy.is_empty(),
        "stored {label} holds programming from before semantic programming contract \
         {supported} ({}); it cannot be converted safely",
        legacy.into_iter().collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

/// Object Undo/Redo gate (TL-552 owner decision). History rows written before the cutover can hold
/// legacy programming; restoring one at contract ≥ 1 would bring rejected content back into the
/// active show. The step is refused: nothing changes and the message says what to do instead.
pub(super) fn check_history_body(
    supported: u16,
    direction: &str,
    kind: &str,
    object_id: &str,
    body: &serde_json::Value,
) -> Result<(), String> {
    if supported == 0 || !light_show::PROGRAMMING_OBJECT_KINDS.contains(&kind) {
        return Ok(());
    }
    let legacy = light_show::legacy_programming_attributes(body);
    if legacy.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{direction} would restore {kind} {object_id} as it was before semantic programming \
         contract {supported} ({}). That version cannot be converted safely, so {direction} \
         left the current version unchanged. Re-record it with the current Color, Position and \
         Zoom controls instead.",
        legacy.into_iter().collect::<Vec<_>>().join(", ")
    ))
}
