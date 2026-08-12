use super::*;

const GROUP_ID: &str = "wash";

fn color(x: f32, y: f32, z: f32) -> AttributeValue {
    AttributeValue::ColorXyz(Xyz { x, y, z })
}

fn group_color_engine(programmers: ProgrammerRegistry) -> (Engine, FixtureId) {
    let (fixture, fixture_id) = fixture();
    let cue_list = test_cue_list(
        "Color playback",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("color".into()),
            color(0.6, 0.2, 0.1),
        )],
    );
    let playback = test_playback(1, cue_list.id);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback, test_group_playback(2, GROUP_ID)].into(),
            groups: vec![GroupDefinition {
                id: GROUP_ID.into(),
                name: "Wash".into(),
                fixtures: vec![fixture_id],
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::On);
    (engine, fixture_id)
}

#[test]
fn group_color_is_structurally_above_playback_and_sampled_dynamics_below_programmer() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (engine, fixture_id) = group_color_engine(programmers.clone());
    let group_color = Xyz {
        x: 0.1,
        y: 0.3,
        z: 0.8,
    };
    assert!(engine.set_group_color(GROUP_ID, Some(group_color)).unwrap());

    let sampled_dynamic = ContributionBatch::new([ContributionSample::independent(TimedValue {
        fixture_id,
        attribute: AttributeKey("color".into()),
        value: color(0.2, 0.8, 0.2),
        priority: i16::MAX,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })]);
    assert_eq!(
        engine.resolved_values_with_contribution_batches(&[sampled_dynamic])
            [&(fixture_id, AttributeKey("color".into()))],
        AttributeValue::ColorXyz(group_color),
        "the structural Group layer wins over playback and sampled Dynamic priority"
    );

    let programmer_color = Xyz {
        x: 0.7,
        y: 0.6,
        z: 0.1,
    };
    programmers.set(
        session,
        fixture_id,
        AttributeKey("color".into()),
        AttributeValue::ColorXyz(programmer_color),
    );
    assert_eq!(
        engine.resolved_values()[&(fixture_id, AttributeKey("color".into()))],
        AttributeValue::ColorXyz(programmer_color),
        "every Programmer remains above the Group color layer"
    );
}

#[test]
fn group_color_release_and_master_zero_preserve_runtime_only_intent() {
    let (engine, fixture_id) = group_color_engine(ProgrammerRegistry::default());
    let group_color = Xyz {
        x: 0.2,
        y: 0.4,
        z: 0.9,
    };
    assert!(engine.set_group_color(GROUP_ID, Some(group_color)).unwrap());
    assert!(!engine.set_group_color(GROUP_ID, Some(group_color)).unwrap());
    assert!(engine.set_group_master(GROUP_ID, 0.0).unwrap());
    assert_eq!(engine.group_color(GROUP_ID), Some(group_color));
    assert_eq!(
        engine.resolved_values()[&(fixture_id, AttributeKey("color".into()))],
        AttributeValue::ColorXyz(group_color),
        "master zero suppresses intensity without silently releasing color intent"
    );

    assert!(engine.set_group_color(GROUP_ID, None).unwrap());
    assert_eq!(engine.group_color(GROUP_ID), None);
    assert_eq!(
        engine.resolved_values()[&(fixture_id, AttributeKey("color".into()))],
        color(0.6, 0.2, 0.1),
        "explicit release reveals the playback color below"
    );
    assert!(!engine.set_group_color(GROUP_ID, None).unwrap());
}

#[test]
fn unsupported_fixture_color_is_safe_and_deleted_groups_drop_runtime_color() {
    let (engine, _) = group_color_engine(ProgrammerRegistry::default());
    assert!(
        engine
            .set_group_color(
                GROUP_ID,
                Some(Xyz {
                    x: 0.3,
                    y: 0.4,
                    z: 0.5,
                }),
            )
            .unwrap()
    );
    engine.render(RenderOptions::default()).unwrap();

    engine.replace_snapshot(EngineSnapshot::default()).unwrap();
    assert_eq!(engine.group_color(GROUP_ID), None);
    assert!(engine.set_group_color(GROUP_ID, None).is_err());
}
