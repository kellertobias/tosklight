use super::*;
use chrono::TimeZone;

struct CaptureScenario {
    registry: ProgrammerRegistry,
    session: SessionId,
    fixtures: Vec<FixtureId>,
}

impl CaptureScenario {
    fn new() -> Self {
        let captured_at = Utc.with_ymd_and_hms(2026, 7, 20, 9, 30, 0).unwrap();
        let clock = Arc::new(ManualClock::new(captured_at));
        let registry = ProgrammerRegistry::with_clock(clock);
        let session = SessionId::new();
        registry.start(session, UserId::new());
        Self {
            registry,
            session,
            fixtures: (0..4).map(|_| FixtureId::new()).collect(),
        }
    }

    fn capture(&self) -> GroupRecordingCapture {
        self.registry
            .capture_group_recording_selection(self.session)
            .unwrap()
    }
}

#[test]
fn overwrite_preserves_existing_presentation_and_portable_programming() {
    let scenario = CaptureScenario::new();
    scenario.registry.select(
        scenario.session,
        [scenario.fixtures[2], scenario.fixtures[0]],
    );
    scenario.registry.set(
        scenario.session,
        scenario.fixtures[0],
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
    );
    let existing = GroupDefinition {
        id: "opaque target".into(),
        name: "Front wash".into(),
        color: Some("#102030".into()),
        icon: Some("star".into()),
        fixtures: scenario.fixtures[..2].to_vec(),
        programming: HashMap::from([(
            AttributeKey("future.portable".into()),
            AttributeValue::Normalized(0.3),
        )]),
        frozen_from: Some(FrozenGroup {
            source_group_id: "former-source".into(),
            source_revision: 4,
            captured_at: Utc::now(),
        }),
        ..Default::default()
    };
    let groups = HashMap::from([("opaque target".into(), existing.clone())]);

    let recorded = scenario
        .capture()
        .overwrite("opaque target", Some(&existing), &groups);

    assert_eq!(recorded.name, "Front wash");
    assert_eq!(recorded.color.as_deref(), Some("#102030"));
    assert_eq!(recorded.icon.as_deref(), Some("star"));
    assert_eq!(recorded.programming, existing.programming);
    assert_eq!(
        recorded.fixtures,
        vec![scenario.fixtures[2], scenario.fixtures[0]]
    );
    assert_eq!(
        recorded.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![scenario.fixtures[2], scenario.fixtures[0]],
        })
    );
    assert!(recorded.derived_from.is_none());
    assert!(recorded.frozen_from.is_none());
}

#[test]
fn new_empty_group_uses_defaults_and_remains_distinct_from_absence() {
    let scenario = CaptureScenario::new();
    scenario.registry.select(scenario.session, []);
    let recorded = scenario.capture().overwrite("A.01", None, &HashMap::new());

    assert_eq!(recorded.id, "A.01");
    assert_eq!(recorded.name, "Group A.01");
    assert!(recorded.fixtures.is_empty());
    assert_eq!(
        recorded.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: Vec::new(),
        })
    );
    assert!(recorded.programming.is_empty());
}

#[test]
fn materialized_overwrite_keeps_first_fixture_occurrence_in_operator_order() {
    let scenario = CaptureScenario::new();
    scenario.registry.select_expression(
        scenario.session,
        vec![
            scenario.fixtures[2],
            scenario.fixtures[0],
            scenario.fixtures[2],
            scenario.fixtures[1],
        ],
        SelectionExpression::Static,
    );

    let recorded = scenario
        .capture()
        .overwrite("ordered", None, &HashMap::new());

    assert_eq!(
        recorded.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![
                scenario.fixtures[2],
                scenario.fixtures[0],
                scenario.fixtures[1],
            ],
        })
    );
}

