//! Play modes and the Once end state.
//!
//! The v2 personality divides the first 192 wire values into eight stable 24-value bands and
//! keeps the Stop and Pause ranges. The two Once bands subdivide into three 8-value end-state
//! sub-bands, because Hold alone cannot clear a layer at the end of a stinger, Transparent alone
//! destroys the common hold-the-final-look use, and Black alone is indistinguishable from a
//! failed source.
//!
//! A mode selector is set deliberately rather than faded, so an 8-value band carries none of the
//! jitter risk that justifies the speed multiplier's broad bands.

use serde::{Deserialize, Serialize};

/// What a layer presents once a single forward or reverse pass has finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnceEndState {
    /// Keep presenting the final frame. Dimmer, tint, mask, effects, and geometry keep applying.
    ///
    /// The lowest sub-band, so a migrated legacy show keeps today's behavior.
    #[default]
    Hold,
    /// Present opaque black, still occupying the layer and masking lower layers. Layer dimmer
    /// still applies, so dimmer 0 is invisible.
    Black,
    /// Contribute nothing; lower layers show through. Visually identical to a failed source, but
    /// it reports a completed state rather than `Failed`.
    Transparent,
}

/// Which direction a pass runs, and whether it repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlayMode {
    /// Play forward, restart at the end.
    #[default]
    Loop,
    /// Play backward, restart from the end.
    Reverse,
    /// Alternate forward and reverse.
    Bounce,
    /// Play forward once, then settle into the configured end state.
    Once { end_state: OnceEndState },
    /// Loop using the configured tempo source and synchronized phase.
    LoopSynced,
    /// Play backward using the tempo source and synchronized phase.
    ReverseSynced,
    /// Bounce using the tempo source and synchronized phase.
    BounceSynced,
    /// Start on the synchronized phase, play once, then settle. Never silently becomes a loop.
    OnceSynced { end_state: OnceEndState },
    /// Pause and seek to the beginning.
    Stop,
    /// Hold the current frame.
    Pause,
}

impl PlayMode {
    /// Decodes the play-mode channel.
    ///
    /// This is the one place the band boundaries exist. Rust, the UI, the tests, and GDTF all
    /// generate their boundaries from it rather than restating the table.
    pub const fn from_dmx(value: u8) -> Self {
        match value {
            0..=23 => Self::Loop,
            24..=47 => Self::Reverse,
            48..=71 => Self::Bounce,
            72..=79 => Self::Once {
                end_state: OnceEndState::Hold,
            },
            80..=87 => Self::Once {
                end_state: OnceEndState::Black,
            },
            88..=95 => Self::Once {
                end_state: OnceEndState::Transparent,
            },
            96..=119 => Self::LoopSynced,
            120..=143 => Self::ReverseSynced,
            144..=167 => Self::BounceSynced,
            168..=175 => Self::OnceSynced {
                end_state: OnceEndState::Hold,
            },
            176..=183 => Self::OnceSynced {
                end_state: OnceEndState::Black,
            },
            184..=191 => Self::OnceSynced {
                end_state: OnceEndState::Transparent,
            },
            192..=223 => Self::Stop,
            224..=255 => Self::Pause,
        }
    }

    /// The inclusive wire range this mode occupies, for GDTF channel functions and UI metadata.
    pub const fn dmx_range(self) -> (u8, u8) {
        match self {
            Self::Loop => (0, 23),
            Self::Reverse => (24, 47),
            Self::Bounce => (48, 71),
            Self::Once {
                end_state: OnceEndState::Hold,
            } => (72, 79),
            Self::Once {
                end_state: OnceEndState::Black,
            } => (80, 87),
            Self::Once {
                end_state: OnceEndState::Transparent,
            } => (88, 95),
            Self::LoopSynced => (96, 119),
            Self::ReverseSynced => (120, 143),
            Self::BounceSynced => (144, 167),
            Self::OnceSynced {
                end_state: OnceEndState::Hold,
            } => (168, 175),
            Self::OnceSynced {
                end_state: OnceEndState::Black,
            } => (176, 183),
            Self::OnceSynced {
                end_state: OnceEndState::Transparent,
            } => (184, 191),
            Self::Stop => (192, 223),
            Self::Pause => (224, 255),
        }
    }

