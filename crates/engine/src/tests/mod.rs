use super::*;
use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use light_core::{
    ApplicationClock, AttributeKey, AttributeValue, FixtureId, ManualClock, MergeMode,
    ProgrammerId, SessionId, SharedClock, TimedValue, UserId, Xyz,
};
use light_fixture::{
    ByteOrder, ChannelBehavior, ChannelComponent, ChannelFunction, ChannelResolution, ColorSystem,
    FixtureChannel, FixtureDefinition, FixtureHead, FixtureProfile, FixtureSplit, GeometryGraph,
    GeometryTemplate, LogicalHead, MultiPatchInstance, Parameter, PatchedFixture, PatchedHead,
    SignalLossPolicy, SplitPatch,
};
use light_playback::{
    Cue, CueChange, CueList, CueListMode, IntensityPriorityMode, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackTarget, RestartMode, WrapMode,
};
use light_programmer::{GroupDefinition, ProgrammerRegistry};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

fn fixture() -> (PatchedFixture, FixtureId) {
    let physical = FixtureId::new();
    let logical = FixtureId::new();
    let parameter = Parameter {
        attribute: AttributeKey::intensity(),
        components: vec![ChannelComponent {
            offset: 0,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    };
    (
        PatchedFixture {
            fixture_id: physical,
            fixture_number: None,
            virtual_fixture_number: None,
            name: "Cell".into(),
            layer_id: "default".into(),
            definition: FixtureDefinition {
                schema_version: 1,
                id: FixtureId::new(),
                revision: 1,
                manufacturer: "Test".into(),
                device_type: "other".into(),
                name: "Cell".into(),
                model: "Cell".into(),
                mode: "1ch".into(),
                footprint: 1,
                heads: vec![LogicalHead {
                    index: 1,
                    name: "Cell".into(),
                    shared: false,
                    parameters: vec![parameter],
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
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![PatchedHead {
                profile_head_id: None,
                head_index: 1,
                fixture_id: logical,
            }],
            multipatch: Vec::new(),
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        },
        logical,
    )
}

fn schema_v2_fixture(
    channels: &[(&str, bool, bool, bool, bool, bool)],
) -> (PatchedFixture, FixtureId) {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Semantic fixture".into();
    profile.short_name = "Semantic".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = channels.len() as u16;
    mode.channels = channels
        .iter()
        .map(
            |(attribute, snap, virtual_intensity, sequence, group, grand)| FixtureChannel {
                id: uuid::Uuid::new_v4(),
                head_id,
                split: 1,
                attribute: AttributeKey((*attribute).into()),
                resolution: ChannelResolution::U8,
                secondary_slots: vec![],
                default_raw: 0,
                highlight_raw: u8::MAX.into(),
                physical_min: Some(0.0),
                physical_max: Some(1.0),
                unit: None,
                invert: false,
                snap: *snap,
                reacts_to_virtual_intensity: *virtual_intensity,
                reacts_to_sequence_master: *sequence,
                reacts_to_group_master: *group,
                reacts_to_grand_master: *grand,
                behavior: ChannelBehavior::Controlled,
                functions: vec![ChannelFunction::continuous(
                    *attribute,
                    AttributeKey((*attribute).into()),
                    u8::MAX.into(),
                )],
            },
        )
        .collect();
    let mode_id = mode.id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let fixture_id = FixtureId::new();
    (
        PatchedFixture {
            fixture_id,
            fixture_number: Some(1),
            virtual_fixture_number: None,
            name: "Semantic fixture".into(),
            definition,
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: "default".into(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        },
        fixture_id,
    )
}

fn test_cue_list(name: &str, changes: Vec<CueChange>) -> CueList {
    let mut cue = Cue::new(1.0);
    cue.changes = changes;
    CueList {
        id: light_core::CueListId::new(),
        name: name.into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    }
}

fn test_playback(number: u16, cue_list_id: light_core::CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        target: PlaybackTarget::CueList { cue_list_id },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn moving_fixture(address: u16, enabled: bool, delay_millis: u64) -> (PatchedFixture, FixtureId) {
    let (mut fixture, logical) = fixture();
    fixture.address = Some(address);
    fixture.definition.footprint = 2;
    fixture.definition.heads[0].parameters.push(Parameter {
        attribute: AttributeKey("pan".into()),
        components: vec![ChannelComponent {
            offset: 1,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    });
    fixture.move_in_black_enabled = enabled;
    fixture.move_in_black_delay_millis = delay_millis;
    (fixture, logical)
}

fn mib_snapshot(fixtures: Vec<PatchedFixture>, fixture_ids: &[FixtureId]) -> EngineSnapshot {
    let mut first = Cue::new(1.0);
    let mut dark = Cue::new(2.0);
    dark.fade_millis = 2_000;
    let mut lit = Cue::new(3.0);
    for fixture_id in fixture_ids {
        first.changes.push(CueChange::set(
            *fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        ));
        first.changes.push(CueChange::set(
            *fixture_id,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.2),
        ));
        dark.changes.push(CueChange::set(
            *fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.0),
        ));
        lit.changes.push(CueChange::set(
            *fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        ));
        let mut position = CueChange::set(
            *fixture_id,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.8),
        );
        position.fade_millis = Some(3_000);
        lit.changes.push(position);
    }
    let cue_list = CueList {
        id: light_core::CueListId::new(),
        name: "MIB".into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![first, dark, lit],
    };
    let playback = PlaybackDefinition {
        number: 1,
        name: "MIB".into(),
        target: PlaybackTarget::CueList {
            cue_list_id: cue_list.id,
        },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    EngineSnapshot {
        fixtures,
        cue_lists: vec![cue_list],
        playbacks: vec![playback],
        playback_pages: vec![],
        routes: vec![],
        control_mappings: vec![],
        groups: vec![],
        revision: 1,
    }
}

fn normalized(
    values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
    fixture_id: FixtureId,
    attribute: &str,
) -> f32 {
    values[&(fixture_id, AttributeKey(attribute.into()))]
        .normalized()
        .unwrap()
}

mod profile_visualization;

mod patch_and_heads;

mod masters;

mod programmer_groups;

mod snapshot_groups;

mod move_in_black;

mod programmer_fades;

mod schema_v2;

mod highlight_masters;

mod highlight_looks;

mod lifecycle;
