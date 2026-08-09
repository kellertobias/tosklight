//! The playback timeline.
//!
//! Where in an asset a layer should be, given how long it has been playing and how it was told to
//! play. This is pure arithmetic over timestamps — no decoder, no clock, no frame counter — so
//! every mode, boundary, and end state is testable without media.
//!
//! Playback advances from monotonic timestamps rather than accumulating frame deltas, so an
//! output that misses a frame does not fall behind the audio, and two outputs at different
//! refresh rates stay in step with each other.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::playback::{OnceEndState, PlayMode};

/// What a decoder knows about an asset's timing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTiming {
    pub duration: Duration,
    /// The greatest presentation timestamp strictly less than [`Self::duration`].
    ///
    /// The final frame is defined by timestamp, never by an accumulated frame count, so Once
    /// holds the same frame across every codec, whatever container padding it carries, on all
    /// three operating systems.
    pub last_frame: Duration,
    /// Beats per minute the asset was authored at, when it has one. Import may parse this from a
    /// filename token, but runtime never re-infers it.
    pub intrinsic_bpm: Option<f64>,
}

impl MediaTiming {
    /// Timing for an asset of `frame_count` frames at a constant rate.
    ///
    /// The last frame is at `(frame_count - 1) / rate`, so an N-frame asset holds frame N and
    /// never frame N-1.
    pub fn from_frames(frame_count: u64, frames_per_second: f64) -> Self {
        let count = frame_count.max(1);
        let frame = 1.0 / frames_per_second.max(f64::MIN_POSITIVE);
        Self {
            duration: Duration::from_secs_f64(count as f64 * frame),
            last_frame: Duration::from_secs_f64((count - 1) as f64 * frame),
            intrinsic_bpm: None,
        }
    }

    /// The same timing with an authored tempo attached.
    pub fn with_intrinsic_bpm(self, bpm: f64) -> Self {
        Self {
            intrinsic_bpm: Some(bpm),
            ..self
        }
    }

    /// A still image, or anything else with no timeline of its own.
    pub const fn still() -> Self {
        Self {
            duration: Duration::ZERO,
            last_frame: Duration::ZERO,
            intrinsic_bpm: None,
        }
    }

    pub const fn is_still(&self) -> bool {
        self.duration.is_zero()
    }
}

/// What a layer should present right now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Presentation {
    /// Show the frame at this position in the asset.
    Frame { position: Duration },
    /// Stop: not advancing, and seeked to the beginning.
    Stopped,
    /// Pause: hold whatever frame is already showing.
    Paused,
    /// A single pass has finished. A real terminal state, not a pause: it holds until the media
    /// selection, the play mode, or the transport state changes.
    Completed {
        end_state: OnceEndState,
        position: Duration,
    },
}

impl Presentation {
    /// The position to seek to, when this presentation names one.
    pub const fn position(self) -> Option<Duration> {
        match self {
            Self::Frame { position } | Self::Completed { position, .. } => Some(position),
            Self::Stopped => Some(Duration::ZERO),
            Self::Paused => None,
        }
    }

    /// Whether the transport is still moving. A completed Once reports not playing while its
    /// source stays selected and ready.
    pub const fn is_playing(self) -> bool {
        matches!(self, Self::Frame { .. })
    }
}

