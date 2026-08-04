use super::super::semantics::{may_activate_playback, may_trigger_auto_off};
use super::*;
use light_application::PlaybackLevel;

#[test]
fn exclusion_candidates_match_only_interactions_that_can_activate() {
    let definition = test_definition();
    assert!(may_activate_playback(PlaybackAction::Temp {
        pressed: true
    }));
    assert!(may_activate_playback(PlaybackAction::Temporary {
        enabled: true,
        pressed: true,
    }));
    assert!(may_activate_playback(PlaybackAction::Master(
        PlaybackLevel::new(0.5)
    )));
    assert!(may_activate_playback(PlaybackAction::Flash {
        pressed: false
    }));
    assert!(may_activate_playback(PlaybackAction::Swap {
        pressed: false
    }));
    assert!(may_activate_playback(PlaybackAction::ConfiguredButton {
        number: 1,
        pressed: false,
    }));
    assert!(may_activate_playback(PlaybackAction::Master(
        PlaybackLevel::new(0.0)
    )));
    assert!(may_trigger_auto_off(
        PlaybackAction::Pause { pressed: true },
        &definition
    ));
    assert!(!may_trigger_auto_off(
        PlaybackAction::Back { pressed: true },
        &definition
    ));
}

#[test]
fn dedicated_virtual_runtime_never_collapses_unsupported_buttons_to_pool_actions() {
    let mut definition = test_definition();
    definition.number = 1_001;
    definition.button_count = 1;
    definition.buttons = [
        light_playback::PlaybackButtonAction::Toggle,
        light_playback::PlaybackButtonAction::None,
        light_playback::PlaybackButtonAction::None,
    ];
    assert_eq!(
        virtual_runtime_action(
            &definition,
            PlaybackAction::ConfiguredButton {
                number: 1,
                pressed: true,
            }
        )
        .unwrap(),
        Some(VirtualPlaybackAction::Toggle)
    );
    assert_eq!(
        virtual_runtime_action(
            &definition,
            PlaybackAction::ConfiguredButton {
                number: 1,
                pressed: false,
            }
        )
        .unwrap(),
        None
    );

    definition.buttons[0] = light_playback::PlaybackButtonAction::Select;
    assert!(
        virtual_runtime_action(
            &definition,
            PlaybackAction::ConfiguredButton {
                number: 1,
                pressed: true,
            }
        )
        .is_err()
    );
}

fn test_definition() -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number: 1,
        name: "test".into(),
        target: light_playback::PlaybackTarget::CueList {
            cue_list_id: light_core::CueListId::new(),
        },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
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
