//! Ordered transport requests for an independently sampled preview. This layer records requests,
//! never Live's resulting held values, Random state, phase maps or controller deletion decisions.
//! Embedded fallback insertion shares the control sequence so it precedes a dependent Start.
//! Full cold generations and selected input captures are ordered by the producer outside this log.
use super::control_log::{ControlBatch, ControlCursor, ControlLog, ControlLogError};
use super::*;
use std::num::NonZeroUsize;

#[derive(Clone, Debug)]
pub enum DynamicControl {
    /// Install only when absent. An independently edited Pending definition remains authoritative.
    InstallFallbackDefinition(Box<DynamicDefinition>),
    Start(Box<DynamicStartRequest>),
    Off {
        controller: Uuid,
        delay: u64,
        duration: u64,
    },
    Pause {
        controller: Uuid,
        paused: bool,
        resume: Option<crate::ActivationPolicy>,
    },
    GlobalPause(bool),
    Update {
        controller: Uuid,
        size: Option<f32>,
        speed: Option<f32>,
        phase: Option<f32>,
    },
    Rank {
        controller: Uuid,
        priority: i16,
        authored_at: u64,
    },
    /// Refresh a surviving controller's current operational owner and priority.
    Owner {
        controller: Uuid,
        source: DynamicControllerSource,
        priority: i16,
    },
    CancelRelease {
        controller: Uuid,
    },
    Lanes {
        controller: Uuid,
        selection: DynamicLaneSelection,
    },
    Targets {
        instance: Uuid,
        scope: DynamicTargetScope,
        positions: HashMap<FixtureId, SpatialPosition>,
        mapping: Option<SpatialSelectionMapping>,
    },
    OutputGate {
        controller: Uuid,
        enabled: bool,
        delay: u64,
        duration: u64,
    },
    ClearOutputGate {
        controller: Uuid,
    },
}

/// Time the operation was accepted/observed. Sequence order remains authoritative, including
/// equal timestamps. A Start request separately retains its original activation/phase origin;
/// delayed source reconciliation must not backdate the acceptance of that operation.
#[derive(Clone, Debug)]
pub struct TimedDynamicControl {
    pub at_millis: u64,
    pub control: DynamicControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicControlOutcome {
    pub instance_id: Option<Uuid>,
    pub changed: bool,
    pub instance_removed: bool,
}

#[derive(Clone, Debug)]
pub(super) struct RecordedControl {
    pub(super) request: TimedDynamicControl,
    pub(super) instance: Option<Uuid>,
    pub(super) preceding_sample: Option<DynamicSampleBoundary>,
}

/// Immutable retained batch. A cloned batch stays usable after the producer prunes its log.
/// Its exact starting cursor prevents duplicate or out-of-order application.
#[derive(Clone)]
pub struct DynamicControlBatch {
    from: ControlCursor,
    to: ControlCursor,
    entries: Vec<Arc<RecordedControl>>,
}

impl DynamicControlBatch {
    /// Consume only the commands before the next selected input/cold boundary. The next read
    /// starts from this prefix's ending cursor; no command is dropped by publication coalescing.
    pub fn prefix(&self, count: usize) -> Result<Self, DynamicRuntimeError> {
        if count > self.entries.len() {
            return Err(DynamicRuntimeError::InvalidReplay(
                "control prefix exceeds batch".into(),
            ));
        }
        let to = self
            .from
            .advance(count)
            .ok_or_else(|| DynamicRuntimeError::InvalidReplay("control cursor overflow".into()))?;
        Ok(Self {
            from: self.from,
            to,
            entries: self.entries[..count].to_vec(),
        })
    }
    /// Locate an exact boundary without scanning or copying records. Other epochs and
    /// boundaries outside this immutable batch are rejected.
    pub fn offset_of(&self, cursor: ControlCursor) -> Option<usize> {
        self.from
            .distance_to(cursor)
            .filter(|offset| *offset <= self.len())
    }

    /// Exact immutable lineage proof. Equal cursors alone are insufficient: two detached
    /// cold candidates can append different controls at the same epoch/sequence.
    pub fn shares_records(&self, other: &Self) -> bool {
        self.from == other.from
            && self.to == other.to
            && self.entries.len() == other.entries.len()
            && self
                .entries
                .iter()
                .zip(&other.entries)
                .all(|(a, b)| Arc::ptr_eq(a, b))
    }

    /// An immutable subinterval for interleaving cold changes and selected samples.
    /// Disjoint intervals copy each record Arc at most once; the original batch stays usable.
    pub fn range(&self, range: std::ops::Range<usize>) -> Result<Self, DynamicRuntimeError> {
        let entries = self.entries.get(range.clone()).ok_or_else(|| {
            DynamicRuntimeError::InvalidReplay("control range exceeds batch".into())
        })?;
        let boundary = |offset| {
            self.from
                .advance(offset)
                .ok_or_else(|| DynamicRuntimeError::InvalidReplay("control cursor overflow".into()))
        };
        Ok(Self {
            from: boundary(range.start)?,
            to: boundary(range.end)?,
            entries: entries.to_vec(),
        })
    }

