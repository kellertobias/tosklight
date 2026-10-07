//! Detached preparation of a restored Dynamic runtime checkpoint.
//!
//! A stored checkpoint carries clocks, controllers, pause, Random streams, held history and the
//! last valid Preset tables of the show it was captured from. Before it may be sampled or
//! published, its native Color lanes must be compiled against the destination provider and its
//! Preset tables must be rebuilt for the destination Groups and positions.
//! [`prepare_restored_dynamic_candidate`] does that on a fork, so the caller's runtime, native
//! pins, source origins and Playback occurrence watermark never change:
//!
//! 1. validate the runtime/origin checkpoint pair;
//! 2. fork the base (`fork_for_cold_install`, detached native pins) and install the exact
//!    destination native provider;
//! 3. restore the checkpoint; historical definitions keep their own original identities, so an
//!    unavailable original stays a suspended, passive lane while an available original whose
//!    retained values are invalid is rejected. No destination model substitutes for an original;
//! 4. materialize the pending Preset tables in one batch (TL-563 compiler and freshness APIs).
//!
//! Nothing is ticked or sampled. The caller (`OutputResource::restore_dynamic_source_state`)
//! keeps preflight, watermark reservation, final recheck, lock order and publication.

use super::cold_preset_materialization::{
    ColdPresetMaterialization, ColdPresetMaterializationError,
    materialize_pending_preset_dependencies,
};
use crate::runtime::dynamic_source_origins::{
    DynamicRuntimeSourceCheckpoint, DynamicSourceOrigins,
};
use light_core::programming::IntentError;
use light_dynamics::{DynamicRuntime, DynamicRuntimeError};

/// A prepared, unpublished restore. Publish `runtime` and `origins` together, after reserving
/// `playback_source_occurrence_watermark` on the Engine.
#[cfg_attr(not(test), allow(dead_code))] // Consumed by TL-548's restore publication.
pub(in crate::runtime) struct RestoredDynamicCandidate {
    pub runtime: DynamicRuntime,
    pub origins: DynamicSourceOrigins,
    pub playback_source_occurrence_watermark: u64,
    /// Passive outcomes: absent Groups, unavailable native originals, incompatible or invalid
    /// authored Preset values that fell back to verified or last-valid values.
    pub presets: ColdPresetMaterialization,
}

#[derive(Debug)]
pub(in crate::runtime) enum RestoredDynamicCandidateError {
    /// The runtime/origin checkpoint pair is malformed or inconsistent.
    Checkpoint(IntentError),
    /// The destination native provider could not be installed on the candidate.
    NativeModels(DynamicRuntimeError),
    /// The stored runtime is malformed or an available original rejects its retained values.
    Runtime(DynamicRuntimeError),
    /// Preset dependency validation or table compilation failed.
    Presets(ColdPresetMaterializationError),
}

impl std::fmt::Display for RestoredDynamicCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Checkpoint(error) => write!(formatter, "Dynamic checkpoint is invalid: {error}"),
            Self::NativeModels(error) => {
                write!(
                    formatter,
                    "native Color models cannot be installed: {error}"
                )
            }
            Self::Runtime(error) => {
                write!(formatter, "Dynamic runtime cannot be restored: {error}")
            }
            Self::Presets(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for RestoredDynamicCandidateError {}

/// Build a detached, sample-ready candidate from a stored checkpoint for `destination`.
///
/// `base` is only forked; it and every published value stay untouched on success and failure.
/// Current Playback owners are not reconciled here: `persistence_events` restores their rows
/// afterwards. A failed preparation must be discarded as a whole.
#[cfg_attr(not(test), allow(dead_code))] // Invoked by TL-548's restore preflight/final recheck.
pub(in crate::runtime) fn prepare_restored_dynamic_candidate(
    destination: &light_engine::EngineSnapshot,
    base: &DynamicRuntime,
    checkpoint: DynamicRuntimeSourceCheckpoint,
) -> Result<RestoredDynamicCandidate, RestoredDynamicCandidateError> {
    let (snapshot, origins) = checkpoint
        .restore()
        .map_err(RestoredDynamicCandidateError::Checkpoint)?;
    let mut runtime = base.fork_for_cold_install();
    // A checkpoint replaces all instances. Discard old suspended history and registry entries
    // before refreshing the provider: newly available originals must validate the incoming
    // state, never an instance or definition that this restore is about to remove. Verified
    // original-model pins remain privately retained by the fork.
    runtime
        .restore_snapshot(Default::default())
        .map_err(RestoredDynamicCandidateError::Runtime)?;
    runtime
        .install_definitions(std::iter::empty())
        .map_err(RestoredDynamicCandidateError::NativeModels)?;
    runtime
        .refresh_native_color_models(destination.native_color_sources.clone())
        .map_err(RestoredDynamicCandidateError::NativeModels)?;
    runtime
        .install_definitions(destination.dynamics.iter().cloned())
        .map_err(RestoredDynamicCandidateError::NativeModels)?;
    runtime
        .restore_snapshot(snapshot)
        .map_err(RestoredDynamicCandidateError::Runtime)?;
    let presets = materialize_pending_preset_dependencies(destination, &mut runtime)
        .map_err(RestoredDynamicCandidateError::Presets)?;
    Ok(RestoredDynamicCandidate {
        playback_source_occurrence_watermark: origins.playback_source_occurrence_watermark(),
        runtime,
        origins,
        presets,
    })
}

#[cfg(test)]
mod tests;
