//! TL-889: Programmer edit ordering and output HTP are separate decisions.
use super::*;
use crate::{ContributionProgrammerLane as Lane, ContributionSourceDescriptor as Source};

struct Rig {
    engine: Engine,
    programmer: ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    lists: [light_core::CueListId; 2],
    clock: Arc<ManualClock>,
}

#[derive(Clone, Copy, Debug)]
enum Owner {
    Live,
    Group,
    Playback(usize),
    Default,
}

impl Rig {
    fn new() -> Self {
        Self::with_priorities([10, 10])
    }

    fn with_priorities(priorities: [i16; 2]) -> Self {
        let clock = Arc::new(ManualClock::new(
            Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap(),
        ));
        let programmer = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmer.start(session);
        let (patched, fixture) = fixture();
        let mut lists = [0.8, 0.6].map(|value| {
            test_cue_list(
                "HTP intensity",
                vec![CueChange::set(
                    fixture,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(value),
                )],
            )
        });
        for (list, priority) in lists.iter_mut().zip(priorities) {
            list.priority = priority;
        }
        let ids = [lists[0].id, lists[1].id];
        let retained_playback = |number, id| {
            let mut playback = test_playback(number, id);
            playback.auto_off = false;
            playback
        };
        let engine = Engine::new(programmer.clone());
        engine.set_control_timing([120.0; 5], 0, 0, 0);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![patched].into(),
                cue_lists: Vec::from(lists).into(),
                playback_pages: vec![playback_page(
                    1,
                    [
                        (1, retained_playback(1, ids[0])),
                        (2, retained_playback(2, ids[1])),
                    ],
                )]
                .into(),
                groups: vec![GroupDefinition {
                    id: "wash".into(),
                    name: "Wash".into(),
                    fixtures: vec![fixture],
                    ..Default::default()
                }]
                .into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        Self {
            engine,
            programmer,
            session,
            fixture,
            lists: ids,
            clock,
        }
    }

    fn playback(&self, index: usize, action: PlaybackAction) {
        self.clock.advance_millis(1);
        execute_address(&self.engine, playback_address(1, index as u16 + 1), action);
    }

    fn edit(&self, group: bool, value: f32) {
        self.clock.advance_millis(1);
        if group {
            self.programmer.set_group(
                self.session,
                "wash".into(),
                AttributeKey::intensity(),
                AttributeValue::Normalized(value),
            );
        } else {
            self.programmer.set(
                self.session,
                self.fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(value),
            );
        }
    }

    fn expect(&self, value: f32, owner: Owner) {
        let rendered = self.engine.render(Default::default()).unwrap();
        assert_eq!(
            rendered.universes[&1][0],
            (value * 255.0).round() as u8,
            "{owner:?}"
        );
        let frame = self.engine.observe_source_frame(&[]);
        let attribute = AttributeKey::intensity();
        let actual = frame
            .values()
            .value(self.fixture, &attribute)
            .and_then(AttributeValue::normalized)
            .unwrap_or(0.0);
        assert!(
            (actual - value).abs() < 1e-6,
            "{owner:?}: {actual} != {value}"
        );
        let source = frame
            .values()
            .contribution_origin(self.fixture, &attribute)
            .map(|origin| origin.source().descriptor());
        match owner {
            Owner::Live => assert!(
                matches!(
                    source,
                    Some(Source::Programmer {
                        lane: Lane::Live,
                        ..
                    })
                ),
                "{source:?}"
            ),
            Owner::Group => assert!(
                matches!(
                    source,
                    Some(Source::Programmer {
                        lane: Lane::Group("wash"),
                        ..
                    })
                ),
                "{source:?}"
            ),
            Owner::Playback(index) => assert!(
                matches!(source, Some(Source::Playback(source)) if source.cue_list_id == self.lists[index] && source.playback_address == Some(playback_address(1,index as u16+1))),
                "{source:?}"
            ),
            Owner::Default => assert!(source.is_none(), "{source:?}"),
        }
    }
    fn cue_priority(&self, index: usize, priority: i16) {
        let mut snapshot = (*self.engine.snapshot()).clone();
        let mut lists = snapshot.cue_lists.to_vec();
        lists[index].priority = priority;
        snapshot.cue_lists = lists.into();
        snapshot.revision += 1;
        self.engine.replace_snapshot(snapshot).unwrap();
    }
}

