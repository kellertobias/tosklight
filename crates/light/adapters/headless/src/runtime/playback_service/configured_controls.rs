//! Resolves typed configured controls against one Playback's persisted control layout.

use light_application::{ActionError, ActionErrorKind, PlaybackAction};
use light_playback::{PlaybackButtonAction, PlaybackDefinition, PlaybackFootprint};

/// Returns a cloned definition narrowed to the addressed control and the legacy-compatible action
/// which dispatches through that clone. Keeping the definition and action together makes semantic
/// transition detection, automatic-off discovery, preload capture, and execution agree.
pub(super) fn configured_control_definition(
    definition: &PlaybackDefinition,
    action: PlaybackAction,
) -> Result<(PlaybackDefinition, PlaybackAction), ActionError> {
    let mut resolved = definition.clone();
    let action = match action {
        PlaybackAction::ConfiguredButton { number, pressed } if (1..=3).contains(&number) => {
            if number > definition.button_count {
                return Err(invalid("button is not present on this playback"));
            }
            PlaybackAction::ConfiguredButton { number, pressed }
        }
        PlaybackAction::ConfiguredButton { number, pressed } if (4..=6).contains(&number) => {
            let button = expanded_button(definition, number)?;
            resolved.buttons = [PlaybackButtonAction::None; 3];
            resolved.buttons[0] = button;
            resolved.button_count = 1;
            PlaybackAction::ConfiguredButton { number: 1, pressed }
        }
        PlaybackAction::ConfiguredFader { number: 1, level } => PlaybackAction::Master(level),
        PlaybackAction::ConfiguredFader { number: 2, level } => {
            let PlaybackFootprint::Wider { right_fader, .. } = &definition.footprint else {
                return Err(invalid("fader is not present on this playback"));
            };
            resolved.fader = *right_fader;
            resolved.has_fader = true;
            PlaybackAction::Master(level)
        }
        PlaybackAction::ConfiguredButton { .. } => {
            return Err(invalid("configured button number must be within 1-6"));
        }
        PlaybackAction::ConfiguredFader { .. } => {
            return Err(invalid("configured fader number must be within 1-2"));
        }
        action => action,
    };
    Ok((resolved, action))
}

fn expanded_button(
    definition: &PlaybackDefinition,
    number: u8,
) -> Result<PlaybackButtonAction, ActionError> {
    match (&definition.footprint, number) {
        (PlaybackFootprint::Taller { upper_button }, 4) => Ok(*upper_button),
        (PlaybackFootprint::Wider { right_buttons, .. }, 4..=6) => {
            Ok(right_buttons[usize::from(number - 4)])
        }
        _ => Err(invalid("button is not present on this playback")),
    }
}

fn invalid(message: &'static str) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_application::PlaybackLevel;
    use light_playback::{PlaybackFaderMode, PlaybackTarget};

    #[test]
    fn taller_button_four_is_dispatched_as_the_cloned_first_button() {
        let mut definition = definition();
        definition.footprint = PlaybackFootprint::Taller {
            upper_button: PlaybackButtonAction::Go,
        };

        let (resolved, action) = configured_control_definition(
            &definition,
            PlaybackAction::ConfiguredButton {
                number: 4,
                pressed: true,
            },
        )
        .unwrap();

        assert_eq!(resolved.buttons[0], PlaybackButtonAction::Go);
        assert_eq!(resolved.button_count, 1);
        assert_eq!(
            action,
            PlaybackAction::ConfiguredButton {
                number: 1,
                pressed: true,
            }
        );
        assert_eq!(definition.buttons[0], PlaybackButtonAction::Toggle);
        assert_eq!(
            super::super::semantics::configured_transition_cause(&resolved, action),
            Some(light_application::PlaybackTransitionCause::Go)
        );
        assert!(super::super::semantics::may_trigger_auto_off(
            action, &resolved
        ));
    }

    #[test]
    fn wider_buttons_four_through_six_select_their_matching_right_button() {
        let mut definition = definition();
        definition.footprint = PlaybackFootprint::Wider {
            right_buttons: [
                PlaybackButtonAction::On,
                PlaybackButtonAction::Pause,
                PlaybackButtonAction::FastForward,
            ],
            right_fader: PlaybackFaderMode::XFade,
        };

        for (number, expected) in [
            (4, PlaybackButtonAction::On),
            (5, PlaybackButtonAction::Pause),
            (6, PlaybackButtonAction::FastForward),
        ] {
            let (resolved, action) = configured_control_definition(
                &definition,
                PlaybackAction::ConfiguredButton {
                    number,
                    pressed: false,
                },
            )
            .unwrap();
            assert_eq!(resolved.buttons[0], expected);
            assert_eq!(
                action,
                PlaybackAction::ConfiguredButton {
                    number: 1,
                    pressed: false,
                }
            );
        }
    }

    #[test]
    fn wider_fader_two_selects_the_right_fader_mode_on_the_clone() {
        let mut definition = definition();
        definition.has_fader = false;
        definition.footprint = PlaybackFootprint::Wider {
            right_buttons: [PlaybackButtonAction::None; 3],
            right_fader: PlaybackFaderMode::Temp,
        };
        let level = PlaybackLevel::new(0.375);

        let (resolved, action) = configured_control_definition(
            &definition,
            PlaybackAction::ConfiguredFader { number: 2, level },
        )
        .unwrap();

        assert_eq!(resolved.fader, PlaybackFaderMode::Temp);
        assert!(resolved.has_fader);
        assert_eq!(action, PlaybackAction::Master(level));
        assert_eq!(definition.fader, PlaybackFaderMode::Master);
        assert!(!definition.has_fader);
        assert!(!super::super::semantics::may_trigger_auto_off(
            action, &resolved
        ));

        definition.footprint = PlaybackFootprint::Wider {
            right_buttons: [PlaybackButtonAction::None; 3],
            right_fader: PlaybackFaderMode::Master,
        };
        let (resolved, action) = configured_control_definition(
            &definition,
            PlaybackAction::ConfiguredFader { number: 2, level },
        )
        .unwrap();
        assert!(super::super::semantics::may_trigger_auto_off(
            action, &resolved
        ));
    }

    #[test]
    fn controls_absent_from_the_selected_footprint_are_rejected() {
        let definition = definition();
        assert!(
            configured_control_definition(
                &definition,
                PlaybackAction::ConfiguredButton {
                    number: 4,
                    pressed: true,
                },
            )
            .is_err()
        );
        assert!(
            configured_control_definition(
                &definition,
                PlaybackAction::ConfiguredFader {
                    number: 2,
                    level: PlaybackLevel::new(0.5),
                },
            )
            .is_err()
        );
    }

    fn definition() -> PlaybackDefinition {
        PlaybackDefinition {
            number: 1,
            name: "Expanded".into(),
            target: PlaybackTarget::CueList {
                cue_list_id: light_core::CueListId::new(),
            },
            buttons: [
                PlaybackButtonAction::Toggle,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader: true,
            footprint: PlaybackFootprint::Normal,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#fff".into(),
            flash_release: light_playback::FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }
}
