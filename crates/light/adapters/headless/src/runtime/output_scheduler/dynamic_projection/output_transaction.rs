//! The authoritative output lane commits Dynamic time/state only after its final render.
//! Automatic Playback transitions belong to capture and are handled by the caller even when
//! this transaction fails. Dynamic notifications and auto-offs belong only to a committed frame.
use super::*;
use crate::runtime::dynamic_source_origins::{DynamicSourceOrigins, SharedDynamicSourceOrigins};

mod family_frame;
#[cfg(test)]
pub(in crate::runtime) use family_frame::lamp_quality;
#[cfg(test)]
pub(in crate::runtime) use family_frame::legacy;
pub(in crate::runtime) use family_frame::{LiveFamilyAdapters, OutputRenderSource};

pub(in crate::runtime) struct CommittedDynamicOutput<T> {
    pub output: T,
    pub auto_offs: Vec<PlaybackIdentity>,
    pub events: Vec<light_application::EventDraft>,
    pub runtime: light_dynamics::DynamicRuntimeSnapshot,
    pub samples: Vec<light_dynamics::DynamicRuntimeSample>,
    pub origins: Arc<DynamicSourceOrigins>,
    /// Newly accepted sampling history associated with this exact input frame. A path that
    /// skips sampling must not relabel an older history anchor with a later capture.
    pub sample_boundary: Option<light_dynamics::DynamicSampleBoundary>,
    /// The captured ordinary resolution (finalized). Readers showing the programmed parameter
    /// use `raw_values`/`raw_value`.
    pub ordinary: Option<light_engine::FrameValues>,
    /// TL-659: the earliest Cue or Dynamic start this frame carries, in application-time
    /// microseconds; the output lane measures the change lead time from it once the frame is sent.
    pub change_lead_start: Option<i64>,
}

/// The caller holds the authoritative Dynamics lock. The catalogue uses the same transaction
/// boundary as clocks/history: immutable readers keep the previous committed pair on failure.
/// An unchanged frame shares its catalogue without another Arc allocation or publication.
fn with_dynamic_source_transaction<T, E>(
    runtime: &mut light_dynamics::DynamicRuntime,
    scratch: &mut light_dynamics::DynamicOutputFrameScratch,
    origins: &SharedDynamicSourceOrigins,
    evaluate: impl FnOnce(
        &mut light_dynamics::DynamicRuntime,
        &mut DynamicSourceOrigins,
    ) -> Result<T, E>,
) -> Result<(T, Arc<DynamicSourceOrigins>), E> {
    let previous = origins.load_full();
    let mut candidate = (*previous).clone();
    let output = runtime
        .with_output_frame_transaction(scratch, |runtime| evaluate(runtime, &mut candidate))?;
    let committed = if previous.shares_storage(&candidate) {
        previous
    } else {
        let committed = Arc::new(candidate);
        origins.store(Arc::clone(&committed));
        committed
    };
    Ok((output, committed))
}

