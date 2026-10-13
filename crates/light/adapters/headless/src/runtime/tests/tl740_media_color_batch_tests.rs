//! TL-740: the Media picker must use a whole current-contract ColorProgram, not RGB channels.
use super::*;
use crate::runtime::command_http::ToIntentWire;
use crate::runtime::output_scheduler::physical_adapters::{
    color::tests::{intent, program},
    media_color::tests::{media_fixture, shipped_media_server},
};
use light_core::{AttributeKey, AttributeValue, FixtureId};

fn media_bytes(scenario: &CommandHttpScenario) -> std::collections::BTreeMap<u16, Vec<u8>> {
    scenario
        .state
        .output
        .render_with_playback_events(
            &scenario.state.active_show.output_projection(),
            &scenario.state.playback.render_capability(),
            RenderOptions::default(),
        )
        .unwrap()
        .rendered
        .universes
        .iter()
        .map(|(universe, bytes)| (*universe, bytes.to_vec()))
        .collect()
}

async fn media_batch(
    scenario: &CommandHttpScenario,
    preload: bool,
    mutations: serde_json::Value,
) -> Response {
    let snapshot = if preload {
        json(scenario.preload_values_snapshot().await).await
    } else {
        json(scenario.values_snapshot().await).await
    };
    let request = serde_json::json!({
        "request_id": Uuid::new_v4().to_string(),
        "expected_revision": snapshot["projection"]["revision"],
        "expected_capture_mode_revision": scenario.state.programming.capture_mode_revision(),
        "action": {"type":"batch", "mutations":mutations}
    });
    if preload {
        scenario.preload_values_action(request).await
    } else {
        scenario.values_action(request).await
    }
}

