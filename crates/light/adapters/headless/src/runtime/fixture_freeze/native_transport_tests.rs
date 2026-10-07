//! TL-609 transport regressions for native Position Freeze.
//!
//! PATH LABEL — PHYSICAL NATIVE SETUP. Every accepted frame in this module is produced by the
//! parent `Rig::render`, which samples *physical native channel values* (literal `pan`/`tilt` or
//! custom motor aliases, root plus an independently inverted copy) through
//! `render_with_contribution_batches` and then publishes them through the ordinary accepted
//! `render_frames_and_publish` boundary. No test here runs the semantic Position coordinator or
//! Target fitter, so no assertion in this module is
//! evidence of production semantic activation or semantic fitted output.
//!
//! What *is* exercised for real: the authenticated `/api/v2/fixture-freeze/actions` HTTP action,
//! the authenticated `/api/v2/command-line/execute` entered-command route, the attached OSC
//! programmer keypad (`Shift+Clear`, `Shift+3`, `Enter`), the `/api/v2/programmer-undo/actions`
//! transport Undo, active-show patch storage, and the engine's native Freeze output hold.
use super::*;
use crate::runtime::{OscArgument, OscSubscriber, handle_programmer_osc};
use axum::http::Method;
use light_fixture::FrozenPositionOutput;
use std::{net::SocketAddr, sync::Arc, time::Instant};

const OSC_SOURCE: &str = "127.0.0.1:9609";

/// PHYSICAL NATIVE SETUP: physical native motor words rendered and published as accepted output.
/// This is not semantic fitted output. The capture must equal the words actually on the wire.
fn publish_physical_native_setup(rig: &Rig, pan: f32, tilt: f32) -> FrozenPositionOutput {
    let frame = rig.render(pan, tilt);
    let expected = rig
        .state
        .output
        .engine()
        .position_freeze_from_physical(
            frame.rendered.generation,
            &frame.rendered.physical,
            rig.owner,
        )
        .expect("physical native setup yields a compiled native Position capture");
    assert_eq!(wire_words(rig, &frame), words_by_instance(&expected));
    rig.publish(&frame, rig.show_id.0);
    expected
}

/// Root at DMX 1, independently inverted copy at DMX 20; both Position channels are 16-bit.
fn wire_words(rig: &Rig, frame: &RenderedSemanticFrame) -> Vec<Vec<u32>> {
    let universe = &frame.rendered.universes[&1];
    [0usize, 19]
        .into_iter()
        .take(1 + usize::from(rig.copy.is_some()))
        .map(|base| {
            (0..2)
                .map(|axis| {
                    let slot = base + 2 * axis;
                    (u32::from(universe[slot]) << 8) | u32::from(universe[slot + 1])
                })
                .collect()
        })
        .collect()
}

fn words_by_instance(payload: &FrozenPositionOutput) -> Vec<Vec<u32>> {
    payload
        .instances
        .iter()
        .map(|instance| {
            instance
                .controls
                .iter()
                .map(|control| control.raw)
                .collect()
        })
        .collect()
}

async fn call(
    rig: &Rig,
    method: Method,
    path: &str,
    payload: Option<Value>,
    show_header: Option<Uuid>,
    bearer: bool,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-tosk-desk", rig.session.desk.id.to_string());
    if bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {}", rig.token));
    }
    if let Some(show) = show_header {
        request = request.header("x-tosk-show", show.to_string());
    }
    let body = match payload {
        Some(payload) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(payload.to_string())
        }
        None => Body::empty(),
    };
    let response = rig
        .app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Actual authenticated HTTP live-control Freeze action (partial families).
async fn http_action(rig: &Rig, operation: &str, show: Option<Uuid>) -> (StatusCode, Value) {
    call(
        rig,
        Method::POST,
        "/api/v2/fixture-freeze/actions",
        Some(json!({"operation":operation,"families":["position"]})),
        show,
        true,
    )
    .await
}

/// Actual authenticated HTTP live-control full Freeze toggle.
async fn http_full_action(rig: &Rig) -> (StatusCode, Value) {
    call(
        rig,
        Method::GET,
        "/api/v2/fixture-freeze/actions",
        None,
        Some(rig.show_id.0),
        true,
    )
    .await
}

/// Actual authenticated HTTP entered command line.
async fn http_command(
    rig: &Rig,
    request_id: &str,
    command: &str,
    show: Option<Uuid>,
) -> (StatusCode, Value) {
    call(
        rig,
        Method::POST,
        "/api/v2/command-line/execute",
        Some(json!({"request_id":request_id,"command":command})),
        show,
        true,
    )
    .await
}

