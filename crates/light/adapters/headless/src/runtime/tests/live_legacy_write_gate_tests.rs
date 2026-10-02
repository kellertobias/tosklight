//! TL-552 follow-up: no accepted live write at programming contract 1 stores legacy programming.
//!
//! Every test drives the real served desk (real startup at contract 1, the HTTP router, the
//! WebSocket action dispatcher, the command line and authenticated OSC keypad input), then
//! closes it and proves the invariant on disk: the show file has no legacy findings
//! (`inspect_show_programming_contract`) and a second real startup restores the Programmer
//! without entering recovery.
use super::semantic_contract_startup_support::{
    ContractDesk, ContractShow, SEMANTIC_CONTRACT, ServedStartupForTests,
};
use super::*;
use crate::runtime::output_scheduler::physical_adapters::{
    color::profiles::{patched, rgbw},
    optics::profiles::wash_a,
    position::tests::moving_head,
};
use light_core::FixtureId;

const HEAD: u128 = 0x5520_0001;
const WASH: u128 = 0x5520_0002;
const OPTICS: u128 = 0x5520_0003;

fn fixture(id: u128) -> FixtureId {
    FixtureId(Uuid::from_u128(id))
}

/// A semantic show with a moving head (Pan/Tilt, number 1), an RGBW wash (number 2) and a Zoom
/// wash (number 3), registered and active in a fresh desk.
fn rigged_show(desk: &ContractDesk) -> ContractShow {
    let show = desk.create_show("TL-552 live write gate");
    for (profile, id, number, address) in [
        (moving_head(), HEAD, 1, 1),
        (rgbw(), WASH, 2, 101),
        (wash_a(), OPTICS, 3, 201),
    ] {
        let mut patched = patched(&profile, fixture(id), address);
        patched.fixture_number = Some(number);
        show.patch(&patched);
    }
    desk.activate(&show);
    show
}

struct Desk {
    app: Router,
    token: String,
    session: Session,
    state: AppState,
}

impl Desk {
    async fn login(state: &AppState) -> Self {
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = state
            .sessions
            .sessions()
            .into_iter()
            .find(|session| session.token == token)
            .unwrap();
        Self {
            app,
            token,
            session,
            state: state.clone(),
        }
    }

    async fn get(&self, path: &str) -> serde_json::Value {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::get(path)
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        json(response).await
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .header("x-tosk-desk", self.session.desk.id.to_string())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    /// One values action against the current revisions, on the Normal or the Preload lane.
    async fn values(
        &self,
        preload: bool,
        action: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let lane = if preload { "preload-values" } else { "values" };
        let revision = self
            .get(&format!("/api/v2/programmer/{lane}/snapshot"))
            .await["projection"]["revision"]
            .clone();
        let capture =
            self.get("/api/v2/programmer/capture-mode/snapshot").await["projection"]["revision"]
                .clone();
        self.post(
            &format!("/api/v2/programmer/{lane}/actions"),
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "expected_revision": revision,
                "expected_capture_mode_revision": capture,
                "action": action,
            }),
        )
        .await
    }

    async fn command(&self, command: &str) -> (StatusCode, serde_json::Value) {
        self.post(
            "/api/v2/command-line/execute",
            serde_json::json!({"request_id": Uuid::new_v4().to_string(), "command": command}),
        )
        .await
    }

    fn programmer(&self) -> serde_json::Value {
        serde_json::to_value(self.state.programming.get(self.session.id).unwrap()).unwrap()
    }

    /// The stored-Programmer validator, exactly as startup applies it.
    fn assert_programmer_is_semantic(&self, label: &str) {
        let programmer = self.programmer();
        assert!(
            light_show::legacy_programming_attributes(&programmer).is_empty(),
            "{label}: {programmer}"
        );
        show_programming_contract::check_programmer(&programmer, SEMANTIC_CONTRACT)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
    }
}

fn set_fixture(id: u128, attribute: &str, value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "set_fixture", "fixture_id": fixture(id).0, "attribute": attribute,
        "value": value, "timing": {"fade": false}
    })
}

fn apply_intent(id: u128, attribute: &str, operation: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "apply_intent", "fixture_ids": [fixture(id).0], "attribute": attribute,
        "operation": operation, "timing": {"fade": false}
    })
}

fn normalized(value: f32) -> serde_json::Value {
    serde_json::json!({"kind": "normalized", "value": value})
}

