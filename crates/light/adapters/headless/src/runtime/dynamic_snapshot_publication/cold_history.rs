//! Bounded cold dependency history. Hold Dynamics while beginning/capturing/publishing; the
//! short history mutex never acquires Dynamics, Playback or the publication lease in return.
use super::*;
use light_dynamics::{
    DynamicControlBatch, DynamicControlCursor, DynamicControlLogError, DynamicOutputFrameScratch,
    DynamicRuntime, replay_dynamic_controls,
};
use std::{collections::VecDeque, num::NonZeroUsize};
use uuid::Uuid;

mod input_history;
use input_history::InputHistory;
pub(in crate::runtime) use input_history::{
    InputCaptureCursor, RetainedFrameCapture, RetainedInputBorrow, RetainedInputCapture,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ColdGenerationCursor {
    epoch: Uuid,
    sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ColdGenerationReadError {
    WrongEpoch,
    HistoryLost,
    FutureCursor,
}

pub(in crate::runtime) struct ColdGenerationBoundary {
    from: ColdGenerationCursor,
    controls: Option<DynamicControlCursor>,
}

pub(in crate::runtime) struct PreparedColdGeneration {
    from: ColdGenerationCursor,
    previous: Arc<EngineSnapshot>,
    destination: Arc<EngineSnapshot>,
    before_controls: Option<DynamicControlCursor>,
    after_controls: Option<DynamicControlCursor>,
    controls: Result<DynamicControlBatch, DynamicControlLogError>,
}

/// Owns immutable inputs, never Live controllers, held values or compiled Preset tables.
pub(in crate::runtime) struct ColdGenerationEvent {
    pub(in crate::runtime) from: ColdGenerationCursor,
    pub(in crate::runtime) to: ColdGenerationCursor,
    previous: Arc<EngineSnapshot>,
    destination: Arc<EngineSnapshot>,
    before_controls: Option<DynamicControlCursor>,
    after_controls: Option<DynamicControlCursor>,
    controls: Result<DynamicControlBatch, DynamicControlLogError>,
}

impl ColdGenerationBoundary {
    /// Called against the private finalized candidate before persistence. Lost domain history
    /// is retained as passive replay evidence; it must never reject an otherwise valid edit.
    pub(in crate::runtime) fn prepare(
        self,
        previous: Arc<EngineSnapshot>,
        destination: Arc<EngineSnapshot>,
        candidate: &DynamicRuntime,
    ) -> PreparedColdGeneration {
        let controls = self
            .controls
            .and_then(|cursor| candidate.controls_since(cursor))
            .unwrap_or(Err(DynamicControlLogError::WrongEpoch));
        PreparedColdGeneration {
            from: self.from,
            previous,
            destination,
            before_controls: self.controls,
            after_controls: candidate.control_cursor(),
            controls,
        }
    }
}

impl ColdGenerationEvent {
    /// Read-only interval proof for detached consumers; no history mutex or Live state access.
    pub(in crate::runtime) fn replay_inputs(
        &self,
    ) -> Result<
        (
            &Arc<EngineSnapshot>,
            &Arc<EngineSnapshot>,
            &DynamicControlBatch,
        ),
        String,
    > {
        let batch = self.controls.as_ref().map_err(ToString::to_string)?;
        if self.from.epoch != self.to.epoch
            || self.from.sequence.checked_add(1) != Some(self.to.sequence)
            || self.before_controls != Some(batch.from())
            || self.after_controls != Some(batch.to())
        {
            return Err("cold control interval does not match the retained event".into());
        }
        Ok((&self.previous, &self.destination, batch))
    }

    /// The caller first consumes ordinary controls through this cold event's beginning.
    pub(in crate::runtime) fn control_boundary(
        &self,
    ) -> Result<DynamicControlCursor, DynamicControlLogError> {
        self.before_controls
            .ok_or(DynamicControlLogError::WrongEpoch)
    }

    /// Stage against Pending's own state. Any validation/replay/compilation failure leaves all
    /// four caller-owned objects unchanged. Error text is passive synchronization evidence.
    pub(in crate::runtime) fn apply(
        &self,
        runtime: &mut DynamicRuntime,
        snapshot: &mut Arc<EngineSnapshot>,
        controls: &mut DynamicControlCursor,
        generations: &mut ColdGenerationCursor,
        scratch: &mut DynamicOutputFrameScratch,
    ) -> Result<(), String> {
        if *generations != self.from || !Arc::ptr_eq(snapshot, &self.previous) {
            return Err("cold generation does not follow the retained branch".into());
        }
        let batch = self.controls.as_ref().map_err(ToString::to_string)?;
        if self.before_controls != Some(batch.from()) || self.after_controls != Some(batch.to()) {
            return Err("cold control interval does not match the retained event".into());
        }
        if *controls != batch.from() || runtime.control_cursor().is_some() {
            return Err(
                "cold replay requires its preceding controls and an independent branch".into(),
            );
        }
        // Pending follows destination edits even when its initial seed came from pinned Live.
        // Pin policy is not a request to replace this branch's held/Random history.
        let mut candidate = runtime.fork_for_pending_preview();
        candidate
            .install_definitions(self.destination.dynamics.iter().cloned())
            .map_err(|error| error.to_string())?;
        candidate
            .refresh_native_color_models(self.destination.native_color_sources.clone())
            .map_err(|error| error.to_string())?;
        let mut next_controls = *controls;
        let sample = candidate.committed_sample_boundary();
        replay_dynamic_controls(&mut candidate, scratch, &mut next_controls, batch, sample)
            .map_err(|error| error.to_string())?;
        crate::runtime::output_scheduler::materialize_cold_preset_dependencies(
            &self.previous,
            &self.destination,
            &mut candidate,
        )
        .map_err(|error| error.to_string())?;
        *runtime = candidate;
        *snapshot = self.destination.clone();
        *controls = next_controls;
        *generations = self.to;
        Ok(())
    }
}

pub(super) struct ColdGenerationLog {
    capacity: NonZeroUsize,
    cursor: ColdGenerationCursor,
    entries: VecDeque<Arc<ColdGenerationEvent>>,
    inputs: InputHistory,
}

impl ColdGenerationLog {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            cursor: ColdGenerationCursor {
                epoch: Uuid::new_v4(),
                sequence: 0,
            },
            entries: VecDeque::new(),
            inputs: InputHistory::new(capacity),
        }
    }

    pub(super) fn reset(&mut self) {
        self.cursor = ColdGenerationCursor {
            epoch: Uuid::new_v4(),
            sequence: 0,
        };
        self.entries.clear();
        self.inputs.reset();
    }

    fn append(&mut self, event: PreparedColdGeneration) {
        // Mismatched preparation must not fabricate continuous history or block Live.
        if event.from != self.cursor || self.cursor.sequence == u64::MAX {
            self.reset();
            return;
        }
        self.cursor.sequence += 1;
        if self.entries.len() == self.capacity.get() {
            self.entries.pop_front();
        }
        self.entries.push_back(Arc::new(ColdGenerationEvent {
            from: event.from,
            to: self.cursor,
            previous: event.previous,
            destination: event.destination,
            controls: event.controls,
            before_controls: event.before_controls,
            after_controls: event.after_controls,
        }));
    }

    fn read(
        &self,
        cursor: ColdGenerationCursor,
    ) -> Result<Vec<Arc<ColdGenerationEvent>>, ColdGenerationReadError> {
        if cursor.epoch != self.cursor.epoch {
            return Err(ColdGenerationReadError::WrongEpoch);
        }
        if cursor.sequence > self.cursor.sequence {
            return Err(ColdGenerationReadError::FutureCursor);
        }
        let oldest = self.cursor.sequence - self.entries.len() as u64;
        if cursor.sequence < oldest {
            return Err(ColdGenerationReadError::HistoryLost);
        }
        Ok(self
            .entries
            .iter()
            .skip((cursor.sequence - oldest) as usize)
            .cloned()
            .collect())
    }
}

