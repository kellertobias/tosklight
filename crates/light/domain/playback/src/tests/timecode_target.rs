use super::*;

#[test]
fn timecode_target_is_portable_and_has_transport_controls() {
    let timecode_id = TimecodeId(uuid::Uuid::from_u128(70));
    let target = PlaybackTarget::Timecode { timecode_id };
    assert_eq!(
        PlaybackDefinition::default_buttons(&target),
        [
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Pause,
            PlaybackButtonAction::Off,
        ]
    );

    let encoded = serde_json::to_value(&target).unwrap();
    assert_eq!(encoded["type"], "timecode");
    assert_eq!(encoded["timecode_id"], timecode_id.0.to_string());
    assert_eq!(
        serde_json::from_value::<PlaybackTarget>(encoded).unwrap(),
        target
    );
}

#[test]
fn timecode_target_rejects_nil_identity_and_non_transport_controls() {
    let mut playback = definition(1, CueListId::new());
    playback.target = PlaybackTarget::Timecode {
        timecode_id: TimecodeId(uuid::Uuid::nil()),
    };
    playback.buttons = PlaybackDefinition::default_buttons(&playback.target);
    playback.has_fader = false;
    assert_eq!(
        playback.validate().unwrap_err(),
        "Timecode Playback target id must not be nil"
    );

    playback.target = PlaybackTarget::Timecode {
        timecode_id: TimecodeId(uuid::Uuid::from_u128(70)),
    };
    playback.buttons[1] = PlaybackButtonAction::Flash;
    assert_eq!(
        playback.validate().unwrap_err(),
        "playback layout is incompatible with its function"
    );
}

#[test]
fn timecode_target_rejects_fader_but_preserves_button_only_wider_layout() {
    let mut playback = definition(1, CueListId::new());
    playback.target = PlaybackTarget::Timecode {
        timecode_id: TimecodeId(uuid::Uuid::from_u128(70)),
    };
    playback.buttons = PlaybackDefinition::default_buttons(&playback.target);
    playback.fader = PlaybackFaderMode::Master;
    playback.has_fader = true;
    assert!(
        playback.validate().is_err(),
        "automation has no dispatched fader capability"
    );
    playback.has_fader = false;
    playback.footprint = PlaybackFootprint::Wider {
        right_buttons: PlaybackDefinition::default_buttons(&playback.target),
        right_fader: PlaybackFaderMode::Master,
    };
    playback.validate().unwrap();
    let restored: PlaybackDefinition =
        serde_json::from_value(serde_json::to_value(&playback).unwrap()).unwrap();
    restored.validate().unwrap();
    assert!(!restored.has_fader);
    assert_eq!(restored.footprint, playback.footprint);
    playback.has_fader = true;
    playback.reset_incompatible_layout();
    assert!(!playback.has_fader);
    assert_eq!(playback.footprint, restored.footprint);
    playback.validate().unwrap();
}
