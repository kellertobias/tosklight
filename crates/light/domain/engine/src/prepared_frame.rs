//! Immutable inputs shared by one output frame and its speculative lanes.

use crate::{Engine, EngineSnapshot, FrameAddresser, RenderOptions, RuntimeGeneration};
use chrono::{DateTime, Utc};
use light_core::FixtureId;
use light_programmer::{HighlightOutputLayer, ProgrammerOutputSourceCapture};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

pub(crate) struct CapturedOutputOverlays {
    pub(crate) options: RenderOptions,
    pub(crate) flashes: HashMap<String, f32>,
    pub(crate) highlights: HashMap<FixtureId, HighlightOutputLayer>,
    pub(crate) highlight_look: light_fixture::HighlightLook,
}

/// Captures sources, generation, sample time and initial lane history once. A baseline or Release
/// branch evaluates these inputs without rereading live controls or advancing Live continuity.
pub struct PreparedOutputFrame {
    pub(crate) identity: Arc<()>,
    pub(crate) generation: Arc<RuntimeGeneration>,
    pub(crate) sampled_at: DateTime<Utc>,
    pub(crate) programmer: ProgrammerOutputSourceCapture,
    dynamic_programmer: Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>>,
    dynamic_programmer_rows: Arc<Vec<crate::CapturedDynamicProgrammerRow>>,
    pub(crate) playback: crate::resolution::PlaybackResolution,
    automatic_transitions_claimed: std::sync::atomic::AtomicBool,
    automatic_cue_actions_claimed: std::sync::atomic::AtomicBool,
    pub(crate) releases: crate::ContributionBatch,
    pub(crate) tracked: Arc<crate::TrackedInputFrame>,
    pub(crate) colors: HashMap<String, crate::engine::GroupColorContribution>,
    pub(crate) overlays: Arc<CapturedOutputOverlays>,
    pub(crate) programmer_fade_millis: u64,
    pub(crate) continuity_revision: crate::output_continuity::OutputContinuityRevision,
    pub(crate) continuity: crate::OutputContinuityState,
    scratch: Arc<parking_lot::Mutex<crate::engine::FrameScratch>>,
}

impl PreparedOutputFrame {
    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.generation.snapshot_arc()
    }
    /// No patched fixture holds a Freeze. Both `Engine::observe_prepared_values` and
    /// `Engine::prepare_static_family_frame` apply the Freeze overrides after the prepared
    /// resolution; with no Freeze both skip that step (TL-553).
    pub fn freezes_nothing(&self) -> bool {
        self.generation
            .snapshot()
            .fixtures
            .iter()
            .all(|fixture| fixture.freeze.is_empty())
    }
    pub fn sampled_at(&self) -> DateTime<Utc> {
        self.sampled_at
    }
    pub fn generation(&self) -> u64 {
        self.generation.identity()
    }
    /// The render options captured with this frame (grand master, blackout).
    pub fn options(&self) -> RenderOptions {
        self.overlays.options
    }
    pub fn frame_addresser(&self) -> FrameAddresser {
        FrameAddresser::new(Arc::clone(self.generation.slots()))
    }
    pub fn programmer(&self) -> &ProgrammerOutputSourceCapture {
        &self.programmer
    }
    pub fn tracking(&self) -> &Arc<crate::TrackedInputFrame> {
        &self.tracked
    }
    pub fn dynamic_programmer_values(
        &self,
    ) -> &Arc<Vec<(uuid::Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        &self.dynamic_programmer
    }
    /// Exact Live/Preload origin and edit stamp, captured alongside each Dynamic tuple.
    /// The vectors share an immutable capture and are aligned by index.
    pub fn dynamic_programmer_rows(&self) -> &Arc<Vec<crate::CapturedDynamicProgrammerRow>> {
        &self.dynamic_programmer_rows
    }
    pub fn cue_dynamic_values(&self) -> &[light_playback::ActiveCueDynamicValue] {
        &self.playback.cue_dynamic_values
    }
    pub fn dynamic_playbacks(&self) -> &[light_playback::ActiveDynamicPlayback] {
        &self.playback.dynamic_playbacks
    }
    pub fn playback_dynamics_paused(&self) -> bool {
        self.playback.dynamics_paused
    }
    /// The optional action-queue simulation base, captured under the same Playback guard and
    /// pinned clock as the rendered source rows. Never install this observer into Live.
    pub fn preload_playback_source(&self) -> Option<&light_playback::PlaybackEngine> {
        self.playback.preview_source.as_ref()
    }
    pub fn automatic_playback_transitions(&self) -> &[light_playback::AutomaticPlaybackTransition] {
        &self.playback.automatic_transitions
    }
    pub fn captured_active_playbacks(&self) -> &[light_playback::ActivePlayback] {
        &self.playback.active_playbacks
    }
    /// Claim only authoritative transitions from this captured scheduler tick. Event publication
    /// has its own claim token, so a failed DMX projection still emits one action and one event.
    pub fn claim_automatic_cue_action_transitions(
        &self,
    ) -> &[light_playback::AutomaticPlaybackTransition] {
        if self
            .automatic_cue_actions_claimed
            .swap(true, Ordering::AcqRel)
        {
            &[]
        } else {
            &self.playback.automatic_transitions
        }
    }
    /// Playback already advanced at capture. Its application events have one owner even when a
    /// physical render fails and a caller retries the same retained capture.
    pub fn claim_automatic_playback_transitions(
        &self,
    ) -> &[light_playback::AutomaticPlaybackTransition] {
        if self
            .automatic_transitions_claimed
            .swap(true, Ordering::AcqRel)
        {
            &[]
        } else {
            &self.playback.automatic_transitions
        }
    }
    pub fn playback_contributions(
        &self,
    ) -> impl Iterator<
        Item = (
            light_playback::SequenceMasterSource,
            &light_core::TimedValue,
        ),
    > {
        self.playback
            .contributions
            .iter()
            .filter_map(crate::EngineContribution::playback_value)
    }
}

