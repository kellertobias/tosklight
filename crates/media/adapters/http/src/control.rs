//! Shared command publication for web controls and autonomous playback.

use arc_swap::ArcSwap;
use media_domain::{Applied, Command, CommandKind, CommandSource, MediaState, Timestamp};
use std::sync::Arc;

/// Retries reduction against the latest authoritative snapshot, never a partial batch.
pub fn submit_commands(
    state: &ArcSwap<MediaState>,
    kinds: &[CommandKind],
    source: CommandSource,
    at: Timestamp,
) -> Result<(), Applied> {
    submit_commands_guarded(state, kinds, source, at, None)
}

/// Checks the restoring owner's generation inside every compare-and-swap attempt.
pub fn submit_commands_guarded(
    state: &ArcSwap<MediaState>,
    kinds: &[CommandKind],
    source: CommandSource,
    at: Timestamp,
    expected_generation: Option<(media_domain::OutputId, u64)>,
) -> Result<(), Applied> {
    if kinds.is_empty() {
        return Ok(());
    }
    let commands: Vec<_> = kinds
        .iter()
        .cloned()
        .map(|kind| Command::new(kind, source, at))
        .collect();
    loop {
        let current = state.load_full();
        if expected_generation.is_some_and(|(id, generation)| {
            current
                .output(id)
                .is_none_or(|output| output.ownership.generation != generation)
        }) {
            return Err(Applied::RejectedNotOwner);
        }
        let Some(next) = media_domain::state::reduce_commands(&current, &commands)? else {
            return Ok(());
        };
        let previous = state.compare_and_swap(&current, Arc::new(next));
        if Arc::ptr_eq(&previous, &current) {
            return Ok(());
        }
    }
}

/// Builds an output transaction against each CAS candidate and returns its exact accepted
/// before/after snapshots. A temporary preview must never capture an earlier, unaccepted read.
pub fn submit_output_transaction<E>(
    state: &ArcSwap<MediaState>,
    output: media_domain::OutputId,
    expected_generation: u64,
    source: CommandSource,
    at: Timestamp,
    build: impl Fn(&media_domain::OutputState) -> Result<Vec<CommandKind>, E>,
    refuse: impl Fn(Applied) -> E,
) -> Result<(media_domain::OutputState, media_domain::OutputState), E> {
    loop {
        let current = state.load_full();
        let before = current
            .output(output)
            .ok_or_else(|| refuse(Applied::RejectedUnknownOutput))?;
        if before.ownership.generation != expected_generation {
            return Err(refuse(Applied::RejectedNotOwner));
        }
        let commands = build(before)?
            .into_iter()
            .map(|kind| Command::new(kind, source, at))
            .collect::<Vec<_>>();
        let Some(next) =
            media_domain::state::reduce_commands(&current, &commands).map_err(&refuse)?
        else {
            return Ok((before.clone(), before.clone()));
        };
        let after = next
            .output(output)
            .expect("the reducer retains outputs")
            .clone();
        let previous = state.compare_and_swap(&current, Arc::new(next));
        if Arc::ptr_eq(&previous, &current) {
            return Ok((before.clone(), after));
        }
    }
}