/// Where playback is, `elapsed` after the current pass began.
///
/// `rate` is the effective playback rate as a positive magnitude; direction comes from the mode.
/// A rate of zero holds the first frame rather than dividing by nothing.
pub fn present(mode: PlayMode, timing: &MediaTiming, rate: f64, elapsed: Duration) -> Presentation {
    match mode {
        PlayMode::Stop => return Presentation::Stopped,
        PlayMode::Pause => return Presentation::Paused,
        _ => {}
    }

    // A still image has no timeline to advance along; it simply shows.
    if timing.is_still() {
        return Presentation::Frame {
            position: Duration::ZERO,
        };
    }

    let duration = timing.duration.as_secs_f64();
    let advanced = elapsed.as_secs_f64() * rate.max(0.0);

    match mode {
        PlayMode::Loop | PlayMode::LoopSynced => Presentation::Frame {
            position: seconds(advanced % duration),
        },
        PlayMode::Reverse | PlayMode::ReverseSynced => {
            // Backward from the end, restarting from the end. At zero elapsed this is the last
            // frame, not one past it.
            let back = duration - (advanced % duration);
            Presentation::Frame {
                position: clamp_to_last(back, timing),
            }
        }
        PlayMode::Bounce | PlayMode::BounceSynced => {
            let cycle = duration * 2.0;
            let position = advanced % cycle;
            let position = if position <= duration {
                position
            } else {
                cycle - position
            };
            Presentation::Frame {
                position: clamp_to_last(position, timing),
            }
        }
        PlayMode::Once { end_state }
        | PlayMode::ReverseOnce { end_state }
        | PlayMode::OnceSynced { end_state }
        | PlayMode::ReverseOnceSynced { end_state } => {
            if advanced >= duration {
                return Presentation::Completed {
                    end_state,
                    position: once_end_position(timing, mode.is_reverse()),
                };
            }
            let position = if mode.is_reverse() {
                duration - advanced
            } else {
                advanced
            };
            Presentation::Frame {
                position: clamp_to_last(position, timing),
            }
        }
        PlayMode::Stop | PlayMode::Pause => unreachable!("handled above"),
    }
}

/// Where a finished single pass rests.
///
/// A forward pass ends on the last frame; a reverse pass ends at the start of the media, so its
/// Hold state is the *first* frame. Black and Transparent behave identically whichever direction
/// the pass ran.
///
/// Both directions are selectable: `ReverseOnce` and `ReverseOnceSynced` reach the reverse
/// branch from the wire.
pub fn once_end_position(timing: &MediaTiming, reverse: bool) -> Duration {
    if reverse {
        Duration::ZERO
    } else {
        timing.last_frame
    }
}

fn seconds(value: f64) -> Duration {
    Duration::from_secs_f64(value.max(0.0))
}

