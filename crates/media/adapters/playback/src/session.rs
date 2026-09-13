//! One playback session.
//!
//! A session belongs to one layer's selected asset, never to the asset itself. Two layers showing
//! the same video have two sessions, so they keep separate positions, transports, and reset
//! counters — the legacy application shared one cache entry per file and could not do that.
//!
//! The session owns no clock. Every call is stamped by the caller, which is what makes the whole
//! transport testable without a GPU, a decoder, or real time passing.

use std::sync::Arc;
use std::time::Duration;

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

/// A layer's In and Out points resolved against one clip, as inclusive frame indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRange {
    pub first: usize,
    pub last: usize,
}

impl FrameRange {
    /// Resolves the wire In and Out points against a clip of `frame_count` frames.
    ///
    /// The In point counts frames from the clip's start and the Out point counts frames back from
    /// its end, so zero on both is the whole clip. An In point past the clip's last frame clamps to
    /// that frame. An Out point that would end the range before its In point, including one longer
    /// than the clip, plays through to the clip's end.
    pub fn resolve(in_point: u16, out_point: u16, frame_count: usize) -> Self {
        let end = frame_count.saturating_sub(1);
        let first = usize::from(in_point).min(end);
        let last = end
            .checked_sub(usize::from(out_point))
            .filter(|last| *last >= first)
            .unwrap_or(end);
        Self { first, last }
    }

    /// The whole clip.
    pub const fn full(frame_count: usize) -> Self {
        Self {
            first: 0,
            last: frame_count.saturating_sub(1),
        }
    }

    const fn clamp(self, frame: usize) -> usize {
        if frame < self.first {
            self.first
        } else if frame > self.last {
            self.last
        } else {
            frame
        }
    }
}

/// The stretch of the clip one pass runs over: where it starts, and its timing as if it were a
/// clip of its own.
struct Pass {
    start: Duration,
    timing: MediaTiming,
    /// Whether a range narrower than the whole clip applies.
    ranged: bool,
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
    /// Seconds of media added to the travelled distance, so a range change can keep the playhead
    /// where it was instead of restarting the pass.
    offset: f64,
    /// Where the transport was when it last stopped advancing, so a pause holds its frame.
    held: Option<usize>,
    mode: PlayMode,
    reset_trigger: u32,
    /// The range the transport currently runs over.
    range: FrameRange,
    /// The range the layer last asked for; adopted on the next delivery.
    requested: FrameRange,
    /// Whether the current pass has shown anything yet. Before it has, a range is simply taken
    /// over: there is no playhead on screen to preserve.
    delivered: bool,
}

