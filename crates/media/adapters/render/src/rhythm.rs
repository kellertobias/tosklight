//! A visualizer's memory of the room between frames: smoothed levels, the beats that landed, and
//! clocks that audio can speed up without the picture jumping.
//!
//! A shader sees one frame. Anything that has to remember -- a streak a beat sent three seconds
//! ago, a level eased over half a second, a phase the energy pushed forward -- is kept here, one
//! per layer, and handed to the shaders in the analysis texture.

/// How many landed beats a shader can look back over. Anything a beat sent that is still on
/// screen after this many further beats simply ends.
pub(crate) const BEAT_HISTORY: usize = 16;

/// Reported as the age of a beat that never landed: long past anything a visualizer still draws.
pub(crate) const NEVER: f32 = 1.0e6;

/// The shortest gap between two beats, as a share of the beat at the current tempo. Anything
/// closer is the same beat heard twice.
const SAME_BEAT: f32 = 0.5;

/// The shortest gap between two beats while no tempo is known: 300 BPM.
const SAME_BEAT_SECONDS: f32 = 0.2;

/// What the shaders are given from this memory.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rhythm {
    /// Bass, mid, treble, energy and peak, eased at the rate `smoothing` asks for.
    pub smoothed: [f32; 5],
    /// `1.0` when a beat lands, easing back to zero.
    pub pulse: f32,
    /// The count of landed beats, eased: it steps up by one on every beat, smoothly.
    pub steps: f32,
    /// Beats landed since this layer started showing a visualizer.
    pub count: f32,
    /// Seconds at `speed`, sped up by the smoothed energy. Only ever goes forward, so a phase
    /// read from it glides when the music gets louder instead of jumping.
    pub flow: f32,
    /// Seconds at `speed`. Moving the speed fader changes the rate, never the position.
    pub clock: f32,
    /// Seconds since each of the most recent beats, newest first; [`NEVER`] where none landed.
    pub ages: [f32; BEAT_HISTORY],
}

/// What one frame tells the memory.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Heard {
    pub seconds: f32,
    /// Bass, mid, treble, energy and peak, as the visualizer hears them.
    pub levels: [f32; 5],
    /// The detector's beat flash: `1.0` on the hop a beat lands, decaying after.
    pub beat: f32,
    pub bpm: f32,
    pub speed: f32,
    pub smoothing: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct RhythmMemory {
    seconds: Option<f32>,
    smoothed: [f32; 5],
    last_flash: f32,
    /// When each recent beat landed, newest first.
    landed: [Option<f32>; BEAT_HISTORY],
    count: u64,
    pulse: f32,
    steps: f32,
    flow: f32,
    clock: f32,
}

impl RhythmMemory {
    pub(crate) fn new() -> Self {
        Self {
            seconds: None,
            smoothed: [0.0; 5],
            last_flash: 0.0,
            landed: [None; BEAT_HISTORY],
            count: 0,
            pulse: 0.0,
            steps: 0.0,
            flow: 0.0,
            clock: 0.0,
        }
    }

    pub(crate) fn advance(&mut self, heard: Heard) -> Rhythm {
        let first = self.seconds.is_none();
        // A clock that jumped or a stall moves by at most a moderate step, so an uneven frame
        // neither empties what is eased nor throws a phase a long way at once.
        let elapsed = match self.seconds {
            Some(last) if heard.seconds > last => (heard.seconds - last).min(0.25),
            _ => 0.0,
        };
        self.seconds = Some(heard.seconds);
        let smoothing = heard.smoothing.clamp(0.0, 1.0);

        // Up to a second and a half to follow a change at full smoothing, none at zero.
        let follow = ease(elapsed, smoothing * smoothing * 1.5);
        for (kept, level) in self.smoothed.iter_mut().zip(heard.levels) {
            let level = if level.is_finite() {
                level.max(0.0)
            } else {
                0.0
            };
            *kept = if first {
                level
            } else {
                *kept + (level - *kept) * follow
            };
        }

        if self.landed_now(heard) {
            self.landed.rotate_right(1);
            self.landed[0] = Some(heard.seconds);
            self.count += 1;
            self.pulse = 1.0;
        } else {
            self.pulse *= (-elapsed / (0.1 + smoothing * 0.4)).exp();
        }
        self.steps += (self.count as f32 - self.steps) * ease(elapsed, 0.04 + smoothing * 0.3);

        let speed = heard.speed.max(0.0);
        self.clock += elapsed * speed;
        self.flow += elapsed * speed * (0.4 + 0.9 * self.smoothed[3].min(1.5));

        let mut ages = [NEVER; BEAT_HISTORY];
        for (age, landed) in ages.iter_mut().zip(self.landed) {
            if let Some(landed) = landed {
                *age = (heard.seconds - landed).max(0.0);
            }
        }
        Rhythm {
            smoothed: self.smoothed,
            pulse: self.pulse,
            steps: self.steps,
            count: self.count as f32,
            flow: self.flow,
            clock: self.clock,
            ages,
        }
    }

    /// Whether a beat landed on this frame.
    ///
    /// The flash is `1.0` on the hop a beat lands and decays by several percent a hop, and a
    /// frame only sees whichever hop was published last. So a landing is the flash *rising*, not
    /// the flash being high: a beat that landed a hop before publication still counts, and a
    /// flash still fading across several frames never counts twice.
    fn landed_now(&mut self, heard: Heard) -> bool {
        let flash = if heard.beat.is_finite() {
            heard.beat
        } else {
            0.0
        };
        let rose = flash >= 0.5 && flash > self.last_flash + 0.02;
        self.last_flash = flash;
        if !rose {
            return false;
        }
        let gap = if heard.bpm > 0.0 {
            SAME_BEAT * 60.0 / heard.bpm
        } else {
            SAME_BEAT_SECONDS
        };
        !matches!(
            self.landed[0],
            Some(last) if heard.seconds >= last && heard.seconds - last < gap
        )
    }
}