impl Drop for PreparedOutputFrame {
    fn drop(&mut self) {
        // The immutable capture borrows the same reusable contribution vector as ordinary output.
        // It returns it only when no branch can still inspect these sources.
        let mut contributions = std::mem::take(&mut self.playback.contributions);
        contributions.clear();
        let mut scratch = self.scratch.lock();
        if scratch.contributions.capacity() < contributions.capacity() {
            scratch.contributions = contributions;
        }
    }
}

impl Engine {
    pub fn prepare_output_frame(&self, options: RenderOptions) -> PreparedOutputFrame {
        crate::timed(crate::RenderPhase::PreparedCapture, || {
            let continuity = self.capture_output_continuity();
            let programmer = self.programmers.capture_output_sources();
            self.prepare_captured_frame(options, programmer, continuity, true)
        })
    }

    /// A compound Programmer edit keeps the previous published frame instead of blocking output.
    pub fn try_prepare_output_frame(&self, options: RenderOptions) -> Option<PreparedOutputFrame> {
        crate::timed(crate::RenderPhase::PreparedCapture, || {
            let continuity = self.capture_output_continuity();
            let programmer = self.programmers.try_capture_output_sources()?;
            Some(self.prepare_captured_frame(options, programmer, continuity, true))
        })
    }

    pub(crate) fn prepare_observer_frame(&self, options: RenderOptions) -> PreparedOutputFrame {
        crate::timed(crate::RenderPhase::PreparedCapture, || {
            let continuity = self.capture_output_continuity();
            let programmer = self.programmers.capture_output_sources();
            self.prepare_captured_frame(options, programmer, continuity, false)
        })
    }

    fn prepare_captured_frame(
        &self,
        options: RenderOptions,
        programmer: ProgrammerOutputSourceCapture,
        (continuity_revision, continuity): (
            crate::output_continuity::OutputContinuityRevision,
            crate::OutputContinuityState,
        ),
        advance: bool,
    ) -> PreparedOutputFrame {
        let sampled_at = self.clock.now();
        if advance {
            self.advance_group_master_transitions_at(sampled_at);
        }
        let generation = self.generation.load_full();
        let playback = crate::timed(crate::RenderPhase::PlaybackResolution, || {
            self.resolve_playback(
                &generation,
                sampled_at,
                advance,
                &[],
                !programmer.preload_playback_actions.is_empty(),
            )
        });
        let (dynamic_programmer, dynamic_programmer_rows) = self
            .captured_dynamic_programmer_values_from_sources(programmer.normal_dynamics.clone());
        let releases = self
            .programmer_releases
            .lock()
            .compile(&programmer.output_states, &generation);
        // The token predates source capture. A reset or completed render while sources were
        // being acquired must invalidate this frame, rather than seed old sources into new history.
        PreparedOutputFrame {
            identity: Arc::new(()),
            generation,
            sampled_at,
            programmer,
            dynamic_programmer,
            dynamic_programmer_rows,
            playback,
            automatic_transitions_claimed: std::sync::atomic::AtomicBool::new(false),
            automatic_cue_actions_claimed: std::sync::atomic::AtomicBool::new(false),
            releases,
            tracked: self.tracking_frame(),
            colors: self.group_colors.read().clone(),
            overlays: Arc::new(self.capture_output_overlays(options)),
            programmer_fade_millis: self.programmer_fade_millis.load(Ordering::Relaxed),
            continuity_revision,
            continuity,
            scratch: Arc::clone(&self.scratch),
        }
    }

    pub(crate) fn capture_output_overlays(&self, options: RenderOptions) -> CapturedOutputOverlays {
        CapturedOutputOverlays {
            options: RenderOptions {
                color_model: self.color_model(),
                ..options
            },
            flashes: self.group_master_flashes.read().clone(),
            highlights: self.highlight_layers.read().clone(),
            highlight_look: self.highlight_look.read().clone(),
        }
    }
}
