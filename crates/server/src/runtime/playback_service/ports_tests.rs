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
