use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, ManualClock};
use light_engine::{
    Engine, EnginePlaybackCommand, EngineSnapshot, PoolPlaybackAction, RenderOptions,
};
use light_fixture::{
    ByteOrder, ChannelComponent, FixtureDefinition, FixtureFreezeState, LogicalHead, Parameter,
    PatchedFixture, PatchedHead, SignalLossPolicy,
};
use light_playback::{
    Cue, CueChange, CueList, CueListMode, CueNumber, IntensityPriorityMode, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackFootprint, PlaybackTarget, RestartMode,
    WrapMode,
};
use light_programmer::ProgrammerRegistry;

#[test]
fn cue_fades_follow_registry_value_types_through_media_output() {
    let started = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (patched, fixture_id) = media_fixture();

    let mut first = Cue::new(CueNumber::from_str("1").unwrap());
    let mut second = Cue::new(CueNumber::from_str("2").unwrap());
    second.fade_millis = 1_000;
    for attribute in [
        "media.mask.opacity",
        "focus",
        "media.play_mode",
        "color.wheel.1",
    ] {
        first.changes.push(CueChange::set(
            fixture_id,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(0.0),
        ));
        second.changes.push(CueChange::set(
            fixture_id,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(1.0),
        ));
    }
    let cue_list = cue_list(vec![first, second]);
    let playback = playback(cue_list.id);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    for _ in 0..2 {
        engine
            .execute_playback(EnginePlaybackCommand::Pool {
                number: 1,
                action: PoolPlaybackAction::Go,
            })
            .unwrap();
    }
    clock.set(started + ChronoDuration::milliseconds(500));
    let rendered = engine.render(RenderOptions::default()).unwrap();

    assert_eq!(
        &rendered.universes[&1][0..4],
        &[128, 128, 255, 255],
        "Mask Opacity and Focus interpolate halfway; Play Mode and the colour-wheel slot snap"
    );
}

fn media_fixture() -> (PatchedFixture, FixtureId) {
    let physical = FixtureId::new();
    let logical = FixtureId::new();
    let parameters = [
        "media.mask.opacity",
        "focus",
        "media.play_mode",
        "color.wheel.1",
    ]
    .into_iter()
    .enumerate()
    .map(|(offset, attribute)| Parameter {
        attribute: AttributeKey(attribute.into()),
        components: vec![ChannelComponent {
            offset: offset as u16,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    })
    .collect();
    (
        PatchedFixture {
            fixture_id: physical,
            fixture_number: Some(1),
            virtual_fixture_number: None,
            name: "Media layer".into(),
            layer_id: "default".into(),
            definition: FixtureDefinition {
                schema_version: 1,
                id: FixtureId::new(),
                revision: 1,
                manufacturer: "Test".into(),
                device_type: "media_server".into(),
                name: "Media layer".into(),
                model: "Media layer".into(),
                mode: "4ch".into(),
                footprint: 4,
                heads: vec![LogicalHead {
                    index: 1,
                    name: "Layer".into(),
                    shared: false,
                    parameters,
                }],
                color_calibration: None,
                physical: Default::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: Vec::new(),
                signal_loss_policy: SignalLossPolicy::HoldLast,
                safe_values: BTreeMap::new(),
                profile_id: None,
                mode_id: None,
                profile_snapshot: None,
            },
            universe: Some(1),
            address: Some(1),
            split_patches: Vec::new(),
            direct_control: None,
            internal_bindings: Default::default(),
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![PatchedHead {
                profile_head_id: None,
                head_index: 1,
                fixture_id: logical,
            }],
            multipatch: Vec::new(),
            group_masters_enabled: true,
            grand_master_enabled: true,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: Default::default(),
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
            freeze: FixtureFreezeState::default(),
        },
        logical,
    )
}

fn cue_list(cues: Vec<Cue>) -> CueList {
    CueList {
        id: light_core::CueListId::new(),
        name: "Registry fades".into(),
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

fn playback(cue_list_id: light_core::CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number: 1,
        name: "Registry fades".into(),
        target: PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        footprint: PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