#[test]
fn live_and_single_gesture_sources_retain_relationships_and_div_rule() {
    let scenario = CaptureScenario::new();
    let source = GroupDefinition {
        id: "source".into(),
        fixtures: scenario.fixtures.clone(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: scenario.fixtures.clone(),
        }),
        ..Default::default()
    };
    let groups = HashMap::from([("source".into(), source)]);
    let rule = SelectionRule::EveryNth { n: 3, offset: 1 };
    scenario.registry.select_expression(
        scenario.session,
        vec![scenario.fixtures[1]],
        SelectionExpression::LiveGroup {
            group_id: "source".into(),
            rule: rule.clone(),
        },
    );
    let divided = scenario.capture().overwrite("derived", None, &groups);
    assert_eq!(
        divided.source,
        Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: "source".into(),
                rule,
            }],
        })
    );
    assert!(divided.derived_from.is_none());

    scenario.registry.select_expression(
        scenario.session,
        scenario.fixtures.clone(),
        SelectionExpression::Sources {
            items: vec![SelectionReference::LiveGroup {
                group_id: "source".into(),
            }],
        },
    );
    let normalized = scenario.capture().overwrite("gesture", None, &groups);
    assert_eq!(
        normalized.source,
        Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: "source".into(),
                rule: SelectionRule::All,
            }],
        })
    );
}

#[test]
fn dereferenced_selection_records_concrete_fixtures_without_source_reference() {
    let scenario = CaptureScenario::new();
    scenario.registry.select_expression(
        scenario.session,
        scenario.fixtures[..2].to_vec(),
        SelectionExpression::Sources {
            items: scenario.fixtures[..2]
                .iter()
                .map(|fixture_id| SelectionReference::Fixture {
                    fixture_id: *fixture_id,
                })
                .collect(),
        },
    );

    let recorded = scenario
        .capture()
        .overwrite("frozen", None, &HashMap::new());
    assert!(recorded.frozen_from.is_none());
    assert!(recorded.derived_from.is_none());
    assert_eq!(recorded.fixtures, scenario.fixtures[..2]);
    assert_eq!(
        recorded.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: scenario.fixtures[..2].to_vec(),
        })
    );
}

#[test]
fn mixed_remove_dereferenced_self_and_transitive_cycles_materialize() {
    let scenario = CaptureScenario::new();
    let source = GroupDefinition {
        id: "source".into(),
        fixtures: scenario.fixtures.clone(),
        ..Default::default()
    };
    let target = GroupDefinition {
        id: "target".into(),
        source: Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: "middle".into(),
                rule: SelectionRule::All,
            }],
        }),
        ..Default::default()
    };
    let middle = GroupDefinition {
        id: "middle".into(),
        source: Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: "target".into(),
                rule: SelectionRule::All,
            }],
        }),
        ..Default::default()
    };
    let groups = HashMap::from([
        ("source".into(), source),
        ("target".into(), target.clone()),
        ("middle".into(), middle),
    ]);

    for expression in [
        SelectionExpression::Sources {
            items: vec![
                SelectionReference::LiveGroup {
                    group_id: "source".into(),
                },
                SelectionReference::Fixture {
                    fixture_id: scenario.fixtures[0],
                },
            ],
        },
        SelectionExpression::Sources {
            items: vec![SelectionReference::RemoveLiveGroup {
                group_id: "source".into(),
            }],
        },
        SelectionExpression::Static,
    ] {
        scenario.registry.select_expression(
            scenario.session,
            scenario.fixtures[..2].to_vec(),
            expression,
        );
        let recorded = scenario.capture().overwrite("plain", None, &groups);
        assert!(recorded.derived_from.is_none());
        assert!(recorded.frozen_from.is_none());
        assert!(matches!(
            recorded.source,
            Some(GroupFixtureSource::Explicit { .. })
        ));
    }

    scenario.registry.select_expression(
        scenario.session,
        scenario.fixtures[..2].to_vec(),
        SelectionExpression::LiveGroup {
            group_id: "target".into(),
            rule: SelectionRule::All,
        },
    );
    assert!(matches!(
        scenario
            .capture()
            .overwrite("target", Some(&target), &groups)
            .source,
        Some(GroupFixtureSource::Explicit { .. })
    ));

    scenario.registry.select_expression(
        scenario.session,
        scenario.fixtures[..2].to_vec(),
        SelectionExpression::LiveGroup {
            group_id: "middle".into(),
            rule: SelectionRule::All,
        },
    );
    assert!(matches!(
        scenario
            .capture()
            .overwrite("target", Some(&target), &groups)
            .source,
        Some(GroupFixtureSource::Explicit { .. })
    ));
}