#[tokio::test]
async fn media_color_current_contract_batch_normal_and_preload_are_scoped_and_legacy_rgb_is_atomic()
{
    for preload in [false, true] {
        let clock = Arc::new(light_core::ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let scenario = CommandHttpScenario::from_state(test_state_with_family_adapters(
            ProgrammerRegistry::with_clock(clock.clone()),
            Some(clock.clone()),
            light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT,
        ))
        .await;
        assert!(
            scenario.state.output.supported_programming_contract()
                >= light_core::programming::PROGRAMMING_CONTRACT_VERSION
        );
        let profile = shipped_media_server();
        let a = FixtureId::new();
        let b = FixtureId::new();
        let a_layers = [FixtureId::new(), FixtureId::new()];
        let b_layers = [FixtureId::new(), FixtureId::new()];
        let fixture_a = media_fixture(&profile, a, &a_layers);
        let mut fixture_b = media_fixture(&profile, b, &b_layers);
        fixture_b.fixture_number = Some(2);
        fixture_b.universe = Some(2);
        let prepared = scenario
            .state
            .output
            .prepare_snapshot(EngineSnapshot {
                fixtures: vec![fixture_a, fixture_b].into(),
                revision: 1,
                ..EngineSnapshot::default()
            })
            .unwrap();
        scenario
            .state
            .output
            .finalize_snapshot(
                &scenario.state.playback.render_capability(),
                prepared,
                || Ok(()),
            )
            .unwrap();
        scenario
            .state
            .programming
            .select(scenario.session.id, vec![a_layers[0]]);
        // Real non-colour values make accidental dimmer/source edits and peer leakage observable.
        for (target, folder, file, dimmer) in [
            (a_layers[0], 150.0, 6.0, 0.7),
            (a_layers[1], 3.0, 1.0, 1.0),
            (b_layers[0], 3.0, 2.0, 1.0),
        ] {
            for (attribute, value) in [
                ("media.folder", folder / 255.0),
                ("media.file", file / 255.0),
                ("intensity", dimmer),
            ] {
                scenario.state.programming.set(
                    scenario.session.id,
                    target,
                    AttributeKey(attribute.into()),
                    AttributeValue::Normalized(value),
                );
            }
        }
        clock.advance_millis(3100);
        let baseline = media_bytes(&scenario);
        let original =
            json(scenario.values_snapshot().await).await["projection"]["fixture_values"].clone();
        if preload {
            assert_eq!(
                scenario
                    .press_key(&scenario.token, "PRE", "tl740-enter")
                    .await
                    .status(),
                StatusCode::OK
            );
        }
        let lane_before = if preload {
            json(scenario.preload_values_snapshot().await).await
        } else {
            json(scenario.values_snapshot().await).await
        };
        let rejected = media_batch(
            &scenario,
            preload,
            serde_json::json!([
                {"type":"set_fixture", "fixture_id":a_layers[0].0, "attribute":"intensity",
                 "value":{"kind":"normalized","value":0.1}},
                {"type":"set_fixture", "fixture_id":a_layers[0].0, "attribute":"color.red",
                 "value":{"kind":"normalized","value":1.0}}
            ]),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        assert!(
            json(rejected).await["error"]
                .as_str()
                .unwrap()
                .contains("color.red")
        );
        let lane_after = if preload {
            json(scenario.preload_values_snapshot().await).await
        } else {
            json(scenario.values_snapshot().await).await
        };
        assert_eq!(
            lane_after["projection"], lane_before["projection"],
            "legacy batch refuses before its dimmer mutation"
        );
        assert_eq!(media_bytes(&scenario), baseline);

        let red = program(&intent([1.0, 0.0, 0.0], 0.0));
        let AttributeValue::ColorProgram(ref color) = red else {
            panic!("whole Color program")
        };
        let wire = light_wire::v2::programming::ProgrammingAttributeValue::ColorProgram(
            color.as_ref().to_intent_wire(),
        );
        let accepted = media_batch(&scenario, preload, serde_json::json!([
            {"type":"set_fixture", "fixture_id":a_layers[0].0, "attribute":"color", "value":wire,
             "timing":{"fade":false,"fade_millis":null,"delay_millis":null}}
        ])).await;
        assert_eq!(
            accepted.status(),
            StatusCode::OK,
            "{}",
            json(accepted).await
        );
        if preload {
            let pending = json(scenario.preload_values_snapshot().await).await;
            let values = pending["projection"]["fixture_values"].as_array().unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0]["fixture_id"], a_layers[0].0.to_string());
            assert_eq!(values[0]["attribute"], "color");
            assert_eq!(values[0]["value"]["kind"], "color_program");
            assert_eq!(
                media_bytes(&scenario),
                baseline,
                "Preload does not change live output"
            );
            assert_eq!(
                json(scenario.values_snapshot().await).await["projection"]["fixture_values"],
                original
            );
            let removed = media_batch(
                &scenario,
                true,
                serde_json::json!([
                    {"type":"release_fixture", "fixture_id":a_layers[0].0, "attribute":"color"}
                ]),
            )
            .await;
            assert_eq!(removed.status(), StatusCode::OK);
            assert!(json(scenario.preload_values_snapshot().await).await["projection"]["fixture_values"].as_array().unwrap().is_empty());
            assert_eq!(media_bytes(&scenario), baseline);
            assert_eq!(
                json(scenario.values_snapshot().await).await["projection"]["fixture_values"],
                original
            );
            let reapplied = media_batch(&scenario, true, serde_json::json!([
                {"type":"set_fixture", "fixture_id":a_layers[0].0, "attribute":"color", "value":wire,
                 "timing":{"fade":false,"fade_millis":null,"delay_millis":null}}
            ])).await;
            assert_eq!(reapplied.status(), StatusCode::OK);
            let pending = json(scenario.preload_values_snapshot().await).await;
            let pending_values = pending["projection"]["fixture_values"].as_array().unwrap();
            assert_eq!(pending_values.len(), 1);
            let pending_color: light_wire::v2::programming::ProgrammingAttributeValue =
                serde_json::from_value(pending_values[0]["value"].clone()).unwrap();
            assert_eq!(
                pending_color, wire,
                "compare canonical f32 values, not JSON's variable decimal formatting"
            );
            assert_eq!(media_bytes(&scenario), baseline);
            assert_eq!(
                json(scenario.values_snapshot().await).await["projection"]["fixture_values"],
                original
            );
            let _ = std::fs::remove_dir_all(scenario.data_dir);
            continue;
        }
        clock.advance_millis(3100);
        let stored = scenario.state.programming.get(scenario.session.id).unwrap();
        assert_eq!(
            stored
                .values
                .iter()
                .find(|v| v.fixture_id == a_layers[0] && v.attribute == AttributeKey::color())
                .unwrap()
                .value,
            red
        );
        assert_eq!(stored.values.len(), original.as_array().unwrap().len() + 1);
        // Shipped 59-slot Media layer: zero-based Cyan/Magenta/Yellow/Grayscale =16..19.
        // Red is subtractive CMY(0,255,255), without a dimmer or source change.
        let mut expected = baseline.clone();
        expected.get_mut(&1).unwrap()[16..20].copy_from_slice(&[0, 255, 255, 0]);
        assert_eq!(
            media_bytes(&scenario),
            expected,
            "only selected A1 tint bytes change; A2, master, B and all source/dimmer bytes stay exact"
        );
        let reset = media_batch(
            &scenario,
            false,
            serde_json::json!([
                {"type":"release_fixture", "fixture_id":a_layers[0].0, "attribute":"color"}
            ]),
        )
        .await;
        assert_eq!(reset.status(), StatusCode::OK);
        clock.advance_millis(3100);
        assert_eq!(media_bytes(&scenario), baseline);
        assert_eq!(
            json(scenario.values_snapshot().await).await["projection"]["fixture_values"],
            original
        );
        let _ = std::fs::remove_dir_all(scenario.data_dir);
    }
}