/// Actual authenticated HTTP transport Undo.
async fn http_undo(rig: &Rig) -> Value {
    let (status, value) = call(
        rig,
        Method::GET,
        "/api/v2/programmer-undo/actions",
        None,
        Some(rig.show_id.0),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}

fn register_osc(rig: &Rig) {
    let source: SocketAddr = OSC_SOURCE.parse().unwrap();
    rig.state.integrations.register_osc_subscriber(
        "tl609-native-freeze".into(),
        OscSubscriber {
            capability: rig.session.capability,
            path: "main".into(),
            target: source,
            command_source: source,
            session_id: rig.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
}

fn osc_key(rig: &Rig, source: &str, action: &str, pressed: bool, request_id: Option<&str>) -> bool {
    let mut arguments = vec![OscArgument::Bool(pressed)];
    if let Some(request_id) = request_id {
        arguments.push(OscArgument::String(request_id.into()));
    }
    handle_programmer_osc(
        &rig.state,
        &format!("/light/main/programmer/{action}"),
        &arguments,
        Some(source),
    )
}

fn command_line(rig: &Rig) -> String {
    rig.state
        .programming
        .get(rig.session.id)
        .unwrap()
        .command_line
}

/// Actual attached OSC keypad: `[^CLR]` (`[^CLR][^CLR]` for Unfreeze), `[^3]` Position, `[ENT]`.
fn osc_entered(rig: &Rig, unfreeze: bool, request_id: Option<&str>) -> bool {
    assert_eq!(
        command_line(rig).trim(),
        "",
        "OSC entry starts from an empty line"
    );
    assert!(osc_key(rig, OSC_SOURCE, "shift", true, None));
    assert!(osc_key(rig, OSC_SOURCE, "clear", true, None));
    if unfreeze {
        assert!(osc_key(rig, OSC_SOURCE, "clear", true, None));
    }
    assert!(osc_key(rig, OSC_SOURCE, "digit-3", true, None));
    assert!(osc_key(rig, OSC_SOURCE, "shift", false, None));
    assert_eq!(
        command_line(rig),
        if unfreeze {
            "UNFREEZE POSITION"
        } else {
            "FREEZE POSITION"
        }
    );
    osc_key(rig, OSC_SOURCE, "enter", true, request_id)
}

fn native(rig: &Rig) -> Option<FrozenPositionOutput> {
    rig.fixture()
        .freeze
        .targets
        .get(&rig.owner)
        .and_then(|target| target.position_native.clone())
}

fn revisions(rig: &Rig) -> (u64, u64) {
    let snapshot = rig
        .state
        .active_show
        .patch_snapshot(
            &rig.context(),
            rig.show_id,
            &ServerShowPatchPorts::new(rig.state.clone()),
        )
        .unwrap();
    (
        snapshot.show_revision.value(),
        snapshot.patch_revision.value(),
    )
}

/// Every `PatchChanged` event after `after`, as `(sequence, patch revision)` in publication order.
fn patch_events(rig: &Rig, after: u64) -> Vec<(u64, u64)> {
    match rig
        .state
        .events
        .replay(after, &light_application::EventFilter::default())
    {
        light_application::EventReplay::Events(events) => events
            .iter()
            .filter_map(|event| match &event.payload {
                light_application::ApplicationEvent::Show(
                    light_application::ShowEvent::PatchChanged(change),
                ) => Some((event.sequence, change.patch_revision.value())),
                _ => None,
            })
            .collect(),
        light_application::EventReplay::Gap(gap) => panic!("event replay gap: {gap:?}"),
    }
}

fn freeze_history_len(rig: &Rig) -> usize {
    rig.state
        .fixture_freeze_history
        .entries
        .lock()
        .get(&rig.session.desk.id)
        .map_or(0, Vec::len)
}

fn accepted_generation(rig: &Rig) -> Option<u64> {
    rig.state
        .output
        .latest_visualization_frame()
        .map(|frame| frame.generation)
}

/// Complete observable mutation surface of one Freeze attempt.
struct Observation {
    revisions: (u64, u64),
    sequence: u64,
    engine: Arc<light_engine::EngineSnapshot>,
    history: usize,
    programmer_undo_depth: Option<usize>,
    freeze: FixtureFreezeState,
    persisted: FixtureFreezeState,
    generation: Option<u64>,
}

fn observe(rig: &Rig) -> Observation {
    Observation {
        revisions: revisions(rig),
        sequence: rig.state.events.latest_sequence(),
        engine: rig.state.output.snapshot(),
        history: freeze_history_len(rig),
        programmer_undo_depth: rig.state.programming.undo_depth(rig.session.id),
        freeze: rig.fixture().freeze,
        persisted: rig.persisted_freeze(),
        generation: accepted_generation(rig),
    }
}

/// Quiet no-op: no show/patch revision, engine reinstall, Freeze Undo checkpoint, Programmer
/// Undo step, patch event, persisted change or new accepted publication.
fn assert_quiet(rig: &Rig, before: &Observation, label: &str) {
    assert_eq!(revisions(rig), before.revisions, "{label}: revision moved");
    assert!(
        patch_events(rig, before.sequence).is_empty(),
        "{label}: a patch event was published"
    );
    assert!(
        Arc::ptr_eq(&rig.state.output.snapshot(), &before.engine),
        "{label}: the engine snapshot was reinstalled"
    );
    assert_eq!(freeze_history_len(rig), before.history, "{label}: Undo");
    assert_eq!(
        rig.state.programming.undo_depth(rig.session.id),
        before.programmer_undo_depth,
        "{label}: Programmer Undo"
    );
    assert_eq!(rig.fixture().freeze, before.freeze, "{label}: live freeze");
    assert_eq!(rig.persisted_freeze(), before.persisted, "{label}: stored");
    assert_eq!(
        accepted_generation(rig),
        before.generation,
        "{label}: no new sample may be requested"
    );
}

/// Exactly one ordered patch mutation since `before`, returned as its patch revision.
fn assert_one_mutation(rig: &Rig, before: &Observation, label: &str) -> u64 {
    let events = patch_events(rig, before.sequence);
    assert_eq!(events.len(), 1, "{label}: {events:?}");
    let (show, patch) = revisions(rig);
    assert_eq!(patch, before.revisions.1 + 1, "{label}: patch revision");
    assert!(show > before.revisions.0, "{label}: show revision");
    assert_eq!(
        events[0].1, patch,
        "{label}: event carries the new revision"
    );
    assert_eq!(rig.persisted_freeze(), rig.fixture().freeze, "{label}");
    patch
}

fn assert_action_outcome(value: &Value, changed: bool, affected: u64, label: &str) {
    assert_eq!(value["changed"], changed, "{label}: {value}");
    assert_eq!(value["affected_fixtures"], affected, "{label}: {value}");
}

fn assert_command_applied(status: StatusCode, value: &Value, applied: u64, label: &str) {
    assert_eq!(status, StatusCode::OK, "{label}: {value}");
    assert_eq!(value["outcome"], "accepted", "{label}: {value}");
    assert_eq!(value["applied"], applied, "{label}: {value}");
}

/// Exact native words and identities for root plus the independently inverted copy, on either
/// literal `pan`/`tilt` channels or custom motor aliases. The portable payload holds per-instance
/// channel words only, never a common baked Angle pair.
fn assert_root_copy_words(rig: &Rig, payload: &FrozenPositionOutput) {
    let fixture = rig.fixture();
    let mode = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .mode(fixture.definition.mode_id.unwrap())
        .unwrap()
        .clone();
    let attribute = |id: Uuid| {
        mode.channels
            .iter()
            .find(|channel| channel.id == id)
            .unwrap()
            .attribute
            .0
            .to_string()
    };
    assert_eq!(payload.version, 1);
    assert_eq!(payload.instances.len(), 2);
    let [root, copy] = [&payload.instances[0], &payload.instances[1]];
    assert_eq!(root.instance_id, rig.root.0, "root instance identity");
    assert_eq!(
        copy.instance_id,
        rig.copy.unwrap(),
        "copy instance identity"
    );
    for instance in [root, copy] {
        assert_eq!(
            instance
                .controls
                .iter()
                .map(|control| attribute(control.channel_id))
                .collect::<Vec<_>>(),
            rig.motor_names,
            "the profile's own Position channels own the hold"
        );
        assert!(
            instance
                .controls
                .iter()
                .all(|control| !control.signature.is_empty())
        );
    }
    // Per-instance identity: the copy's pan signature is its own, not the root's.
    assert_ne!(root.controls[0].signature, copy.controls[0].signature);
    let words = words_by_instance(payload);
    // Full-width 16-bit words: the fine byte is captured, not only the coarse byte.
    assert!(
        words
            .iter()
            .flatten()
            .all(|raw| *raw <= 65535 && raw & 0xff != 0),
        "{words:?}"
    );
    if rig.motor_names[0] == "pan" {
        // Independent inversion: only the copy's pan is mirrored on the wire (normalized 1 - n,
        // each side rounded to 16 bits, so the pair sums to 65535 or 65536); tilt is identical.
        assert!(
            (65535..=65536).contains(&(words[0][0] + words[1][0])),
            "{words:?}"
        );
    }
    assert_eq!(words[0][1], words[1][1]);
    let serialized = serde_json::to_value(payload).unwrap();
    let mut keys = serialized
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, ["instances", "version"], "no common baked Angle pair");
    let mut control_keys = serialized["instances"][0]["controls"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    control_keys.sort();
    assert_eq!(control_keys, ["channel_id", "raw", "signature"]);
}

/// Actual engine output hold path: a later physical native render and its DMX universe keep the
/// held words for root and copy. Physical native setup, not semantic fitted output.
fn assert_output_holds(rig: &Rig, payload: &FrozenPositionOutput) {
    let later = rig.render(0.9, 0.15);
    let fixture = rig.fixture();
    let mode = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .mode(fixture.definition.mode_id.unwrap())
        .unwrap()
        .clone();
    for (instance, base) in payload.instances.iter().zip([0usize, 19]) {
        let physical = later
            .rendered
            .physical
            .instances
            .iter()
            .find(|output| output.instance_id == instance.instance_id)
            .unwrap();
        let universe = &later.rendered.universes[&1];
        for control in &instance.controls {
            let index = mode
                .channels
                .iter()
                .position(|channel| channel.id == control.channel_id)
                .unwrap();
            assert_eq!(physical.native_raw[index], control.raw, "held native word");
            let slot = base + 2 * index;
            assert_eq!(
                [universe[slot], universe[slot + 1]],
                [(control.raw >> 8) as u8, (control.raw & 0xff) as u8],
                "held DMX coarse/fine at slot {}",
                slot + 1
            );
        }
    }
}

async fn http_and_osc_hold_identical_words(rig: &Rig) {
    register_osc(rig);
    let expected = publish_physical_native_setup(rig, 0.3, 0.6);
    assert_root_copy_words(rig, &expected);

    let before = observe(rig);
    let (status, outcome) = http_action(rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_action_outcome(&outcome, true, 1, "HTTP Freeze");
    let revision = assert_one_mutation(rig, &before, "HTTP Freeze");
    assert_eq!(outcome["patch_revision"], revision);
    let via_http = native(rig).unwrap();
    assert_eq!(via_http, expected);
    assert_output_holds(rig, &via_http);

    let before = observe(rig);
    let (status, outcome) = http_action(rig, "unfreeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_action_outcome(&outcome, true, 1, "HTTP Unfreeze");
    assert_one_mutation(rig, &before, "HTTP Unfreeze");
    assert!(rig.fixture().freeze.is_empty());

    // Same physical native pose in a new accepted generation, now captured through OSC Enter.
    assert_eq!(publish_physical_native_setup(rig, 0.3, 0.6), expected);
    let before = observe(rig);
    assert!(osc_entered(rig, false, None), "OSC FREEZE POSITION");
    assert_one_mutation(rig, &before, "OSC Freeze");
    assert_eq!(command_line(rig), "", "accepted command clears the line");
    let via_osc = native(rig).unwrap();
    assert_eq!(via_osc, via_http, "HTTP and OSC hold identical payloads");
    assert_eq!(
        serde_json::to_value(&via_osc).unwrap(),
        serde_json::to_value(&via_http).unwrap()
    );
    assert_output_holds(rig, &via_osc);

    let before = observe(rig);
    assert!(osc_entered(rig, true, None), "OSC UNFREEZE POSITION");
    assert_one_mutation(rig, &before, "OSC Unfreeze");
    assert!(rig.fixture().freeze.is_empty());
    assert!(rig.persisted_freeze().is_empty());
}

#[tokio::test]
async fn physical_native_setup_http_action_and_osc_enter_hold_identical_inverted_root_copy_words() {
    http_and_osc_hold_identical_words(&Rig::new(false, false, true).await).await;
}

#[tokio::test]
async fn physical_native_setup_http_action_and_osc_enter_hold_identical_motor_alias_words() {
    http_and_osc_hold_identical_words(&Rig::new(false, true, true).await).await;
}

#[tokio::test]
async fn physical_native_setup_transport_undo_replay_and_later_output_keep_exact_payload_and_order()
{
    let rig = Rig::new(false, true, true).await;
    register_osc(&rig);
    let accepted = publish_physical_native_setup(&rig, 0.3, 0.6);
    let start = rig.state.events.latest_sequence();

    let before = observe(&rig);
    let (status, value) = http_command(&rig, "tl609-freeze", "FREEZE POSITION", None).await;
    assert_command_applied(status, &value, 1, "HTTP FREEZE POSITION");
    let frozen_revision = assert_one_mutation(&rig, &before, "HTTP FREEZE POSITION");
    assert_eq!(native(&rig).as_ref(), Some(&accepted));
    let frozen = rig.fixture().freeze;
    assert_eq!(freeze_history_len(&rig), 1);

    // Exact request replay: no second mutation, event or Undo checkpoint.
    let before = observe(&rig);
    let (status, value) = http_command(&rig, "tl609-freeze", "FREEZE POSITION", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_quiet(&rig, &before, "HTTP command replay");

    // A changed later accepted publication does not replace the selected payload on repeat.
    let later = rig.render(0.9, 0.15);
    rig.publish(&later, rig.show_id.0);
    let before = observe(&rig);
    let (status, outcome) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_action_outcome(&outcome, false, 1, "repeated HTTP Freeze");
    assert_quiet(&rig, &before, "repeated HTTP Freeze");
    assert!(osc_entered(&rig, false, None));
    assert_quiet(&rig, &before, "repeated OSC Freeze");
    assert_eq!(native(&rig).as_ref(), Some(&accepted));

    let before = observe(&rig);
    assert!(osc_entered(&rig, true, Some("tl609-osc-unfreeze")));
    let unfrozen_revision = assert_one_mutation(&rig, &before, "OSC Unfreeze");
    assert!(native(&rig).is_none());
    assert_eq!(freeze_history_len(&rig), 2);

    let before = observe(&rig);
    let undone = http_undo(&rig).await;
    assert_eq!(undone["changed"], true, "{undone}");
    let undo_revision = assert_one_mutation(&rig, &before, "transport Undo");
    assert_eq!(
        rig.fixture().freeze,
        frozen,
        "Undo restores the exact payload"
    );
    assert_eq!(rig.persisted_freeze(), frozen);
    assert_eq!(native(&rig).as_ref(), Some(&accepted));
    assert_eq!(freeze_history_len(&rig), 1);
    assert_output_holds(&rig, &accepted);

    // Mutation and event ordering: one PatchChanged per change, strictly sequential revisions.
    assert_eq!(
        patch_events(&rig, start)
            .into_iter()
            .map(|(_, revision)| revision)
            .collect::<Vec<_>>(),
        [frozen_revision, unfrozen_revision, undo_revision]
    );
    assert_eq!(unfrozen_revision, frozen_revision + 1);
    assert_eq!(undo_revision, unfrozen_revision + 1);

    // Explicit replacement: Unfreeze, publish a new physical native pose, Freeze again.
    let before = observe(&rig);
    let (status, value) = http_command(&rig, "tl609-unfreeze", "UNFREEZE POSITION", None).await;
    assert_command_applied(status, &value, 1, "HTTP UNFREEZE POSITION");
    assert_one_mutation(&rig, &before, "HTTP UNFREEZE POSITION");
    let replacement = publish_physical_native_setup(&rig, 0.8, 0.25);
    assert_ne!(replacement, accepted);
    assert!(osc_entered(&rig, false, None));
    assert_eq!(native(&rig).as_ref(), Some(&replacement));

    // Replaying an older request after intervening changes returns its stored outcome only: the
    // replacement hold, revisions, events and Undo checkpoints stay untouched.
    let before = observe(&rig);
    let (status, value) = http_command(&rig, "tl609-unfreeze", "UNFREEZE POSITION", None).await;
    assert_command_applied(status, &value, 1, "stale HTTP command replay");
    assert_quiet(&rig, &before, "stale HTTP command replay");
    assert_eq!(native(&rig).as_ref(), Some(&replacement));
}

#[tokio::test]
async fn missing_foreign_and_stale_accepted_output_is_quiet_on_every_transport() {
    let rig = Rig::new(false, true, true).await;
    register_osc(&rig);
    let attempts = |label: &'static str| {
        let rig = &rig;
        async move {
            let before = observe(rig);
            let (status, outcome) = http_action(rig, "freeze", Some(rig.show_id.0)).await;
            assert_eq!(status, StatusCode::OK, "{label}: {outcome}");
            assert_action_outcome(&outcome, false, 0, label);
            assert_eq!(outcome["patch_revision"], before.revisions.1);
            assert_quiet(rig, &before, label);
            let (status, outcome) = http_action(rig, "toggle", Some(rig.show_id.0)).await;
            assert_eq!(status, StatusCode::OK, "{label}: {outcome}");
            assert_action_outcome(&outcome, false, 0, label);
            assert_quiet(rig, &before, label);
            let (status, outcome) = http_full_action(rig).await;
            assert_eq!(status, StatusCode::OK, "{label}: {outcome}");
            assert_action_outcome(&outcome, false, 0, label);
            assert_quiet(rig, &before, label);
            let (status, value) = http_command(
                rig,
                &format!("tl609-quiet-{}", Uuid::new_v4()),
                "FREEZE POSITION",
                Some(rig.show_id.0),
            )
            .await;
            assert_command_applied(status, &value, 0, label);
            assert_quiet(rig, &before, label);
            assert!(osc_entered(rig, false, None), "{label}: OSC Enter");
            assert_quiet(rig, &before, label);
            assert!(rig.fixture().freeze.is_empty(), "{label}");
        }
    };

    assert!(rig.state.output.latest_visualization_frame().is_none());
    attempts("missing accepted output").await;

    let frame = rig.render(0.3, 0.6);
    rig.publish(&frame, Uuid::new_v4());
    attempts("foreign-show accepted output").await;

    rig.publish(&frame, rig.show_id.0);
    let mut replacement = rig.state.output.snapshot().as_ref().clone();
    replacement.revision += 1;
    rig.state.output.replace_snapshot(replacement).unwrap();
    attempts("stale accepted output").await;
    assert_eq!(freeze_history_len(&rig), 0);
    // No Freeze checkpoint exists: transport Undo falls through to the Programmer and never
    // touches the patch.
    let before = observe(&rig);
    http_undo(&rig).await;
    assert!(patch_events(&rig, before.sequence).is_empty());
    assert_eq!(revisions(&rig), before.revisions);
    assert!(rig.fixture().freeze.is_empty());
}

#[tokio::test]
async fn idempotent_removal_and_repetition_need_no_new_sample_on_every_transport() {
    let rig = Rig::new(false, true, true).await;
    register_osc(&rig);
    let accepted = publish_physical_native_setup(&rig, 0.3, 0.6);
    let (status, outcome) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_action_outcome(&outcome, true, 1, "initial Freeze");
    let frozen = rig.fixture().freeze;
    // The accepted publication now belongs to the pre-Freeze engine snapshot: it is stale.
    let generation = accepted_generation(&rig);

    let before = observe(&rig);
    let (_, outcome) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_action_outcome(&outcome, false, 1, "repeat HTTP");
    let (status, value) = http_command(&rig, "tl609-repeat", "FREEZE POSITION", None).await;
    assert_command_applied(status, &value, 1, "repeat HTTP command");
    assert!(osc_entered(&rig, false, None));
    assert_quiet(&rig, &before, "repeated Freeze with stale output");

    for (index, removal) in ["OSC", "HTTP command", "HTTP action"]
        .into_iter()
        .enumerate()
    {
        let before = observe(&rig);
        match index {
            0 => assert!(osc_entered(&rig, true, None)),
            1 => {
                let (status, value) =
                    http_command(&rig, "tl609-remove", "UNFREEZE POSITION", None).await;
                assert_command_applied(status, &value, 1, removal);
            }
            _ => {
                let (status, outcome) = http_action(&rig, "unfreeze", Some(rig.show_id.0)).await;
                assert_eq!(status, StatusCode::OK);
                assert_action_outcome(&outcome, true, 1, removal);
            }
        }
        assert_one_mutation(&rig, &before, removal);
        assert!(rig.fixture().freeze.is_empty(), "{removal}");
        assert_eq!(
            accepted_generation(&rig),
            generation,
            "{removal}: no sample"
        );

        // Idempotent repeated removal is quiet.
        let repeat = observe(&rig);
        let (_, outcome) = http_action(&rig, "unfreeze", Some(rig.show_id.0)).await;
        assert_eq!(outcome["changed"], false, "{removal}: {outcome}");
        assert_quiet(&rig, &repeat, removal);

        // Transport Undo restores the exact payload, again without a new sample.
        let undone = http_undo(&rig).await;
        assert_eq!(undone["changed"], true, "{removal}: {undone}");
        assert_eq!(rig.fixture().freeze, frozen, "{removal}: Undo");
        assert_eq!(native(&rig).as_ref(), Some(&accepted));
        assert_eq!(accepted_generation(&rig), generation);
    }
}

#[tokio::test]
async fn show_guard_authentication_and_unregistered_osc_reject_without_mutation() {
    let rig = Rig::new(false, true, true).await;
    register_osc(&rig);
    let accepted = publish_physical_native_setup(&rig, 0.3, 0.6);
    let before = observe(&rig);

    let foreign_show = Some(Uuid::new_v4());
    let (status, _) = http_action(&rig, "freeze", foreign_show).await;
    assert_eq!(status, StatusCode::CONFLICT, "foreign X-Tosk-Show action");
    let (status, _) = http_command(&rig, "tl609-foreign", "FREEZE POSITION", foreign_show).await;
    assert_eq!(status, StatusCode::CONFLICT, "foreign X-Tosk-Show command");
    let (status, _) = call(
        &rig,
        Method::POST,
        "/api/v2/fixture-freeze/actions",
        Some(json!({"operation":"freeze","families":["position"]})),
        Some(rig.show_id.0),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unauthenticated action");
    rig.state
        .programming
        .set_command_line(rig.session.id, "FREEZE POSITION".into());
    assert!(
        !osc_key(&rig, "127.0.0.1:9610", "enter", true, None),
        "an unregistered OSC source is not part of this desk"
    );
    rig.state
        .programming
        .set_command_line(rig.session.id, String::new());
    assert_quiet(&rig, &before, "rejected transports");

    // The rejected attempts did not consume or invalidate the accepted publication.
    let (status, outcome) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK);
    assert_action_outcome(&outcome, true, 1, "authorized Freeze");
    assert_eq!(native(&rig).as_ref(), Some(&accepted));
}

/// TL-609 defect D1 regression (engine output, not Freeze transport), repaired by TL-630. On an
/// independently inverted copy, a custom motor alias bound to the Pan role by the compiled
/// Position model must be mirrored on the wire exactly like a literal `pan` channel. The copy's
/// physical prediction then decodes its mirrored motor word back to the root's calibrated angle,
/// and the captured native Freeze baseline equals the wire. Before TL-630 the alias word was not
/// mirrored (`[19661, 39321]` on both), so the copy reported the opposite angle (+288° against
/// the root's -288°) and Freeze would hold a word that disagreed with the operator's intent.
/// PHYSICAL NATIVE SETUP only.
#[tokio::test]
async fn defect_d1_inverted_copy_motor_alias_wire_word_is_mirrored() {
    let rig = Rig::new(false, true, true).await;
    let canonical = Rig::new(false, false, true).await;
    let frame = rig.render(0.3, 0.6);
    let wire = wire_words(&rig, &frame);
    let axes = |frame: &RenderedSemanticFrame, instance: Uuid| {
        frame
            .rendered
            .physical
            .instances
            .iter()
            .find(|output| output.instance_id == instance)
            .unwrap()
            .axes()
            .iter()
            .map(|axis| axis.absolute_degrees().map(|degrees| degrees.round()))
            .collect::<Vec<_>>()
    };
    // The copy's Pan-role alias word is mirrored (normalized 1 - n, each side rounded to 16 bits,
    // so the pair sums to 65535 or 65536); its uninverted Tilt-role alias is identical.
    assert_eq!(wire[0], [19661, 39321], "root keeps the authored words");
    assert!(
        (65535..=65536).contains(&(wire[0][0] + wire[1][0])),
        "wire {wire:?}"
    );
    assert_eq!(wire[0][1], wire[1][1], "wire {wire:?}");
    // Exactly the words of literal `pan`/`tilt` channels in the same installation.
    let canonical_frame = canonical.render(0.3, 0.6);
    assert_eq!(wire, wire_words(&canonical, &canonical_frame));
    // The physical prediction agrees with the wire: the inverted copy's mirrored motor word is
    // reported at the root's calibrated angle, as for the canonical channels.
    let copy = rig.copy.unwrap();
    assert_eq!(axes(&frame, rig.root.0), [Some(-288.), Some(144.)]);
    assert_eq!(axes(&frame, copy), axes(&frame, rig.root.0));
    assert_eq!(
        axes(&frame, copy),
        axes(&canonical_frame, canonical.copy.unwrap())
    );
    // The captured native Freeze baseline holds exactly the wire words of root and copy.
    let captured = rig
        .state
        .output
        .engine()
        .position_freeze_from_physical(
            frame.rendered.generation,
            &frame.rendered.physical,
            rig.owner,
        )
        .expect("physical native setup yields a compiled native Position capture");
    assert_eq!(words_by_instance(&captured), wire);
}

/// Programmer values as comparable `(fixture, attribute, value)` rows.
fn programmer_values(rig: &Rig) -> Vec<String> {
    rig.state
        .programming
        .get(rig.session.id)
        .unwrap()
        .values
        .iter()
        .map(|value| {
            format!(
                "{:?} {:?} {:?}",
                value.fixture_id, value.attribute, value.value
            )
        })
        .collect()
}

/// The attached OSC `[UND]` key, pressed once.
fn osc_undo(rig: &Rig) -> bool {
    osc_key(rig, OSC_SOURCE, "undo", true, None)
}

/// TL-609 defect D2 / TL-631 regression (OSC Undo parity). The help contract says `[UND]`
/// restores the state from before the most recent Freeze or Unfreeze. HTTP
/// `/api/v2/programmer-undo/actions` and the desk WebSocket run `fixture_freeze::undo_latest`
/// first; the attached OSC `undo` key must run the same Freeze-aware step, restore the exact
/// previous payload with one patch change, and only then fall through to the Programmer.
/// PHYSICAL NATIVE SETUP only.
#[tokio::test]
async fn defect_d2_osc_undo_key_restores_state_before_the_latest_freeze() {
    let rig = Rig::new(false, false, true).await;
    register_osc(&rig);
    let accepted = publish_physical_native_setup(&rig, 0.3, 0.6);
    let start = rig.state.events.latest_sequence();
    assert!(rig.fixture().freeze.is_empty());

    let before = observe(&rig);
    let (status, outcome) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK);
    assert_action_outcome(&outcome, true, 1, "HTTP Freeze");
    let frozen_revision = assert_one_mutation(&rig, &before, "HTTP Freeze");
    let frozen = rig.fixture().freeze;
    assert_eq!(native(&rig).as_ref(), Some(&accepted));
    assert_eq!(freeze_history_len(&rig), 1);

    let before = observe(&rig);
    assert!(osc_entered(&rig, true, Some("tl631-osc-unfreeze")));
    let unfrozen_revision = assert_one_mutation(&rig, &before, "OSC Unfreeze");
    assert!(native(&rig).is_none());
    assert_eq!(freeze_history_len(&rig), 2);

    // OSC [UND] undoes the Unfreeze: the exact earlier Freeze payload returns, once.
    let before = observe(&rig);
    assert!(osc_undo(&rig), "OSC [UND] is handled");
    let first_undo_revision = assert_one_mutation(&rig, &before, "OSC [UND] after Unfreeze");
    assert_eq!(
        rig.fixture().freeze,
        frozen,
        "OSC [UND] restores the exact payload"
    );
    assert_eq!(rig.persisted_freeze(), frozen);
    assert_eq!(native(&rig).as_ref(), Some(&accepted));
    assert_output_holds(&rig, &accepted);
    assert_eq!(freeze_history_len(&rig), 1);
    assert_eq!(
        command_line(&rig).trim(),
        "",
        "Undo leaves the command line alone"
    );

    // OSC [UND] again undoes the Freeze itself: the pre-Freeze (empty) state returns, once.
    let before = observe(&rig);
    assert!(osc_undo(&rig));
    let second_undo_revision = assert_one_mutation(&rig, &before, "OSC [UND] after Freeze");
    assert!(
        rig.fixture().freeze.is_empty(),
        "OSC [UND] must remove the hold"
    );
    assert!(rig.persisted_freeze().is_empty());
    assert!(native(&rig).is_none());
    assert_eq!(freeze_history_len(&rig), 0);

    // No Freeze step remains: a further OSC [UND] is an ordinary Programmer Undo and never
    // touches the patch.
    let before = observe(&rig);
    assert!(osc_undo(&rig));
    assert!(patch_events(&rig, before.sequence).is_empty());
    assert_eq!(revisions(&rig), before.revisions);

    assert_eq!(
        patch_events(&rig, start)
            .into_iter()
            .map(|(_, revision)| revision)
            .collect::<Vec<_>>(),
        [
            frozen_revision,
            unfrozen_revision,
            first_undo_revision,
            second_undo_revision
        ]
    );
}

/// TL-631: OSC `[UND]` and HTTP Programmer Undo land on the identical Freeze payload, revision
/// step and history depth from identical starting points.
#[tokio::test]
async fn osc_and_http_undo_restore_identical_freeze_state() {
    let mut results = Vec::new();
    for osc in [false, true] {
        let rig = Rig::new(false, false, true).await;
        register_osc(&rig);
        let accepted = publish_physical_native_setup(&rig, 0.3, 0.6);
        let (status, _) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
        assert_eq!(status, StatusCode::OK);
        let frozen = rig.fixture().freeze;
        let (status, _) = http_action(&rig, "unfreeze", Some(rig.show_id.0)).await;
        assert_eq!(status, StatusCode::OK);
        let before = observe(&rig);
        if osc {
            assert!(osc_undo(&rig));
        } else {
            assert_eq!(http_undo(&rig).await["changed"], true);
        }
        let revision = assert_one_mutation(&rig, &before, if osc { "OSC" } else { "HTTP" });
        assert_eq!(rig.fixture().freeze, frozen);
        assert_eq!(native(&rig).as_ref(), Some(&accepted));
        results.push((
            revision - before.revisions.1,
            freeze_history_len(&rig),
            rig.state.programming.undo_depth(rig.session.id),
        ));
    }
    assert_eq!(results[0], results[1], "HTTP vs OSC [UND]");
}

/// TL-631: routing OSC `[UND]` through the Freeze-aware step keeps Programmer Undo ordering. A
/// Programmer change made after a Freeze is undone first, without touching the patch; the next
/// `[UND]` then undoes the Freeze.
#[tokio::test]
async fn osc_undo_key_undoes_a_later_programmer_change_before_the_freeze() {
    let rig = Rig::new(false, false, true).await;
    register_osc(&rig);
    publish_physical_native_setup(&rig, 0.3, 0.6);
    let (status, _) = http_action(&rig, "freeze", Some(rig.show_id.0)).await;
    assert_eq!(status, StatusCode::OK);
    let frozen = rig.fixture().freeze;
    assert!(!frozen.is_empty());
    let values_before = programmer_values(&rig);
    let depth_before = rig.state.programming.undo_depth(rig.session.id).unwrap();

    let (status, value) = http_command(&rig, "tl631-later-value", "FIXTURE 1 AT 50", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_ne!(
        programmer_values(&rig),
        values_before,
        "the command changed the Programmer"
    );
    assert_eq!(
        rig.state.programming.undo_depth(rig.session.id),
        Some(depth_before + 1)
    );

    let before = observe(&rig);
    assert!(osc_undo(&rig));
    assert_eq!(
        programmer_values(&rig),
        values_before,
        "Programmer change undone first"
    );
    assert_eq!(
        rig.state.programming.undo_depth(rig.session.id),
        Some(depth_before)
    );
    assert!(patch_events(&rig, before.sequence).is_empty());
    assert_eq!(revisions(&rig), before.revisions);
    assert_eq!(rig.fixture().freeze, frozen, "the Freeze is still held");
    assert_eq!(freeze_history_len(&rig), 1);

    let before = observe(&rig);
    assert!(osc_undo(&rig));
    assert_one_mutation(&rig, &before, "OSC [UND] of the Freeze");
    assert!(rig.fixture().freeze.is_empty());
    assert_eq!(freeze_history_len(&rig), 0);
    assert_eq!(programmer_values(&rig), values_before);
}

/// TL-631 guard: without any Freeze step, OSC `[UND]` is the unchanged Programming-service
/// Programmer Undo. It reverts the latest Programmer change, publishes no patch change and
/// matches what the pre-TL-631 key path did.
#[tokio::test]
async fn osc_undo_key_without_freeze_history_is_ordinary_programmer_undo() {
    let rig = Rig::new(false, false, true).await;
    register_osc(&rig);
    let empty = programmer_values(&rig);
    let (status, value) = http_command(&rig, "tl631-first", "FIXTURE 1 AT 50", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let first = programmer_values(&rig);
    let (status, value) = http_command(&rig, "tl631-second", "FIXTURE 1 AT 80", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let second = programmer_values(&rig);
    assert_ne!(first, empty);
    assert_ne!(second, first);
    let depth = rig.state.programming.undo_depth(rig.session.id).unwrap();

    let before = observe(&rig);
    assert!(osc_undo(&rig));
    assert_eq!(
        programmer_values(&rig),
        first,
        "latest Programmer change undone"
    );
    assert_eq!(
        rig.state.programming.undo_depth(rig.session.id),
        Some(depth - 1)
    );
    assert!(patch_events(&rig, before.sequence).is_empty());
    assert_eq!(revisions(&rig), before.revisions);
    assert_eq!(freeze_history_len(&rig), 0);

    assert!(osc_undo(&rig));
    assert_eq!(programmer_values(&rig), empty, "then the earlier one");
    assert!(rig.fixture().freeze.is_empty());
}
