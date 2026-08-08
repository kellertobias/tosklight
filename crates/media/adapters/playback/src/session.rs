//! One playback session.
//!
//! A session belongs to one layer's selected asset, never to the asset itself. Two layers showing
//! the same video have two sessions, so they keep separate positions, transports, and reset
//! counters — the legacy application shared one cache entry per file and could not do that.
//!
//! The session owns no clock. Every call is stamped by the caller, which is what makes the whole
//! transport testable without a GPU, a decoder, or real time passing.

use std::sync::Arc;

use media_domain::timeline::{MediaTiming, Presentation, present};
use media_domain::{
    AssetId, LayerState, PlayMode, ResolvedTempo, SourceStatus, Timestamp, effective_rate,
};

/// What the session wants shown this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    /// The frame index to present, if one should be.
    pub frame: Option<usize>,
    /// What the layer's runtime status should now say.
    pub status: SourceStatus,
    /// The presentation the timeline resolved, for status projections and diagnostics.
    pub presentation: Presentation,
}

/// One layer's playback of one asset.
#[derive(Debug, Clone)]
pub struct PlaybackSession {
    asset: AssetId,
    timing: MediaTiming,
    /// Presentation timestamps, one per frame, in order. Resolving a position to a frame is a
    /// binary search over this rather than arithmetic, so a variable frame rate stays correct.
    presentation_micros: Arc<[u64]>,
    /// When the current pass began. Moved on a reset, a mode change, or a re-selection.
    anchor: Timestamp,
    /// Where the transport was when it last stopped advancing, so a pause holds its frame.
    held: Option<usize>,
    mode: PlayMode,
    reset_trigger: u32,
}

impl PlaybackSession {
    pub fn new(
        asset: AssetId,
        timing: MediaTiming,
        presentation_micros: Arc<[u64]>,
        started_at: Timestamp,
        mode: PlayMode,
    ) -> Self {
        Self {
            asset,
            timing,
            presentation_micros,
            anchor: started_at,
            held: None,
            mode,
            reset_trigger: 0,
        }
    }

    pub const fn asset(&self) -> AssetId {
        self.asset
    }

    pub const fn timing(&self) -> &MediaTiming {
        &self.timing
    }

    pub fn frame_count(&self) -> usize {
        self.presentation_micros.len()
    }

    /// Restarts the current pass from `now`.
    ///
    /// A completed Once restarts here, which is what makes it a terminal state rather than a dead
    /// end: it holds until the selection, the mode, or the transport changes.
    ///
    /// The held frame deliberately survives: it is the frame currently on screen, and a restart
    /// into Pause has to keep showing it rather than showing nothing. Any restart into a mode that
    /// names a position overwrites it on the very next delivery anyway.
    pub fn restart(&mut self, now: Timestamp) {
        self.anchor = now;
    }

    /// Applies a layer's current state, restarting the pass when something that defines the pass
    /// has changed.
    ///
    /// A play-mode change or an operator reset starts a new pass; a change to dimmer, tint, or
    /// geometry does not, because those do not belong to the transport.
    pub fn reconcile(&mut self, layer: &LayerState, now: Timestamp) {
        if layer.reset_trigger_id != self.reset_trigger {
            self.reset_trigger = layer.reset_trigger_id;
            self.restart(now);
        }
        if layer.play_mode != self.mode {
            self.mode = layer.play_mode;
            self.restart(now);
        }
    }

    /// What to show at `now`.
    pub fn deliver(
        &mut self,
        layer: &LayerState,
        tempo: ResolvedTempo,
        now: Timestamp,
    ) -> Delivery {
        let rate = effective_rate(
            self.mode.is_synchronized(),
            self.timing.intrinsic_bpm,
            tempo,
            f64::from(layer.speed_multiplier.factor()),
        );
        let presentation = present(self.mode, &self.timing, rate, now.since(self.anchor));

        let frame = match presentation.position() {
            Some(position) => {
                let index = self.frame_at(position);
                self.held = index;
                index
            }
            // A pause holds whatever frame was already showing rather than seeking anywhere.
            None => self.held,
        };

        let status = match presentation {
            Presentation::Completed { .. } => SourceStatus::Completed,
            _ => SourceStatus::Ready,
        };

        Delivery {
            frame,
            status,
            presentation,
        }
    }