#[test]
fn lower_fixture_edit_wins_over_group_and_one_or_two_normal_htp_playbacks() {
    for two in [false, true] {
        let rig = Rig::new();
        rig.playback(0, PlaybackAction::Go);
        rig.expect(0.8, Owner::Playback(0));
        rig.edit(true, 0.8);
        rig.expect(0.8, Owner::Group); // same numeric output, different actual owner
        rig.edit(false, 0.3);
        rig.expect(0.3, Owner::Live);
        if two {
            rig.playback(1, PlaybackAction::Go);
        }
        rig.expect(0.3, Owner::Live); // later playback cannot bypass a higher priority
        rig.edit(false, 0.0);
        rig.expect(0.0, Owner::Live); // explicitly programmed zero is still owned
        rig.edit(true, 0.2);
        rig.expect(0.2, Owner::Group); // Group editing remains LTP too
        rig.edit(false, 0.1);
        rig.expect(0.1, Owner::Live);
        rig.programmer.clear_values(rig.session);
        rig.expect(0.8, Owner::Playback(0));
        rig.playback(0, PlaybackAction::Off);
        rig.expect(
            if two { 0.6 } else { 0.0 },
            if two {
                Owner::Playback(1)
            } else {
                Owner::Default
            },
        );
        if two {
            rig.playback(1, PlaybackAction::Off);
            rig.expect(0.0, Owner::Default);
        }
    }
}

#[test]
fn equal_priority_htp_can_keep_fixture_on_until_programmer_priority_is_raised() {
    let rig = Rig::new();
    rig.playback(0, PlaybackAction::Go);
    rig.playback(1, PlaybackAction::Go);
    rig.edit(true, 0.8);
    rig.edit(false, 0.3);
    rig.expect(0.3, Owner::Live);
    for (priority, value, owner) in [
        (10, 0.8, Owner::Playback(0)),
        (11, 0.3, Owner::Live),
        (9, 0.8, Owner::Playback(0)),
    ] {
        rig.programmer.set_priority(rig.session, priority);
        rig.expect(value, owner);
    }
    rig.programmer.set_priority(rig.session, 10);
    rig.edit(false, 0.8);
    rig.expect(0.8, Owner::Playback(0)); // equal HTP value does not transfer ownership
    rig.edit(false, 0.9);
    rig.expect(0.9, Owner::Live);
    rig.edit(false, 0.0);
    rig.expect(0.8, Owner::Playback(0));
    rig.programmer.clear_values(rig.session);
    rig.expect(0.8, Owner::Playback(0));
}

#[test]
fn declared_playback_priority_precedes_htp_level_and_newer_programmer_edits() {
    for priority in [20, 101] {
        let rig = Rig::with_priorities([10, priority]);
        rig.playback(0, PlaybackAction::Go);
        rig.playback(1, PlaybackAction::Go);
        rig.expect(0.6, Owner::Playback(1)); // priority before highest intensity
        rig.edit(false, 0.1);
        if priority < 100 {
            rig.expect(0.1, Owner::Live);
        } else {
            rig.expect(0.6, Owner::Playback(1));
        }
        rig.edit(false, 0.0);
        if priority < 100 {
            rig.expect(0.0, Owner::Live);
        } else {
            rig.expect(0.6, Owner::Playback(1));
        }
        rig.programmer.set_priority(rig.session, priority + 1);
        rig.expect(0.0, Owner::Live);
        rig.programmer.clear_values(rig.session);
        rig.expect(0.6, Owner::Playback(1));
        rig.playback(1, PlaybackAction::Off);
        rig.expect(0.8, Owner::Playback(0));
    }
}

#[test]
fn changing_running_playback_priority_reveals_the_correct_retained_owner() {
    let rig = Rig::new();
    rig.playback(0, PlaybackAction::Go);
    rig.playback(1, PlaybackAction::Go);
    rig.expect(0.8, Owner::Playback(0));
    rig.cue_priority(1, 20);
    rig.expect(0.6, Owner::Playback(1));
    rig.edit(false, 0.3);
    rig.expect(0.3, Owner::Live);
    rig.cue_priority(1, 101);
    rig.expect(0.6, Owner::Playback(1));
    rig.edit(false, 0.0);
    rig.expect(0.6, Owner::Playback(1));
    rig.cue_priority(1, 10);
    rig.expect(0.0, Owner::Live);
    rig.programmer.clear_values(rig.session);
    rig.expect(0.8, Owner::Playback(0));
}