    pub fn operation_times(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().map(|entry| entry.request.at_millis)
    }
    pub fn from(&self) -> ControlCursor {
        self.from
    }
    pub fn to(&self) -> ControlCursor {
        self.to
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Live anchors identify evidence only. Pending must use its own selected sample history;
    /// it must not copy Live expressions or require its branch marker to equal this marker.
    pub fn preceding_samples(&self) -> impl Iterator<Item = Option<DynamicSampleBoundary>> + '_ {
        self.entries.iter().map(|entry| entry.preceding_sample)
    }
}

#[derive(Clone)]
pub struct DynamicControlJournal {
    log: ControlLog<Arc<RecordedControl>>,
}

impl DynamicControlJournal {
    pub(super) fn append_recorded(&mut self, records: Vec<Arc<RecordedControl>>) {
        self.log.append(records);
    }
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            log: ControlLog::new(capacity),
        }
    }
    pub fn cursor(&self) -> ControlCursor {
        self.log.cursor()
    }
    pub fn read(&self, cursor: ControlCursor) -> Result<DynamicControlBatch, ControlLogError> {
        let ControlBatch { from, to, entries } = self.log.read(cursor)?;
        Ok(DynamicControlBatch { from, to, entries })
    }
    /// The producer supplies the minimum acknowledged cursor across all retained episodes.
    pub fn acknowledge(&mut self, cursor: ControlCursor) -> Result<(), ControlLogError> {
        self.log.acknowledge(cursor)
    }
    /// Successful restore/show activation begins a new stream. Old consumers must re-seed
    /// explicitly; they cannot silently continue against unrelated runtime state.
    pub fn reset(&mut self) {
        self.log.reset();
    }

    /// Execute an authored control batch and publish only after the runtime transaction commits.
    /// Do not call inside an output transaction, or use this for automatic sample cleanup.
    /// Failed commands and unwinds leave both runtime and journal unchanged. Bounded log eviction
    /// cannot block the operator: a lagging preview instead receives an explicit HistoryLost.
    pub fn execute(
        &mut self,
        runtime: &mut DynamicRuntime,
        scratch: &mut DynamicOutputFrameScratch,
        controls: Vec<TimedDynamicControl>,
    ) -> Result<Vec<DynamicControlOutcome>, DynamicRuntimeError> {
        if runtime.control_recording.is_some() {
            return Err(DynamicRuntimeError::InvalidReplay(
                "runtime already owns its control journal".into(),
            ));
        }
        let (outcomes, records) = runtime.with_output_frame_transaction(scratch, |runtime| {
            let mut outcomes = Vec::with_capacity(controls.len());
            let mut records = Vec::new();
            for request in controls {
                let preceding_sample = runtime.committed_sample_boundary();
                let outcome = apply(runtime, &request, None)?;
                if outcome.changed {
                    records.push(Arc::new(RecordedControl {
                        request,
                        instance: outcome.instance_id,
                        preceding_sample,
                    }));
                }
                outcomes.push(outcome);
            }
            Ok((outcomes, records))
        })?;
        self.log.append(records);
        Ok(outcomes)
    }
}

/// Apply controls atomically, then acknowledge the branch cursor. The caller interleaves selected
/// pending samples and cold generations before replay and supplies its expected branch history
/// marker. A mismatch is a stale replay plan, not permission to replace pending with Live history.
/// A subsequent failed pending sample leaves these accepted controls in place.
pub fn replay_dynamic_controls(
    runtime: &mut DynamicRuntime,
    scratch: &mut DynamicOutputFrameScratch,
    cursor: &mut ControlCursor,
    batch: &DynamicControlBatch,
    expected_branch_sample: Option<DynamicSampleBoundary>,
) -> Result<(), DynamicRuntimeError> {
    if runtime.control_recording.is_some() {
        return Err(DynamicRuntimeError::InvalidReplay(
            "replay branch cannot publish authoritative controls".into(),
        ));
    }
    if *cursor != batch.from || runtime.committed_sample_boundary() != expected_branch_sample {
        return Err(DynamicRuntimeError::InvalidReplay(
            "stale control cursor or branch history".into(),
        ));
    }
    runtime.with_output_frame_transaction(scratch, |runtime| {
        for entry in &batch.entries {
            apply(runtime, &entry.request, Some(entry.instance))?;
        }
        Ok(())
    })?;
    *cursor = batch.to;
    Ok(())
}

