use super::*;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_playback::{Cue, CueChange, CueList, GroupCueChange};
use std::path::PathBuf;

/// The shipped ToskLight Media Server profile, patched like a discovered server.
pub(in crate::runtime) fn media_server(
    mode: &str,
    ip: [u8; 4],
    output: &str,
) -> light_fixture::PatchedFixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/tosklight--media-server.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode_id = profile
        .modes
        .iter()
        .find(|candidate| candidate.name == mode)
        .unwrap()
        .id;
    let definition = profile.resolved_definition(mode_id).unwrap();
    let mut fixture: light_fixture::PatchedFixture = serde_json::from_value(serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": definition,
    }))
    .unwrap();
    fixture.universe = Some(3);
    fixture.address = Some(40);
    fixture.direct_control = Some(light_fixture::DirectControlEndpoint {
        protocol: light_fixture::DirectControlProtocol::Citp,
        ip_address: std::net::IpAddr::from(ip),
        port: 4809,
    });
    fixture.internal_bindings.output = Some(output.to_owned());
    light_fixture::reconcile_logical_heads(&mut fixture);
    fixture
}

pub(in crate::runtime) fn layer(
    fixture: &light_fixture::PatchedFixture,
    index: usize,
) -> FixtureId {
    let mut heads = fixture.logical_heads.clone();
    heads.sort_by_key(|head| head.head_index);
    heads[index].fixture_id
}

pub(in crate::runtime) fn set(fixture_id: FixtureId, attribute: &str, value: f32) -> CueChange {
    CueChange::set(
        fixture_id,
        AttributeKey(attribute.into()),
        AttributeValue::Normalized(value),
    )
}

pub(in crate::runtime) fn cue(number: u32, changes: Vec<CueChange>) -> Cue {
    let mut cue: Cue = serde_json::from_value(serde_json::json!({
        "number": number.to_string(),
        "name": format!("Cue {number}"),
        "changes": [],
        "fade_millis": 0,
        "delay_millis": 0,
        "trigger": {"type": "manual"},
    }))
    .unwrap();
    cue.changes = changes;
    cue
}

pub(in crate::runtime) fn cue_list(cues: Vec<Cue>) -> CueList {
    let mut list: CueList = serde_json::from_value(serde_json::json!({
        "id": light_core::CueListId::new(),
        "name": "Media",
        "priority": 0,
        "mode": "sequence",
        "looped": false,
        "cues": [],
    }))
    .unwrap();
    list.cues = cues;
    list
}

pub(in crate::runtime) fn wash() -> light_fixture::PatchedFixture {
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Wash".into();
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    serde_json::from_value(serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": definition,
    }))
    .unwrap()
}

pub(in crate::runtime) const OUTPUT_A: &str = "00000000-0000-4000-8000-00000000000a";
const OUTPUT_B: &str = "00000000-0000-4000-8000-00000000000b";

#[test]
fn a_cue_is_pictured_by_the_program_or_its_single_layer_on_its_own_server_only() {
    let a = media_server("2 layers", [192, 0, 2, 10], OUTPUT_A);
    let b = media_server("2 layers", [192, 0, 2, 11], OUTPUT_B);
    let other = wash();
    let fixtures = vec![a.clone(), b.clone(), other.clone()];

    let master = media_preview_target(&fixtures, &[a.fixture_id]).unwrap();
    assert_eq!(master.server, a.fixture_id);
    assert_eq!(master.layer, None, "the master is the Program image");

    let second_layer = media_preview_target(&fixtures, &[layer(&a, 1)]).unwrap();
    assert_eq!(second_layer.server, a.fixture_id);
    assert_eq!(second_layer.layer, Some((1, layer(&a, 1))));

    let on_b = media_preview_target(&fixtures, &[layer(&b, 0)]).unwrap();
    assert_eq!(
        on_b.server, b.fixture_id,
        "each layer belongs to its own server"
    );
    assert_eq!(on_b.layer, Some((0, layer(&b, 0))));

    let both_layers = media_preview_target(&fixtures, &[layer(&a, 0), layer(&a, 1)]).unwrap();
    assert_eq!(both_layers.layer, None, "two layers are the Program image");
    let layer_and_master = media_preview_target(&fixtures, &[layer(&a, 0), a.fixture_id]).unwrap();
    assert_eq!(layer_and_master.layer, None);

    assert_eq!(
        media_preview_target(&fixtures, &[layer(&a, 0), layer(&b, 0)]),
        None,
        "a Cue across two servers has no single Media Server picture"
    );
    assert_eq!(
        media_preview_target(&fixtures, &[layer(&a, 0), other.fixture_id]),
        None,
        "a Cue with lighting content keeps its Stage preview"
    );
    assert_eq!(media_preview_target(&fixtures, &[]), None);
}