impl PlaybackSession {
    pub fn new(
        asset: AssetId,
        timing: MediaTiming,
        presentation_micros: Arc<[u64]>,
        started_at: Timestamp,
        mode: PlayMode,
    ) -> Self {
        let full = FrameRange::full(presentation_micros.len());
        Self {
            asset,
            timing,
            presentation_micros,
            anchor: started_at,
            offset: 0.0,
            held: None,
            mode,
            reset_trigger: 0,
            range: full,
            requested: full,
            delivered: false,
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

    /// The range the transport currently runs over.
    pub const fn range(&self) -> FrameRange {
        self.range
    }

    /// Restarts the current pass from `now`, over the range the layer last asked for.
    ///
    /// A completed Once restarts here, which is what makes it a terminal state rather than a dead
    /// end: it holds until the selection, the mode, or the transport changes.
    ///
    /// The held frame deliberately survives: it is the frame currently on screen, and a restart
    /// into Pause has to keep showing it rather than showing nothing. Any restart into a mode that
    /// names a position overwrites it on the very next delivery anyway.
    pub fn restart(&mut self, now: Timestamp) {
        self.anchor = now;
        self.offset = 0.0;
        self.range = self.requested;
        self.delivered = false;
    }

    /// Applies a layer's current state, restarting the pass when something that defines the pass
    /// has changed.
    ///
    /// A play-mode change or an operator reset starts a new pass; a change to dimmer, tint, or
    /// geometry does not, because those do not belong to the transport. Neither does a change to
    /// the In or Out point: the playhead stays where it is, or moves into the new range.
    pub fn reconcile(&mut self, layer: &LayerState, now: Timestamp) {
        self.requested = if self.ignores_range() {
            FrameRange::full(self.frame_count())
        } else {
            FrameRange::resolve(layer.in_point, layer.out_point, self.frame_count())
        };
        if layer.reset_trigger_id != self.reset_trigger {
            self.reset_trigger = layer.reset_trigger_id;
            self.restart(now);
        }
        if layer.play_mode != self.mode {
            self.mode = layer.play_mode;
            self.restart(now);
        }
        if !self.delivered {
            self.range = self.requested;
        }
    }

    /// What to show at `now`.
    pub fn deliver(
        &mut self,
        layer: &LayerState,
        tempo: ResolvedTempo,
        now: Timestamp,
    ) -> Delivery {
        if self.requested != self.range {
            self.adopt_requested_range(layer, tempo, now);
        }
        self.delivered = true;
        let (relative, pass, _) = self.locate(layer, tempo, now);
        let presentation = shifted(relative, pass.start);

        let frame = match relative.position() {
            Some(position) => {
                let index = self.frame_at(pass.start + position);
                self.held = index;
                index
            }
            // A pause holds whatever frame was already showing rather than seeking anywhere, as
            // long as that frame is still inside the range.
            None => {
                let range = self.range;
                self.held = self.held.map(|frame| range.clamp(frame));
                self.held
            }
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

    /// Stills, and anything else with a single frame, have no range to honour.
    fn ignores_range(&self) -> bool {
        self.timing.is_still() || self.frame_count() <= 1
    }

    /// The stretch of the clip the current range covers.
    fn pass(&self) -> Pass {
        let count = self.frame_count();
        if self.ignores_range() || self.range == FrameRange::full(count) {
            return Pass {
                start: Duration::ZERO,
                timing: self.timing,
                ranged: false,
            };
        }
        let start = Duration::from_micros(self.presentation_micros[self.range.first]);
        let end = match self.presentation_micros.get(self.range.last + 1) {
            Some(&next) => Duration::from_micros(next),
            None => self.timing.duration,
        };
        let last_frame =
            Duration::from_micros(self.presentation_micros[self.range.last]).saturating_sub(start);
        let duration = end
            .saturating_sub(start)
            .max(last_frame + Duration::from_micros(1));
        Pass {
            start,
            timing: MediaTiming {
                duration,
                last_frame,
                intrinsic_bpm: self.timing.intrinsic_bpm,
            },
            ranged: true,
        }
    }

    /// The effective rate over a pass.
    ///
    /// A synchronized layer with a range fits the range, not the whole clip, to the beat: the range
    /// spans the whole number of beats it is closest to at its authored tempo (at least one), and
    /// those beats last as long as they do at the target tempo.
    fn rate(&self, layer: &LayerState, tempo: ResolvedTempo, pass: &Pass) -> f64 {
        let multiplier = f64::from(layer.speed_multiplier.factor());
        let synchronized = self.mode.is_synchronized();
        let whole_clip = effective_rate(synchronized, self.timing.intrinsic_bpm, tempo, multiplier);
        if !(synchronized && pass.ranged) {
            return whole_clip;
        }
        match (tempo.bpm(), self.timing.intrinsic_bpm) {
            (Some(target), Some(intrinsic)) if target > 0.0 && intrinsic > 0.0 => {
                let length = pass.timing.duration.as_secs_f64();
                let beats = (length * intrinsic / 60.0).round().max(1.0);
                length * target / (beats * 60.0) * multiplier.max(0.0)
            }
            _ => whole_clip,
        }
    }

    /// Where the transport is at `now` over the current range: the presentation relative to the
    /// range start, the pass, and how many seconds of media the pass has travelled.
    fn locate(
        &self,
        layer: &LayerState,
        tempo: ResolvedTempo,
        now: Timestamp,
    ) -> (Presentation, Pass, f64) {
        let pass = self.pass();
        let rate = self.rate(layer, tempo, &pass);
        let elapsed = now.since(self.anchor);
        let advanced = elapsed.as_secs_f64() * rate.max(0.0) + self.offset;
        let relative = if self.offset == 0.0 {
            present(self.mode, &pass.timing, rate, elapsed)
        } else {
            let travelled = Duration::try_from_secs_f64(advanced.max(0.0)).unwrap_or(Duration::MAX);
            present(self.mode, &pass.timing, 1.0, travelled)
        };
        (relative, pass, advanced)
    }

    /// Switches to the requested range without restarting the transport.
    ///
    /// A playhead inside the new range keeps its frame and direction. One outside it moves to where
    /// the mode would naturally take it: a loop wraps to the start of its pass, a single pass past
    /// its end completes, a bounce turns at the nearer bound. A completed single pass stays
    /// completed, now resting on the new range's end.
    fn adopt_requested_range(&mut self, layer: &LayerState, tempo: ResolvedTempo, now: Timestamp) {
        let (before, old_pass, old_advanced) = self.locate(layer, tempo, now);
        self.range = self.requested;
        if !self.mode.is_transport_running() {
            return;
        }
        let pass = self.pass();
        let rate = self.rate(layer, tempo, &pass);
        let length = pass.timing.duration.as_secs_f64();
        // Keeps a reverse pass on the frame it showed rather than exactly on its boundary, where
        // the reverse arithmetic would wrap to the other end.
        const NUDGE: f64 = 1e-6;

        let target = if let Presentation::Completed { .. } = before {
            length
        } else {
            let Some(position) = before.position() else {
                return;
            };
            let absolute = (old_pass.start + position).as_secs_f64();
            let relative = absolute - pass.start.as_secs_f64();
            let before_range = relative < 0.0;
            let after_range = relative >= length;
            let inside = !before_range && !after_range;
            match self.mode {
                PlayMode::Loop | PlayMode::LoopSynced => {
                    if inside {
                        relative
                    } else {
                        0.0
                    }
                }
                PlayMode::Once { .. } | PlayMode::OnceSynced { .. } => {
                    if before_range {
                        0.0
                    } else if after_range {
                        length
                    } else {
                        relative
                    }
                }
                PlayMode::Reverse | PlayMode::ReverseSynced => {
                    if inside {
                        (length - relative - NUDGE).max(0.0)
                    } else {
                        0.0
                    }
                }
                PlayMode::ReverseOnce { .. } | PlayMode::ReverseOnceSynced { .. } => {
                    if after_range {
                        0.0
                    } else if before_range {
                        length
                    } else {
                        (length - relative - NUDGE).max(0.0)
                    }
                }
                PlayMode::Bounce | PlayMode::BounceSynced => {
                    if before_range {
                        0.0
                    } else if after_range {
                        length
                    } else {
                        let old_length = old_pass.timing.duration.as_secs_f64();
                        let returning = old_length > 0.0
                            && old_advanced.max(0.0) % (2.0 * old_length) > old_length;
                        if returning {
                            2.0 * length - relative
                        } else {
                            relative
                        }
                    }
                }
                PlayMode::Stop | PlayMode::Pause => return,
            }
        };
        self.offset = target - now.since(self.anchor).as_secs_f64() * rate.max(0.0);
    }

    /// The frame showing at a position: the last one whose presentation timestamp has arrived,
    /// kept inside the current range.
    fn frame_at(&self, position: Duration) -> Option<usize> {
        let micros = position.as_micros() as u64;
        let index = match self.presentation_micros.binary_search(&micros) {
            Ok(exact) => Some(exact),
            Err(0) => self.presentation_micros.first().map(|_| 0),
            Err(after) => Some(after - 1),
        };
        index.map(|frame| self.range.clamp(frame))
    }
}

/// A presentation relative to a range start, moved onto the clip's own timeline.
fn shifted(presentation: Presentation, start: Duration) -> Presentation {
    match presentation {
        Presentation::Frame { position } => Presentation::Frame {
            position: start + position,
        },
        Presentation::Completed {
            end_state,
            position,
        } => Presentation::Completed {
            end_state,
            position: start + position,
        },
        other => other,
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

    // ---- In point / Out point ----

    fn ranged(mode: PlayMode, in_point: u16, out_point: u16) -> LayerState {
        LayerState {
            in_point,
            out_point,
            ..layer(mode)
        }
    }

    fn frames_at(
        session: &mut PlaybackSession,
        layer: &LayerState,
        millis: &[u64],
    ) -> Vec<Option<usize>> {
        millis
            .iter()
            .map(|&millis| {
                session
                    .deliver(layer, ResolvedTempo::None, at(millis))
                    .frame
            })
            .collect()
    }

    fn started(mode: PlayMode, layer: &LayerState) -> PlaybackSession {
        let mut session = session(mode);
        session.reconcile(layer, at(0));
        session
    }

    #[test]
    fn in_and_out_points_resolve_with_clamping_and_play_through() {
        assert_eq!(
            FrameRange::resolve(3, 2, 10),
            FrameRange { first: 3, last: 7 },
            "in counts from the start, out counts back from the end"
        );
        assert_eq!(
            FrameRange::resolve(0, 0, 10),
            FrameRange::full(10),
            "zero is the whole clip"
        );
        assert_eq!(
            FrameRange::resolve(4, 0, 10),
            FrameRange { first: 4, last: 9 },
            "out 0 plays through"
        );
        assert_eq!(
            FrameRange::resolve(5, 4, 10),
            FrameRange { first: 5, last: 5 },
            "out landing on the in point holds that one frame"
        );
        assert_eq!(
            FrameRange::resolve(6, 5, 10),
            FrameRange { first: 6, last: 9 },
            "out before in plays through"
        );
        assert_eq!(
            FrameRange::resolve(50, 0, 10),
            FrameRange { first: 9, last: 9 },
            "in past the end clamps to the last frame"
        );
        assert_eq!(
            FrameRange::resolve(2, 400, 10),
            FrameRange { first: 2, last: 9 },
            "out longer than the clip plays through"
        );
    }

    #[test]
    fn a_ranged_loop_wraps_within_in_and_out() {
        let layer = ranged(PlayMode::Loop, 3, 2);
        let mut session = started(PlayMode::Loop, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 250, 450, 550, 750]),
            [Some(3), Some(5), Some(7), Some(3), Some(5)]
        );
    }

    #[test]
    fn a_ranged_reverse_runs_from_out_back_to_in() {
        let layer = ranged(PlayMode::Reverse, 3, 2);
        let mut session = started(PlayMode::Reverse, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 250, 450, 550]),
            [Some(7), Some(5), Some(3), Some(7)]
        );
    }

    #[test]
    fn a_ranged_bounce_turns_at_in_and_out() {
        let layer = ranged(PlayMode::Bounce, 3, 2);
        let mut session = started(PlayMode::Bounce, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 450, 550, 750, 950, 1_150]),
            [Some(3), Some(7), Some(7), Some(5), Some(3), Some(4)]
        );
    }

    #[test]
    fn a_ranged_once_ends_on_the_out_point_in_every_end_state() {
        for end_state in [
            OnceEndState::Hold,
            OnceEndState::Black,
            OnceEndState::Transparent,
        ] {
            let mode = PlayMode::Once { end_state };
            let layer = ranged(mode, 3, 2);
            let mut session = started(mode, &layer);
            let playing = session.deliver(&layer, ResolvedTempo::None, at(250));
            assert_eq!(playing.frame, Some(5));
            assert_eq!(playing.status, SourceStatus::Ready);
            for millis in [500, 5_000] {
                let done = session.deliver(&layer, ResolvedTempo::None, at(millis));
                assert_eq!(done.frame, Some(7), "{end_state:?} at {millis}ms");
                assert_eq!(done.status, SourceStatus::Completed);
            }
        }
    }

    #[test]
    fn a_ranged_reverse_once_ends_on_the_in_point() {
        let mode = PlayMode::ReverseOnce {
            end_state: OnceEndState::Hold,
        };
        let layer = ranged(mode, 3, 2);
        let mut session = started(mode, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 250]),
            [Some(7), Some(5)]
        );
        let done = session.deliver(&layer, ResolvedTempo::None, at(500));
        assert_eq!(done.frame, Some(3));
        assert_eq!(done.status, SourceStatus::Completed);
    }

    fn synced_session(mode: PlayMode) -> PlaybackSession {
        let timings: Vec<u64> = (0..10).map(|index| index * 100_000).collect();
        PlaybackSession::new(
            AssetId::new(),
            MediaTiming::from_frames(10, 10.0).with_intrinsic_bpm(60.0),
            Arc::from(timings.into_boxed_slice()),
            Timestamp::ZERO,
            mode,
        )
    }

    #[test]
    fn a_synced_range_fits_its_own_length_to_the_beat() {
        // Frames 0..=4 are half a second: at the authored 60 BPM that rounds to one beat, which at
        // 120 BPM lasts half a second, so the range plays at its own speed rather than double.
        let layer = ranged(PlayMode::LoopSynced, 0, 5);
        let mut session = synced_session(PlayMode::LoopSynced);
        session.reconcile(&layer, at(0));
        let tempo = ResolvedTempo::Live { bpm: 120.0 };
        let frames: Vec<_> = [0, 250, 450, 550]
            .into_iter()
            .map(|millis| session.deliver(&layer, tempo, at(millis)).frame)
            .collect();
        assert_eq!(frames, [Some(0), Some(2), Some(4), Some(0)]);

        // At 60 BPM the same beat lasts a second.
        let mut slower = synced_session(PlayMode::LoopSynced);
        slower.reconcile(&layer, at(0));
        let slow = ResolvedTempo::Live { bpm: 60.0 };
        slower.deliver(&layer, slow, at(0));
        assert_eq!(slower.deliver(&layer, slow, at(500)).frame, Some(2));
    }

    #[test]
    fn a_synced_once_and_bounce_honour_the_range() {
        let tempo = ResolvedTempo::Live { bpm: 120.0 };
        let once = PlayMode::OnceSynced {
            end_state: OnceEndState::Hold,
        };
        let layer = ranged(once, 5, 9);
        let mut session = synced_session(once);
        session.reconcile(&layer, at(0));
        assert_eq!(session.deliver(&layer, tempo, at(0)).frame, Some(5));
        let done = session.deliver(&layer, tempo, at(500));
        assert_eq!(done.frame, Some(9));
        assert_eq!(done.status, SourceStatus::Completed);

        let layer = ranged(PlayMode::BounceSynced, 5, 0);
        let mut session = synced_session(PlayMode::BounceSynced);
        session.reconcile(&layer, at(0));
        let frames: Vec<_> = [0, 450, 750, 950]
            .into_iter()
            .map(|millis| session.deliver(&layer, tempo, at(millis)).frame)
            .collect();
        assert_eq!(frames, [Some(5), Some(9), Some(7), Some(5)]);
    }

    #[test]
    fn stop_seeks_to_the_in_point_and_pause_holds_inside_the_range() {
        let layer = ranged(PlayMode::Stop, 3, 2);
        let mut session = session(PlayMode::Loop);
        session.reconcile(&layer, at(0));
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 900]),
            [Some(3), Some(3)]
        );

        let mut session = session_playing_to(850);
        let paused = ranged(PlayMode::Pause, 2, 4);
        session.reconcile(&paused, at(850));
        assert_eq!(
            frames_at(&mut session, &paused, &[850, 2_000]),
            [Some(5), Some(5)],
            "a held frame past the new out point moves onto it"
        );

        let mut session = session_playing_to(450);
        let paused = ranged(PlayMode::Pause, 2, 4);
        session.reconcile(&paused, at(450));
        assert_eq!(
            session.deliver(&paused, ResolvedTempo::None, at(900)).frame,
            Some(4),
            "a held frame inside the range stays"
        );
    }

    /// A full-clip loop that has been delivered at `millis`.
    fn session_playing_to(millis: u64) -> PlaybackSession {
        let layer = layer(PlayMode::Loop);
        let mut session = started(PlayMode::Loop, &layer);
        session.deliver(&layer, ResolvedTempo::None, at(millis));
        session
    }

    #[test]
    fn an_in_point_past_the_end_holds_the_last_frame_and_an_out_point_past_it_plays_through() {
        let layer = ranged(PlayMode::Loop, 50, 0);
        let mut session = started(PlayMode::Loop, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 350, 1_750]),
            [Some(9), Some(9), Some(9)]
        );

        let layer = ranged(PlayMode::Loop, 2, 60);
        let mut session = started(PlayMode::Loop, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 750, 850]),
            [Some(2), Some(9), Some(2)]
        );
    }

    #[test]
    fn an_out_point_of_zero_or_before_the_in_point_plays_to_the_end() {
        let layer = ranged(PlayMode::Loop, 4, 0);
        let mut session = started(PlayMode::Loop, &layer);
        assert_eq!(
            frames_at(&mut session, &layer, &[0, 550, 650]),
            [Some(4), Some(9), Some(4)]
        );

        for out_point in [4, 9] {
            let layer = ranged(PlayMode::Loop, 6, out_point);
            let mut session = started(PlayMode::Loop, &layer);
            assert_eq!(
                frames_at(&mut session, &layer, &[0, 350, 450]),
                [Some(6), Some(9), Some(6)],
                "out {out_point}"
            );
        }
    }

    #[test]
    fn changing_the_range_while_playing_keeps_the_playhead() {
        let mut session = session_playing_to(550);
        let layer = ranged(PlayMode::Loop, 2, 1);
        session.reconcile(&layer, at(550));
        let delivery = session.deliver(&layer, ResolvedTempo::None, at(550));
        assert_eq!(delivery.frame, Some(5), "no jump back to the in point");
        assert_eq!(delivery.status, SourceStatus::Ready);
        assert_eq!(
            frames_at(&mut session, &layer, &[750, 850, 950]),
            [Some(7), Some(8), Some(2)],
            "and it now wraps at the new out point"
        );
    }

    #[test]
    fn a_playhead_outside_a_new_range_moves_into_it() {
        // A loop past the new out point wraps to the in point.
        let mut session = session_playing_to(850);
        let layer = ranged(PlayMode::Loop, 2, 4);
        session.reconcile(&layer, at(850));
        assert_eq!(
            frames_at(&mut session, &layer, &[850, 950]),
            [Some(2), Some(3)]
        );

        // A reverse loop past the new out point starts again from it.
        let reverse = layer_with(PlayMode::Reverse, 0, 0);
        let mut session = started(PlayMode::Reverse, &reverse);
        assert_eq!(
            session
                .deliver(&reverse, ResolvedTempo::None, at(250))
                .frame,
            Some(7)
        );
        let narrowed = layer_with(PlayMode::Reverse, 2, 4);
        session.reconcile(&narrowed, at(250));
        assert_eq!(
            frames_at(&mut session, &narrowed, &[250, 400]),
            [Some(5), Some(4)]
        );

        // A single pass past its new end completes there.
        let once = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        let mut session = started(once, &layer_with(once, 0, 0));
        session.deliver(&layer_with(once, 0, 0), ResolvedTempo::None, at(850));
        let narrowed = layer_with(once, 2, 4);
        session.reconcile(&narrowed, at(850));
        let done = session.deliver(&narrowed, ResolvedTempo::None, at(850));
        assert_eq!(done.frame, Some(5));
        assert_eq!(done.status, SourceStatus::Completed);
    }

    fn layer_with(mode: PlayMode, in_point: u16, out_point: u16) -> LayerState {
        ranged(mode, in_point, out_point)
    }

    #[test]
    fn a_bounce_keeps_its_direction_across_a_range_change() {
        let full = layer(PlayMode::Bounce);
        let mut session = started(PlayMode::Bounce, &full);
        assert_eq!(
            session.deliver(&full, ResolvedTempo::None, at(1_250)).frame,
            Some(7),
            "on the way back"
        );
        let layer = ranged(PlayMode::Bounce, 2, 0);
        session.reconcile(&layer, at(1_250));
        assert_eq!(
            frames_at(&mut session, &layer, &[1_250, 1_350, 1_550]),
            [Some(7), Some(6), Some(4)],
            "still heading back"
        );
    }

    #[test]
    fn stills_ignore_the_range() {
        let mut still = PlaybackSession::new(
            AssetId::new(),
            MediaTiming::still(),
            Arc::from(vec![0u64].into_boxed_slice()),
            Timestamp::ZERO,
            PlayMode::Loop,
        );
        let layer = ranged(PlayMode::Loop, 5, 1);
        still.reconcile(&layer, at(0));
        assert_eq!(
            frames_at(&mut still, &layer, &[0, 500, 5_000]),
            [Some(0), Some(0), Some(0)]
        );
    }

    #[test]
    fn a_ranged_session_never_leaves_its_range_in_any_mode() {
        for mode in PlayMode::ALL {
            let layer = ranged(mode, 3, 2);
            let mut session = session(PlayMode::Loop);
            session.reconcile(&layer, at(0));
            for millis in (0..3_000).step_by(7) {
                if let Some(frame) = session
                    .deliver(&layer, ResolvedTempo::None, at(millis))
                    .frame
                {
                    assert!(
                        (3..=7).contains(&frame),
                        "{} showed frame {frame} at {millis}ms",
                        mode.label()
                    );
                }
            }
        }
    }
}
