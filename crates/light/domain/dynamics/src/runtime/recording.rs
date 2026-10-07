//! Optional authoritative control recording shares the output transaction's commit boundary.
use super::control_batch::{RecordedControl, apply};
use super::*;
use std::num::NonZeroUsize;

impl DynamicRuntime {
    /// Begin collecting controls before forking a retained episode. Existing recording is not
    /// reset by another observer. The owner serializes this with its runtime/capture operation.
    pub fn begin_control_recording(&mut self, capacity: NonZeroUsize) -> DynamicControlCursor {
        assert!(
            self.output_frame_undo.is_none(),
            "control recording starts outside output transactions"
        );
        self.control_recording
            .get_or_insert_with(|| DynamicControlJournal::new(capacity))
            .cursor()
    }

    pub fn control_cursor(&self) -> Option<DynamicControlCursor> {
        self.control_recording
            .as_ref()
            .map(DynamicControlJournal::cursor)
    }

    pub fn controls_since(
        &self,
        cursor: DynamicControlCursor,
    ) -> Option<Result<DynamicControlBatch, DynamicControlLogError>> {
        self.control_recording
            .as_ref()
            .map(|journal| journal.read(cursor))
    }

    pub fn acknowledge_controls(
        &mut self,
        cursor: DynamicControlCursor,
    ) -> Option<Result<(), DynamicControlLogError>> {
        assert!(
            self.output_frame_undo.is_none(),
            "acknowledgement requires committed output"
        );
        self.control_recording
            .as_mut()
            .map(|journal| journal.acknowledge(cursor))
    }

    pub fn end_control_recording(&mut self) {
        assert!(
            self.output_frame_undo.is_none(),
            "control recording ends outside output transactions"
        );
        self.control_recording = None;
    }

    /// Reconcile borrowed spatial inputs once, retaining them only when they change the
    /// instance's target/phase mapping. Unchanged output frames never clone the position map
    /// or construct an owned journal payload. Automatic sampling cleanup uses direct methods.
    pub fn reconcile_instance_targets_recorded(
        &mut self,
        instance: Uuid,
        scope: DynamicTargetScope,
        positions: &HashMap<FixtureId, SpatialPosition>,
        mapping: Option<&SpatialSelectionMapping>,
        at_millis: u64,
    ) -> Result<bool, DynamicRuntimeError> {
        if self.control_recording.is_none() {
            return self.reconcile_instance_targets(instance, scope, positions, mapping);
        }
        if self.output_frame_undo.is_none() {
            return self.with_output_frame_transaction(
                &mut DynamicOutputFrameScratch::default(),
                |runtime| {
                    runtime.reconcile_instance_targets_recorded(
                        instance, scope, positions, mapping, at_millis,
                    )
                },
            );
        }
        self.check_recording_boundary()?;
        let preceding_sample = self.committed_sample_boundary();
        let changed = self.reconcile_instance_targets(instance, scope, positions, mapping)?;
        if changed {
            let request = TimedDynamicControl {
                at_millis,
                control: DynamicControl::Targets {
                    instance,
                    scope: DynamicTargetScope {
                        ordered_targets: self.instances[&instance].targets.clone(),
                    },
                    positions: positions.clone(),
                    mapping: mapping.cloned(),
                },
            };
            self.output_frame_undo
                .as_mut()
                .expect("active transaction")
                .recorded_controls
                .push(Arc::new(RecordedControl {
                    request,
                    instance: Some(instance),
                    preceding_sample,
                }));
        }
        Ok(changed)
    }

    /// Apply an authored or reconciliation control. Automatic sampling cleanup uses the direct
    /// domain mutations instead. Inside a frame, records stay provisional with runtime state;
    /// outside a frame this establishes its own transaction. Disabled recording adds no journal.
    pub fn apply_recorded_control(
        &mut self,
        request: TimedDynamicControl,
    ) -> Result<DynamicControlOutcome, DynamicRuntimeError> {
        if self.control_recording.is_none() {
            return apply(self, &request, None);
        }
        if self.output_frame_undo.is_none() {
            return self.with_output_frame_transaction(
                &mut DynamicOutputFrameScratch::default(),
                |runtime| runtime.apply_recorded_control(request),
            );
        }
        self.check_recording_boundary()?;
        let preceding_sample = self.committed_sample_boundary();
        let outcome = apply(self, &request, None)?;
        if outcome.changed {
            self.output_frame_undo
                .as_mut()
                .expect("active transaction")
                .recorded_controls
                .push(Arc::new(RecordedControl {
                    request,
                    instance: outcome.instance_id,
                    preceding_sample,
                }));
        }
        Ok(outcome)
    }

    fn check_recording_boundary(&self) -> Result<(), DynamicRuntimeError> {
        if self
            .output_frame_undo
            .as_ref()
            .is_some_and(|undo| undo.sample_boundary.is_some())
        {
            return Err(DynamicRuntimeError::InvalidReplay(
                "recorded controls must precede sampling in an output transaction".into(),
            ));
        }
        Ok(())
    }
}