impl DynamicSnapshotPublication {
    /// Idempotent while active. Capture these cursors and the Pending seed under one Dynamics
    /// guard; readers must not combine them with a newer runtime or snapshot.
    pub(in crate::runtime) fn begin_retained_history(
        &self,
        runtime: &mut DynamicRuntime,
        snapshot: &Arc<EngineSnapshot>,
        capacity: NonZeroUsize,
    ) -> Result<(ColdGenerationCursor, DynamicControlCursor), &'static str> {
        if !self.matches(snapshot) {
            return Err("snapshot does not match the installed runtime");
        }
        let controls = runtime.begin_control_recording(capacity);
        let mut history = self.cold_history.lock();
        let history = history.get_or_insert_with(|| ColdGenerationLog::new(capacity));
        self.retaining
            .store(true, std::sync::atomic::Ordering::Release);
        Ok((history.cursor, controls))
    }

    pub(in crate::runtime) fn cold_boundary(
        &self,
        runtime: &DynamicRuntime,
    ) -> Option<ColdGenerationBoundary> {
        self.cold_history
            .lock()
            .as_ref()
            .map(|history| ColdGenerationBoundary {
                from: history.cursor,
                controls: runtime.control_cursor(),
            })
    }

    pub(in crate::runtime) fn cold_generations_since(
        &self,
        cursor: ColdGenerationCursor,
    ) -> Result<Vec<Arc<ColdGenerationEvent>>, ColdGenerationReadError> {
        self.cold_history
            .lock()
            .as_ref()
            .ok_or(ColdGenerationReadError::WrongEpoch)?
            .read(cursor)
    }

    /// Call under Dynamics only after persistence and Engine/runtime installation succeed.
    /// Unlike ordinary installed(), this preserves lineage through the matching cold event.
    pub(in crate::runtime) fn installed_with_cold_event(
        &self,
        snapshot: Arc<EngineSnapshot>,
        event: Option<PreparedColdGeneration>,
    ) {
        let mut history = self.cold_history.lock();
        if let Some(history) = history.as_mut() {
            match event {
                Some(event)
                    if Arc::ptr_eq(&event.destination, &snapshot)
                        && Arc::ptr_eq(&event.previous, &self.installed.load_full()) =>
                {
                    history.append(event)
                }
                _ => history.reset(),
            }
        }
        self.installed.store(snapshot);
    }
}

#[cfg(test)]
mod tests;