#[test]
fn a_media_cue_carries_the_slots_it_would_transmit_with_tracking_and_group_values() {
    let a = media_server("2 layers", [192, 0, 2, 10], OUTPUT_A);
    let b = media_server("2 layers", [192, 0, 2, 11], OUTPUT_B);
    let first = cue(
        1,
        vec![
            set(layer(&a, 1), "media.folder", 5.0 / 255.0),
            set(layer(&a, 1), "media.file", 7.0 / 255.0),
            set(layer(&a, 1), "intensity", 1.0),
        ],
    );
    let mut second = cue(2, vec![set(a.fixture_id, "intensity", 0.5)]);
    second.group_changes.push(GroupCueChange {
        group_id: "screens".into(),
        attribute: AttributeKey("media.file".into()),
        value: Some(AttributeValue::Normalized(9.0 / 255.0)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let released = cue(
        3,
        vec![CueChange {
            fixture_id: layer(&a, 1),
            attribute: AttributeKey("media.file".into()),
            value: None,
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        }],
    );
    let on_b = cue(4, vec![set(layer(&b, 0), "media.folder", 2.0 / 255.0)]);
    let list = cue_list(vec![first, second, released, on_b]);
    let group: light_programmer::GroupDefinition = serde_json::from_value(serde_json::json!({
        "id": "screens",
        "name": "Screens",
        "color": null,
        "icon": null,
        "fixtures": [layer(&a, 1)],
    }))
    .unwrap();
    let snapshot = EngineSnapshot {
        fixtures: vec![a.clone(), b.clone()].into(),
        cue_lists: vec![list.clone()].into(),
        groups: vec![group].into(),
        revision: 1,
        ..EngineSnapshot::default()
    };

    let prepared = prepare_media_previews(&snapshot, None);
    assert_eq!(prepared.len(), 4);
    let [first, second, third, fourth] = prepared.as_slice() else {
        unreachable!()
    };
    assert_eq!(
        first.slots.len(),
        158,
        "a two-layer personality is 158 slots"
    );
    assert_eq!(first.entry.scope, wire::CueMediaPreviewScope::Layer);
    assert_eq!(first.entry.layer, Some(1));
    assert_eq!(first.entry.server_fixture_id, a.fixture_id.0);
    assert_eq!(first.entry.output_id.as_deref(), Some(OUTPUT_A));
    assert_eq!(
        first.endpoint,
        Some(std::net::IpAddr::from([192, 0, 2, 10]))
    );
    // Layer 2 starts at slot 59: folder, file, then Dimmer at offset 14.
    assert_eq!(&first.slots[59..61], &[5, 7]);
    assert_eq!(first.slots[59 + 14], 255);
    assert_eq!(first.slots[0..2], [0, 0], "layer 1 selects nothing");

    // Cue 2 addresses the master and, through the Group, layer 2: the Program picture, with the
    // file spread from the Group and the folder tracked from Cue 1.
    assert_eq!(second.entry.scope, wire::CueMediaPreviewScope::Program);
    assert_eq!(second.entry.layer, None);
    assert_eq!(&second.slots[59..61], &[5, 9]);
    assert_eq!(second.slots[118], 128, "master dimmer at half");
    assert_ne!(first.entry.preview_key, second.entry.preview_key);

    // A released value leaves the tracked state.
    assert_eq!(&third.slots[59..61], &[5, 0]);

    // Cue 4 is pictured by server B alone.
    assert_eq!(fourth.entry.server_fixture_id, b.fixture_id.0);
    assert_eq!(fourth.entry.output_id.as_deref(), Some(OUTPUT_B));
    assert_eq!(fourth.slots[0], 2);

    // The same state on another server is a different picture.
    let mut moved = snapshot.clone();
    let mut relocated = a.clone();
    relocated.direct_control.as_mut().unwrap().ip_address = std::net::IpAddr::from([192, 0, 2, 99]);
    moved.fixtures = vec![relocated, b.clone()].into();
    let again = prepare_media_previews(&moved, Some(list.cues[0].id));
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].slots, first.slots);
    assert_ne!(again[0].entry.preview_key, first.entry.preview_key);
}

#[test]
fn an_eight_layer_server_is_pictured_from_its_full_universe() {
    let a = media_server("8 layers", [192, 0, 2, 10], OUTPUT_A);
    let list = cue_list(vec![cue(
        1,
        vec![set(layer(&a, 7), "media.folder", 3.0 / 255.0)],
    )]);
    let snapshot = EngineSnapshot {
        fixtures: vec![a].into(),
        cue_lists: vec![list].into(),
        revision: 1,
        ..EngineSnapshot::default()
    };
    let prepared = prepare_media_previews(&snapshot, None);
    assert_eq!(prepared[0].slots.len(), 512);
    assert_eq!(prepared[0].entry.layer, Some(7));
    assert_eq!(prepared[0].slots[7 * 59], 3);
}

type Seen = std::sync::Arc<parking_lot::Mutex<Vec<(String, serde_json::Value)>>>;

async fn mock_media_server(
    snapshot: axum::response::Response,
    outputs: serde_json::Value,
) -> (String, Seen, tokio::task::JoinHandle<()>) {
    let seen: Seen = Default::default();
    let body = std::sync::Arc::new(parking_lot::Mutex::new(Some(snapshot)));
    let recorded = seen.clone();
    let app = Router::new()
        .route(
            "/api/v2/outputs",
            get(move || {
                let outputs = outputs.clone();
                async move { Json(outputs) }
            }),
        )
        .route(
            "/api/v2/outputs/{output}/snapshot",
            post(
                move |Path(output): Path<String>, Json(request): Json<serde_json::Value>| {
                    let recorded = recorded.clone();
                    let body = body.clone();
                    async move {
                        recorded.lock().push((output, request));
                        body.lock()
                            .take()
                            .unwrap_or_else(|| StatusCode::GONE.into_response())
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/v2", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, seen, server)
}

fn png_response(content: &str) -> axum::response::Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header("x-tosklight-snapshot-content", content)
        .body(Body::from(b"\x89PNG-layer".to_vec()))
        .unwrap()
}

fn refusal(status: StatusCode, code: &str) -> axum::response::Response {
    (
        status,
        Json(serde_json::json!({"code": code, "message": "refused"})),
    )
        .into_response()
}

#[tokio::test]
async fn the_desk_asks_the_patched_output_for_the_cue_state_and_reports_empty_pictures() {
    let (base, seen, server) =
        mock_media_server(png_response("empty"), serde_json::json!([])).await;
    let outcome = fetch_snapshot(&base, Some(OUTPUT_B), Some(1), &[1, 2, 3], 240, 135).await;
    let SnapshotOutcome::Picture { png, empty } = outcome else {
        panic!("expected a picture, got {outcome:?}");
    };
    assert_eq!(png, b"\x89PNG-layer");
    assert!(empty);
    let seen = seen.lock().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].0, OUTPUT_B,
        "the bound output is drawn, never another"
    );
    assert_eq!(
        seen[0].1,
        serde_json::json!({"slots": [1, 2, 3], "layer": 1, "width": 240, "height": 135})
    );
    server.abort();
}

#[tokio::test]
async fn an_unbound_fixture_follows_the_first_output() {
    let (base, seen, server) = mock_media_server(
        png_response("content"),
        serde_json::json!([{"id": OUTPUT_A}, {"id": OUTPUT_B}]),
    )
    .await;
    let outcome = fetch_snapshot(&base, None, None, &[0; 4], 320, 180).await;
    assert!(matches!(
        outcome,
        SnapshotOutcome::Picture { empty: false, .. }
    ));
    assert_eq!(seen.lock()[0].0, OUTPUT_A);
    assert_eq!(seen.lock()[0].1["layer"], serde_json::Value::Null);
    server.abort();
}

#[tokio::test]
async fn loading_missing_and_offline_servers_are_named_states() {
    use wire::CueMediaPreviewFailureState as S;
    let expect = |outcome: SnapshotOutcome, state: S| match outcome {
        SnapshotOutcome::Failed(failed) => assert_eq!(failed.state, state, "{}", failed.error),
        SnapshotOutcome::Picture { .. } => panic!("expected {state:?}"),
    };
    for (response, state) in [
        (
            refusal(StatusCode::SERVICE_UNAVAILABLE, "snapshot-not-ready"),
            S::Loading,
        ),
        (refusal(StatusCode::NOT_FOUND, "unknown-output"), S::Missing),
        (
            refusal(StatusCode::BAD_REQUEST, "snapshot-slots-mismatch"),
            S::Missing,
        ),
        (
            refusal(StatusCode::SERVICE_UNAVAILABLE, "snapshot-unavailable"),
            S::Missing,
        ),
        (StatusCode::NOT_FOUND.into_response(), S::Missing),
    ] {
        let (base, _, server) = mock_media_server(response, serde_json::json!([])).await;
        expect(
            fetch_snapshot(&base, Some(OUTPUT_A), None, &[0], 32, 18).await,
            state,
        );
        server.abort();
    }
    let (base, _, server) = mock_media_server(png_response("content"), serde_json::json!([])).await;
    expect(
        fetch_snapshot(&base, None, None, &[0], 32, 18).await,
        S::Missing,
    );
    server.abort();

    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = closed.local_addr().unwrap();
    drop(closed);
    expect(
        fetch_snapshot(
            &format!("http://{address}/api/v2"),
            Some(OUTPUT_A),
            None,
            &[0],
            32,
            18,
        )
        .await,
        S::Offline,
    );
}
