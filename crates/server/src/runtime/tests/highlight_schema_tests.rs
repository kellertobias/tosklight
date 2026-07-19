fn schema_v2_direct_fixture() -> (light_fixture::PatchedFixture, Uuid, [Uuid; 2]) {
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Test".into();
    profile.name = "Semantic fixture".into();
    profile.short_name = "Semantic".into();
    let mode_id = profile.modes[0].id;
    let head_id = profile.modes[0].heads[0].id;
    let indexed_channel = Uuid::new_v4();
    let reset_channel = Uuid::new_v4();
    let action_id = Uuid::new_v4();
    profile.modes[0].splits[0].footprint = 2;
    profile.modes[0].channels = vec![
        light_fixture::FixtureChannel {
            id: indexed_channel,
            head_id,
            split: 1,
            attribute: light_core::AttributeKey("gobo.1".into()),
            resolution: light_fixture::ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 0,
            highlight_raw: 255,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: true,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: light_fixture::ChannelBehavior::Controlled,
            functions: vec![light_fixture::ChannelFunction {
                id: Uuid::new_v4(),
                name: "Dots".into(),
                dmx_from: 0,
                dmx_to: 127,
                attribute: light_core::AttributeKey("gobo.1".into()),
                priority: 100,
                behavior: light_fixture::ChannelFunctionBehavior::Indexed {
                    semantic_id: "gobo.dots".into(),
                    label: "Dots".into(),
                    raw_value: 93,
                },
            }],
        },
        light_fixture::FixtureChannel {
            id: reset_channel,
            head_id,
            split: 1,
            attribute: light_core::AttributeKey("control.reset".into()),
            resolution: light_fixture::ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 7,
            highlight_raw: 7,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: true,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: light_fixture::ChannelBehavior::Controlled,
            functions: vec![],
        },
    ];
    profile.modes[0].control_actions = vec![light_fixture::ControlAction {
        id: action_id,
        name: "Reset".into(),
        semantic: light_fixture::ControlActionSemantic::Reset,
        kind: light_fixture::ControlActionKind::Momentary,
        duration_millis: None,
        assignments: vec![
            light_fixture::ControlActionAssignment {
                channel_id: indexed_channel,
                active_raw: 201,
                inactive_raw: 0,
            },
            light_fixture::ControlActionAssignment {
                channel_id: reset_channel,
                active_raw: 255,
                inactive_raw: 7,
            },
        ],
    }];
    let definition = profile.resolved_definition(mode_id).unwrap();
    (
        light_fixture::PatchedFixture {
            fixture_id: light_core::FixtureId::new(),
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
            highlight_overrides: Default::default(),
        },
        action_id,
        [indexed_channel, reset_channel],
    )
}

fn highlight_test_fixtures() -> Vec<light_fixture::PatchedFixture> {
    let fixture = schema_v2_direct_fixture().0;
    (0..3)
        .map(|index| {
            let mut fixture = fixture.clone();
            fixture.fixture_id = light_core::FixtureId::new();
            fixture.fixture_number = Some(index + 1);
            fixture.name = format!("Highlight fixture {}", index + 1);
            fixture.address = Some(1 + index as u16 * 10);
            fixture
        })
        .collect()
}

