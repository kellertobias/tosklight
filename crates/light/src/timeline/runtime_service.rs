use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};

use light_engine::{CueListPlaybackAction, Engine, EnginePlaybackCommand};
use light_playback::{
    TimecodeCueListClipExecution, TimecodeCueListClipExecutionKind, TimecodeDefinition,
    TimecodeFrame, TimecodeFrameRate, TimecodeId, TimecodeReconstructedState,
    TimecodeTransportAction, TimecodeTransportState,
};

use super::{TimecodeAudioService, WavMetadata};
use crate::AssetReference;

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
    ExternalSync { transport_frame: TimecodeFrame },
    Tick { completed_loops: u64 },
    CueListExecution,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimecodeRuntimeSnapshot {
    pub timecode_id: TimecodeId,
    pub revision: u64,
    pub transport: TimecodeTransportState,
    pub frame: TimecodeFrame,
    pub duration: TimecodeFrame,
    pub reconstructed: TimecodeReconstructedState,
    pub cue_list_clips: Vec<TimecodeCueListClipExecution>,
    pub audio_linked: bool,
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
    audio: Option<Arc<TimecodeAudioService>>,
    cue_list_runtime: Arc<Mutex<CueListRuntime>>,
}

impl TimecodeRuntimeService {
    pub fn frame_rate(&self) -> TimecodeFrameRate {
        self.rate
    }

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
            audio: None,
            cue_list_runtime: Arc::new(Mutex::new(CueListRuntime::default())),
        }
    }

    /// Attaches the server-owned audio transport to the same clock and lifecycle as Timecode.
    pub fn with_audio(mut self, audio: Arc<TimecodeAudioService>) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn prepare_audio(
        &self,
        timecode_id: TimecodeId,
        asset: AssetReference,
        metadata: WavMetadata,
        looping: bool,
    ) -> Result<TimecodeFrame, TimecodeRuntimeError> {
        self.audio
            .as_ref()
            .ok_or_else(|| {
                TimecodeRuntimeError::new("native Timecode audio output is unavailable")
            })?
            .prepare(timecode_id, asset, metadata, self.rate, looping)
            .map_err(TimecodeRuntimeError::new)
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
        let snapshot = runtime.snapshot(self.audio.as_deref());
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
            .map(|runtime| runtime.snapshot(self.audio.as_deref()))
            .ok_or_else(|| TimecodeRuntimeError::new("Timecode runtime is not installed"))
    }

    pub fn snapshots(&self) -> Vec<TimecodeRuntimeSnapshot> {
        self.runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .values()
            .map(|runtime| runtime.snapshot(self.audio.as_deref()))
            .collect()
    }

    /// Reconciles every running Timecode's Cuelist clips into the one shared playback runtime.
    /// Stable source ownership prevents one Timecode from releasing a Cuelist still owned by
    /// another active or held clip.
    pub fn reconcile_cue_lists(&self, engine: &Engine) {
        let installed = self
            .runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .values()
            .map(|runtime| {
                (
                    runtime.definition.clone(),
                    runtime.state,
                    runtime.frame,
                    runtime.cue_list_reconcile_mode,
                )
            })
            .collect::<Vec<_>>();
        let cue_lists = engine.snapshot().cue_lists.as_ref().clone();
        let (sequence_fade_millis, release_fade_millis) = engine.cue_timing_masters();
        let cue_timing = light_playback::TimecodeCueTimingDefaults {
            frame_rate: self.rate,
            sequence_fade_millis,
            release_fade_millis,
        };
        let mut statuses = BTreeMap::<TimecodeId, Vec<TimecodeCueListClipExecution>>::new();
        let mut desired = HashMap::new();
        for (definition, transport, frame, reconcile_mode) in installed {
            let resolved =
                definition.cue_list_clip_execution(&cue_lists, frame, transport, cue_timing);
            for clip in &resolved {
                if matches!(
                    clip.kind,
                    TimecodeCueListClipExecutionKind::Active
                        | TimecodeCueListClipExecutionKind::Held
                ) && let Some(cue_id) = clip.cue_id
                {
                    desired.insert(
                        clip.cue_list_id,
                        AppliedCueListClip {
                            source: CueListClipSource::from(clip),
                            cue_id,
                            cue_start_frame: clip.cue_start_frame.unwrap_or(frame),
                            start_behavior: clip.start_behavior,
                            transport,
                            reconcile_mode,
                            frame,
                            frame_rate: self.rate,
                        },
                    );
                }
            }
            statuses.insert(definition.id, resolved);
        }

        let mut runtime = self
            .cue_list_runtime
            .lock()
            .expect("Timecode Cuelist runtime lock poisoned");
        let previous = runtime.applied.clone();
        let mut cue_list_ids = previous
            .keys()
            .chain(desired.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        cue_list_ids.sort_by_key(|id| id.0);
        for cue_list_id in cue_list_ids {
            let before = previous.get(&cue_list_id);
            let next = desired.get(&cue_list_id);
            let result = match next {
                None => engine.execute_playback(EnginePlaybackCommand::CueList {
                    id: cue_list_id,
                    action: CueListPlaybackAction::Release,
                }),
                Some(next) => reconcile_desired_cue_list(engine, cue_list_id, before, next),
            };
            if let Err(message) = result {
                if let Some(next) = next
                    && let Some(clips) = statuses.get_mut(&next.source.timecode_id)
                    && let Some(status) = clips.iter_mut().find(|clip| {
                        clip.lane_id == next.source.lane_id && clip.clip_id == next.source.clip_id
                    })
                {
                    status.kind = TimecodeCueListClipExecutionKind::Unable;
                    status.message = Some(message);
                }
                desired.remove(&cue_list_id);
            }
        }
        runtime.applied = desired;
        drop(runtime);
        let mut runtimes = self
            .runtimes
            .lock()
            .expect("Timecode runtime lock poisoned");
        let mut changes = Vec::new();
        for (id, clips) in statuses {
            if let Some(runtime) = runtimes.get_mut(&id)
                && runtime.cue_list_clips != clips
            {
                runtime.cue_list_clips = clips;
                runtime.revision = runtime.revision.saturating_add(1);
                changes.push(TimecodeRuntimeChange {
                    cause: TimecodeRuntimeChangeCause::CueListExecution,
                    snapshot: runtime.snapshot(self.audio.as_deref()),
                });
            }
        }
        drop(runtimes);
        for change in changes {
            self.publisher.publish(&change);
        }
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
        // Reject a missing runtime before touching the output transport. Once installed, the
        // in-memory action cannot fail; applying audio first avoids advertising a transport state
        // that the device rejected.
        let installed = self.snapshot(id)?;
        if installed.audio_linked
            && let Some(audio) = &self.audio
        {
            audio
                .handle(id, action, now)
                .map_err(TimecodeRuntimeError::new)?;
        }
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
            if action_changed {
                runtime.cue_list_reconcile_mode = match action {
                    TimecodeTransportAction::Go if before.0 == TimecodeTransportState::Stopped => {
                        CueListReconcileMode::Enter
                    }
                    TimecodeTransportAction::Pause => runtime.cue_list_reconcile_mode,
                    _ => CueListReconcileMode::Reconstruct,
                };
            }
            let changed = action_changed || runtime.operator_state() != before;
            if changed {
                runtime.revision = runtime.revision.saturating_add(1);
            }
            (changed, runtime.snapshot(self.audio.as_deref()))
        };
        if changed {
            if snapshot.audio_linked
                && let Some(audio) = &self.audio
            {
                audio
                    .synchronize(
                        id,
                        snapshot.frame,
                        f64::from(snapshot.reconstructed.audio_volume),
                        now,
                    )
                    .map_err(TimecodeRuntimeError::new)?;
            }
            self.publisher.publish(&TimecodeRuntimeChange {
                cause: TimecodeRuntimeChangeCause::Action(action),
                snapshot: snapshot.clone(),
            });
        }
        Ok(TimecodeRuntimeOutcome { changed, snapshot })
    }

    pub fn remaining_millis(
        &self,
        id: TimecodeId,
        start: TimecodeFrame,
    ) -> Result<u64, TimecodeRuntimeError> {
        let snapshot = self.snapshot(id)?;
        let frames = snapshot
            .duration
            .0
            .saturating_sub(start.0.min(snapshot.duration.0));
        let micros = u128::from(frames)
            .saturating_mul(u128::from(self.rate.denominator()))
            .saturating_mul(1_000_000)
            / u128::from(self.rate.numerator());
        Ok(u64::try_from((micros.saturating_add(999)) / 1_000).unwrap_or(u64::MAX))
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
                    runtime.cue_list_reconcile_mode = if advance.completed_loops == 0 {
                        CueListReconcileMode::Forward
                    } else {
                        CueListReconcileMode::Reconstruct
                    };
                    runtime.revision = runtime.revision.saturating_add(1);
                    Some(TimecodeRuntimeChange {
                        cause: TimecodeRuntimeChangeCause::Tick {
                            completed_loops: advance.completed_loops,
                        },
                        snapshot: runtime.snapshot(self.audio.as_deref()),
                    })
                })
                .collect::<Vec<_>>()
        };
        for change in &changes {
            if change.snapshot.audio_linked
                && let Some(audio) = &self.audio
                && let Err(error) = audio.synchronize(
                    change.snapshot.timecode_id,
                    change.snapshot.frame,
                    f64::from(change.snapshot.reconstructed.audio_volume),
                    now,
                )
            {
                // A device failure must not stop DMX and playback scheduler progress. The
                // transport route still reports synchronous output errors to its caller.
                tracing::warn!(
                    timecode_id = %change.snapshot.timecode_id.0,
                    %error,
                    "Timecode audio synchronization failed"
                );
            }
            self.publisher.publish(change);
        }
        changes
    }

    /// Synchronizes eligible Timecodes to an authoritative external transport frame.
    ///
    /// Every definition applies its own offset. An auto-start definition begins exactly when the
    /// source reaches that offset; an operator-started definition continues to follow the source;
    /// an inactive, non-auto definition is intentionally untouched.
    pub fn synchronize_external(
        &self,
        transport_frame: TimecodeFrame,
    ) -> Vec<TimecodeRuntimeChange> {
        let now = self.clock.now_micros();
        let (changes, started) = {
            let mut runtimes = self
                .runtimes
                .lock()
                .expect("Timecode runtime lock poisoned");
            let mut started = Vec::new();
            let changes = runtimes
                .values_mut()
                .filter_map(|runtime| {
                    let eligible =
                        runtime.external_armed || runtime.state == TimecodeTransportState::Playing;
                    if !eligible {
                        return None;
                    }
                    let Some(local) = runtime.definition.local_frame_at_transport(transport_frame)
                    else {
                        return None;
                    };
                    let before = runtime.operator_state();
                    if runtime.external_armed {
                        runtime.external_armed = false;
                        runtime.state = TimecodeTransportState::Playing;
                        if runtime.definition.audio.is_some() {
                            started.push(runtime.definition.id);
                        }
                    }
                    runtime.frame = TimecodeFrame(local.0 % runtime.duration.0);
                    runtime.cue_list_reconcile_mode = CueListReconcileMode::Reconstruct;
                    // This anchor is dormant while an external source is locked, then gives
                    // Continue Internal a seamless takeover point if that source is lost.
                    runtime.start(now);
                    if runtime.operator_state() == before {
                        return None;
                    }
                    runtime.revision = runtime.revision.saturating_add(1);
                    Some(TimecodeRuntimeChange {
                        cause: TimecodeRuntimeChangeCause::ExternalSync { transport_frame },
                        snapshot: runtime.snapshot(self.audio.as_deref()),
                    })
                })
                .collect::<Vec<_>>();
            (changes, started)
        };
        if let Some(audio) = &self.audio {
            for id in started {
                if let Err(error) = audio.handle(id, TimecodeTransportAction::Go, now) {
                    tracing::warn!(timecode_id = %id.0, %error, "Timecode audio auto-start failed");
                }
            }
            for change in &changes {
                if !change.snapshot.audio_linked {
                    continue;
                }
                if let Err(error) = audio.synchronize(
                    change.snapshot.timecode_id,
                    change.snapshot.frame,
                    f64::from(change.snapshot.reconstructed.audio_volume),
                    now,
                ) {
                    tracing::warn!(
                        timecode_id = %change.snapshot.timecode_id.0,
                        %error,
                        "external Timecode audio synchronization failed"
                    );
                }
            }
        }
        for change in &changes {
            self.publisher.publish(change);
        }
        changes
    }
}