#[test]
fn merge_and_subtract_materialize_resolved_order_without_touching_metadata() {
    let scenario = CaptureScenario::new();
    let source = GroupDefinition {
        id: "source".into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![
                scenario.fixtures[0],
                scenario.fixtures[1],
                scenario.fixtures[0],
                scenario.fixtures[2],
            ],
        }),
        ..Default::default()
    };
    let derived = GroupDefinition {
        id: "derived".into(),
        name: "Derived".into(),
        source: Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: "source".into(),
                rule: SelectionRule::Odd,
            }],
        }),
        ..Default::default()
    };
    let groups = HashMap::from([
        ("source".into(), source),
        ("derived".into(), derived.clone()),
    ]);

    scenario.registry.select(
        scenario.session,
        [scenario.fixtures[2], scenario.fixtures[3]],
    );
    let merged = scenario
        .capture()
        .merge("derived", &derived, &groups)
        .unwrap();
    assert_eq!(
        merged.fixtures,
        vec![
            scenario.fixtures[0],
            scenario.fixtures[2],
            scenario.fixtures[3]
        ]
    );
    assert!(merged.derived_from.is_none());
    assert_eq!(
        merged.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![
                scenario.fixtures[0],
                scenario.fixtures[2],
                scenario.fixtures[3],
            ],
        })
    );
    assert_eq!(merged.name, "Derived");

    let materialized_groups = HashMap::from([("derived".into(), merged.clone())]);
    scenario.registry.select(
        scenario.session,
        [scenario.fixtures[0], scenario.fixtures[3]],
    );
    let subtracted = scenario
        .capture()
        .subtract("derived", &merged, &materialized_groups)
        .unwrap();
    assert_eq!(subtracted.fixtures, vec![scenario.fixtures[2]]);
    assert!(subtracted.derived_from.is_none());
    assert_eq!(
        subtracted.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: vec![scenario.fixtures[2]],
        })
    );
}

#[test]
fn recording_never_copies_selection_grid_and_preserves_existing_target_grid() {
    let scenario = CaptureScenario::new();
    scenario
        .registry
        .select(scenario.session, scenario.fixtures[..2].iter().copied());
    let selection_configuration = GridMethodConfiguration {
        method: GridMethod::RoomDepthAxisY,
        axis_origin: AxisOrigin {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        },
    };
    let revision = scenario
        .registry
        .selection(scenario.session)
        .unwrap()
        .revision;
    scenario
        .registry
        .set_selection_grid_configuration_if_revision(
            scenario.session,
            revision,
            selection_configuration,
        )
        .unwrap();

    let target_configuration = GridMethodConfiguration {
        method: GridMethod::BackToFront,
        ..Default::default()
    };
    let target = GroupDefinition {
        id: "target".into(),
        fixtures: scenario.fixtures[2..].to_vec(),
        grid: target_configuration,
        ..Default::default()
    };
    let groups = HashMap::from([("target".into(), target.clone())]);
    let capture = scenario.capture();

    assert_eq!(
        capture.overwrite("target", Some(&target), &groups).grid,
        target_configuration
    );
    assert_eq!(
        capture.overwrite("new", None, &groups).grid,
        GridMethodConfiguration::default()
    );
    assert_eq!(
        capture.merge("target", &target, &groups).unwrap().grid,
        target_configuration
    );
    assert_eq!(
        capture.subtract("target", &target, &groups).unwrap().grid,
        target_configuration
    );
}

#[test]
fn direct_derived_dependency_blocks_deletion_but_frozen_provenance_does_not() {
    let captured_at = Utc::now();
    let groups = HashMap::from([
        (
            "derived".into(),
            GroupDefinition {
                id: "derived".into(),
                source: Some(GroupFixtureSource::References {
                    references: vec![GroupReference {
                        group_id: "source".into(),
                        rule: SelectionRule::All,
                    }],
                }),
                ..Default::default()
            },
        ),
        (
            "a-dependent".into(),
            GroupDefinition {
                id: "a-dependent".into(),
                source: Some(GroupFixtureSource::References {
                    references: vec![GroupReference {
                        group_id: "source".into(),
                        rule: SelectionRule::Even,
                    }],
                }),
                ..Default::default()
            },
        ),
        (
            "frozen".into(),
            GroupDefinition {
                id: "frozen".into(),
                frozen_from: Some(FrozenGroup {
                    source_group_id: "other".into(),
                    source_revision: 3,
                    captured_at,
                }),
                ..Default::default()
            },
        ),
    ]);
    assert_eq!(group_delete_blocker("source", &groups), Some("a-dependent"));
    assert_eq!(group_delete_blocker("other", &groups), None);
}
