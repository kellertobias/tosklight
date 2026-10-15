//! An isolated transport/clip participant in one ordered Preload transaction.
use super::*;
use std::{cell::RefCell, sync::MutexGuard};

struct BufferedChanges(Mutex<Vec<TimecodeRuntimeChange>>);
impl TimecodeChangePublisher for BufferedChanges {
    fn publish(&self, change: &TimecodeRuntimeChange) {
        self.0
            .lock()
            .expect("Timecode change buffer poisoned")
            .push(change.clone());
    }
}
struct CapturedClock(u64);
impl TimecodeClock for CapturedClock {
    fn now_micros(&self) -> u64 {
        self.0
    }
}

/// Keeps the transport gate until installation or discard. Its service owns fresh runtime,
/// ownership, publisher and audio maps; cloning the live service would share those maps.
pub struct PreparedTimecodeBatch<'a> {
    source: &'a TimecodeRuntimeService,
    _commands: Option<MutexGuard<'a, ()>>,
    candidate: TimecodeRuntimeService,
    changes: Arc<BufferedChanges>,
    before: Vec<TimecodeRuntimeSnapshot>,
    before_owners: HashMap<light_core::CueListId, AppliedCueListClip>,
    audio: Option<super::super::audio::PreparedTimecodeAudioBatch<'a>>,
}

impl TimecodeRuntimeService {
    /// Discovery owns only copied maps. It cannot install and must not hold a source transport
    /// gate while immutable assets are read or decoded.
    pub fn prepare_detached_transport_batch(
        &self,
        captured_micros: u64,
    ) -> PreparedTimecodeBatch<'_> {
        let mut candidate = self.prepare_transport_batch(captured_micros);
        candidate._commands.take();
        candidate
    }
    pub fn capture_transport_micros(&self) -> u64 {
        self.clock.now_micros()
    }

    pub fn prepare_transport_batch(&self, captured_micros: u64) -> PreparedTimecodeBatch<'_> {
        let commands = self
            .transport_commands
            .lock()
            .expect("Timecode command lock poisoned");
        let audio = self
            .audio
            .as_deref()
            .map(TimecodeAudioService::prepare_transport_batch);
        let changes = Arc::new(BufferedChanges(Mutex::new(Vec::new())));
        let candidate = Self {
            clock: Arc::new(CapturedClock(captured_micros)),
            publisher: changes.clone(),
            rate: self.rate,
            transport_commands: Arc::new(Mutex::new(())),
            runtimes: Arc::new(Mutex::new(
                self.runtimes
                    .lock()
                    .expect("Timecode runtime lock poisoned")
                    .clone(),
            )),
            audio: audio.as_ref().map(|audio| audio.candidate()),
            cue_list_runtime: Arc::new(Mutex::new(
                self.cue_list_runtime
                    .lock()
                    .expect("Timecode Cuelist runtime lock poisoned")
                    .clone(),
            )),
        };
        PreparedTimecodeBatch {
            source: self,
            _commands: Some(commands),
            before: self.snapshots(),
            before_owners: self
                .cue_list_runtime
                .lock()
                .expect("Timecode Cuelist runtime lock poisoned")
                .applied
                .clone(),
            candidate,
            changes,
            audio,
        }
    }
}