#[tokio::test]
async fn patch_preview_highlight_is_default_off_scoped_and_released() {
    let (state, data_dir) = test_state();
    let fixtures = highlight_test_fixtures();
    let fixture_id = fixtures[0].fixture_id;
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let request = |active| {
        Request::put("/api/v1/patch-preview-highlight")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({
                    "active":active,
                    "fixture_ids":[fixture_id]
                }))
                .unwrap(),
            ))
            .unwrap()
    };

    let disabled = json(app.clone().oneshot(request(true)).await.unwrap()).await;
    assert_eq!(disabled["allowed"], false);
    assert!(state.engine.highlighted_fixtures().is_empty());

    state.configuration.write().patch_preview_highlight_dmx = true;
    let enabled = json(app.clone().oneshot(request(true)).await.unwrap()).await;
    assert_eq!(enabled["active"], true);
    assert_eq!(state.engine.highlighted_fixtures(), vec![fixture_id]);

    let released = json(app.oneshot(request(false)).await.unwrap()).await;
    assert_eq!(released["active"], false);
    assert!(state.engine.highlighted_fixtures().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

fn highlight_multi_head_fixture() -> (light_fixture::PatchedFixture, [light_core::FixtureId; 2]) {
    let (mut fixture, _, _) = schema_v2_direct_fixture();
    fixture.fixture_number = Some(1);
    fixture.name = "Two-cell Highlight fixture".into();
    let mut profile = *fixture.definition.profile_snapshot.take().unwrap();
    let mode_id = profile.modes[0].id;
    profile.modes[0].heads.extend([
        light_fixture::FixtureHead {
            id: Uuid::new_v4(),
            name: "Cell 1".into(),
            master_shared: false,
        },
        light_fixture::FixtureHead {
            id: Uuid::new_v4(),
            name: "Cell 2".into(),
            master_shared: false,
        },
    ]);
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture.logical_heads.clear();
    assert!(light_fixture::reconcile_logical_heads(&mut fixture));
    let children = ordered_child_ids(&fixture);
    (fixture, [children[0], children[1]])
}

#[test]
fn highlight_participation_uses_logical_fixture_identities_independent_of_patch() {
    let mut fixture = schema_v2_direct_fixture().0;
    let parent = fixture.fixture_id;
    let head = light_core::FixtureId::new();
    fixture.universe = None;
    fixture.address = None;
    fixture.logical_heads = vec![light_fixture::PatchedHead {
        profile_head_id: None,
        head_index: 1,
        fixture_id: head,
    }];
    fixture.multipatch = vec![
        light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "First physical copy".into(),
            universe: Some(2),
            address: Some(1),
            split_patches: vec![],
            location: Default::default(),
            rotation: Default::default(),
        },
        light_fixture::MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Visualizer-only copy".into(),
            universe: None,
            address: None,
            split_patches: vec![],
            location: Default::default(),
            rotation: Default::default(),
        },
    ];

    let summaries = highlight_fixture_summaries(&[fixture.clone(), fixture]);
    assert_eq!(
        summaries
            .iter()
            .map(|summary| summary.fixture_id)
            .collect::<Vec<_>>(),
        vec![parent, head],
        "the unpatched parent participates once, multipatch copies add no step identities, and a logical head may participate independently"
    );

    let registry = HighlightRegistry::default();
    let selection = light_programmer::ProgrammerSelection {
        selected: vec![head, parent, head, parent],
        expression: Some(light_programmer::SelectionExpression::Static),
        revision: 1,
        gesture_open: false,
    };
    let stepped = registry
        .action(
            Uuid::new_v4(),
            light_core::UserId::new(),
            None,
            HighlightAction::Next,
            &selection,
            &summaries,
            &HashMap::new(),
            false,
        )
        .unwrap();
    assert_eq!(
        stepped
            .state
            .remembered
            .iter()
            .map(|summary| summary.fixture_id)
            .collect::<Vec<_>>(),
        vec![head, parent],
        "overlapping or duplicate selections de-duplicate without changing their first authoritative order"
    );
}

fn enable_highlight_test_feedback(state: &AppState) {
    *state.active_show.write() = Some(ShowEntry {
        id: light_core::ShowId::new(),
        name: "Highlight feedback test".into(),
        path: state
            .data_dir
            .join("shows/highlight-feedback-test.show")
            .display()
            .to_string(),
        revision: 0,
        updated_at: chrono::Utc::now().to_rfc3339(),
        revision_copy: None,
    });
}

#[test]
fn schema_v2_direct_actions_are_channel_atomic_and_presets_are_opt_in_semantic_values() {
    let (fixture, action_id, channel_ids) = schema_v2_direct_fixture();
    let fixture_id = fixture.fixture_id;
    let snapshot = EngineSnapshot {
        fixtures: vec![fixture],
        ..EngineSnapshot::default()
    };
    let (assignments, duration, kind) =
        control_action_programmer_values(&snapshot, fixture_id, action_id, true).unwrap();
    assert_eq!(duration, None);
    assert_eq!(kind, light_fixture::ControlActionKind::Momentary);
    assert_eq!(assignments.len(), 2);
    assert_eq!(
        assignments
            .iter()
            .map(|(_, attribute, value)| (attribute.clone(), value.clone()))
            .collect::<HashMap<_, _>>(),
        HashMap::from([
            (
                light_fixture::FixtureMode::control_action_attribute(channel_ids[0]),
                light_core::AttributeValue::RawDmxExact(201),
            ),
            (
                light_fixture::FixtureMode::control_action_attribute(channel_ids[1]),
                light_core::AttributeValue::RawDmxExact(255),
            ),
        ])
    );

    let mut timed_snapshot = snapshot.clone();
    let timed_action = &mut timed_snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_mut()
        .unwrap()
        .modes[0]
        .control_actions[0];
    timed_action.kind = light_fixture::ControlActionKind::TimedPulse;
    timed_action.duration_millis = Some(750);
    assert_eq!(
        control_action_programmer_values(&timed_snapshot, fixture_id, action_id, true)
            .unwrap()
            .1,
        Some(750)
    );

    let generated = generated_profile_presets(&snapshot, &HashSet::from([fixture_id])).unwrap();
    assert_eq!(generated.len(), 1);
    assert_eq!(generated[0].semantic_id, "gobo.dots");
    assert_eq!(generated[0].family, "Beam");
    assert_eq!(
        generated[0].values[&fixture_id][&light_core::AttributeKey("gobo.1".into())],
        light_core::AttributeValue::Discrete("gobo.dots".into())
    );
}