/// The caller already owns the ordered Playback operation. `finish` includes final encoding
/// and the engine's continuity commit; returning Err rolls back reconciliation and sampling.
/// Keep network I/O outside this boundary: a send failure must not rewind a completed frame.
/// When `family` is engaged (`family_frame.rs`), the all-family hybrid frame replaces legacy
/// sampling and rendering, and `finish` receives its rendered result instead of batches.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime) fn dynamic_output_frame<T>(
    engine: &Engine,
    frame: &light_engine::PreparedOutputFrame,
    retained: Option<crate::runtime::dynamic_snapshot_publication::RetainedInputBorrow<'_>>,
    baseline_samples: &[ContributionBatch],
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    dynamic_snapshot: &crate::runtime::DynamicSnapshotPublication,
    origins: &SharedDynamicSourceOrigins,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    cache: &ProgrammerReconciliationCache,
    family: &LiveFamilyAdapters,
    finish: impl FnOnce(OutputRenderSource<'_>) -> Result<T, EngineError>,
) -> Result<CommittedDynamicOutput<T>, EngineError> {
    // Retention is observational. A stale/mismatched token must never reject Live output.
    let retained = retained.filter(|input| input.matches(frame));
    let now = frame.sampled_at();
    let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
    let snapshot = frame.snapshot();
    let addresser = frame.frame_addresser();
    let speed_transports = capture_dynamic_speed_transports(speed_groups, now_millis);
    let sources = TickSources::prepared(engine, frame, baseline_samples);
    let mut inputs = CapturedDynamicInputs {
        now,
        speed_transports: &speed_transports,
        rate: rate.load(Ordering::Relaxed),
        snapshot: &snapshot,
        programmer_values: frame.dynamic_programmer_values(),
        programmer_rows: Some(frame.dynamic_programmer_rows()),
        cue_values: frame.cue_dynamic_values(),
        dynamic_playbacks: frame.dynamic_playbacks(),
        playback_paused: frame.playback_dynamics_paused(),
        addresser: &addresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: Some(cache),
        force_source_reconciliation: false,
    };
    let (((output, sampled), origins), sample_boundary, change_lead_start) = {
        // All authoritative paths take these in the same order. The runtime stays locked
        // through rendering, so operator mutations cannot be erased by a failed rollback.
        let mut dynamics = dynamics.lock();
        if !dynamic_snapshot.matches(&snapshot) {
            return Err(EngineError::StalePreparedFrame);
        }
        let mut scratch = cache.transaction.lock();
        let previous_boundary = dynamics.committed_sample_boundary();
        inputs.force_source_reconciliation = cache.sources_changed(&origins.load_full());
        let result = with_dynamic_source_transaction(
            &mut dynamics,
            &mut scratch,
            origins,
            |runtime, origins| {
                if family.engaged(engine) {
                    return family_frame::family_output_frame(
                        engine,
                        frame,
                        baseline_samples,
                        runtime,
                        origins,
                        &inputs,
                        family,
                        finish,
                    );
                }
                let sampled = sample_captured_dynamic_inputs_with(
                    runtime,
                    &inputs,
                    |runtime, now, interval, assignments, _| {
                        prepare_captured_preset_dependencies(runtime, &inputs)?;
                        source_bindings::bind_captured_sources(
                            origins,
                            runtime,
                            &inputs,
                            assignments,
                        )
                        .map_err(|error| {
                            light_dynamics::DynamicRuntimeError::InvalidSample(error.to_string())
                        })?;
                        runtime.sample_all_programming_addressed(
                            now,
                            interval,
                            inputs.speed_transports,
                            &sources,
                            &source_bindings::AuthoredDynamicSources(origins),
                            Some(inputs.addresser),
                        )
                    },
                )
                .map_err(|error| EngineError::Invalid(error.to_string()))?;
                source_bindings::retire_removed_controllers(
                    origins,
                    &sampled.before_runtime,
                    &sampled.after_runtime,
                );
                let mut batches = project_captured_dynamic_sample(&inputs, &sampled, &sources);
                batches.extend_from_slice(baseline_samples);
                let output = finish(OutputRenderSource::Legacy(&batches))?;
                Ok::<_, EngineError>((output, sampled))
            },
        )?;
        // Acknowledgment is infallible and follows the semantic commit while the same runtime
        // is still locked. A failed frame must retry reconciliation with the original inputs.
        cache.acknowledge(inputs.programmer_values, &snapshot);
        cache.acknowledge_sources(&result.1);
        let boundary = dynamics
            .committed_sample_boundary()
            .filter(|boundary| Some(*boundary) != previous_boundary);
        if let Some(retained) = retained {
            dynamic_snapshot.retain_accepted_input(
                &dynamics,
                retained,
                baseline_samples,
                &speed_transports,
                inputs.rate,
                boundary,
            );
        }
        let starts = claim_carried_starts(&mut dynamics, frame, now_millis);
        (result, boundary, starts)
    };
    let events =
        dynamic_transition_events(&sampled.before_runtime, &sampled.after_runtime, now_millis);
    let auto_offs = if sampled.idle {
        Vec::new()
    } else {
        let controlled = fully_controlled_dynamic_playbacks_from(
            frame.playback_contributions(),
            &sampled.samples,
            &sampled.playback_controls,
            &sampled.after_runtime,
            inputs.programmer_values,
            inputs.cue_values,
        );
        engine.auto_off_fully_controlled_dynamic_playbacks_at(controlled)
    };
    Ok(CommittedDynamicOutput {
        output,
        auto_offs,
        events,
        runtime: sampled.after_runtime,
        samples: sampled.samples,
        origins,
        sample_boundary,
        change_lead_start,
        ordinary: match sources.values.get() {
            Some(TickValues::Prepared(source)) => Some(source.clone()),
            Some(TickValues::Legacy(_)) | None => None,
        },
    })
}

/// TL-659: a committed frame carries every Cue and Dynamic start due by its sample instant. The
/// caller still holds the Dynamics lock it committed under.
fn claim_carried_starts(
    dynamics: &mut light_dynamics::DynamicRuntime,
    frame: &light_engine::PreparedOutputFrame,
    now_millis: u64,
) -> Option<i64> {
    let dynamic_start = dynamics.claim_change_lead_start(now_millis);
    light_core::earliest_start(frame.claim_change_lead_start(), dynamic_start)
}

#[cfg(test)]
mod tests;
