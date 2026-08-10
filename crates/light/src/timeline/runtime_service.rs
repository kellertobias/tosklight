use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use light_playback::{
    TimecodeDefinition, TimecodeFrame, TimecodeFrameRate, TimecodeId, TimecodeReconstructedState,
    TimecodeTransportAction, TimecodeTransportState,
};

pub trait TimecodeClock: Send + Sync {
    fn now_micros(&self) -> u64;
}

#[derive(Debug)]
pub struct SystemTimecodeClock {
    started: Instant,
}

impl Default for SystemTimecodeClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl TimecodeClock for SystemTimecodeClock {
    fn now_micros(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

pub trait TimecodeChangePublisher: Send + Sync {
    fn publish(&self, change: &TimecodeRuntimeChange);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimecodeRuntimeChangeCause {
    Installed,
    Action(TimecodeTransportAction),
    Tick { completed_loops: u64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeRuntimeSnapshot {
    pub timecode_id: TimecodeId,
    pub revision: u64,
    pub transport: TimecodeTransportState,
    pub frame: TimecodeFrame,
    pub duration: TimecodeFrame,
    pub reconstructed: TimecodeReconstructedState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeRuntimeChange {
    pub cause: TimecodeRuntimeChangeCause,
    pub snapshot: TimecodeRuntimeSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeRuntimeOutcome {
    pub changed: bool,
    pub snapshot: TimecodeRuntimeSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimecodeRuntimeError {
    pub message: String,
}

impl TimecodeRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct TimecodeRuntimeService {
    clock: Arc<dyn TimecodeClock>,
    publisher: Arc<dyn TimecodeChangePublisher>,
    rate: TimecodeFrameRate,
    runtimes: Arc<Mutex<BTreeMap<TimecodeId, Runtime>>>,
}

impl TimecodeRuntimeService {
    pub fn new(
        clock: Arc<dyn TimecodeClock>,
        publisher: Arc<dyn TimecodeChangePublisher>,
        rate: TimecodeFrameRate,
    ) -> Self {
        Self {
            clock,
            publisher,
            rate,
            runtimes: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn install(
        &self,
        definition: TimecodeDefinition,
        audio_duration: Option<TimecodeFrame>,
    ) -> Result<TimecodeRuntimeSnapshot, TimecodeRuntimeError> {
        definition
            .validate()
            .map_err(|error| TimecodeRuntimeError::new(error.message))?;
        let duration = definition.duration.or(audio_duration).ok_or_else(|| {
            TimecodeRuntimeError::new("Timecode has no configured or audio duration")
        })?;
        if duration.0 == 0 {
            return Err(TimecodeRuntimeError::new(
                "resolved Timecode duration must be positive",
            ));
        }
        let id = definition.id;
        let runtime = Runtime::new(definition, duration);
        let snapshot = runtime.snapshot();
        self.runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .insert(id, runtime);
        self.publisher.publish(&TimecodeRuntimeChange {
            cause: TimecodeRuntimeChangeCause::Installed,
            snapshot: snapshot.clone(),
        });
        Ok(snapshot)
    }

    pub fn snapshot(
        &self,
        id: TimecodeId,
    ) -> Result<TimecodeRuntimeSnapshot, TimecodeRuntimeError> {
        self.runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .get(&id)
            .map(Runtime::snapshot)
            .ok_or_else(|| TimecodeRuntimeError::new("Timecode runtime is not installed"))
    }

    pub fn snapshots(&self) -> Vec<TimecodeRuntimeSnapshot> {
        self.runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .values()
            .map(Runtime::snapshot)
            .collect()
    }

    /// Removes stale runtime state after a portable Timecode is deleted or becomes unrunnable.
    pub fn uninstall(&self, id: TimecodeId) -> bool {
        self.runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .remove(&id)
            .is_some()
    }

    pub fn handle(
        &self,
        id: TimecodeId,
        action: TimecodeTransportAction,
    ) -> Result<TimecodeRuntimeOutcome, TimecodeRuntimeError> {
        let now = self.clock.now_micros();
        let (changed, snapshot) = {
            let mut runtimes = self
                .runtimes
                .lock()
                .expect("Timecode runtime lock poisoned");
            let runtime = runtimes
                .get_mut(&id)
                .ok_or_else(|| TimecodeRuntimeError::new("Timecode runtime is not installed"))?;
            let before = runtime.operator_state();
            runtime.synchronize(now, self.rate);
            let action_changed = runtime.apply(action, now);
            let changed = action_changed || runtime.operator_state() != before;
            if changed {
                runtime.revision = runtime.revision.saturating_add(1);
            }
            (changed, runtime.snapshot())
        };
        if changed {
            self.publisher.publish(&TimecodeRuntimeChange {
                cause: TimecodeRuntimeChangeCause::Action(action),
                snapshot: snapshot.clone(),
            });
        }
        Ok(TimecodeRuntimeOutcome { changed, snapshot })
    }

    /// Advances every running Timecode in stable object-id order.
    pub fn tick(&self) -> Vec<TimecodeRuntimeChange> {
        let now = self.clock.now_micros();
        let changes = {
            let mut runtimes = self
                .runtimes
                .lock()
                .expect("Timecode runtime lock poisoned");
            runtimes
                .values_mut()
                .filter_map(|runtime| {
                    let advance = runtime.synchronize(now, self.rate)?;
                    runtime.revision = runtime.revision.saturating_add(1);
                    Some(TimecodeRuntimeChange {
                        cause: TimecodeRuntimeChangeCause::Tick {
                            completed_loops: advance.completed_loops,
                        },
                        snapshot: runtime.snapshot(),
                    })
                })
                .collect::<Vec<_>>()
        };
        for change in &changes {
            self.publisher.publish(change);
        }
        changes
    }
}

#[derive(Clone)]
struct Runtime {
    definition: TimecodeDefinition,
    duration: TimecodeFrame,
    revision: u64,
    state: TimecodeTransportState,
    frame: TimecodeFrame,
    running: Option<RunningAnchor>,
}

#[derive(Clone, Copy)]
struct RunningAnchor {
    started_at_micros: u64,
    started_at_frame: TimecodeFrame,
    observed_advanced_frames: u64,
    observed_completed_loops: u64,
}

#[derive(Clone, Copy)]
struct TickAdvance {
    completed_loops: u64,
}

impl Runtime {
    fn new(definition: TimecodeDefinition, duration: TimecodeFrame) -> Self {
        Self {
            definition,
            duration,
            revision: 1,
            state: TimecodeTransportState::Stopped,
            frame: TimecodeFrame::ZERO,
            running: None,
        }
    }

    fn snapshot(&self) -> TimecodeRuntimeSnapshot {
        TimecodeRuntimeSnapshot {
            timecode_id: self.definition.id,
            revision: self.revision,
            transport: self.state,
            frame: self.frame,
            duration: self.duration,
            reconstructed: self.definition.state_at(self.frame),
        }
    }

    fn operator_state(&self) -> (TimecodeTransportState, TimecodeFrame) {
        (self.state, self.frame)
    }

    fn synchronize(&mut self, now: u64, rate: TimecodeFrameRate) -> Option<TickAdvance> {
        let anchor = self.running.as_mut()?;
        let elapsed = now.saturating_sub(anchor.started_at_micros);
        let advanced = rate.frame_at_micros(elapsed).0;
        if advanced == anchor.observed_advanced_frames {
            return None;
        }
        anchor.observed_advanced_frames = advanced;
        let total = anchor.started_at_frame.0.saturating_add(advanced);
        let completed_loops = total / self.duration.0;
        let newly_completed = completed_loops.saturating_sub(anchor.observed_completed_loops);
        anchor.observed_completed_loops = completed_loops;
        self.frame = TimecodeFrame(total % self.duration.0);
        Some(TickAdvance {
            completed_loops: newly_completed,
        })
    }

    fn apply(&mut self, action: TimecodeTransportAction, now: u64) -> bool {
        match action {
            TimecodeTransportAction::Go | TimecodeTransportAction::Rewind => {
                self.state = TimecodeTransportState::Playing;
                self.frame = TimecodeFrame::ZERO;
                self.start(now);
                true
            }
            TimecodeTransportAction::Pause => match self.state {
                TimecodeTransportState::Playing => {
                    self.state = TimecodeTransportState::Paused;
                    self.running = None;
                    true
                }
                TimecodeTransportState::Paused => {
                    self.state = TimecodeTransportState::Playing;
                    self.start(now);
                    true
                }
                TimecodeTransportState::Stopped => false,
            },
            TimecodeTransportAction::Stop => {
                let changed = self.state != TimecodeTransportState::Stopped
                    || self.frame != TimecodeFrame::ZERO;
                self.state = TimecodeTransportState::Stopped;
                self.frame = TimecodeFrame::ZERO;
                self.running = None;
                changed
            }
            TimecodeTransportAction::Seek { frame } => {
                let frame = TimecodeFrame(frame.0.min(self.duration.0));
                let changed = self.frame != frame;
                self.frame = frame;
                if changed && self.state == TimecodeTransportState::Playing {
                    self.start(now);
                }
                changed
            }
        }
    }

    fn start(&mut self, now: u64) {
        self.running = Some(RunningAnchor {
            started_at_micros: now,
            started_at_frame: self.frame,
            observed_advanced_frames: 0,
            observed_completed_loops: 0,
        });
    }
}

#[cfg(test)]
mod tests;
