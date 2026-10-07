use super::*;
use light_core::programming::*;

fn apply(
    registry: &ProgrammerRegistry,
    session: SessionId,
    preload: bool,
    fixture: FixtureId,
    entries: &[(AttributeKey, AttributeValue)],
    preset: bool,
) {
    if preload {
        let mutations = entries
            .iter()
            .flat_map(|(attribute, value)| {
                [
                    PreloadProgrammerValueMutation::SetFixture {
                        fixture_id: fixture,
                        attribute: attribute.clone(),
                        value: value.clone(),
                        timing: Default::default(),
                    },
                    PreloadProgrammerValueMutation::SetGroup {
                        group_id: "front".into(),
                        attribute: attribute.clone(),
                        value: value.clone(),
                        timing: Default::default(),
                    },
                ]
            })
            .collect::<Vec<_>>();
        registry.apply_preload_values(session, &mutations);
    } else {
        let mutations = entries
            .iter()
            .flat_map(|(attribute, value)| {
                [
                    NormalProgrammerValueMutation::SetFixture {
                        fixture_id: fixture,
                        attribute: attribute.clone(),
                        value: value.clone(),
                        timing: Default::default(),
                    },
                    NormalProgrammerValueMutation::SetGroup {
                        group_id: "front".into(),
                        attribute: attribute.clone(),
                        value: value.clone(),
                        timing: Default::default(),
                    },
                ]
            })
            .collect::<Vec<_>>();
        if preset {
            registry.apply_normal_preset_recall(session, &mutations, "preset".into());
        } else {
            registry.apply_normal_values(session, &mutations);
        }
    }
}

#[test]
fn complete_families_replace_independent_values_in_normal_preload_and_preset_recall_with_one_undo()
{
    for (preload, preset) in [(false, false), (true, false), (false, true)] {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let fixture = FixtureId::new();
        registry.start(session);
        if preload {
            registry.arm_preload(session, true);
        }
        let initial = [
            "pan",
            "tilt",
            "color.red",
            "color.green",
            "color.uv",
            "color.wheel.1",
            "focus",
            "zoom",
            "intensity",
            "gobo.wheel.1",
        ]
        .into_iter()
        .map(|key| (AttributeKey(key.into()), AttributeValue::Normalized(0.4)))
        .collect::<Vec<_>>();
        apply(&registry, session, preload, fixture, &initial, false);
        let before = registry.get(session).unwrap();
        let complete = vec![
            (
                ProgrammingOwner::Color.key(),
                AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                    intent: ColorIntent {
                        uv: UvIntent { amount: 0.7 },
                        ..Default::default()
                    },
                })),
            ),
            (
                ProgrammingOwner::Position.key(),
                AttributeValue::Position(Arc::new(PositionIntent::target(
                    TargetReference::Origin,
                    [1.0, 2.0, 3.0],
                ))),
            ),
            (
                ProgrammingOwner::Zoom.key(),
                AttributeValue::Zoom(Arc::new(ZoomIntent {
                    opening_degrees: ScalarIntent::Value(30.0),
                    convention: light_core::OpeningConvention::Beam,
                })),
            ),
        ];
        apply(&registry, session, preload, fixture, &complete, preset);
        let after = registry.get(session).unwrap();
        assert_eq!(after.undo.len(), before.undo.len() + 1);
        let values = if preload {
            &after.preload_pending
        } else {
            after.values.as_ref()
        };
        let groups = if preload {
            &after.preload_group_pending
        } else {
            &after.group_values
        };
        let fixture_values = values
            .iter()
            .map(|v| (v.attribute.clone(), v.value.clone()))
            .collect::<HashMap<_, _>>();
        let group_values = groups["front"]
            .iter()
            .map(|(key, v)| (key.clone(), v.value.clone()))
            .collect::<HashMap<_, _>>();
        let expected = initial
            .iter()
            .filter(|(key, _)| ["focus", "intensity", "gobo.wheel.1"].contains(&key.0.as_ref()))
            .cloned()
            .chain(complete)
            .collect::<HashMap<_, _>>();
        assert_eq!(fixture_values, expected);
        assert_eq!(group_values, expected);
        assert!(registry.undo(session));
        let restored = registry.get(session).unwrap();
        assert_eq!(restored.values, before.values);
        assert_eq!(restored.group_values, before.group_values);
        assert_eq!(restored.preload_pending, before.preload_pending);
        assert_eq!(restored.preload_group_pending, before.preload_group_pending);
    }
}