    /// The frame showing at a position: the last one whose presentation timestamp has arrived.
    fn frame_at(&self, position: std::time::Duration) -> Option<usize> {
        let micros = position.as_micros() as u64;
        match self.presentation_micros.binary_search(&micros) {
            Ok(exact) => Some(exact),
            Err(0) => self.presentation_micros.first().map(|_| 0),
            Err(after) => Some(after - 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{MediaAddress, OnceEndState, SpeedMultiplier};

    /// Ten frames at 10 fps.
    fn session(mode: PlayMode) -> PlaybackSession {
        let timings: Vec<u64> = (0..10).map(|index| index * 100_000).collect();
        PlaybackSession::new(
            AssetId::new(),
            MediaTiming::from_frames(10, 10.0),
            Arc::from(timings.into_boxed_slice()),
            Timestamp::ZERO,
            mode,
        )
    }

    fn layer(mode: PlayMode) -> LayerState {
        LayerState {
            address: MediaAddress::new(1, 1),
            play_mode: mode,
            source_status: SourceStatus::Ready,
            ..Default::default()
        }
    }

    fn at(millis: u64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn a_loop_walks_the_frames_and_starts_again() {
        let mut session = session(PlayMode::Loop);
        let layer = layer(PlayMode::Loop);
        for (millis, expected) in [(0, 0), (250, 2), (500, 5), (950, 9), (1_000, 0), (1_250, 2)] {
            let delivery = session.deliver(&layer, ResolvedTempo::None, at(millis));
            assert_eq!(delivery.frame, Some(expected), "at {millis}ms");
            assert_eq!(delivery.status, SourceStatus::Ready);
        }
    }

    #[test]
    fn reverse_walks_the_frames_backward() {
        let mut session = session(PlayMode::Reverse);
        let layer = layer(PlayMode::Reverse);
        let frames: Vec<Option<usize>> = [0, 200, 400, 600]
            .into_iter()
            .map(|millis| {
                session
                    .deliver(&layer, ResolvedTempo::None, at(millis))
                    .frame
            })
            .collect();
        assert_eq!(frames, [Some(9), Some(8), Some(6), Some(4)]);
    }

    #[test]
    fn a_once_completes_on_its_last_frame_and_stays_there() {
        let mode = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        let mut session = session(mode);
        let layer = layer(mode);

        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(500)).status,
            SourceStatus::Ready
        );

        for millis in [1_000, 2_000, 60_000] {
            let delivery = session.deliver(&layer, ResolvedTempo::None, at(millis));
            assert_eq!(
                delivery.frame,
                Some(9),
                "holds the last frame at {millis}ms"
            );
            assert_eq!(
                delivery.status,
                SourceStatus::Completed,
                "and reports completed"
            );
        }
    }

    #[test]
    fn a_reverse_once_completes_on_the_first_frame() {
        let mode = PlayMode::ReverseOnce {
            end_state: OnceEndState::Hold,
        };
        let mut session = session(mode);
        let layer = layer(mode);

        let delivery = session.deliver(&layer, ResolvedTempo::None, at(1_000));
        assert_eq!(delivery.frame, Some(0));
        assert_eq!(delivery.status, SourceStatus::Completed);
    }

    #[test]
    fn a_completed_once_restarts_on_a_reset_rather_than_being_a_dead_end() {
        let mode = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        let mut session = session(mode);
        let mut layer = layer(mode);
        assert_eq!(
            session
                .deliver(&layer, ResolvedTempo::None, at(2_000))
                .status,
            SourceStatus::Completed
        );

        layer.reset_trigger_id += 1;
        session.reconcile(&layer, at(2_000));
        let delivery = session.deliver(&layer, ResolvedTempo::None, at(2_000));
        assert_eq!(delivery.frame, Some(0), "the pass restarted");
        assert_eq!(delivery.status, SourceStatus::Ready);
    }

    #[test]
    fn a_pause_holds_the_frame_that_was_showing() {
        let mut session = session(PlayMode::Loop);
        let mut layer = layer(PlayMode::Loop);
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(400)).frame,
            Some(4)
        );

        layer.play_mode = PlayMode::Pause;
        // A mode change restarts the pass, but pause names no position, so the held frame stands.
        session.reconcile(&layer, at(400));
        for millis in [400, 900, 5_000] {
            let delivery = session.deliver(&layer, ResolvedTempo::None, at(millis));
            assert_eq!(delivery.frame, Some(4), "still frame 4 at {millis}ms");
        }
    }

