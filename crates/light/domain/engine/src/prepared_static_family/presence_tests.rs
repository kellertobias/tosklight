use super::*;

fn values() -> [(ProgrammingOwner, AttributeValue); 4] {
    [
        (
            ProgrammingOwner::Position,
            AttributeValue::Position(Arc::new(PositionIntent::angles(10., 20.))),
        ),
        (
            ProgrammingOwner::Color,
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: Default::default(),
            })),
        ),
        (
            ProgrammingOwner::Zoom,
            AttributeValue::Zoom(Arc::new(ZoomIntent {
                opening_degrees: ScalarIntent::Value(30.),
                convention: light_core::OpeningConvention::Beam,
            })),
        ),
        (ProgrammingOwner::Focus, AttributeValue::Normalized(0.5)),
    ]
}

fn frame() -> (Engine, PreparedStaticFamilyFrame, FixtureId) {
    let target = FixtureId::new();
    let engine = Engine::new(Default::default());
    engine
        .replace_snapshot(crate::EngineSnapshot {
            fixtures: vec![crate::frame_slots::legacy_test_fixture(
                target,
                &["position", "color", "zoom", "focus"],
            )]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let capture = engine.prepare_output_frame(Default::default());
    let frame = engine.prepare_static_family_frame(&capture, &[]);
    (engine, frame, target)
}

// The original per-target exact-key/type queries, independently of the new dense walk.
fn oracle(frame: &PreparedStaticFamilyFrame, targets: &[FixtureId]) -> StaticFamilyPresence {
    StaticFamilyPresence {
        position: targets.iter().any(|&t| {
            matches!(
                frame.value(t, ProgrammingOwner::Position.key_ref()),
                Some(AttributeValue::Position(_))
            )
        }),
        color: targets.iter().any(|&t| {
            matches!(
                frame.value(t, ProgrammingOwner::Color.key_ref()),
                Some(AttributeValue::ColorProgram(_))
            )
        }),
        zoom: targets.iter().any(|&t| {
            matches!(
                frame.value(t, ProgrammingOwner::Zoom.key_ref()),
                Some(AttributeValue::Zoom(_))
            )
        }),
        focus: targets.iter().any(|&t| {
            matches!(
                frame.value(t, ProgrammingOwner::Focus.key_ref()),
                Some(AttributeValue::Normalized(_))
            )
        }),
    }
}

#[test]
fn exact_current_dense_and_overflow_winners_match_old_lookup() {
    for overflow in [false, true] {
        let (_, mut frame, dense) = frame();
        let target = if overflow { FixtureId::new() } else { dense };
        let slots = Arc::new(crate::SlotTable::compile(
            1,
            &[crate::frame_slots::legacy_test_fixture(
                dense,
                &["position", "color", "zoom", "focus"],
            )],
        ));
        let mut resolver = crate::contribution::EngineContributionResolver::unpooled(&slots);
        let rows = values()
            .into_iter()
            .map(|(owner, _)| owner.key())
            .chain([AttributeKey("unrelated".into())])
            .map(|attribute| {
                crate::contribution::EngineContribution::unscaled(light_core::TimedValue {
                    fixture_id: target,
                    attribute,
                    value: AttributeValue::Discrete("seed".into()),
                    priority: 0,
                    changed_at: Utc::now(),
                    programmer_order: 0,
                    merge_mode: light_core::MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            });
        resolver.extend(rows);
        frame.resolved = resolver.finish();
        assert_eq!(
            frame.static_family_presence(),
            StaticFamilyPresence::default()
        );
        for (owner, value) in values() {
            frame
                .resolved
                .override_value(target, owner.key_ref(), value, None);
            assert_eq!(
                frame.static_family_presence(),
                oracle(&frame, &[dense, target])
            );
        }
        assert_eq!(
            frame.static_family_presence(),
            StaticFamilyPresence {
                position: true,
                color: true,
                zoom: true,
                focus: true
            }
        );
        // Same typed values at unrelated keys and wrong values at owner keys must not count.
        for (owner, value) in values() {
            frame
                .resolved
                .override_value(target, &AttributeKey("unrelated".into()), value, None);
            frame.resolved.override_value(
                target,
                owner.key_ref(),
                AttributeValue::Discrete("wrong".into()),
                None,
            );
            assert_eq!(
                frame.static_family_presence(),
                oracle(&frame, &[dense, target])
            );
        }
        assert_eq!(
            frame.static_family_presence(),
            StaticFamilyPresence::default()
        );
    }
}

#[test]
fn fresh_epochs_missing_dense_frame_and_old_held_frame_are_independent() {
    let (engine, mut held, target) = frame();
    for (owner, value) in values() {
        held.resolved
            .override_value(target, owner.key_ref(), value, None);
    }
    let held_presence = held.static_family_presence();
    for _ in 0..8 {
        let capture = engine.prepare_output_frame(Default::default());
        let fresh = engine.prepare_static_family_frame(&capture, &[]);
        assert_eq!(
            fresh.static_family_presence(),
            StaticFamilyPresence::default()
        );
        assert_eq!(held.static_family_presence(), held_presence);
    }
    held.resolved.frame = None;
    assert_eq!(
        held.static_family_presence(),
        StaticFamilyPresence::default()
    );
}

/// TL-842: the numbered reader must answer exactly what the per-call named lookup answers,
/// for dense slots, overflow pairs, names the show never numbered, foreign targets and a
/// missing frame, returning the very same winner.
#[test]
fn numbered_attribute_reader_matches_named_lookup() {
    for overflow in [false, true] {
        let (_, mut frame, dense) = frame();
        let target = if overflow { FixtureId::new() } else { dense };
        let slots = Arc::new(crate::SlotTable::compile(
            1,
            &[crate::frame_slots::legacy_test_fixture(
                dense,
                &["position", "color", "zoom", "focus"],
            )],
        ));
        let mut resolver = crate::contribution::EngineContributionResolver::unpooled(&slots);
        resolver.extend(
            values()
                .into_iter()
                .map(|(owner, _)| owner.key())
                .chain([AttributeKey("unrelated".into())])
                .map(|attribute| {
                    crate::contribution::EngineContribution::unscaled(light_core::TimedValue {
                        fixture_id: target,
                        attribute,
                        value: AttributeValue::Discrete("seed".into()),
                        priority: 0,
                        changed_at: Utc::now(),
                        programmer_order: 0,
                        merge_mode: light_core::MergeMode::Ltp,
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    })
                }),
        );
        frame.resolved = resolver.finish();
        for (owner, value) in values() {
            frame
                .resolved
                .override_value(target, owner.key_ref(), value, None);
        }
        let attributes = values()
            .into_iter()
            .map(|(owner, _)| owner.key())
            .chain([
                AttributeKey("unrelated".into()),
                AttributeKey("never-named".into()),
            ])
            .collect::<Vec<_>>();
        let mut answered = 0;
        for attribute in &attributes {
            let reader = frame.attribute_values(attribute);
            for candidate in [dense, target, FixtureId::new()] {
                let (numbered, named) =
                    (reader.value(candidate), frame.value(candidate, attribute));
                assert_eq!(
                    numbered.map(|v| v as *const _),
                    named.map(|v| v as *const _)
                );
                answered += usize::from(named.is_some());
            }
        }
        assert!(answered >= 5, "owners and the unrelated row must be found");
        frame.resolved.frame = None;
        assert!(
            frame
                .attribute_values(&attributes[0])
                .value(target)
                .is_none()
        );
    }
}
