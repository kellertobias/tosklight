//! Play modes and the Once end state.
//!
//! The v2 personality lays the channel out in three blocks: unsynchronized modes, then their
//! synchronized counterparts, then the transport. Each Once family subdivides into three 8-value
//! end-state sub-bands, because Hold alone cannot clear a layer at the end of a stinger,
//! Transparent alone destroys the common hold-the-final-look use, and Black alone is
//! indistinguishable from a failed source.
//!
//! A mode selector is set deliberately rather than faded, so an 8-value band carries none of the
//! jitter risk that justifies the speed multiplier's broad bands.
//!
//! Every direction can run either continuously or as a single pass, so a reverse pass is
//! selectable rather than merely describable: `Reverse` loops backward for ever, `ReverseOnce`
//! runs backward once and rests on the first frame.

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
    /// Play backward once, then settle. A reverse pass ends at the start of the media, so its
    /// Hold state is the first frame.
    ReverseOnce { end_state: OnceEndState },
    /// Loop using the configured tempo source and synchronized phase.
    LoopSynced,
    /// Play backward using the tempo source and synchronized phase.
    ReverseSynced,
    /// Bounce using the tempo source and synchronized phase.
    BounceSynced,
    /// Start on the synchronized phase, play once, then settle. Never silently becomes a loop.
    OnceSynced { end_state: OnceEndState },
    /// Start on the synchronized phase, play backward once, then settle.
    ReverseOnceSynced { end_state: OnceEndState },
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
            0..=19 => Self::Loop,
            20..=39 => Self::Reverse,
            40..=59 => Self::Bounce,
            60..=67 => Self::Once {
                end_state: OnceEndState::Hold,
            },
            68..=75 => Self::Once {
                end_state: OnceEndState::Black,
            },
            76..=83 => Self::Once {
                end_state: OnceEndState::Transparent,
            },
            84..=91 => Self::ReverseOnce {
                end_state: OnceEndState::Hold,
            },
            92..=99 => Self::ReverseOnce {
                end_state: OnceEndState::Black,
            },
            100..=107 => Self::ReverseOnce {
                end_state: OnceEndState::Transparent,
            },
            108..=127 => Self::LoopSynced,
            128..=147 => Self::ReverseSynced,
            148..=167 => Self::BounceSynced,
            168..=175 => Self::OnceSynced {
                end_state: OnceEndState::Hold,
            },
            176..=183 => Self::OnceSynced {
                end_state: OnceEndState::Black,
            },
            184..=191 => Self::OnceSynced {
                end_state: OnceEndState::Transparent,
            },
            192..=199 => Self::ReverseOnceSynced {
                end_state: OnceEndState::Hold,
            },
            200..=207 => Self::ReverseOnceSynced {
                end_state: OnceEndState::Black,
            },
            208..=215 => Self::ReverseOnceSynced {
                end_state: OnceEndState::Transparent,
            },
            216..=235 => Self::Stop,
            236..=255 => Self::Pause,
        }
    }

    /// The inclusive wire range this mode occupies, for GDTF channel functions and UI metadata.
    pub const fn dmx_range(self) -> (u8, u8) {
        match self {
            Self::Loop => (0, 19),
            Self::Reverse => (20, 39),
            Self::Bounce => (40, 59),
            Self::Once {
                end_state: OnceEndState::Hold,
            } => (60, 67),
            Self::Once {
                end_state: OnceEndState::Black,
            } => (68, 75),
            Self::Once {
                end_state: OnceEndState::Transparent,
            } => (76, 83),
            Self::ReverseOnce {
                end_state: OnceEndState::Hold,
            } => (84, 91),
            Self::ReverseOnce {
                end_state: OnceEndState::Black,
            } => (92, 99),
            Self::ReverseOnce {
                end_state: OnceEndState::Transparent,
            } => (100, 107),
            Self::LoopSynced => (108, 127),
            Self::ReverseSynced => (128, 147),
            Self::BounceSynced => (148, 167),
            Self::OnceSynced {
                end_state: OnceEndState::Hold,
            } => (168, 175),
            Self::OnceSynced {
                end_state: OnceEndState::Black,
            } => (176, 183),
            Self::OnceSynced {
                end_state: OnceEndState::Transparent,
            } => (184, 191),
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Hold,
            } => (192, 199),
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Black,
            } => (200, 207),
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Transparent,
            } => (208, 215),
            Self::Stop => (216, 235),
            Self::Pause => (236, 255),
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
            Self::ReverseOnce {
                end_state: OnceEndState::Hold,
            } => "Reverse Once — Hold",
            Self::ReverseOnce {
                end_state: OnceEndState::Black,
            } => "Reverse Once — Black",
            Self::ReverseOnce {
                end_state: OnceEndState::Transparent,
            } => "Reverse Once — Transparent",
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
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Hold,
            } => "Reverse Once Synced — Hold",
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Black,
            } => "Reverse Once Synced — Black",
            Self::ReverseOnceSynced {
                end_state: OnceEndState::Transparent,
            } => "Reverse Once Synced — Transparent",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
        }
    }

    /// Every mode in wire order. The canonical table the personality, GDTF, and UI enumerate.
    pub const ALL: [Self; 20] = [
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
        Self::ReverseOnce {
            end_state: OnceEndState::Hold,
        },
        Self::ReverseOnce {
            end_state: OnceEndState::Black,
        },
        Self::ReverseOnce {
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
        Self::ReverseOnceSynced {
            end_state: OnceEndState::Hold,
        },
        Self::ReverseOnceSynced {
            end_state: OnceEndState::Black,
        },
        Self::ReverseOnceSynced {
            end_state: OnceEndState::Transparent,
        },
        Self::Stop,
        Self::Pause,
    ];

    /// Whether this mode follows the output's tempo source.
    pub const fn is_synchronized(self) -> bool {
        matches!(
            self,
            Self::LoopSynced
                | Self::ReverseSynced
                | Self::BounceSynced
                | Self::OnceSynced { .. }
                | Self::ReverseOnceSynced { .. }
        )
    }

    /// Whether playback runs backward through the media.
    pub const fn is_reverse(self) -> bool {
        matches!(
            self,
            Self::Reverse
                | Self::ReverseSynced
                | Self::ReverseOnce { .. }
                | Self::ReverseOnceSynced { .. }
        )
    }

    /// The single-pass end state, when this mode has one. Bounce has no single-pass end, so it is
    /// unaffected.
    pub const fn once_end_state(self) -> Option<OnceEndState> {
        match self {
            Self::Once { end_state }
            | Self::ReverseOnce { end_state }
            | Self::OnceSynced { end_state }
            | Self::ReverseOnceSynced { end_state } => Some(end_state),
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
        assert_eq!(PlayMode::from_dmx(19), PlayMode::Loop);
        assert_eq!(PlayMode::from_dmx(20), PlayMode::Reverse);
        assert_eq!(PlayMode::from_dmx(39), PlayMode::Reverse);
        assert_eq!(PlayMode::from_dmx(40), PlayMode::Bounce);
        assert_eq!(PlayMode::from_dmx(59), PlayMode::Bounce);
        assert_eq!(PlayMode::from_dmx(108), PlayMode::LoopSynced);
        assert_eq!(PlayMode::from_dmx(147), PlayMode::ReverseSynced);
        assert_eq!(PlayMode::from_dmx(167), PlayMode::BounceSynced);
        assert_eq!(PlayMode::from_dmx(216), PlayMode::Stop);
        assert_eq!(PlayMode::from_dmx(235), PlayMode::Stop);
        assert_eq!(PlayMode::from_dmx(236), PlayMode::Pause);
        assert_eq!(PlayMode::from_dmx(255), PlayMode::Pause);
    }

    #[test]
    fn the_channel_is_laid_out_as_unsynchronized_then_synchronized_then_transport() {
        for value in 0..=107u8 {
            let mode = PlayMode::from_dmx(value);
            assert!(!mode.is_synchronized(), "{value} is {}", mode.label());
            assert!(mode.is_transport_running(), "{value} is {}", mode.label());
        }
        for value in 108..=215u8 {
            assert!(PlayMode::from_dmx(value).is_synchronized(), "{value}");
        }
        for value in 216..=255u8 {
            assert!(!PlayMode::from_dmx(value).is_transport_running(), "{value}");
        }
    }

    #[test]
    fn a_reverse_pass_is_selectable_in_both_families() {
        assert_eq!(
            PlayMode::from_dmx(84),
            PlayMode::ReverseOnce {
                end_state: OnceEndState::Hold
            }
        );
        assert_eq!(
            PlayMode::from_dmx(107),
            PlayMode::ReverseOnce {
                end_state: OnceEndState::Transparent
            }
        );
        assert_eq!(
            PlayMode::from_dmx(192),
            PlayMode::ReverseOnceSynced {
                end_state: OnceEndState::Hold
            }
        );
        assert_eq!(
            PlayMode::from_dmx(215),
            PlayMode::ReverseOnceSynced {
                end_state: OnceEndState::Transparent
            }
        );
    }

    #[test]
    fn every_direction_can_run_continuously_or_as_a_single_pass() {
        let mut continuous_reverse = false;
        let mut single_pass_reverse = false;
        let mut continuous_forward = false;
        let mut single_pass_forward = false;
        for mode in PlayMode::ALL {
            if !mode.is_transport_running() {
                continue;
            }
            match (mode.is_reverse(), mode.once_end_state().is_some()) {
                (true, true) => single_pass_reverse = true,
                (true, false) => continuous_reverse = true,
                (false, true) => single_pass_forward = true,
                (false, false) => continuous_forward = true,
            }
        }
        assert!(continuous_forward && single_pass_forward);
        assert!(
            continuous_reverse && single_pass_reverse,
            "a reverse pass must be selectable"
        );
    }

    #[test]
    fn every_once_family_subdivides_into_eight_value_bands_with_hold_lowest() {
        for base in [60u8, 84, 168, 192] {
            let family = PlayMode::from_dmx(base);
            for (offset, expected) in [
                (0, OnceEndState::Hold),
                (7, OnceEndState::Hold),
                (8, OnceEndState::Black),
                (15, OnceEndState::Black),
                (16, OnceEndState::Transparent),
                (23, OnceEndState::Transparent),
            ] {
                assert_eq!(
                    PlayMode::from_dmx(base + offset).once_end_state(),
                    Some(expected),
                    "{} at +{offset}",
                    family.label()
                );
            }
        }
    }

    #[test]
    fn hold_is_the_lowest_sub_band_of_every_once_family() {
        for mode in PlayMode::ALL {
            if mode.once_end_state() != Some(OnceEndState::Hold) {
                continue;
            }
            let (low, _) = mode.dmx_range();
            for other in [OnceEndState::Black, OnceEndState::Transparent] {
                let sibling = PlayMode::ALL
                    .into_iter()
                    .find(|candidate| {
                        candidate.once_end_state() == Some(other)
                            && candidate.is_reverse() == mode.is_reverse()
                            && candidate.is_synchronized() == mode.is_synchronized()
                    })
                    .expect("every family has all three end states");
                assert!(
                    low < sibling.dmx_range().0,
                    "{} is not lowest",
                    mode.label()
                );
            }
        }
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
                "Reverse Once Synced — Hold",
                "Reverse Once Synced — Black",
                "Reverse Once Synced — Transparent",
            ]
        );
    }

    #[test]
    fn reverse_runs_backward_in_every_one_of_its_forms() {
        assert!(PlayMode::Reverse.is_reverse());
        assert!(PlayMode::ReverseSynced.is_reverse());
        assert!(
            PlayMode::ReverseOnce {
                end_state: OnceEndState::Hold
            }
            .is_reverse()
        );
        assert!(
            PlayMode::ReverseOnceSynced {
                end_state: OnceEndState::Black
            }
            .is_reverse()
        );
        assert!(!PlayMode::Loop.is_reverse());
        assert!(!PlayMode::Bounce.is_reverse());
        assert!(
            !PlayMode::Once {
                end_state: OnceEndState::Hold
            }
            .is_reverse()
        );
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