/// Keeps a position on a real frame. Nothing may present at or past the duration: there is no
/// frame there, and asking a decoder for one yields either nothing or the first frame of the next
/// loop.
fn clamp_to_last(position: f64, timing: &MediaTiming) -> Duration {
    let position = seconds(position);
    if position > timing.last_frame {
        timing.last_frame
    } else {
        position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten frames at 10 fps: one second long, last frame at 0.9s.
    fn ten_frames() -> MediaTiming {
        MediaTiming::from_frames(10, 10.0)
    }

    fn at(mode: PlayMode, elapsed_millis: u64) -> Presentation {
        present(
            mode,
            &ten_frames(),
            1.0,
            Duration::from_millis(elapsed_millis),
        )
    }

    fn position_millis(presentation: Presentation) -> u64 {
        presentation
            .position()
            .expect("this presentation names a position")
            .as_millis() as u64
    }

    #[test]
    fn frame_timing_puts_the_last_frame_before_the_duration() {
        let timing = ten_frames();
        assert_eq!(timing.duration, Duration::from_millis(1_000));
        assert_eq!(timing.last_frame, Duration::from_millis(900));
        assert!(timing.last_frame < timing.duration);
    }

    #[test]
    fn loop_wraps_at_the_duration() {
        assert_eq!(position_millis(at(PlayMode::Loop, 0)), 0);
        assert_eq!(position_millis(at(PlayMode::Loop, 500)), 500);
        assert_eq!(
            position_millis(at(PlayMode::Loop, 1_000)),
            0,
            "the wrap is exact"
        );
        assert_eq!(position_millis(at(PlayMode::Loop, 1_500)), 500);
    }

    #[test]
    fn reverse_starts_at_the_end_and_runs_backward() {
        assert_eq!(
            position_millis(at(PlayMode::Reverse, 0)),
            900,
            "the last frame, not past it"
        );
        assert_eq!(position_millis(at(PlayMode::Reverse, 250)), 750);
        assert_eq!(position_millis(at(PlayMode::Reverse, 900)), 100);
        assert_eq!(
            position_millis(at(PlayMode::Reverse, 1_000)),
            900,
            "and restarts from the end"
        );
    }

    #[test]
    fn bounce_alternates_without_repeating_the_turning_frames() {
        assert_eq!(position_millis(at(PlayMode::Bounce, 0)), 0);
        assert_eq!(position_millis(at(PlayMode::Bounce, 500)), 500);
        assert_eq!(position_millis(at(PlayMode::Bounce, 900)), 900);
        assert_eq!(
            position_millis(at(PlayMode::Bounce, 1_500)),
            500,
            "coming back"
        );
        assert_eq!(
            position_millis(at(PlayMode::Bounce, 2_000)),
            0,
            "and round again"
        );
    }

    #[test]
    fn bounce_never_completes_because_it_has_no_single_pass_end() {
        for elapsed in [0, 1_000, 5_000, 50_000] {
            assert!(at(PlayMode::Bounce, elapsed).is_playing(), "{elapsed}");
        }
    }

    #[test]
    fn once_holds_exactly_frame_n_of_an_n_frame_asset() {
        let hold = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        assert_eq!(position_millis(at(hold, 500)), 500, "still playing");

        let completed = at(hold, 1_000);
        assert_eq!(
            completed,
            Presentation::Completed {
                end_state: OnceEndState::Hold,
                position: Duration::from_millis(900),
            },
            "frame 10 of 10, never frame 9 and never a frame that does not exist"
        );
        assert!(!completed.is_playing());
    }

    #[test]
    fn a_completed_once_stays_completed() {
        let hold = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        for elapsed in [1_000, 1_001, 10_000, 1_000_000] {
            assert!(
                matches!(at(hold, elapsed), Presentation::Completed { .. }),
                "at {elapsed} it must not have looped"
            );
        }
    }

    #[test]
    fn every_once_end_state_completes_at_the_same_moment() {
        for end_state in [
            OnceEndState::Hold,
            OnceEndState::Black,
            OnceEndState::Transparent,
        ] {
            let mode = PlayMode::Once { end_state };
            assert!(
                at(mode, 999).is_playing(),
                "{end_state:?} still playing at 999ms"
            );
            assert_eq!(
                at(mode, 1_000),
                Presentation::Completed {
                    end_state,
                    position: Duration::from_millis(900)
                },
                "{end_state:?}"
            );
        }
    }

    #[test]
    fn a_finished_pass_rests_on_the_last_frame_forward_and_the_first_frame_backward() {
        let timing = ten_frames();
        assert_eq!(
            once_end_position(&timing, false),
            Duration::from_millis(900)
        );
        assert_eq!(once_end_position(&timing, true), Duration::ZERO);
    }

    #[test]
    fn a_reverse_once_runs_backward_and_completes_on_the_first_frame() {
        let timing = ten_frames();
        let mode = PlayMode::ReverseOnce {
            end_state: OnceEndState::Hold,
        };

        assert_eq!(
            present(mode, &timing, 1.0, Duration::from_millis(0)).position(),
            Some(Duration::from_millis(900)),
            "it starts on the last frame"
        );
        assert_eq!(
            present(mode, &timing, 1.0, Duration::from_millis(250)).position(),
            Some(Duration::from_millis(750)),
            "and runs backward"
        );
        assert_eq!(
            present(mode, &timing, 1.0, Duration::from_millis(1_000)),
            Presentation::Completed {
                end_state: OnceEndState::Hold,
                position: Duration::ZERO
            },
            "resting on the first frame, not looping"
        );
    }

    #[test]
    fn a_reverse_once_never_becomes_a_reverse_loop() {
        let timing = ten_frames();
        let mode = PlayMode::ReverseOnceSynced {
            end_state: OnceEndState::Transparent,
        };
        for elapsed in [1_000, 1_500, 10_000] {
            assert!(
                matches!(
                    present(mode, &timing, 1.0, Duration::from_millis(elapsed)),
                    Presentation::Completed { .. }
                ),
                "at {elapsed}ms"
            );
        }
    }

    #[test]
    fn stop_and_pause_stay_distinguishable() {
        assert_eq!(at(PlayMode::Stop, 500), Presentation::Stopped);
        assert_eq!(at(PlayMode::Stop, 500).position(), Some(Duration::ZERO));
        assert!(!at(PlayMode::Stop, 500).is_playing());

        assert_eq!(at(PlayMode::Pause, 500), Presentation::Paused);
        assert_eq!(
            at(PlayMode::Pause, 500).position(),
            None,
            "pause holds the current frame"
        );
        assert!(!at(PlayMode::Pause, 500).is_playing());
    }

    #[test]
    fn a_completed_once_is_not_a_pause_and_not_a_stop() {
        let completed = at(
            PlayMode::Once {
                end_state: OnceEndState::Hold,
            },
            2_000,
        );
        assert_ne!(completed, Presentation::Paused);
        assert_ne!(completed, Presentation::Stopped);
    }

    #[test]
    fn the_rate_scales_how_far_playback_has_travelled() {
        let timing = ten_frames();
        let half = present(PlayMode::Loop, &timing, 0.5, Duration::from_millis(1_000));
        assert_eq!(half.position(), Some(Duration::from_millis(500)));

        let double = present(PlayMode::Loop, &timing, 2.0, Duration::from_millis(250));
        assert_eq!(double.position(), Some(Duration::from_millis(500)));
    }

    #[test]
    fn a_rate_of_zero_holds_the_first_frame_rather_than_dividing_by_nothing() {
        let timing = ten_frames();
        let held = present(PlayMode::Loop, &timing, 0.0, Duration::from_secs(10));
        assert_eq!(held.position(), Some(Duration::ZERO));
        assert!(held.position().unwrap().as_secs_f64().is_finite());
    }

    #[test]
    fn a_faster_rate_completes_a_once_sooner() {
        let timing = ten_frames();
        let mode = PlayMode::Once {
            end_state: OnceEndState::Hold,
        };
        assert!(present(mode, &timing, 2.0, Duration::from_millis(499)).is_playing());
        assert!(!present(mode, &timing, 2.0, Duration::from_millis(500)).is_playing());
    }

    #[test]
    fn no_mode_ever_presents_at_or_past_the_duration() {
        let timing = ten_frames();
        for mode in PlayMode::ALL {
            for elapsed in 0..3_000u64 {
                let presentation = present(mode, &timing, 1.0, Duration::from_millis(elapsed));
                if let Some(position) = presentation.position() {
                    assert!(
                        position < timing.duration,
                        "{} presented {position:?} at {elapsed}ms, which is not a real frame",
                        mode.label()
                    );
                }
            }
        }
    }

    #[test]
    fn a_still_image_shows_regardless_of_mode_or_elapsed_time() {
        let still = MediaTiming::still();
        assert!(still.is_still());
        for mode in [
            PlayMode::Loop,
            PlayMode::Bounce,
            PlayMode::Once {
                end_state: OnceEndState::Hold,
            },
        ] {
            let presentation = present(mode, &still, 1.0, Duration::from_secs(3_600));
            assert_eq!(
                presentation,
                Presentation::Frame {
                    position: Duration::ZERO
                },
                "{mode:?}"
            );
        }
    }

    #[test]
    fn a_single_frame_asset_has_a_last_frame_at_zero() {
        let timing = MediaTiming::from_frames(1, 25.0);
        assert_eq!(timing.last_frame, Duration::ZERO);
        assert_eq!(timing.duration, Duration::from_millis(40));

        let completed = present(
            PlayMode::Once {
                end_state: OnceEndState::Hold,
            },
            &timing,
            1.0,
            Duration::from_millis(40),
        );
        assert_eq!(completed.position(), Some(Duration::ZERO));
    }

    #[test]
    fn intrinsic_bpm_is_attached_metadata_and_never_inferred() {
        let plain = ten_frames();
        assert_eq!(
            plain.intrinsic_bpm, None,
            "a duration says nothing about tempo"
        );
        assert_eq!(plain.with_intrinsic_bpm(128.0).intrinsic_bpm, Some(128.0));
    }
}
