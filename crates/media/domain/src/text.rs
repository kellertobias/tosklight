//! Text sources.
//!
//! Folders `200–249` address fifty banks of text entries. An entry is static text, a clock, or a
//! countdown; the countdown is the one with a lifecycle, because it has to start, freeze, resume,
//! and reset in step with a layer's transport.
//!
//! Nothing here reads a clock. Wall-clock time and the layer's transport are both passed in, which
//! is what makes the whole lifecycle testable without waiting.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::playback::PlayMode;

/// How a text entry produces its string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TextKind {
    /// Fixed text.
    Static { text: String },
    /// The current time of day, in the entry's format.
    Clock,
    /// Counts down from a duration, starting when the layer becomes visible.
    CountdownFromDuration { duration: Duration },
    /// Counts down to a fixed moment, expressed as milliseconds since the Unix epoch so the domain
    /// needs no calendar library.
    CountdownToTarget { target_unix_millis: i64 },
}

/// Operator-visible formatting, stored beside the kind so old unit variants remain compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFormat {
    #[serde(default)]
    pub clock: ClockFormat,
    #[serde(default)]
    pub countdown: CountdownFormat,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self {
            clock: ClockFormat::default(),
            countdown: CountdownFormat::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockFormat {
    #[serde(default)]
    pub pattern: ClockPattern,
    #[serde(default = "default_separator")]
    pub separator: String,
    /// A deterministic fixed offset from UTC for this clock alone. `None` follows the server's
    /// configured offset, which is what a new clock does; an explicit value overrides it.
    #[serde(default)]
    pub utc_offset_minutes: Option<i16>,
}

impl Default for ClockFormat {
    fn default() -> Self {
        Self {
            pattern: ClockPattern::default(),
            separator: default_separator(),
            utc_offset_minutes: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClockPattern {
    HoursMinutes24,
    #[default]
    HoursMinutesSeconds24,
    HoursMinutes12,
    HoursMinutesSeconds12,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountdownFormat {
    #[serde(default)]
    pub pattern: CountdownPattern,
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default)]
    pub after_zero: CountdownAfterZero,
    /// When true, minutes in a minutes/seconds display wrap at 60 instead of staying total.
    #[serde(default)]
    pub rollover: bool,
}

impl Default for CountdownFormat {
    fn default() -> Self {
        Self {
            pattern: CountdownPattern::default(),
            separator: default_separator(),
            after_zero: CountdownAfterZero::default(),
            rollover: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CountdownPattern {
    Seconds,
    Minutes,
    MinutesSeconds,
    #[default]
    HoursMinutesSeconds,
    CompactHoursMinutesSeconds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CountdownAfterZero {
    #[default]
    Hold,
    Negative,
    CountUp,
}

fn default_separator() -> String {
    ":".to_owned()
}

/// One addressable text entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEntry {
    pub kind: TextKind,
    /// A disabled entry produces nothing, which is how an operator parks one without deleting it.
    pub enabled: bool,
    /// Absent in old configuration files; the default reproduces their rendered meaning.
    #[serde(default)]
    pub format: TextFormat,
}

impl TextEntry {
    pub fn new(kind: TextKind) -> Self {
        Self {
            kind,
            enabled: true,
            format: TextFormat::default(),
        }
    }
}

/// Whether the layer showing a countdown is visible and moving.
///
/// This is what the countdown's lifecycle keys on, and it comes from the layer rather than from
/// anything text-specific: the same entry behaves differently on a stopped layer and a running one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    /// The layer is drawing at all.
    pub visible: bool,
    pub play_mode: PlayMode,
}

impl Visibility {
    pub const fn hidden() -> Self {
        Self {
            visible: false,
            play_mode: PlayMode::Stop,
        }
    }

    pub const fn playing(&self) -> bool {
        self.visible && self.play_mode.is_transport_running()
    }

    pub const fn paused(&self) -> bool {
        self.visible && matches!(self.play_mode, PlayMode::Pause)
    }
}

/// Where a countdown is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    /// Not started, or reset back to the beginning.
    #[default]
    Idle,
    Running {
        /// The moment the current run began.
        anchor: u64,
        /// How much had already elapsed when it began, so a resume continues rather than restarts.
        carried: Duration,
    },
    Frozen {
        elapsed: Duration,
    },
}

/// A countdown's live state.
///
/// The lifecycle the contract specifies: hidden → visible in any playing mode starts it; stop →
/// running starts from the configured duration; pause → running resumes; running → pause freezes;
/// running or pause → stop resets; visible → hidden resets; and reaching zero holds at zero.
#[derive(Debug, Clone, Default)]
pub struct Countdown {
    phase: Phase,
}

impl Countdown {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances the lifecycle. `now_millis` is a monotonic stamp supplied by the caller.
    pub fn observe(&mut self, visibility: Visibility, now_millis: u64) {
        // Leaving the screen resets, whatever the transport was doing.
        if !visibility.visible {
            self.phase = Phase::Idle;
            return;
        }
        // Stop resets and stays reset, which is what distinguishes it from pause.
        if matches!(visibility.play_mode, PlayMode::Stop) {
            self.phase = Phase::Idle;
            return;
        }

        if visibility.paused() {
            if let Phase::Running { anchor, carried } = self.phase {
                self.phase = Phase::Frozen {
                    elapsed: carried + Duration::from_millis(now_millis.saturating_sub(anchor)),
                };
            }
            return;
        }

        if visibility.playing() {
            match self.phase {
                // Becoming visible in a playing mode starts it; so does resuming from pause,
                // carrying whatever had already elapsed.
                Phase::Idle => {
                    self.phase = Phase::Running {
                        anchor: now_millis,
                        carried: Duration::ZERO,
                    };
                }
                Phase::Frozen { elapsed } => {
                    self.phase = Phase::Running {
                        anchor: now_millis,
                        carried: elapsed,
                    };
                }
                Phase::Running { .. } => {}
            }
        }
    }

    /// How long the countdown has been running.
    pub fn elapsed(&self, now_millis: u64) -> Duration {
        match self.phase {
            Phase::Idle => Duration::ZERO,
            Phase::Frozen { elapsed } => elapsed,
            Phase::Running { anchor, carried } => {
                carried + Duration::from_millis(now_millis.saturating_sub(anchor))
            }
        }
    }

    /// What remains of `total`, holding at zero once it is reached.
    pub fn remaining(&self, total: Duration, now_millis: u64) -> Duration {
        total.saturating_sub(self.elapsed(now_millis))
    }

    pub const fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Running { .. })
    }
}

/// Renders an entry's text.
///
/// `now_unix_millis` is wall-clock time, which only the clock and the target countdown consult;
/// `now_millis` is the monotonic stamp the countdown lifecycle runs on. Keeping them separate means
/// a clock change cannot make a running countdown jump. `server_utc_offset_minutes` is the media
/// server's configured offset, which every clock follows unless it carries one of its own.
pub fn render(
    entry: &TextEntry,
    countdown: &Countdown,
    now_unix_millis: i64,
    now_millis: u64,
    server_utc_offset_minutes: i16,
) -> Option<String> {
    if !entry.enabled {
        return None;
    }
    Some(match &entry.kind {
        TextKind::Static { text } => text.clone(),
        TextKind::Clock => format_clock(
            now_unix_millis,
            &entry.format.clock,
            server_utc_offset_minutes,
        ),
        TextKind::CountdownFromDuration { duration } => {
            let remaining =
                duration.as_millis() as i128 - countdown.elapsed(now_millis).as_millis() as i128;
            format_countdown(remaining, &entry.format.countdown)
        }
        TextKind::CountdownToTarget { target_unix_millis } => {
            let remaining = i128::from(*target_unix_millis) - i128::from(now_unix_millis);
            format_countdown(remaining, &entry.format.countdown)
        }
    })
}

/// `HH:MM:SS` of the day, from a Unix millisecond stamp. No calendar library needed for a clock.
fn format_clock(unix_millis: i64, format: &ClockFormat, server_offset_minutes: i16) -> String {
    let offset = format.utc_offset_minutes.unwrap_or(server_offset_minutes);
    let shifted = unix_millis.saturating_add(i64::from(offset) * 60_000);
    let seconds_of_day = shifted.div_euclid(1_000).rem_euclid(86_400);
    let hours24 = seconds_of_day / 3_600;
    let minutes = (seconds_of_day % 3_600) / 60;
    let seconds = seconds_of_day % 60;
    let hours12 = match hours24 % 12 {
        0 => 12,
        hour => hour,
    };
    let separator = &format.separator;
    match format.pattern {
        ClockPattern::HoursMinutes24 => format!("{hours24:02}{separator}{minutes:02}"),
        ClockPattern::HoursMinutesSeconds24 => {
            format!("{hours24:02}{separator}{minutes:02}{separator}{seconds:02}")
        }
        ClockPattern::HoursMinutes12 => format!("{hours12:02}{separator}{minutes:02}"),
        ClockPattern::HoursMinutesSeconds12 => {
            format!("{hours12:02}{separator}{minutes:02}{separator}{seconds:02}")
        }
    }
}

/// Formats signed remaining milliseconds, rounding away from zero until a whole second passes.
fn format_countdown(remaining_millis: i128, format: &CountdownFormat) -> String {
    let (negative, millis) = if remaining_millis < 0 {
        match format.after_zero {
            CountdownAfterZero::Hold => (false, 0),
            CountdownAfterZero::Negative => (true, remaining_millis.unsigned_abs()),
            CountdownAfterZero::CountUp => (false, remaining_millis.unsigned_abs()),
        }
    } else {
        (false, remaining_millis as u128)
    };
    let seconds = millis.div_ceil(1_000) as u64;
    let sign = if negative { "-" } else { "" };
    let separator = &format.separator;
    match format.pattern {
        CountdownPattern::Seconds => format!("{sign}{seconds:02}"),
        CountdownPattern::Minutes => format!("{sign}{:02}", seconds.div_ceil(60)),
        CountdownPattern::MinutesSeconds => {
            let minutes = if format.rollover {
                (seconds / 60) % 60
            } else {
                seconds / 60
            };
            format!("{sign}{minutes:02}{separator}{:02}", seconds % 60)
        }
        CountdownPattern::HoursMinutesSeconds => format!(
            "{sign}{:02}{separator}{:02}{separator}{:02}",
            seconds / 3_600,
            (seconds % 3_600) / 60,
            seconds % 60
        ),
        CountdownPattern::CompactHoursMinutesSeconds => format!(
            "{sign}{}{separator}{:02}{separator}{:02}",
            seconds / 3_600,
            (seconds % 3_600) / 60,
            seconds % 60
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    fn visible(mode: PlayMode) -> Visibility {
        Visibility {
            visible: true,
            play_mode: mode,
        }
    }

    #[test]
    fn a_disabled_entry_produces_nothing() {
        let mut entry = TextEntry::new(TextKind::Static {
            text: "Hello".into(),
        });
        entry.enabled = false;
        assert_eq!(render(&entry, &Countdown::new(), 0, 0, 0), None);
    }

    #[test]
    fn static_text_is_itself() {
        let entry = TextEntry::new(TextKind::Static {
            text: "Doors 19:30".into(),
        });
        assert_eq!(
            render(&entry, &Countdown::new(), 0, 0, 0).unwrap(),
            "Doors 19:30"
        );
    }

    #[test]
    fn a_clock_reads_the_time_of_day() {
        let entry = TextEntry::new(TextKind::Clock);
        // 13:45:30 UTC on any day.
        let stamp = (13 * 3_600 + 45 * 60 + 30) * 1_000;
        assert_eq!(
            render(&entry, &Countdown::new(), stamp, 0, 0).unwrap(),
            "13:45:30"
        );
        assert_eq!(
            render(&entry, &Countdown::new(), stamp + 86_400_000, 0, 0).unwrap(),
            "13:45:30"
        );
    }

    #[test]
    fn a_clock_before_the_epoch_does_not_produce_a_negative_time() {
        let entry = TextEntry::new(TextKind::Clock);
        let rendered = render(&entry, &Countdown::new(), -1_000, 0, 0).unwrap();
        assert_eq!(rendered, "23:59:59");
    }

    #[test]
    fn clocks_support_original_12_24_hour_formats_offsets_and_separators() {
        let mut entry = TextEntry::new(TextKind::Clock);
        entry.format.clock = ClockFormat {
            pattern: ClockPattern::HoursMinutes12,
            separator: ".".to_owned(),
            utc_offset_minutes: Some(60),
        };
        let stamp = (13 * 3_600 + 45 * 60 + 30) * 1_000;
        assert_eq!(
            render(&entry, &Countdown::new(), stamp, 0, 0).unwrap(),
            "02.45"
        );
    }

    #[test]
    fn a_clock_without_its_own_offset_follows_the_server_offset() {
        let entry = TextEntry::new(TextKind::Clock);
        let stamp = (13 * 3_600 + 45 * 60 + 30) * 1_000;
        assert_eq!(
            render(&entry, &Countdown::new(), stamp, 0, 0).unwrap(),
            "13:45:30"
        );
        assert_eq!(
            render(&entry, &Countdown::new(), stamp, 0, 120).unwrap(),
            "15:45:30"
        );
        // An explicit offset stays the operator's decision whatever the server is set to.
        let mut fixed = TextEntry::new(TextKind::Clock);
        fixed.format.clock.utc_offset_minutes = Some(-60);
        assert_eq!(
            render(&fixed, &Countdown::new(), stamp, 0, 120).unwrap(),
            "12:45:30"
        );
    }

    #[test]
    fn becoming_visible_in_a_playing_mode_starts_the_countdown() {
        let mut countdown = Countdown::new();
        countdown.observe(Visibility::hidden(), 0);
        assert!(!countdown.is_running());

        countdown.observe(visible(PlayMode::Loop), 1_000);
        assert!(countdown.is_running());
        assert_eq!(countdown.remaining(MINUTE, 11_000), Duration::from_secs(50));
    }

    #[test]
    fn stop_resets_it_and_a_run_starts_from_the_configured_duration() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        countdown.observe(visible(PlayMode::Loop), 10_000);
        assert_eq!(countdown.remaining(MINUTE, 10_000), Duration::from_secs(50));

        countdown.observe(visible(PlayMode::Stop), 10_000);
        assert_eq!(countdown.remaining(MINUTE, 10_000), MINUTE, "reset");

        countdown.observe(visible(PlayMode::Loop), 10_000);
        assert_eq!(
            countdown.remaining(MINUTE, 20_000),
            Duration::from_secs(50),
            "and starts again from the top"
        );
    }

    #[test]
    fn pause_freezes_and_resuming_continues_rather_than_restarting() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        countdown.observe(visible(PlayMode::Pause), 20_000);
        assert!(!countdown.is_running());

        // Frozen: time passing changes nothing.
        assert_eq!(countdown.remaining(MINUTE, 20_000), Duration::from_secs(40));
        assert_eq!(countdown.remaining(MINUTE, 50_000), Duration::from_secs(40));

        countdown.observe(visible(PlayMode::Loop), 50_000);
        assert_eq!(
            countdown.remaining(MINUTE, 60_000),
            Duration::from_secs(30),
            "it carried the twenty seconds it had already spent"
        );
    }

    #[test]
    fn going_hidden_resets_it() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        countdown.observe(Visibility::hidden(), 30_000);
        assert_eq!(countdown.remaining(MINUTE, 30_000), MINUTE);
        assert!(!countdown.is_running());
    }

    #[test]
    fn a_paused_countdown_that_goes_hidden_resets_too() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        countdown.observe(visible(PlayMode::Pause), 10_000);
        countdown.observe(Visibility::hidden(), 20_000);
        assert_eq!(countdown.remaining(MINUTE, 20_000), MINUTE);
    }

    #[test]
    fn reaching_zero_holds_at_zero() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        for now in [60_000u64, 90_000, 3_600_000] {
            assert_eq!(countdown.remaining(MINUTE, now), Duration::ZERO, "at {now}");
        }

        let entry = TextEntry::new(TextKind::CountdownFromDuration { duration: MINUTE });
        assert_eq!(
            render(&entry, &countdown, 0, 3_600_000, 0).unwrap(),
            "00:00:00"
        );
    }

    #[test]
    fn a_countdown_renders_as_hours_minutes_and_seconds() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        let entry = TextEntry::new(TextKind::CountdownFromDuration {
            duration: Duration::from_secs(3_661),
        });
        assert_eq!(render(&entry, &countdown, 0, 0, 0).unwrap(), "01:01:01");
    }

    #[test]
    fn countdowns_support_original_formats_rollover_and_after_zero_rules() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        let mut entry = TextEntry::new(TextKind::CountdownFromDuration {
            duration: Duration::from_secs(3_661),
        });
        entry.format.countdown.pattern = CountdownPattern::MinutesSeconds;
        entry.format.countdown.separator = ".".to_owned();
        assert_eq!(render(&entry, &countdown, 0, 0, 0).unwrap(), "61.01");
        entry.format.countdown.rollover = true;
        assert_eq!(render(&entry, &countdown, 0, 0, 0).unwrap(), "01.01");

        entry.format.countdown.pattern = CountdownPattern::Seconds;
        entry.format.countdown.after_zero = CountdownAfterZero::Negative;
        assert_eq!(render(&entry, &countdown, 0, 3_662_000, 0).unwrap(), "-01");
        entry.format.countdown.after_zero = CountdownAfterZero::CountUp;
        assert_eq!(render(&entry, &countdown, 0, 3_662_000, 0).unwrap(), "01");
    }

    #[test]
    fn old_persisted_entries_keep_the_previous_default_format() {
        let entry: TextEntry = serde_json::from_str(r#"{"kind":{"kind":"clock"},"enabled":true}"#)
            .expect("old text entry");
        let stamp = (13 * 3_600 + 45 * 60 + 30) * 1_000;
        assert_eq!(
            render(&entry, &Countdown::new(), stamp, 0, 0).unwrap(),
            "13:45:30"
        );
    }

    #[test]
    fn the_last_second_is_shown_until_it_has_truly_gone() {
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        let entry = TextEntry::new(TextKind::CountdownFromDuration {
            duration: Duration::from_secs(1),
        });
        assert_eq!(render(&entry, &countdown, 0, 500, 0).unwrap(), "00:00:01");
        assert_eq!(render(&entry, &countdown, 0, 1_000, 0).unwrap(), "00:00:00");
    }

    #[test]
    fn a_target_countdown_follows_wall_clock_and_holds_at_zero() {
        let entry = TextEntry::new(TextKind::CountdownToTarget {
            target_unix_millis: 100_000,
        });
        let countdown = Countdown::new();

        assert_eq!(
            render(&entry, &countdown, 40_000, 0, 0).unwrap(),
            "00:01:00"
        );
        assert_eq!(
            render(&entry, &countdown, 100_000, 0, 0).unwrap(),
            "00:00:00"
        );
        assert_eq!(
            render(&entry, &countdown, 500_000, 0, 0).unwrap(),
            "00:00:00",
            "past the target it holds rather than counting up"
        );
    }

    #[test]
    fn a_target_countdown_ignores_the_layers_transport() {
        // Only the on-visible countdown has a lifecycle; a target date is absolute.
        let entry = TextEntry::new(TextKind::CountdownToTarget {
            target_unix_millis: 100_000,
        });
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Pause), 0);
        assert_eq!(
            render(&entry, &countdown, 40_000, 999_999, 0).unwrap(),
            "00:01:00"
        );
    }

    #[test]
    fn a_clock_change_cannot_make_a_running_countdown_jump() {
        // The two time sources are separate on purpose.
        let mut countdown = Countdown::new();
        countdown.observe(visible(PlayMode::Loop), 0);
        let entry = TextEntry::new(TextKind::CountdownFromDuration { duration: MINUTE });

        let before = render(&entry, &countdown, 0, 10_000, 0).unwrap();
        let after_clock_change = render(&entry, &countdown, 9_999_999, 10_000, 0).unwrap();
        assert_eq!(before, after_clock_change);
    }
}
