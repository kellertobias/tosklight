use super::*;

#[test]
fn macro_target_is_portable_and_has_one_press_control() {
    let macro_id = uuid::Uuid::from_u128(71);
    let target = PlaybackTarget::Macro { macro_id };
    assert_eq!(
        PlaybackDefinition::default_buttons(&target),
        [
            PlaybackButtonAction::Go,
            PlaybackButtonAction::None,
            PlaybackButtonAction::None,
        ]
    );

    let encoded = serde_json::to_value(&target).unwrap();
    assert_eq!(encoded["type"], "macro");
    assert_eq!(encoded["macro_id"], macro_id.to_string());
    assert_eq!(
        serde_json::from_value::<PlaybackTarget>(encoded).unwrap(),
        target
    );
}

#[test]
fn macro_target_rejects_nil_identity_and_incompatible_controls() {
    let mut playback = definition(1, CueListId::new());
    playback.target = PlaybackTarget::Macro {
        macro_id: uuid::Uuid::nil(),
    };
    playback.buttons = PlaybackDefinition::default_buttons(&playback.target);
    playback.has_fader = false;
    assert_eq!(
        playback.validate().unwrap_err(),
        "Macro Playback target id must not be nil"
    );

    playback.target = PlaybackTarget::Macro {
        macro_id: uuid::Uuid::from_u128(71),
    };
    playback.buttons[1] = PlaybackButtonAction::Flash;
    assert_eq!(
        playback.validate().unwrap_err(),
        "playback layout is incompatible with its function"
    );
}