    #[test]
    fn a_stop_seeks_to_the_beginning_and_stays_there() {
        let mut session = session(PlayMode::Loop);
        let mut layer = layer(PlayMode::Loop);
        session.deliver(&layer, ResolvedTempo::None, at(400));

        layer.play_mode = PlayMode::Stop;
        session.reconcile(&layer, at(400));
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(900)).frame,
            Some(0)
        );
    }

    #[test]
    fn a_change_that_is_not_the_transport_does_not_restart_the_pass() {
        let mut session = session(PlayMode::Loop);
        let mut layer = layer(PlayMode::Loop);
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(500)).frame,
            Some(5)
        );

        layer.dimmer = 0.25;
        layer.rotation = 90.0;
        session.reconcile(&layer, at(500));
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(500)).frame,
            Some(5),
            "dimming a layer must not restart its video"
        );
    }

    #[test]
    fn the_speed_multiplier_changes_how_fast_the_frames_advance() {
        let mut session = session(PlayMode::Loop);
        let mut layer = layer(PlayMode::Loop);
        layer.speed_multiplier = SpeedMultiplier::Multiply(2);
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(250)).frame,
            Some(5)
        );

        layer.speed_multiplier = SpeedMultiplier::Divide(2);
        assert_eq!(
            session.deliver(&layer, ResolvedTempo::None, at(400)).frame,
            Some(2)
        );
    }

    #[test]
    fn a_synchronized_layer_retimes_to_the_tempo_source() {
        let timings: Vec<u64> = (0..10).map(|index| index * 100_000).collect();
        let mut session = PlaybackSession::new(
            AssetId::new(),
            MediaTiming::from_frames(10, 10.0).with_intrinsic_bpm(60.0),
            Arc::from(timings.into_boxed_slice()),
            Timestamp::ZERO,
            PlayMode::LoopSynced,
        );
        let layer = layer(PlayMode::LoopSynced);

        // A 60 BPM asset against a 120 BPM master runs at double speed.
        let delivery = session.deliver(&layer, ResolvedTempo::Live { bpm: 120.0 }, at(250));
        assert_eq!(delivery.frame, Some(5));
    }

    #[test]
    fn two_sessions_on_one_asset_keep_separate_positions() {
        let asset = AssetId::new();
        let timings: Arc<[u64]> = Arc::from(
            (0..10)
                .map(|index| index * 100_000)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let make = |mode| {
            PlaybackSession::new(
                asset,
                MediaTiming::from_frames(10, 10.0),
                timings.clone(),
                Timestamp::ZERO,
                mode,
            )
        };
        let mut first = make(PlayMode::Loop);
        let mut second = make(PlayMode::Reverse);
        let forward = layer(PlayMode::Loop);
        let backward = layer(PlayMode::Reverse);

        assert_eq!(
            first.deliver(&forward, ResolvedTempo::None, at(300)).frame,
            Some(3)
        );
        assert_eq!(
            second
                .deliver(&backward, ResolvedTempo::None, at(300))
                .frame,
            Some(7)
        );
        assert_eq!(
            first.asset(),
            second.asset(),
            "the same asset, two independent transports"
        );
    }

    #[test]
    fn a_session_never_asks_for_a_frame_that_does_not_exist() {
        for mode in PlayMode::ALL {
            let mut session = session(mode);
            let layer = layer(mode);
            for millis in (0..3_000).step_by(7) {
                let delivery = session.deliver(&layer, ResolvedTempo::None, at(millis));
                if let Some(frame) = delivery.frame {
                    assert!(
                        frame < session.frame_count(),
                        "{} asked for frame {frame} of {} at {millis}ms",
                        mode.label(),
                        session.frame_count()
                    );
                }
            }
        }
    }
}