/// A values action answers 400; the command line answers its execution outcome `rejected`.
fn assert_refused(label: &str, (status, body): (StatusCode, serde_json::Value), address: &str) {
    if body.get("outcome").is_some() {
        assert_eq!(status, StatusCode::OK, "{label}: {body}");
        assert_eq!(body["outcome"], "rejected", "{label}: {body}");
    } else {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}: {body}");
    }
    let message = body.to_string();
    assert!(message.contains(address), "{label}: {message}");
    assert!(
        message.contains("before semantic programming contract 1"),
        "{label}: {message}"
    );
}

/// Closes the served desk, then proves the invariant: the show has no legacy findings and a
/// real contract-1 restart restores this Programmer without entering recovery.
async fn assert_reopens_cleanly(
    desk: &ContractDesk,
    show: &ContractShow,
    served: ServedStartupForTests,
    session: Option<SessionId>,
) {
    served.close().await;
    let report = light_show::inspect_show_programming_contract(&show.path).unwrap();
    assert!(report.legacy.is_empty(), "{:?}", report.legacy);
    let startup = desk.load_at(SEMANTIC_CONTRACT);
    assert_eq!(startup.active_show_error, None);
    if let Some(session) = session {
        assert!(
            startup.programmers.get(session).is_some(),
            "the Programmer restores without recovery"
        );
    }
    assert!(
        !desk.data_dir.join("backups").exists()
            || std::fs::read_dir(desk.data_dir.join("backups"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("runtime-recovery-")),
        "no runtime recovery report"
    );
}

#[tokio::test]
async fn api_values_route_legacy_input_or_refuse_it_on_every_lane_and_transport() {
    let desk_dir = ContractDesk::new("tl552-api-gate");
    let show = rigged_show(&desk_dir);
    let served = desk_dir.serve(desk_dir.load_at(SEMANTIC_CONTRACT)).await;
    let desk = Desk::login(&served.state).await;
    assert_eq!(
        desk.state.output.supported_programming_contract(),
        SEMANTIC_CONTRACT
    );

    // Refused before mutation, on both lanes: raw sets, batches and percentage intents at a
    // legacy Position, Color component or Zoom address.
    for preload in [false, true] {
        if preload {
            let (status, body) = desk
                .post(
                    "/api/v2/command-line/keys",
                    serde_json::json!({"key": "PRE", "phase": "press", "request_id": "enter-preload"}),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        for (label, action, address) in [
            (
                "set pan",
                set_fixture(HEAD, "pan", normalized(0.5)),
                "`pan`",
            ),
            (
                "set tilt raw",
                set_fixture(
                    HEAD,
                    "tilt",
                    serde_json::json!({"kind": "raw_dmx", "value": 12}),
                ),
                "`tilt`",
            ),
            (
                "set color.red",
                set_fixture(WASH, "color.red", normalized(1.0)),
                "`color.red`",
            ),
            (
                "set zoom %",
                set_fixture(OPTICS, "zoom", normalized(0.4)),
                "`zoom`",
            ),
            (
                "atomic batch",
                serde_json::json!({"type": "batch", "mutations": [
                    set_fixture(HEAD, "intensity", normalized(1.0)),
                    set_fixture(WASH, "color.white", normalized(0.5)),
                ]}),
                "`color.white`",
            ),
            (
                "pan intent",
                apply_intent(
                    HEAD,
                    "pan",
                    serde_json::json!({"type": "absolute_set", "value": normalized(0.25)}),
                ),
                "`pan`",
            ),
            (
                "tilt step",
                apply_intent(
                    HEAD,
                    "tilt",
                    serde_json::json!({"type": "relative_step", "delta": 0.01}),
                ),
                "`tilt`",
            ),
            (
                "hue intent",
                apply_intent(
                    WASH,
                    "color.hue",
                    serde_json::json!({"type": "absolute_set", "value": normalized(0.5)}),
                ),
                "`color.hue`",
            ),
            (
                "selection range",
                serde_json::json!({"type": "set_selection", "fixture_ids": [fixture(HEAD).0],
                    "attribute": "pan", "value": normalized(0.5), "timing": {"fade": false}}),
                "`pan`",
            ),
        ] {
            if preload && label == "selection range" {
                continue; // the Preload lane has no server-side selection fan-out
            }
            let before = desk.programmer();
            let sequence = desk.state.events.latest_sequence();
            assert_refused(
                &format!("{label} preload={preload}"),
                desk.values(preload, action).await,
                address,
            );
            assert_eq!(desk.programmer(), before, "{label}: nothing changed");
            assert_eq!(
                desk.state.events.latest_sequence(),
                sequence,
                "{label}: no event"
            );
        }
    }
    let (status, body) = desk
        .post(
            "/api/v2/command-line/keys",
            serde_json::json!({"key": "PRE", "phase": "press", "request_id": "leave-preload"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // The WebSocket transport applies the same gate.
    let request_id = "tl552-ws-pan".to_owned();
    let response = dispatch_live_action(
        &desk.state,
        &desk.session,
        live_action_frame(
            &desk.session,
            &request_id,
            serde_json::from_value(serde_json::json!({
                "type": "programming_values",
                "request": {
                    "request_id": request_id,
                    "expected_revision": desk.get("/api/v2/programmer/values/snapshot").await["projection"]["revision"],
                    "expected_capture_mode_revision": desk.get("/api/v2/programmer/capture-mode/snapshot").await["projection"]["revision"],
                    "action": set_fixture(HEAD, "pan", normalized(0.5)),
                },
            }))
            .unwrap(),
        ),
    );
    assert!(!response.ok, "{response:?}");
    assert!(
        serde_json::to_string(&response.error)
            .unwrap()
            .contains("`pan`"),
        "{response:?}"
    );

    // Converted where the operator meaning is unambiguous: a Red percentage is the Red
    // component of the semantic Color owner, exactly as the Color encoder writes it. On a fresh
    // head the edit starts from open white, as the first Color encoder turn does.
    let (status, body) = desk
        .values(
            false,
            apply_intent(
                WASH,
                "color.red",
                serde_json::json!({"type": "absolute_set", "value": normalized(0.75)}),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = desk
        .values(
            false,
            apply_intent(
                WASH,
                "color.green",
                serde_json::json!({"type": "relative_step", "delta": 0.1}),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let values = desk.programmer()["values"].clone();
    let color = values
        .as_array()
        .unwrap()
        .iter()
        .filter(|value| value["fixture_id"] == serde_json::json!(fixture(WASH).0))
        .collect::<Vec<_>>();
    assert_eq!(color.len(), 1, "{values}");
    assert_eq!(color[0]["attribute"], "color", "{values}");
    assert_eq!(color[0]["value"]["kind"], "color_program", "{values}");
    // Semantic and non-family writes are unaffected.
    let (status, body) = desk
        .values(false, set_fixture(HEAD, "intensity", normalized(1.0)))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    desk.assert_programmer_is_semantic("after the API writes");

    // Recording what was accepted stores semantic content only.
    let (status, body) = desk.command("RECORD CUELIST 1 CUE 1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = desk.session.id;
    drop(desk);
    assert_reopens_cleanly(&desk_dir, &show, served, Some(session)).await;
}

#[tokio::test]
async fn command_line_and_osc_keypad_store_releases_on_semantic_owners_and_refuse_percentage_fix_at()
 {
    let desk_dir = ContractDesk::new("tl552-command-gate");
    let show = rigged_show(&desk_dir);
    let served = desk_dir.serve(desk_dir.load_at(SEMANTIC_CONTRACT)).await;
    let desk = Desk::login(&served.state).await;

    // A percentage FixAT has no meaning in degrees or as a colour: refused, nothing changes.
    for (command, address) in [
        ("FIXTURE 1 ATTRIBUTE pan FIXAT 50", "`pan`"),
        ("FIXTURE 3 ATTRIBUTE zoom FIXAT 50", "`zoom`"),
    ] {
        let mut before = desk.programmer();
        assert_refused(command, desk.command(command).await, address);
        let mut after = desk.programmer();
        // A rejected command stays on the command line for correction; nothing else changes.
        before["command_line"] = serde_json::Value::Null;
        after["command_line"] = serde_json::Value::Null;
        assert_eq!(after, before, "{command}");
    }
    // The HTTP FixAT action (the command line cannot spell a dotted Color channel) as well.
    let before = desk.programmer();
    assert_refused(
        "HTTP FixAT color.red",
        desk.post(
            "/api/v2/programmer/values/fix-at",
            serde_json::json!({"targets": [fixture(WASH).0], "attribute": "color.red", "value": 0.5}),
        )
        .await,
        "`color.red`",
    );
    assert_eq!(desk.programmer(), before);
    // Releasing a family stores the release of its semantic owner, never the native channels.
    for command in [
        "FIXTURE 1 AT RELEASE POSITION",
        "FIXTURE 2 AT RELEASE COLOR",
        "FIXTURE 3 AT RELEASE BEAM",
        "FIXTURE 1 THRU 3 AT RELEASE ALL",
    ] {
        let (status, body) = desk.command(command).await;
        assert_eq!(status, StatusCode::OK, "{command}: {body}");
        desk.assert_programmer_is_semantic(command);
    }
    let released = desk.programmer()["dynamic_values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["attribute"].as_str().unwrap().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(released.contains("position"), "{released:?}");
    assert!(released.contains("color"), "{released:?}");
    assert!(
        !released.contains("pan") && !released.contains("color.red"),
        "{released:?}"
    );

    // The same Release typed on an attached OSC keypad: `1 RELEASE POSITION`.
    let (status, body) = desk
        .values(false, serde_json::json!({"type": "clear"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(desk.programmer()["dynamic_values"], serde_json::json!([]));
    desk.state
        .programming
        .set_command_line(desk.session.id, String::new());
    let source: SocketAddr = "127.0.0.1:9552".parse().unwrap();
    desk.state.integrations.register_osc_subscriber(
        "tl552-keypad".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "desk".into(),
            target: source,
            command_source: source,
            session_id: desk.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    let key = |key: &str, pressed: bool| {
        assert!(handle_programmer_osc(
            &desk.state,
            &format!("/light/desk/programmer/{key}"),
            &[OscArgument::Bool(pressed)],
            Some("127.0.0.1:9552"),
        ));
    };
    key("digit-1", true);
    key("shift", true);
    key("off", true);
    key("digit-3", true);
    key("shift", false);
    let typed = desk
        .state
        .programming
        .get(desk.session.id)
        .unwrap()
        .command_line;
    assert!(typed.ends_with("1 RELEASE POSITION"), "{typed}");
    key("enter", true);
    let programmer = desk.programmer();
    assert!(
        programmer["dynamic_values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value["attribute"] == "position"),
        "{programmer}"
    );
    desk.assert_programmer_is_semantic("OSC keypad release");

    // A recorded Release is semantic too, so the show reopens.
    let (status, body) = desk.command("RECORD CUELIST 1 CUE 1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = desk.session.id;
    drop(desk);
    assert_reopens_cleanly(&desk_dir, &show, served, Some(session)).await;
}

#[tokio::test]
async fn object_writes_holding_legacy_programming_are_refused_before_the_show_changes() {
    let desk_dir = ContractDesk::new("tl552-object-gate");
    let show = rigged_show(&desk_dir);
    let served = desk_dir.serve(desk_dir.load_at(SEMANTIC_CONTRACT)).await;
    let desk = Desk::login(&served.state).await;
    let revision = show.store().portable_revision().unwrap().value();

    // The unit-of-work commit (every recording, Update and object intent) refuses it.
    let mut transaction = show.store().portable_document().unwrap().transaction();
    transaction.put(
        "preset",
        "3.9",
        serde_json::json!({"family": "Position", "number": 9, "name": "Legacy", "group_values": {},
            "values": {fixture(HEAD).0.to_string(): {"pan": normalized(0.5)}}}),
    );
    let error =
        show_programming_contract::check_transaction(&transaction, SEMANTIC_CONTRACT).unwrap_err();
    assert_eq!(error.kind, light_application::ActionErrorKind::Invalid);
    assert!(
        error
            .message
            .contains("preset 3.9 pan (normalized Position)"),
        "{}",
        error.message
    );
    // The direct object writers refuse it as well, through the active-show repository.
    let error = ActiveShowRepository::open(&show.path)
        .unwrap()
        .put_object(
            "cue_list",
            "00000000-0000-4004-8552-000000000001",
            &serde_json::json!({"cues": [{"changes": [{"fixture_id": fixture(WASH).0,
                "attribute": "color.red", "value": normalized(1.0)}]}]}),
            0,
        )
        .unwrap_err();
    assert!(
        matches!(&error, light_show::StoreError::Invalid(message) if message.contains("color.red")),
        "{error:?}"
    );
    assert_eq!(show.store().portable_revision().unwrap().value(), revision);
    drop(desk);
    assert_reopens_cleanly(&desk_dir, &show, served, None).await;
}