fn reconcile_desired_cue_list(
    engine: &Engine,
    cue_list_id: light_core::CueListId,
    before: Option<&AppliedCueListClip>,
    next: &AppliedCueListClip,
) -> Result<light_engine::EnginePlaybackOutcome, String> {
    let actual = engine.playback_runtime_status_for_cue_list(cue_list_id);
    let desired_changed =
        before.is_none_or(|before| before.source != next.source || before.cue_id != next.cue_id);
    let actual_drifted = actual
        .as_ref()
        .is_none_or(|status| status.playback.current_cue_id != Some(next.cue_id));
    let reconstruct_timing_changed = next.reconcile_mode == CueListReconcileMode::Reconstruct
        && before.is_some_and(|before| before.frame != next.frame);
    let changed_cue = desired_changed || actual_drifted || reconstruct_timing_changed;
    let mut outcome = if changed_cue {
        let entering = before.is_none_or(|before| before.source != next.source);
        let execute = desired_changed
            && match next.reconcile_mode {
                CueListReconcileMode::Forward => {
                    !entering || next.start_behavior == light_playback::TimecodeClipStart::Cue
                }
                CueListReconcileMode::Enter => {
                    entering && next.start_behavior == light_playback::TimecodeClipStart::Cue
                }
                CueListReconcileMode::Reconstruct => false,
            };
        let action = if execute {
            CueListPlaybackAction::ExecuteCueId(next.cue_id)
        } else {
            CueListPlaybackAction::ReconstructCueId {
                cue_id: next.cue_id,
                elapsed_millis: frames_to_millis(
                    next.frame.0.saturating_sub(next.cue_start_frame.0),
                    next.frame_rate,
                ),
            }
        };
        engine.execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action,
        })?
    } else {
        light_engine::EnginePlaybackOutcome::Applied
    };
    if next.transport == TimecodeTransportState::Paused
        && (changed_cue || actual.as_ref().is_none_or(|status| !status.playback.paused))
    {
        outcome = engine.execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: CueListPlaybackAction::Pause,
        })?;
    } else if !changed_cue
        && next.transport == TimecodeTransportState::Playing
        && actual.as_ref().is_some_and(|status| status.playback.paused)
    {
        outcome = engine.execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: CueListPlaybackAction::Resume,
        })?;
    }
    Ok(outcome)
}