pub(super) fn apply(
    runtime: &mut DynamicRuntime,
    request: &TimedDynamicControl,
    replay_instance: Option<Option<Uuid>>,
) -> Result<DynamicControlOutcome, DynamicRuntimeError> {
    let at = request.at_millis;
    let resolve = |runtime: &DynamicRuntime, id| -> Result<Uuid, DynamicRuntimeError> {
        let instance = runtime
            .controller(id)
            .ok_or(DynamicRuntimeError::MissingController)?
            .0;
        if replay_instance.is_some_and(|expected| expected != Some(instance)) {
            return Err(DynamicRuntimeError::InvalidReplay(
                "controller instance differs from recorded identity".into(),
            ));
        }
        Ok(instance)
    };
    let (instance, changed) = match &request.control {
        DynamicControl::InstallFallbackDefinition(definition) => {
            let absent = !runtime.definitions.contains_key(&definition.id);
            runtime.install_fallback_definition((**definition).clone())?;
            (None, absent)
        }
        DynamicControl::Start(start) => {
            let id = match replay_instance {
                Some(Some(id)) => runtime.start_with_instance_identity((**start).clone(), id)?,
                Some(None) => {
                    return Err(DynamicRuntimeError::InvalidReplay(
                        "start lacks instance identity".into(),
                    ));
                }
                None => runtime.start((**start).clone())?,
            };
            (Some(id), true)
        }
        DynamicControl::GlobalPause(paused) => {
            let changed = runtime.global_paused != *paused;
            runtime.set_global_paused(*paused, at);
            (None, changed)
        }
        DynamicControl::Targets {
            instance,
            scope,
            positions,
            mapping,
        } => {
            if replay_instance.is_some_and(|expected| expected != Some(*instance)) {
                return Err(DynamicRuntimeError::InvalidReplay(
                    "target instance differs from recorded identity".into(),
                ));
            }
            (
                Some(*instance),
                runtime.reconcile_instance_targets(
                    *instance,
                    scope.clone(),
                    positions,
                    mapping.as_ref(),
                )?,
            )
        }
        DynamicControl::Off {
            controller,
            delay,
            duration,
        } => {
            let id = resolve(runtime, *controller)?;
            let before = runtime.instances[&id]
                .controller_transitions
                .get(controller)
                .copied();
            runtime.off_controller(id, *controller, at, *delay, *duration)?;
            let after = runtime
                .instances
                .get(&id)
                .and_then(|v| v.controller_transitions.get(controller))
                .copied();
            (Some(id), before != after)
        }
        DynamicControl::Pause {
            controller,
            paused,
            resume,
        } => {
            let id = resolve(runtime, *controller)?;
            let before = (
                runtime.controller(*controller).unwrap().1.paused,
                runtime.instances[&id].paused_at_millis,
            );
            runtime.set_controller_paused_with_resume(id, *controller, *paused, at, *resume)?;
            let after = (
                runtime.controller(*controller).unwrap().1.paused,
                runtime.instances[&id].paused_at_millis,
            );
            (Some(id), before != after)
        }
        DynamicControl::Update {
            controller,
            size,
            speed,
            phase,
        } => {
            let id = resolve(runtime, *controller)?;
            let before = runtime.controller(*controller).unwrap().1;
            runtime.update_controller(*controller, *size, *speed, *phase)?;
            (
                Some(id),
                before != runtime.controller(*controller).unwrap().1,
            )
        }
        DynamicControl::Rank {
            controller,
            priority,
            authored_at,
        } => {
            let id = resolve(runtime, *controller)?;
            let before = runtime.controller(*controller).unwrap().1;
            runtime.update_controller_rank(*controller, *priority, *authored_at, at)?;
            (
                Some(id),
                before != runtime.controller(*controller).unwrap().1,
            )
        }
        DynamicControl::Owner {
            controller,
            source,
            priority,
        } => {
            let id = resolve(runtime, *controller)?;
            (
                Some(id),
                runtime.update_controller_owner(*controller, source.clone(), *priority)?,
            )
        }
        DynamicControl::CancelRelease { controller } => {
            let id = resolve(runtime, *controller)?;
            let changed = runtime.source_scope_is_releasing(id, *controller);
            runtime.cancel_controller_release(*controller)?;
            (Some(id), changed)
        }
        DynamicControl::Lanes {
            controller,
            selection,
        } => {
            let id = resolve(runtime, *controller)?;
            (
                Some(id),
                runtime.set_controller_lane_selection(id, *controller, selection.clone())?,
            )
        }
        DynamicControl::OutputGate {
            controller,
            enabled,
            delay,
            duration,
        } => {
            let id = resolve(runtime, *controller)?;
            (
                Some(id),
                runtime.set_controller_output_enabled(
                    *controller,
                    *enabled,
                    at,
                    *delay,
                    *duration,
                )?,
            )
        }
        DynamicControl::ClearOutputGate { controller } => {
            let id = resolve(runtime, *controller)?;
            (Some(id), runtime.clear_controller_output_gate(*controller)?)
        }
    };
    Ok(DynamicControlOutcome {
        instance_id: instance,
        changed,
        instance_removed: instance.is_some_and(|id| !runtime.instances.contains_key(&id)),
    })
}