    /// The name a desk shows for this mode.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loop => "Loop",
            Self::Reverse => "Reverse",
            Self::Bounce => "Bounce",
            Self::Once {
                end_state: OnceEndState::Hold,
            } => "Once — Hold",
            Self::Once {
                end_state: OnceEndState::Black,
            } => "Once — Black",
            Self::Once {
                end_state: OnceEndState::Transparent,
            } => "Once — Transparent",
            Self::LoopSynced => "Loop Synced",
            Self::ReverseSynced => "Reverse Synced",
            Self::BounceSynced => "Bounce Synced",
            Self::OnceSynced {
                end_state: OnceEndState::Hold,
            } => "Once Synced — Hold",
            Self::OnceSynced {
                end_state: OnceEndState::Black,
            } => "Once Synced — Black",
            Self::OnceSynced {
                end_state: OnceEndState::Transparent,
            } => "Once Synced — Transparent",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
        }
    }

    /// Every mode in wire order. The canonical table the personality, GDTF, and UI enumerate.
    pub const ALL: [Self; 14] = [
        Self::Loop,
        Self::Reverse,
        Self::Bounce,
        Self::Once {
            end_state: OnceEndState::Hold,
        },
        Self::Once {
            end_state: OnceEndState::Black,
        },
        Self::Once {
            end_state: OnceEndState::Transparent,
        },
        Self::LoopSynced,
        Self::ReverseSynced,
        Self::BounceSynced,
        Self::OnceSynced {
            end_state: OnceEndState::Hold,
        },
        Self::OnceSynced {
            end_state: OnceEndState::Black,
        },
        Self::OnceSynced {
            end_state: OnceEndState::Transparent,
        },
        Self::Stop,
        Self::Pause,
    ];

    /// Whether this mode follows the output's tempo source.
    pub const fn is_synchronized(self) -> bool {
        matches!(
            self,
            Self::LoopSynced | Self::ReverseSynced | Self::BounceSynced | Self::OnceSynced { .. }
        )
    }

    /// Whether playback runs backward through the media.
    pub const fn is_reverse(self) -> bool {
        matches!(self, Self::Reverse | Self::ReverseSynced)
    }

    /// The single-pass end state, when this mode has one. Bounce has no single-pass end, so it is
    /// unaffected.
    pub const fn once_end_state(self) -> Option<OnceEndState> {
        match self {
            Self::Once { end_state } | Self::OnceSynced { end_state } => Some(end_state),
            _ => None,
        }
    }

    /// Whether the transport is advancing at all. Stop and Pause are both stationary but stay
    /// distinguishable: Stop seeks to the beginning, Pause holds the current frame.
    pub const fn is_transport_running(self) -> bool {
        !matches!(self, Self::Stop | Self::Pause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wire_value_decodes_inside_the_band_it_reports() {
        for value in 0..=255u8 {
            let mode = PlayMode::from_dmx(value);
            let (low, high) = mode.dmx_range();
            assert!(
                (low..=high).contains(&value),
                "value {value} decoded to {} which claims {low}..={high}",
                mode.label()
            );
        }
    }

    #[test]
    fn the_bands_tile_the_whole_channel_without_gaps_or_overlap() {
        let mut next = 0u16;
        for mode in PlayMode::ALL {
            let (low, high) = mode.dmx_range();
            assert_eq!(
                u16::from(low),
                next,
                "gap or overlap before {}",
                mode.label()
            );
            next = u16::from(high) + 1;
        }
        assert_eq!(next, 256);
    }

    #[test]
    fn the_documented_boundaries_hold() {
        assert_eq!(PlayMode::from_dmx(0), PlayMode::Loop);
        assert_eq!(PlayMode::from_dmx(23), PlayMode::Loop);
        assert_eq!(PlayMode::from_dmx(24), PlayMode::Reverse);
        assert_eq!(PlayMode::from_dmx(47), PlayMode::Reverse);
        assert_eq!(PlayMode::from_dmx(48), PlayMode::Bounce);
        assert_eq!(PlayMode::from_dmx(71), PlayMode::Bounce);
        assert_eq!(PlayMode::from_dmx(96), PlayMode::LoopSynced);
        assert_eq!(PlayMode::from_dmx(143), PlayMode::ReverseSynced);
        assert_eq!(PlayMode::from_dmx(167), PlayMode::BounceSynced);
        assert_eq!(PlayMode::from_dmx(192), PlayMode::Stop);
        assert_eq!(PlayMode::from_dmx(223), PlayMode::Stop);
        assert_eq!(PlayMode::from_dmx(224), PlayMode::Pause);
        assert_eq!(PlayMode::from_dmx(255), PlayMode::Pause);
    }

    #[test]
    fn the_once_sub_bands_are_eight_values_each_with_hold_lowest() {
        assert_eq!(
            PlayMode::from_dmx(72).once_end_state(),
            Some(OnceEndState::Hold)
        );
        assert_eq!(
            PlayMode::from_dmx(79).once_end_state(),
            Some(OnceEndState::Hold)
        );
        assert_eq!(
            PlayMode::from_dmx(80).once_end_state(),
            Some(OnceEndState::Black)
        );
        assert_eq!(
            PlayMode::from_dmx(87).once_end_state(),
            Some(OnceEndState::Black)
        );
        assert_eq!(
            PlayMode::from_dmx(88).once_end_state(),
            Some(OnceEndState::Transparent)
        );
        assert_eq!(
            PlayMode::from_dmx(95).once_end_state(),
            Some(OnceEndState::Transparent)
        );

        assert_eq!(
            PlayMode::from_dmx(168).once_end_state(),
            Some(OnceEndState::Hold)
        );
        assert_eq!(
            PlayMode::from_dmx(175).once_end_state(),
            Some(OnceEndState::Hold)
        );
        assert_eq!(
            PlayMode::from_dmx(183).once_end_state(),
            Some(OnceEndState::Black)
        );
        assert_eq!(
            PlayMode::from_dmx(191).once_end_state(),
            Some(OnceEndState::Transparent)
        );
    }

    #[test]
    fn hold_is_the_default_end_state() {
        assert_eq!(OnceEndState::default(), OnceEndState::Hold);
    }

    #[test]
    fn bounce_has_no_single_pass_end() {
        assert_eq!(PlayMode::Bounce.once_end_state(), None);
        assert_eq!(PlayMode::BounceSynced.once_end_state(), None);
    }

    #[test]
    fn only_the_synced_modes_follow_the_tempo_source() {
        let synced: Vec<&str> = PlayMode::ALL
            .into_iter()
            .filter(|mode| mode.is_synchronized())
            .map(PlayMode::label)
            .collect();
        assert_eq!(
            synced,
            [
                "Loop Synced",
                "Reverse Synced",
                "Bounce Synced",
                "Once Synced — Hold",
                "Once Synced — Black",
                "Once Synced — Transparent",
            ]
        );
    }

    #[test]
    fn reverse_runs_backward_in_both_its_forms() {
        assert!(PlayMode::Reverse.is_reverse());
        assert!(PlayMode::ReverseSynced.is_reverse());
        assert!(!PlayMode::Loop.is_reverse());
        assert!(!PlayMode::Bounce.is_reverse());
    }

    #[test]
    fn stop_and_pause_are_the_only_stationary_modes() {
        for mode in PlayMode::ALL {
            let stationary = matches!(mode, PlayMode::Stop | PlayMode::Pause);
            assert_eq!(mode.is_transport_running(), !stationary, "{}", mode.label());
        }
    }

    #[test]
    fn every_mode_has_a_distinct_label() {
        let mut labels: Vec<&str> = PlayMode::ALL.into_iter().map(PlayMode::label).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count);
    }
}
