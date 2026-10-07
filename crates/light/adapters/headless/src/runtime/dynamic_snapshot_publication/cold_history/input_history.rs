//! Selected immutable inputs, retained independently of consumer publication coalescing.
use super::*;
use light_dynamics::{DynamicSampleBoundary, DynamicSpeedTransport};
use light_engine::{ContributionBatch, PreparedOutputFrame};
use std::{ops::Deref, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct InputCaptureCursor {
    epoch: Uuid,
    sequence: u64,
}

impl InputCaptureCursor {
    pub(in crate::runtime) fn immediately_follows(self, previous: Self) -> bool {
        self.epoch == previous.epoch && previous.sequence.checked_add(1) == Some(self.sequence)
    }
}

/// The boxed alternative is intentionally avoided: disabled retention must not add a frame
/// allocation. Only a due capture is moved into shared ownership before evaluation.
#[allow(clippy::large_enum_variant)]
pub(in crate::runtime) enum RetainedFrameCapture {
    Owned(PreparedOutputFrame),
    Shared {
        frame: Arc<PreparedOutputFrame>,
        selected_at: Instant,
        selected_in: InputCaptureCursor,
    },
}

impl RetainedFrameCapture {
    pub(in crate::runtime) fn select(
        frame: PreparedOutputFrame,
        publication: &DynamicSnapshotPublication,
        now: Instant,
    ) -> Self {
        if let Some(selected_in) = publication.input_capture_selection(now) {
            Self::Shared {
                frame: Arc::new(frame),
                selected_at: now,
                selected_in,
            }
        } else {
            Self::Owned(frame)
        }
    }

    pub(in crate::runtime) fn retained(&self) -> Option<RetainedInputBorrow<'_>> {
        match self {
            Self::Shared {
                frame,
                selected_at,
                selected_in,
            } => Some(RetainedInputBorrow {
                frame,
                selected_at: *selected_at,
                selected_in: *selected_in,
            }),
            Self::Owned(_) => None,
        }
    }
}

impl Deref for RetainedFrameCapture {
    type Target = PreparedOutputFrame;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(frame) => frame,
            Self::Shared { frame, .. } => frame,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::runtime) struct RetainedInputBorrow<'a> {
    frame: &'a Arc<PreparedOutputFrame>,
    selected_at: Instant,
    selected_in: InputCaptureCursor,
}

impl RetainedInputBorrow<'_> {
    pub(in crate::runtime) fn matches(&self, frame: &PreparedOutputFrame) -> bool {
        std::ptr::eq(self.frame.as_ref(), frame)
    }
}

/// A selected Pending attempt, not proof of a successful Pending render or physical delivery.
/// Consumers process every retained entry in order; only result publication may be coalesced.
pub(in crate::runtime) struct RetainedInputCapture {
    pub(in crate::runtime) from: InputCaptureCursor,
    pub(in crate::runtime) to: InputCaptureCursor,
    pub(in crate::runtime) cold: ColdGenerationCursor,
    pub(in crate::runtime) controls: DynamicControlCursor,
    pub(in crate::runtime) frame: Arc<PreparedOutputFrame>,
    pub(in crate::runtime) baseline: Arc<[ContributionBatch]>,
    pub(in crate::runtime) speed_transports: [DynamicSpeedTransport; 5],
    pub(in crate::runtime) rate: u16,
    pub(in crate::runtime) live_sample: Option<DynamicSampleBoundary>,
}

pub(super) struct InputHistory {
    capacity: NonZeroUsize,
    cursor: InputCaptureCursor,
    last_selected: Option<Instant>,
    entries: VecDeque<Arc<RetainedInputCapture>>,
}

impl InputHistory {
    pub(super) fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            cursor: InputCaptureCursor {
                epoch: Uuid::new_v4(),
                sequence: 0,
            },
            last_selected: None,
            entries: VecDeque::new(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.cursor = InputCaptureCursor {
            epoch: Uuid::new_v4(),
            sequence: 0,
        };
        self.last_selected = None;
        self.entries.clear();
    }

    fn due(&self, now: Instant) -> bool {
        self.last_selected.is_none_or(|previous| {
            now.checked_duration_since(previous).is_some_and(|elapsed| {
                elapsed >= crate::runtime::visualization_frame::VISUALIZATION_SOURCE_SAMPLE_INTERVAL
            })
        })
    }

    fn read(
        &self,
        cursor: InputCaptureCursor,
    ) -> Result<Vec<Arc<RetainedInputCapture>>, ColdGenerationReadError> {
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
    pub(in crate::runtime) fn input_capture_due(&self, now: Instant) -> bool {
        self.input_capture_selection(now).is_some()
    }

    fn input_capture_selection(&self, now: Instant) -> Option<InputCaptureCursor> {
        if !self.retaining.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        self.cold_history
            .lock()
            .as_ref()
            .and_then(|history| history.inputs.due(now).then_some(history.inputs.cursor))
    }

    /// Capture with the same Dynamics guard as episode seeding and its control/cold cursors.
    pub(in crate::runtime) fn input_capture_cursor(&self) -> Option<InputCaptureCursor> {
        self.cold_history
            .lock()
            .as_ref()
            .map(|history| history.inputs.cursor)
    }

    pub(in crate::runtime) fn input_captures_since(
        &self,
        cursor: InputCaptureCursor,
    ) -> Result<Vec<Arc<RetainedInputCapture>>, ColdGenerationReadError> {
        self.cold_history
            .lock()
            .as_ref()
            .ok_or(ColdGenerationReadError::WrongEpoch)?
            .inputs
            .read(cursor)
    }

    /// Append only after the complete authoritative frame transaction succeeds, still holding
    /// Dynamics. The retained frame and cursors therefore cannot straddle an edit/publication.
    pub(in crate::runtime) fn retain_accepted_input(
        &self,
        runtime: &DynamicRuntime,
        input: RetainedInputBorrow<'_>,
        baseline: &[ContributionBatch],
        speed_transports: &[DynamicSpeedTransport; 5],
        rate: u16,
        live_sample: Option<DynamicSampleBoundary>,
    ) {
        let mut history = self.cold_history.lock();
        let Some(history) = history.as_mut() else {
            return;
        };
        if input.selected_in.epoch != history.inputs.cursor.epoch
            || !self.matches(&input.frame.snapshot())
            || !history.inputs.due(input.selected_at)
        {
            return;
        }
        let Some(controls) = runtime.control_cursor() else {
            history.reset();
            return;
        };
        if history.inputs.cursor.sequence == u64::MAX {
            history.reset();
        }
        let from = history.inputs.cursor;
        history.inputs.cursor.sequence += 1;
        let retained = Arc::new(RetainedInputCapture {
            from,
            to: history.inputs.cursor,
            cold: history.cursor,
            controls,
            frame: input.frame.clone(),
            baseline: baseline.into(),
            speed_transports: *speed_transports,
            rate,
            live_sample,
        });
        if history.inputs.entries.len() == history.inputs.capacity.get() {
            history.inputs.entries.pop_front();
        }
        history.inputs.entries.push_back(retained);
        history.inputs.last_selected = Some(input.selected_at);
    }
}

#[cfg(test)]
mod tests;