/// The share of the way to a target covered in `elapsed` when the time constant is `constant`.
fn ease(elapsed: f32, constant: f32) -> f32 {
    if constant <= 1.0e-4 {
        1.0
    } else {
        1.0 - (-elapsed / constant).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(seconds: f32, beat: f32) -> Heard {
        Heard {
            seconds,
            levels: [0.0; 5],
            beat,
            bpm: 120.0,
            speed: 1.0,
            smoothing: 0.5,
        }
    }

    /// Plays a steady beat at 120 BPM, a frame at a time, publishing the flash the detector would.
    fn play(memory: &mut RhythmMemory, seconds: f32, flash_on_landing: f32) -> Rhythm {
        let frame = 1.0 / 60.0;
        let mut now = 0.0;
        let mut rhythm = memory.advance(heard(now, 0.0));
        while now < seconds {
            now += frame;
            let since = now % 0.5;
            // Half-life of a tenth of a second from the moment it landed.
            let flash = if since < 0.45 {
                flash_on_landing * 0.5f32.powf(since / 0.1)
            } else {
                0.0
            };
            rhythm = memory.advance(heard(now, flash));
        }
        rhythm
    }

    #[test]
    fn a_beat_published_after_it_began_fading_still_lands_once() {
        // A window holds two hops, so a beat that landed on the first is published already down
        // to about 0.93. It must count, and exactly once.
        let mut memory = RhythmMemory::new();
        let rhythm = play(&mut memory, 4.0, 0.93);
        assert_eq!(
            rhythm.count, 9.0,
            "one landing per beat from zero to four seconds"
        );
    }

    #[test]
    fn the_same_beat_heard_twice_lands_once() {
        let mut memory = RhythmMemory::new();
        memory.advance(heard(0.0, 0.0));
        memory.advance(heard(1.0, 1.0));
        memory.advance(heard(1.02, 0.6));
        // A second rise 50 ms later is the same beat, at any tempo a desk runs.
        let rhythm = memory.advance(heard(1.05, 1.0));
        assert_eq!(rhythm.count, 1.0);
        let rhythm = memory.advance(heard(1.1, 0.5));
        let rhythm2 = memory.advance(heard(1.5, 1.0));
        assert_eq!((rhythm.count, rhythm2.count), (1.0, 2.0));
    }

    #[test]
    fn ages_run_newest_first_and_report_never_where_none_landed() {
        let mut memory = RhythmMemory::new();
        memory.advance(heard(0.0, 0.0));
        memory.advance(heard(1.0, 1.0));
        memory.advance(heard(1.2, 0.3));
        memory.advance(heard(1.5, 1.0));
        let rhythm = memory.advance(heard(2.0, 0.1));
        assert!((rhythm.ages[0] - 0.5).abs() < 1e-5);
        assert!((rhythm.ages[1] - 1.0).abs() < 1e-5);
        assert_eq!(rhythm.ages[2], NEVER);
    }

    #[test]
    fn steps_climb_one_per_beat_without_jumping() {
        let mut memory = RhythmMemory::new();
        memory.advance(heard(0.0, 0.0));
        let landed = memory.advance(heard(1.0 / 60.0, 1.0));
        assert!(landed.steps > 0.0 && landed.steps < 0.6, "{}", landed.steps);
        let mut now = 1.0 / 60.0;
        let mut rhythm = landed;
        for _ in 0..60 {
            now += 1.0 / 60.0;
            rhythm = memory.advance(heard(now, 0.0));
        }
        assert!((rhythm.steps - 1.0).abs() < 0.01, "{}", rhythm.steps);
    }

    #[test]
    fn louder_music_speeds_the_flow_up_without_moving_it_back() {
        let mut quiet = RhythmMemory::new();
        let mut loud = RhythmMemory::new();
        let mut now = 0.0;
        let (mut calm, mut driven) = (
            quiet.advance(heard(0.0, 0.0)),
            loud.advance(heard(0.0, 0.0)),
        );
        for _ in 0..60 {
            now += 1.0 / 60.0;
            calm = quiet.advance(heard(now, 0.0));
            let before = driven.flow;
            driven = loud.advance(Heard {
                levels: [1.0; 5],
                ..heard(now, 0.0)
            });
            assert!(driven.flow >= before);
        }
        assert!(driven.flow > calm.flow * 1.5);
        assert!((calm.clock - 1.0).abs() < 0.02);
    }

    #[test]
    fn smoothing_eases_a_level_and_zero_follows_it_at_once() {
        let mut raw = RhythmMemory::new();
        let mut eased = RhythmMemory::new();
        raw.advance(Heard {
            smoothing: 0.0,
            ..heard(0.0, 0.0)
        });
        eased.advance(Heard {
            smoothing: 1.0,
            ..heard(0.0, 0.0)
        });
        let loud = |memory: &mut RhythmMemory, smoothing| {
            memory.advance(Heard {
                levels: [1.0; 5],
                smoothing,
                ..heard(1.0 / 60.0, 0.0)
            })
        };
        assert_eq!(loud(&mut raw, 0.0).smoothed[0], 1.0);
        assert!(loud(&mut eased, 1.0).smoothed[0] < 0.05);
    }
}
