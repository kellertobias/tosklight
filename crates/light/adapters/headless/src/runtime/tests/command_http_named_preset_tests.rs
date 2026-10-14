//! TL-771: named AT Preset dispatch through the real command route and guarded executor.
use super::*;
use crate::runtime::output_scheduler::physical_adapters::color::profiles::{patched, rgbw};

async fn named_preset_rig(
    ids: [light_core::FixtureId; 2],
    instance: uuid::Uuid,
) -> (CommandHttpScenario, [light_core::FixtureId; 2], uuid::Uuid) {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Named Presets").await;
    let path = PathBuf::from(scenario.state.active_show.current().unwrap().path);
    let profile = rgbw();
    let fixtures = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut fixture = patched(&profile, *id, 1 + index as u16 * 20);
            fixture.fixture_number = Some(index as u32 + 1);
            fixture
        })
        .collect::<Vec<_>>();
    let group = light_programmer::GroupDefinition {
        id: "1".into(),
        name: "Colours".into(),
        fixtures: vec![ids[0]],
        ..Default::default()
    };
    let store = ActiveShowRepository::open(&path).unwrap();
    let revision = store
        .objects("group")
        .unwrap()
        .into_iter()
        .find(|o| o.id == "1")
        .map(|o| o.revision)
        .unwrap_or(0);
    store
        .put_object(
            "group",
            "1",
            &serde_json::to_value(&group).unwrap(),
            revision,
        )
        .unwrap();
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            groups: vec![group].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let color = light_core::AttributeValue::ColorProgram(Arc::new(
        light_core::programming::ColorProgram::Semantic {
            intent: Default::default(),
        },
    ));
    let preset = light_programmer::Preset {
        instance_id: Some(instance),
        name: "White".into(),
        family: light_programmer::PresetFamily::Color,
        number: 1,
        values: ids
            .iter()
            .map(|id| {
                (
                    *id,
                    HashMap::from([(light_core::AttributeKey("color".into()), color.clone())]),
                )
            })
            .collect(),
        group_values: Default::default(),
        universal_values: Default::default(),
        aim_at_fixture_number: None,
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
    };
    store
        .put_object("preset", "2.1", &serde_json::to_value(preset).unwrap(), 0)
        .unwrap();
    (scenario, ids, instance)
}

#[tokio::test]
async fn named_preset_recall_matches_numeric_alias_in_normal_and_preload() {
    for preload in [false, true] {
        for prefix in ["GROUP 1", "DEGRP 1", "FIXTURE 1", "GROUP 1 + FIXTURE 2", ""] {
            let ids = [light_core::FixtureId::new(), light_core::FixtureId::new()];
            let instance = uuid::Uuid::new_v4();
            let mut results = Vec::new();
            for (index, address) in ["2.1", "COLOR PRESET 1"].iter().enumerate() {
                let (scenario, _, _) = named_preset_rig(ids, instance).await;
                scenario.state.programming.set_modes(
                    scenario.session.id,
                    Some(preload),
                    None,
                    None,
                    None,
                );
                scenario
                    .state
                    .programming
                    .arm_preload(scenario.session.id, preload);
                if prefix.is_empty() {
                    scenario
                        .state
                        .programming
                        .select(scenario.session.id, [ids[0]]);
                }
                let command = format!("{prefix} AT {address}");
                let response = scenario
                    .execute(&format!("recall-{index}"), Some(&command))
                    .await;
                let status = response.status();
                let body = json(response).await;
                assert_eq!(status, StatusCode::OK, "{command}: {body}");
                assert_eq!(
                    body["outcome"], "accepted",
                    "{command} preload={preload}: {body}"
                );
                let state = scenario.state.programming.get(scenario.session.id).unwrap();
                let values = if preload {
                    &state.preload_pending
                } else {
                    state.values.as_ref()
                };
                let groups = if preload {
                    &state.preload_group_pending
                } else {
                    &state.group_values
                };
                assert!(!values.is_empty() || !groups.is_empty(), "{command}");
                assert!(
                    state
                        .preset_provenance
                        .values()
                        .any(|origin| origin.preset_instance_id == instance)
                );
                if preload {
                    assert!(state.values.is_empty() && state.group_values.is_empty());
                }
                results.push((
                    state.selected,
                    state.selection_expression,
                    values
                        .iter()
                        .map(|v| (v.fixture_id, v.attribute.clone(), v.value.clone()))
                        .collect::<Vec<_>>(),
                    groups
                        .iter()
                        .map(|(id, values)| {
                            (
                                id.clone(),
                                values
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.value.clone()))
                                    .collect::<HashMap<_, _>>(),
                            )
                        })
                        .collect::<HashMap<_, _>>(),
                ));
                let _ = std::fs::remove_dir_all(scenario.data_dir);
            }
            assert_eq!(results[0], results[1], "{prefix}, preload={preload}");
        }
    }
}

#[tokio::test]
async fn named_preset_missing_and_inapplicable_preserve_selection_and_values() {
    let (scenario, ids, _) = named_preset_rig(
        [light_core::FixtureId::new(), light_core::FixtureId::new()],
        uuid::Uuid::new_v4(),
    )
    .await;
    for preload in [false, true] {
        scenario
            .state
            .programming
            .set_modes(scenario.session.id, Some(preload), None, None, None);
        scenario
            .state
            .programming
            .arm_preload(scenario.session.id, preload);
        scenario
            .state
            .programming
            .select(scenario.session.id, [ids[1]]);
        for prefix in ["GROUP 1", "DEGRP 1", "FIXTURE 1", "GROUP 1 + FIXTURE 2", ""] {
            let before =
                serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap())
                    .unwrap();
            let error = execute_programmer_command(
                &scenario.state,
                &scenario.session,
                &format!("{prefix} AT COLOR PRESET 999"),
            )
            .unwrap_err();
            assert!(
                error.contains("preset 2.999 does not exist"),
                "{prefix}: {error}"
            );
            assert_eq!(
                serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap())
                    .unwrap(),
                before
            );
        }
        // A valid preset with no applicable values is a no-op, even for explicit mixed targets.
        let store =
            ActiveShowRepository::open(scenario.state.active_show.current().unwrap().path).unwrap();
        let object = store
            .objects("preset")
            .unwrap()
            .into_iter()
            .find(|o| o.id == "2.1")
            .unwrap();
        let mut preset: light_programmer::Preset = serde_json::from_value(object.body).unwrap();
        preset.values.clear();
        store
            .put_object(
                "preset",
                "2.1",
                &serde_json::to_value(preset).unwrap(),
                object.revision,
            )
            .unwrap();
        for prefix in ["GROUP 1", "DEGRP 1", "FIXTURE 1", "GROUP 1 + FIXTURE 2", ""] {
            let before =
                serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap())
                    .unwrap();
            assert_eq!(
                execute_programmer_command(
                    &scenario.state,
                    &scenario.session,
                    &format!("{prefix} AT COLOR PRESET 1")
                )
                .unwrap(),
                0
            );
            assert_eq!(
                serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap())
                    .unwrap(),
                before
            );
        }
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