fn frames_to_millis(frames: u64, rate: TimecodeFrameRate) -> u64 {
    let numerator = u128::from(frames)
        .saturating_mul(u128::from(rate.denominator()))
        .saturating_mul(1_000);
    let denominator = u128::from(rate.numerator());
    u64::try_from((numerator.saturating_add(denominator / 2)) / denominator).unwrap_or(u64::MAX)
}

#[derive(Clone)]
struct Runtime {
    definition: TimecodeDefinition,
    duration: TimecodeFrame,
    revision: u64,
    state: TimecodeTransportState,
    frame: TimecodeFrame,
    running: Option<RunningAnchor>,
    external_armed: bool,
    cue_list_clips: Vec<TimecodeCueListClipExecution>,
    cue_list_reconcile_mode: CueListReconcileMode,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CueListClipSource {
    timecode_id: TimecodeId,
    lane_id: light_playback::TimecodeLaneId,
    clip_id: light_playback::TimecodeClipId,
}

impl From<&TimecodeCueListClipExecution> for CueListClipSource {
    fn from(value: &TimecodeCueListClipExecution) -> Self {
        Self {
            timecode_id: value.timecode_id,
            lane_id: value.lane_id,
            clip_id: value.clip_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct AppliedCueListClip {
    source: CueListClipSource,
    cue_id: uuid::Uuid,
    cue_start_frame: TimecodeFrame,
    start_behavior: light_playback::TimecodeClipStart,
    transport: TimecodeTransportState,
    reconcile_mode: CueListReconcileMode,
    frame: TimecodeFrame,
    frame_rate: TimecodeFrameRate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CueListReconcileMode {
    Enter,
    Forward,
    Reconstruct,
}

#[derive(Default)]
struct CueListRuntime {
    applied: HashMap<light_core::CueListId, AppliedCueListClip>,
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
        let external_armed = definition.auto_start;
        Self {
            definition,
            duration,
            revision: 1,
            state: TimecodeTransportState::Stopped,
            frame: TimecodeFrame::ZERO,
            running: None,
            external_armed,
            cue_list_clips: Vec::new(),
            cue_list_reconcile_mode: CueListReconcileMode::Reconstruct,
        }
    }

    fn snapshot(&self, audio: Option<&TimecodeAudioService>) -> TimecodeRuntimeSnapshot {
        TimecodeRuntimeSnapshot {
            timecode_id: self.definition.id,
            revision: self.revision,
            transport: self.state,
            frame: self.frame,
            duration: self.duration,
            reconstructed: self.definition.state_at(self.frame),
            cue_list_clips: self.cue_list_clips.clone(),
            audio_linked: audio.is_some_and(|audio| audio.is_prepared(self.definition.id)),
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
                self.external_armed = false;
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
                self.external_armed = false;
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
