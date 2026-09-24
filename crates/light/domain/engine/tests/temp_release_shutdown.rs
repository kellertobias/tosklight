//! A Temp runs its Cuelist with the Cues' own timing, follows on by itself, and ends when the list
//! has released everything it held.
//!
//! Cue 1 fades white and full up in 0.2 s. Cue 2 follows 0.3 s after Cue 1 completes and releases
//! both over 1 s. Once nothing is held any more, the Cuelist turns itself off and the Temp button
//! reads off again, without a second press.

use std::{collections::HashMap, sync::Arc};

use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, ManualClock};
use light_engine::{
    Engine, EnginePlaybackCommand, EngineSnapshot, RenderOptions, VirtualPlaybackAction,
};
use light_playback::{
    Cue, CueChange, CueList, CueListMode, CueTrigger, IntensityPriorityMode, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackFootprint, PlaybackIdentity, PlaybackPage,
    PlaybackTarget, RestartMode, VirtualPlaybackAddress, WrapMode,
};
use light_programmer::ProgrammerRegistry;

const VIRTUAL: u16 = 1_001;

#[test]
fn temp_runs_the_list_with_its_timing_and_turns_off_once_everything_is_released() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let strip = FixtureId::new();
    let held = [
        AttributeKey::intensity(),
        AttributeKey("color.red".into()),
        AttributeKey("color.green".into()),
        AttributeKey("color.blue".into()),
    ];

    let mut white = Cue::new(1_u16.into());
    white.fade_millis = 200;
    // Intensity leaving Cue 1 takes Cue 1's out-fade, so both fade out over the same second.
    white.out_fade_millis = Some(1_000);
    let mut release = Cue::new(2_u16.into());
    release.fade_millis = 1_000;
    release.trigger = CueTrigger::Follow { delay_millis: 300 };
    for attribute in &held {
        white.changes.push(CueChange::set(
            strip,
            attribute.clone(),
            AttributeValue::Normalized(1.0),
        ));
        release.changes.push(CueChange {
            value: None,
            ..CueChange::set(strip, attribute.clone(), AttributeValue::Normalized(0.0))
        });
    }
    let cue_list = cue_list(vec![white, release]);
    let address = VirtualPlaybackAddress::new(1, VIRTUAL).unwrap();
    engine
        .replace_snapshot(EngineSnapshot {
            playback_pages: vec![PlaybackPage {
                number: 1,
                name: "Virtual".into(),
                slots: HashMap::new(),
                virtual_playbacks: HashMap::from([(VIRTUAL, temp_playback(cue_list.id))]),
            }]
            .into(),
            cue_lists: vec![cue_list].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let at = |millis: i64| {
        clock.set(started + ChronoDuration::milliseconds(millis));
        engine.render(RenderOptions::default()).unwrap();
        engine.resolved_values()
    };
    let level = |values: &light_engine::ResolvedValues, attribute: &AttributeKey| {
        values
            .get(&(strip, attribute.clone()))
            .and_then(|value| match value {
                AttributeValue::Normalized(level) => Some(*level),
                _ => None,
            })
    };
    let temp_active = || {
        engine
            .playback_runtime_status_at(PlaybackIdentity::Virtual(address))
            .is_some_and(|status| status.temporary_active)
    };
    let cue_number = || {
        engine
            .playback_runtime_status_at(PlaybackIdentity::Virtual(address))
            .and_then(|status| status.playback.current_cue_number)
            .map(|number| number.to_string())
    };

    engine
        .execute_playback(EnginePlaybackCommand::Virtual {
            address,
            action: VirtualPlaybackAction::ToggleTemp,
            exclusion_zones: Vec::new(),
            activation_origin: None,
        })
        .unwrap();
    assert!(temp_active(), "pressing Temp starts the list");

    // Cue 1 fades in over its 0.2 s rather than snapping.
    let halfway = at(100);
    let intensity = level(&halfway, &held[0]).expect("fading in");
    assert!(
        (0.4..=0.6).contains(&intensity),
        "halfway into Cue 1: {intensity}"
    );
    let full = at(210);
    for attribute in &held {
        assert_eq!(
            level(&full, attribute),
            Some(1.0),
            "{attribute:?} at full after 0.2 s"
        );
    }
    assert_eq!(cue_number().as_deref(), Some("1"));

    // Held for 0.3 s, then Cue 2 follows by itself.
    at(460);
    assert_eq!(cue_number().as_deref(), Some("1"), "still holding");
    at(510);
    assert_eq!(
        cue_number().as_deref(),
        Some("2"),
        "Cue 2 follows without a press"
    );
    assert!(temp_active(), "releasing is still playing");

    // Everything fades into release over 1 s.
    let releasing = at(1_010);
    for attribute in &held {
        let value = level(&releasing, attribute).expect("still fading out");
        assert!(
            (0.4..=0.6).contains(&value),
            "{attribute:?} halfway out: {value}"
        );
    }
    assert!(temp_active());

    // Once nothing is held, the list is off and so is Temp.
    let released = at(1_520);
    for attribute in &held {
        assert_eq!(level(&released, attribute), None, "{attribute:?} released");
    }
    assert!(!temp_active(), "Temp returns to off by itself");
    let status = engine.playback_runtime_status_at(PlaybackIdentity::Virtual(address));
    assert!(
        status.is_none_or(|status| !status.playback.enabled && !status.temporary_active),
        "the Playback is inactive"
    );
}

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: CueListId::new(),
        name: "Sunstrip release".into(),
        priority: 10,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
    }
}

fn temp_playback(cue_list_id: CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number: VIRTUAL,
        name: "Sunstrip release".into(),
        target: PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::Temp,
            PlaybackButtonAction::None,
            PlaybackButtonAction::None,
        ],
        button_count: 1,
        fader: PlaybackFaderMode::Master,
        has_fader: false,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#ffffff".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
