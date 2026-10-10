//! Static native Color cues: no Dynamic mask or injected DMX substitutes for the playback.
use super::*;
use crate::{
    EnginePlaybackCommand, NativeColorSourceCatalog, NativeColorSourceRevisionKey,
    VirtualPlaybackAction,
};
use light_core::{NativeColorValue, programming::NativeColorObservation};
use light_playback::{PlaybackPage, VirtualPlaybackAddress};

#[path = "physical_start.rs"]
mod physical_start;

fn rig() -> (
    Engine,
    FixtureId,
    Arc<ManualClock>,
    AttributeValue,
    AttributeValue,
) {
    let fixture = wash(true);
    let target = fixture.fixture_id;
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    let catalogue = Arc::new(
        NativeColorSourceCatalog::from_revisions([NativeColorSourceCatalog::compile_revision(
            NativeColorSourceRevisionKey {
                profile_id: profile.id,
                revision: u64::from(profile.revision),
                raw_store_digest: "solo-native-test-original".into(),
            },
            None,
            || Ok(profile.clone()),
        )])
        .unwrap(),
    );
    let mode = &profile.modes[0];
    let source = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    let recipe = |rgb: [u32; 3]| {
        let observation = NativeColorObservation {
            source: source.clone(),
            values: rgb
                .into_iter()
                .enumerate()
                .map(|(index, raw)| {
                    let channel = &mode.channels[RED + index];
                    NativeColorValue {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                        raw,
                    }
                })
                .collect(),
        };
        AttributeValue::ColorProgram(Arc::new(
            catalogue
                .capture_direct(observation)
                .unwrap()
                .program()
                .clone(),
        ))
    };
    let white = recipe([255, 255, 255]);
    let red = recipe([255, 0, 0]);
    let mut lists = Vec::new();
    let virtual_playbacks = [(1001, white.clone()), (1002, red.clone())]
        .into_iter()
        .map(|(number, color)| {
            let mut cue = Cue::new(1_u16.into());
            cue.changes = vec![
                CueChange::set(target, ProgrammingOwner::Color.key(), color),
                CueChange::set(
                    target,
                    AttributeKey("intensity".into()),
                    AttributeValue::Normalized(1.),
                ),
            ];
            let mut list = test_cue_list("Native Solo", vec![]);
            list.cues = vec![cue];
            let mut definition = test_playback(number, list.id);
            definition.auto_off = false;
            lists.push(list);
            (number, definition)
        })
        .collect();
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    registry.start(SessionId::new());
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: lists.into(),
            playback_pages: vec![PlaybackPage {
                number: 1,
                name: "Solo".into(),
                slots: HashMap::new(),
                virtual_playbacks,
            }]
            .into(),
            native_color_sources: catalogue,
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, target, clock, white, red)
}

fn operate(engine: &Engine, number: u16, action: VirtualPlaybackAction, solo: bool) {
    engine
        .execute_playback(EnginePlaybackCommand::Virtual {
            address: VirtualPlaybackAddress::new(1, number).unwrap(),
            action,
            exclusion_zones: if solo {
                vec![vec![
                    VirtualPlaybackAddress::new(1, 1001).unwrap(),
                    VirtualPlaybackAddress::new(1, 1002).unwrap(),
                ]]
            } else {
                vec![]
            },
            activation_origin: None,
        })
        .unwrap();
}

fn channels(engine: &Engine, target: FixtureId) -> Vec<u32> {
    let Some(AttributeValue::ColorProgram(program)) =
        value(engine, target, ProgrammingOwner::Color)
    else {
        panic!("native Color must never disappear during Solo handover")
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        panic!("native source identity must survive the fade")
    };
    let snapshot = engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    profile.modes[0].channels[RED..RED + 3]
        .iter()
        .map(|channel| {
            recipe
                .channels
                .iter()
                .find(|value| value.channel_id == channel.id)
                .unwrap()
                .raw
        })
        .collect()
}

#[test]
fn solo_native_color_crosses_from_live_white_without_blackout() {
    let (engine, target, clock, _, _) = rig();
    operate(&engine, 1001, VirtualPlaybackAction::On, false);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
    clock.advance_millis(3000);
    engine.set_control_timing([120.; 5], 0, 2000, 2000);
    operate(&engine, 1002, VirtualPlaybackAction::On, true);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert_eq!(halfway[0], 255);
    assert!(
        halfway[1].abs_diff(128) <= 1 && halfway[2].abs_diff(128) <= 1,
        "{halfway:?}"
    );
    clock.advance_millis(1000);
    assert_eq!(channels(&engine, target), vec![255, 0, 0]);
    operate(&engine, 1002, VirtualPlaybackAction::Off, false);
    assert!(
        value(&engine, target, ProgrammingOwner::Color).is_none(),
        "ordinary Off stays immediate"
    );
}

#[test]
fn solo_interruption_starts_at_current_native_recipe() {
    let (engine, target, clock, _, _) = rig();
    operate(&engine, 1001, VirtualPlaybackAction::On, false);
    clock.advance_millis(3000);
    engine.set_control_timing([120.; 5], 0, 2000, 2000);
    operate(&engine, 1002, VirtualPlaybackAction::On, true);
    clock.advance_millis(1000);
    let before = channels(&engine, target);
    operate(&engine, 1001, VirtualPlaybackAction::On, true);
    assert_eq!(channels(&engine, target), before);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert!(halfway[1].abs_diff(192) <= 1, "{halfway:?}");
    clock.advance_millis(1000);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
}

#[test]
fn static_same_source_direct_cue_fades_continuous_channels() {
    let (engine, target, clock, _, red) = rig();
    let mut snapshot = (*engine.snapshot()).clone();
    let lists = Arc::make_mut(&mut snapshot.cue_lists);
    let mut second = Cue::new(2_u16.into());
    second.fade_millis = 2000;
    second.changes = vec![CueChange::set(target, ProgrammingOwner::Color.key(), red)];
    lists[0].cues.push(second);
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    operate(&engine, 1001, VirtualPlaybackAction::Go, false);
    assert_eq!(channels(&engine, target), vec![255, 255, 255]);
    operate(&engine, 1001, VirtualPlaybackAction::Go, false);
    clock.advance_millis(1000);
    let halfway = channels(&engine, target);
    assert_eq!(halfway[0], 255);
    assert!(
        halfway[1].abs_diff(128) <= 1 && halfway[2].abs_diff(128) <= 1,
        "{halfway:?}"
    );
    clock.advance_millis(1000);
    assert_eq!(channels(&engine, target), vec![255, 0, 0]);
}