impl PreparedTimecodeBatch<'_> {
    pub fn snapshot(
        &self,
        id: TimecodeId,
    ) -> Result<TimecodeRuntimeSnapshot, TimecodeRuntimeError> {
        self.candidate.snapshot(id)
    }
    pub fn installed_definition_matches(&self, definition: &TimecodeDefinition) -> bool {
        self.candidate.installed_definition_matches(definition)
    }
    pub fn stage_definition(
        &self,
        definition: TimecodeDefinition,
        audio: Option<(crate::AssetReference, WavMetadata)>,
    ) -> Result<(), TimecodeRuntimeError> {
        let audio_duration = match audio {
            Some((asset, metadata)) if self.candidate.audio.is_some() => Some(
                self.candidate
                    .prepare_audio(definition.id, asset, metadata, true)?,
            ),
            _ => None,
        };
        self.candidate
            .install(definition, audio_duration)
            .map(|_| ())
    }

    pub fn handle(
        &self,
        id: TimecodeId,
        action: TimecodeTransportAction,
    ) -> Result<TimecodeRuntimeOutcome, TimecodeRuntimeError> {
        if action == TimecodeTransportAction::Go {
            self.candidate.handle_playback_go(id)
        } else {
            self.candidate.handle(id, action)
        }
    }
    /// Direct Cue execution is settled against the same cloned Playback before the next queued
    /// action. In particular a later Solo winner can release a just-started or held clip.
    pub fn reconcile(
        &self,
        playback: &mut light_playback::PlaybackEngine,
        cue_lists: &[light_playback::CueList],
        timing: (u64, u64),
    ) -> Result<(), String> {
        let playback = RefCell::new(playback);
        let report = self.candidate.reconcile_cue_lists_from(
            cue_lists,
            timing,
            |id| playback.borrow().runtime_status_for_cue_list(id),
            |command, _| Engine::execute_prepared_cue_command(&mut playback.borrow_mut(), command),
        );
        if let Some(failure) = report.failures.first() {
            Err(failure.message.clone())
        } else {
            Ok(())
        }
    }
    pub fn preflight_audio(&mut self) -> Result<(), String> {
        if let Some(audio) = self.audio.as_mut() {
            audio.preflight_output()?;
        }
        Ok(())
    }
    pub fn take_audio_resources(
        &mut self,
    ) -> Option<super::super::audio::PreparedTimecodeAudioResources> {
        self.audio.as_mut().map(|audio| audio.take_resources())
    }
    pub fn use_audio_resources(
        &mut self,
        resources: Option<super::super::audio::PreparedTimecodeAudioResources>,
    ) -> Result<(), String> {
        match (self.audio.as_mut(), resources) {
            (Some(audio), Some(resources)) => audio.use_resources(resources),
            (None, None) => Ok(()),
            _ => Err("Timecode audio output changed after resource preflight".into()),
        }
    }
    /// Call after Engine/Dynamics/event validation, while every install guard is retained.
    /// No fallible logical work is allowed after this output acknowledgment.
    pub fn accept_audio(&mut self) -> Result<(), String> {
        if self._commands.is_none() {
            return Err("a detached Timecode discovery cannot be installed".into());
        }
        if self.source.snapshots() != self.before
            || self
                .source
                .cue_list_runtime
                .lock()
                .expect("Timecode Cuelist runtime lock poisoned")
                .applied
                != self.before_owners
        {
            return Err("Timecode runtime changed while Preload GO was being prepared".into());
        }
        if let Some(audio) = self.audio.as_mut() {
            audio.accept_output()?;
        }
        Ok(())
    }
    /// Infallible map installation. Publication is returned to the caller for after Live guards.
    pub fn install(self) -> Vec<TimecodeRuntimeChange> {
        assert!(
            self._commands.is_some(),
            "a detached Timecode discovery cannot be installed"
        );
        if let Some(audio) = self.audio {
            audio.install();
        }
        *self
            .source
            .runtimes
            .lock()
            .expect("Timecode runtime lock poisoned") = self
            .candidate
            .runtimes
            .lock()
            .expect("Timecode runtime lock poisoned")
            .clone();
        *self
            .source
            .cue_list_runtime
            .lock()
            .expect("Timecode Cuelist runtime lock poisoned") = self
            .candidate
            .cue_list_runtime
            .lock()
            .expect("Timecode Cuelist runtime lock poisoned")
            .clone();
        self.changes
            .0
            .lock()
            .expect("Timecode change buffer poisoned")
            .clone()
    }
}

impl TimecodeRuntimeService {
    pub fn publish_prepared_changes(&self, changes: Vec<TimecodeRuntimeChange>) {
        for change in changes {
            self.publisher.publish(&change);
        }
    }
}
