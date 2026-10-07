//! TL-560 matrix row "Undo", column Color Direct: Programmer undo and Record (show-history)
//! undo of a tagged Direct recipe through the real HTTP routes of one desk.
//!
//! An RGBW fixture is patched into the active SQLite show and the show is re-opened through the
//! show-open route, so the desk's retained native Color catalogue is the compiled one. Direct
//! values travel as their own v2 wire (`ToIntentWire`) through `/api/v2/programmer/values/
//! actions`; Cues are recorded through `/api/v2/cues/record`; every undo is the desk's `UND`
//! key. The recipe, its pinned native identity and its portable estimate must come back exactly,
//! both in the Programmer and in the stored Cue list body.
use super::*;
use crate::runtime::command_http::ToIntentWire;
use crate::runtime::output_scheduler::physical_adapters::color::profiles::{patched, rgbw};
use crate::runtime::output_scheduler::physical_adapters::color::tests::direct::direct;
use light_core::{AttributeKey, AttributeValue};

const PLAYBACK: u16 = 27;

fn wire(value: &AttributeValue) -> serde_json::Value {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("a Color program")
    };
    serde_json::to_value(
        light_wire::v2::programming::ProgrammingAttributeValue::ColorProgram(
            program.as_ref().to_intent_wire(),
        ),
    )
    .unwrap()
}

async fn set_direct(
    scenario: &CommandHttpScenario,
    request_id: &str,
    fixture: light_core::FixtureId,
    value: &AttributeValue,
) {
    let response = scenario
        .values_action(serde_json::json!({
            "request_id": request_id,
            "expected_revision": scenario.state.programming.normal_values_revision(),
            "expected_capture_mode_revision": scenario.state.programming.capture_mode_revision(),
            "action": {
                "type": "set_fixture",
                "fixture_id": fixture.0,
                "attribute": "color",
                "value": wire(value),
            }
        }))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
}

fn programmer_color(
    scenario: &CommandHttpScenario,
    fixture: light_core::FixtureId,
) -> Option<AttributeValue> {
    scenario
        .state
        .programming
        .get(scenario.session.id)
        .unwrap()
        .values
        .iter()
        .find(|value| value.fixture_id == fixture && value.attribute == AttributeKey::color())
        .map(|value| value.value.clone())
}

fn stored_cue_list(scenario: &CommandHttpScenario) -> serde_json::Value {
    let entry = scenario.state.active_show.current().clone().unwrap();
    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    let mut lists = document.objects_of_kind("cue_list");
    let list = lists.next().expect("one recorded Cue list");
    assert!(lists.next().is_none());
    list.body().clone()
}

fn stored_color(body: &serde_json::Value) -> AttributeValue {
    let changes = body
        .pointer("/cues/0/changes")
        .and_then(|c| c.as_array())
        .unwrap();
    let [change] = changes.as_slice() else {
        panic!("one stored change: {body}")
    };
    assert_eq!(change["attribute"], "color");
    serde_json::from_value(change["value"].clone()).unwrap()
}

async fn record(scenario: &CommandHttpScenario, show_id: &str, request_id: &str) {
    let response = scenario
        .cue_recording_action(
            show_id,
            Some(&scenario.token),
            Some(active_show_revision(scenario)),
            cue_record_request(
                request_id,
                serde_json::json!({"kind":"pool","playback_number":PLAYBACK}),
                Some(1.0),
                "current_capture",
                "hold",
            ),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
}

async fn undo(scenario: &CommandHttpScenario, request_id: &str) {
    let response = scenario.press_key(&scenario.token, "UND", request_id).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
}

#[tokio::test]
async fn direct_color_programmer_undo_and_record_undo_restore_the_exact_tagged_recipe() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("TL-560 Direct undo").await;
    // Patch an RGBW fixture into the SQLite show and re-open it through the show-open route.
    let profile = rgbw();
    let fixture_id = light_core::FixtureId::new();
    let fixture = patched(&profile, fixture_id, 1);
    let entry = scenario.state.active_show.current().clone().unwrap();
    {
        let store = ShowStore::open(&entry.path).unwrap();
        store
            .insert_fixture_profile_revision(
                &light_show::FixtureProfileRevision::from_profile(
                    serde_json::to_value(&profile).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        let body = light_fixture::PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
            .unwrap()
            .into_body();
        store
            .put_object("patched_fixture", &fixture_id.0.to_string(), &body, 0)
            .unwrap();
    }
    let response = scenario
        .app
        .clone()
        .oneshot(open_show_request(&scenario.token, &show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let catalogue = Arc::clone(&scenario.state.output.snapshot().native_color_sources);
    let orange = direct(&catalogue, &profile, &[65535, 90, 0, 0]);
    let dim = direct(&catalogue, &profile, &[1000, 0, 7, 3]);
    assert_ne!(orange, dim);

    // Programmer undo: the second Direct edit is undone to the exact first recipe.
    set_direct(&scenario, "tl560-direct-orange", fixture_id, &orange).await;
    assert_eq!(
        programmer_color(&scenario, fixture_id),
        Some(orange.clone())
    );
    set_direct(&scenario, "tl560-direct-dim", fixture_id, &dim).await;
    assert_eq!(programmer_color(&scenario, fixture_id), Some(dim.clone()));
    undo(&scenario, "tl560-undo-dim").await;
    assert_eq!(
        programmer_color(&scenario, fixture_id),
        Some(orange.clone()),
        "Programmer undo restores the exact tagged recipe, identity and estimate"
    );

    // Record, then overwrite with the other recipe; Record undo restores the stored body.
    record(&scenario, &show_id, "tl560-record-orange").await;
    let recorded = stored_cue_list(&scenario);
    assert_eq!(
        stored_color(&recorded),
        orange,
        "the tagged recipe is stored"
    );
    assert_eq!(
        recorded.pointer("/cues/0/changes/0/value/value/kind"),
        Some(&serde_json::Value::String("direct".into()))
    );
    set_direct(&scenario, "tl560-direct-dim-again", fixture_id, &dim).await;
    record(&scenario, &show_id, "tl560-record-dim").await;
    let overwritten = stored_cue_list(&scenario);
    assert_eq!(stored_color(&overwritten), dim);
    assert_eq!(overwritten["cues"].as_array().unwrap().len(), 1);
    undo(&scenario, "tl560-undo-record-dim").await;
    assert_eq!(
        stored_cue_list(&scenario),
        recorded,
        "Record undo restores the previous Direct Cue body exactly"
    );
    assert_eq!(
        programmer_color(&scenario, fixture_id),
        Some(dim.clone()),
        "Record undo does not touch the Programmer"
    );
    // The next undo is the Programmer edit made before that recording.
    undo(&scenario, "tl560-undo-dim-again").await;
    assert_eq!(programmer_color(&scenario, fixture_id), Some(orange));
    assert_eq!(
        stored_cue_list(&scenario),
        recorded,
        "the show is unchanged"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
