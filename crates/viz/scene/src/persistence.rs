//! Persistence of vision: how long a light goes on being seen after it has gone dark.
//!
//! A rendered frame is an instant, and an eye is not. A strobe firing at 15 Hz and a laser
//! painting a figure at 30 000 points per second both put light in front of an observer for a
//! fraction of the time a display can show, and both are seen anyway — the strobe as a bright
//! flash that lingers, the laser as a solid line rather than a travelling dot. Sample either one
//! at the instant a frame happens to fall and the result is neither: a strobe that flickers at the
//! beat frequency between its rate and the refresh, and a laser made of dots.
//!
//! So a displayed level is not the level the desk is asking for right now. It is the brightest
//! thing the observer still has, which decays towards the current level over a set time. That one
//! rule covers a strobe, a laser, a bumped flash and a fast chase, and it is why this lives beside
//! the atmosphere rather than inside any one fixture's decode.
//!
//! Like the haze amount, this belongs to the renderer and never to the show: it describes the
//! observer and the camera, not the rig, and an operator who wants to see individual strobe
//! flashes must be able to turn it down without editing the show everyone else is running.

/// How long a light takes to fall from full to black, in seconds.
///
/// A tenth of a second is the figure the eye is usually credited with, and it is what makes a
/// strobe read as a strobe rather than as a stutter.
pub const DEFAULT_DECAY_SECONDS: f32 = 0.1;

/// How sharply the tail falls once it is below [`DEFAULT_THRESHOLD`]. `2.0` is the square falloff
/// that keeps a bright flash bright for most of its life and then drops it quickly, instead of
/// leaving every fixture in a permanent haze of its own last cue.
pub const DEFAULT_FALLOFF: f32 = 2.0;

/// The level below which the falloff exponent takes over. Above it the tail is linear, which is
/// what keeps the first, brightest part of a flash honest; below it the exponent bends the
/// remainder down to nothing.
pub const DEFAULT_THRESHOLD: f32 = 0.5;

/// Renderer-local persistence setting, persisted independently from the show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersistencePreference {
    /// Seconds from full brightness to black. `0` disables persistence entirely, and every frame
    /// then shows exactly the level the desk is sending at that instant.
    pub decay_seconds: f32,
    /// Falloff exponent applied below [`Self::threshold`]. `1.0` is a straight line; higher values
    /// shorten the visible tail without shortening the bright part of it.
    pub falloff: f32,
    /// Level below which the falloff exponent applies, `0..=1`.
    pub threshold: f32,
}

impl Default for PersistencePreference {
    fn default() -> Self {
        Self {
            decay_seconds: DEFAULT_DECAY_SECONDS,
            falloff: DEFAULT_FALLOFF,
            threshold: DEFAULT_THRESHOLD,
        }
    }
}

impl PersistencePreference {
    /// Whether any decay happens at all. A zero time is the operator asking for the raw sampled
    /// value, and the whole mechanism is then skipped rather than approximated.
    pub fn is_active(&self) -> bool {
        self.decay_seconds > 0.0
    }

    /// The level still visible from `held`, `elapsed` seconds after it was seen.
    ///
    /// The decay is defined against full brightness rather than against whatever the level
    /// happened to be, so a half-brightness flash fades in half the time a full one does. That is
    /// both what the eye does and what an operator expects: dimmer things disappear sooner.
    pub fn decayed(&self, held: f32, elapsed: f32) -> f32 {
        if !self.is_active() || held <= 0.0 {
            return 0.0;
        }
        let fallen = (elapsed.max(0.0) / self.decay_seconds).clamp(0.0, 1.0);
        let remaining = (held - fallen).max(0.0);
        let threshold = self.threshold.clamp(0.0, 1.0);
        if remaining >= threshold || threshold <= 0.0 {
            return remaining;
        }
        // Below the threshold the straight line is bent by the exponent, normalised so the two
        // pieces meet exactly at the threshold and the tail has no step in it.
        let within = remaining / threshold;
        threshold * within.powf(self.falloff.max(1.0))
    }

    /// What to display now, given what was displayed before and what the desk is asking for.
    ///
    /// A rise is instant: light that has just been turned on is seen at once. Only the fall is
    /// slowed, which is the asymmetry that makes this persistence rather than a smoothing filter.
    pub fn hold(&self, previous: f32, current: f32, elapsed: f32) -> f32 {
        if !self.is_active() {
            return current;
        }
        current.max(self.decayed(previous, elapsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The headline behaviour the setting exists for: full brightness reaches black in the
    /// configured time, and not before.
    #[test]
    fn a_light_cut_to_black_takes_the_configured_time_to_disappear() {
        let preference = PersistencePreference::default();
        assert!(
            preference.hold(1.0, 0.0, 0.05) > 0.0,
            "gone at half the time"
        );
        assert_eq!(preference.hold(1.0, 0.0, DEFAULT_DECAY_SECONDS), 0.0);
        assert_eq!(preference.hold(1.0, 0.0, 10.0), 0.0);
    }

    /// Persistence must never dim anything. A light coming up is seen immediately, and a light
    /// held above its decayed tail is shown at its real level.
    #[test]
    fn a_rise_is_instant_and_a_held_level_is_never_reduced() {
        let preference = PersistencePreference::default();
        assert_eq!(preference.hold(0.0, 1.0, 0.001), 1.0);
        assert_eq!(preference.hold(0.2, 0.8, 0.001), 0.8);
        assert_eq!(preference.hold(1.0, 1.0, 1.0), 1.0);
    }

    /// The square falloff has to actually shorten the tail: below the threshold the exponent must
    /// leave less light than the straight line it replaces.
    #[test]
    fn the_falloff_exponent_shortens_the_tail_below_the_threshold() {
        let squared = PersistencePreference::default();
        let linear = PersistencePreference {
            falloff: 1.0,
            ..PersistencePreference::default()
        };
        // 0.8 of the decay time has passed, so the straight line is at 0.2 — below the threshold.
        let elapsed = 0.08;
        assert!(linear.decayed(1.0, elapsed) > squared.decayed(1.0, elapsed));
        // Above the threshold the two must agree exactly, or the tail would have a step in it.
        assert_eq!(linear.decayed(1.0, 0.02), squared.decayed(1.0, 0.02));
    }

    /// A dimmer flash is a shorter one. Decaying against full brightness rather than against the
    /// held level is what produces that.
    #[test]
    fn a_dimmer_flash_disappears_sooner_than_a_bright_one() {
        let preference = PersistencePreference::default();
        let half = DEFAULT_DECAY_SECONDS * 0.5;
        assert_eq!(preference.decayed(0.5, half), 0.0);
        assert!(preference.decayed(1.0, half) > 0.0);
    }

    /// Turning the setting off has to give back the raw sampled value, not a very fast decay.
    #[test]
    fn a_zero_decay_time_shows_exactly_what_the_desk_is_sending() {
        let off = PersistencePreference {
            decay_seconds: 0.0,
            ..PersistencePreference::default()
        };
        assert!(!off.is_active());
        assert_eq!(off.hold(1.0, 0.0, 0.0), 0.0);
        assert_eq!(off.hold(1.0, 0.3, 0.0), 0.3);
    }
}
