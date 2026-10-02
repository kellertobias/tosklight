//! TL-560: semantic shows through the real headless startup path at contract 1, the dormant
//! contract-0 behaviour, and the legacy-programming validator with its recovery guarantees.
//!
//! Every test runs `StartupState::load` (not unit helpers) through the `#[cfg(test)]` startup
//! harness in `semantic_contract_startup_support`. Show content is written as raw portable
//! objects; nothing here runs `light-headless --show` or touches a committed `.show` asset.
use super::semantic_contract_startup_support::{
    ContractDesk, ContractShow, SEMANTIC_CONTRACT, load_startup_at,
};
use super::*;
use crate::runtime::output_scheduler::physical_adapters::{
    color::{
        profiles::{patched, rgbw, rgbwauv},
        tests::{magenta, program, uv_only_black},
        tests_direct::{catalogue, direct},
    },
    media_color::tests::{media_fixture, shipped_media_server},
    optics::{
        profiles::wash_a,
        tests::{field, focus},
    },
    position::tests::moving_head,
};
use light_core::programming::{PositionIntent, TargetReference};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_fixture::PatchedFixture;
use light_programmer::{GroupDefinition, Preset, PresetFamily};

const GROUP: &str = "tl560-live";
const LIST: &str = "00000000-0000-4004-8560-000000000001";

/// One family's authored intent on a fixture that can carry it.
struct Family {
    label: &'static str,
    fixture: PatchedFixture,
    /// The programmed fixture: the root, or a logical head such as a Media layer.
    target: FixtureId,
    attribute: &'static str,
    value: AttributeValue,
    /// Contract the content requires (Focus stays a contract-0 normalized owner).
    requires: u16,
    /// Also store the value on a live Group (Preset group values and a Cue Group change).
    live_group: bool,
}

fn fixture_for(profile: &light_fixture::FixtureProfile, seed: u128) -> PatchedFixture {
    patched(profile, FixtureId(Uuid::from_u128(seed)), 1)
}

fn position_angles() -> Family {
    Family {
        label: "position-angles",
        fixture: fixture_for(&moving_head(), 0x5601),
        target: FixtureId(Uuid::from_u128(0x5601)),
        attribute: "position",
        value: AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25))),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn position_target() -> Family {
    Family {
        label: "position-target",
        fixture: fixture_for(&moving_head(), 0x5602),
        target: FixtureId(Uuid::from_u128(0x5602)),
        attribute: "position",
        value: AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: Uuid::from_u128(0x560beef),
            },
            [0.5, -1.25, 2.0],
        ))),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn color_semantic() -> Family {
    Family {
        label: "color-semantic",
        fixture: fixture_for(&rgbwauv(None), 0x5603),
        target: FixtureId(Uuid::from_u128(0x5603)),
        attribute: "color",
        value: program(&magenta()),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn color_direct() -> Family {
    let profile = rgbw();
    let value = direct(&catalogue(&[&profile]), &profile, &[200, 10, 180, 40]);
    Family {
        label: "color-direct",
        fixture: fixture_for(&profile, 0x5604),
        target: FixtureId(Uuid::from_u128(0x5604)),
        attribute: "color",
        value,
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn uv() -> Family {
    Family {
        label: "uv",
        fixture: fixture_for(&rgbwauv(None), 0x5605),
        target: FixtureId(Uuid::from_u128(0x5605)),
        attribute: "color",
        value: program(&uv_only_black()),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn focus_family() -> Family {
    Family {
        label: "focus",
        fixture: fixture_for(&wash_a(), 0x5606),
        target: FixtureId(Uuid::from_u128(0x5606)),
        attribute: "focus",
        value: focus(0.37),
        requires: 0,
        live_group: false,
    }
}

fn zoom() -> Family {
    Family {
        label: "zoom",
        fixture: fixture_for(&wash_a(), 0x5607),
        target: FixtureId(Uuid::from_u128(0x5607)),
        attribute: "zoom",
        value: field(20.0),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn live_group_color() -> Family {
    Family {
        live_group: true,
        label: "live-group-color",
        ..color_semantic()
    }
}

fn media_color() -> Family {
    let profile = shipped_media_server();
    let layer = FixtureId(Uuid::from_u128(0x56081));
    Family {
        label: "media-color",
        fixture: media_fixture(
            &profile,
            FixtureId(Uuid::from_u128(0x5608)),
            &[layer, FixtureId(Uuid::from_u128(0x56082))],
        ),
        target: layer,
        attribute: "color",
        value: program(&magenta()),
        requires: SEMANTIC_CONTRACT,
        live_group: false,
    }
}

fn preset_body(family: &Family) -> serde_json::Value {
    let attribute = AttributeKey(family.attribute.into());
    let mut preset = Preset {
        name: format!("TL-560 {}", family.label),
        family: PresetFamily::Mixed,
        number: 1,
        ..Preset::default()
    };
    preset.values.insert(
        family.target,
        HashMap::from([(attribute.clone(), family.value.clone())]),
    );
    if family.live_group {
        preset.group_values.insert(
            GROUP.into(),
            HashMap::from([(attribute, family.value.clone())]),
        );
    }
    serde_json::to_value(preset).unwrap()
}

fn cue_list_body(
    changes: serde_json::Value,
    group_changes: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "id": LIST, "name": "TL-560", "mode": "sequence", "priority": 0, "looped": false,
        "restart_mode": "first_cue", "wrap_mode": "off", "intensity_priority_mode": "htp",
        "speed_group": null, "speed_multiplier": 1.0, "auto_off_at_zero": false,
        "auto_off_flash_release": false, "chaser_step_millis": 1000, "chaser_xfade_percent": 0,
        "disable_cue_timing": false, "force_cue_timing": false,
        "cues": [{
            "id": "00000000-0000-4003-8560-000000000001", "number": "1", "name": "Look",
            "actions": [], "cue_only": false, "delay_millis": 0, "fade_millis": 0,
            "dynamic_changes": [], "trigger": {"type": "manual"},
            "changes": changes, "group_changes": group_changes
        }]
    })
}

/// A show holding the family's intent in a Preset and a Cue (and a live Group when asked).
fn semantic_show(desk: &ContractDesk, family: &Family) -> ContractShow {
    let show = desk.create_show(&format!("TL-560 {}", family.label));
    show.patch(&family.fixture);
    let fixture = family.target;
    show.put("preset", "0.1", preset_body(family));
    let mut group_changes = serde_json::json!([]);
    if family.live_group {
        let group = GroupDefinition {
            id: GROUP.into(),
            name: "Live".into(),
            fixtures: vec![fixture],
            ..Default::default()
        };
        show.put("group", GROUP, serde_json::to_value(group).unwrap());
        group_changes = serde_json::json!([{
            "group_id": GROUP, "attribute": family.attribute, "value": family.value,
            "automatic_restore": false
        }]);
    }
    show.put(
        "cue_list",
        LIST,
        cue_list_body(
            serde_json::json!([{
                "fixture_id": fixture, "attribute": family.attribute, "value": family.value,
                "automatic_restore": false
            }]),
            group_changes,
        ),
    );
    desk.activate(&show);
    show
}

fn stored_cue_value(startup: &startup_state::StartupState) -> AttributeValue {
    let snapshot = startup.engine.snapshot();
    let list = snapshot
        .cue_lists
        .iter()
        .find(|list| list.id.0.to_string() == LIST)
        .expect("the semantic Cue list compiled");
    list.cues[0].changes[0]
        .value
        .clone()
        .expect("an authored value, not a release")
}

fn stored_preset(show: &ContractShow) -> Preset {
    let document = show.store().portable_document().unwrap();
    serde_json::from_value(document.object("preset", "0.1").unwrap().body().clone()).unwrap()
}

/// Real startup at contract 1 loads the family twice (save/reload) with exact intent: the first
/// load may commit the compiler's ordinary compatibility migration, after which the file is
/// stable. Real startup at contract 0 then enters recovery without changing those bytes (Focus,
/// a contract-0 owner, loads at both).
fn assert_family_round_trips(family: Family) {
    let desk = ContractDesk::new(family.label);
    let show = semantic_show(&desk, &family);
    let mut stable = None;
    for restart in 0..2 {
        let startup = desk.load_at(SEMANTIC_CONTRACT);
        assert_eq!(
            startup.active_show_error, None,
            "{} restart {restart}",
            family.label
        );
        assert_eq!(
            startup.engine.supported_programming_contract(),
            SEMANTIC_CONTRACT
        );
        let snapshot = startup.engine.snapshot();
        assert_eq!(
            snapshot.required_programming_contract(),
            family.requires,
            "{}",
            family.label
        );
        assert_eq!(snapshot.fixtures.len(), 1, "{}", family.label);
        assert_eq!(stored_cue_value(&startup), family.value, "{}", family.label);
        if family.live_group {
            let list = snapshot
                .cue_lists
                .iter()
                .find(|l| l.id.0.to_string() == LIST);
            let group_change = &list.unwrap().cues[0].group_changes[0];
            assert_eq!(group_change.group_id, GROUP);
            assert_eq!(group_change.value.as_ref(), Some(&family.value));
        }
        drop(startup);
        let preset = stored_preset(&show);
        assert_eq!(
            preset.values[&family.target][&AttributeKey(family.attribute.into())],
            family.value,
            "{}",
            family.label
        );
        let bytes = show.bytes();
        if let Some(stable) = &stable {
            assert_eq!(
                &bytes, stable,
                "{}: a reload does not rewrite",
                family.label
            );
        }
        stable = Some(bytes);
    }
    let stable = stable.unwrap();
    let startup = desk.load_at(0);
    if family.requires == 0 {
        assert_eq!(startup.active_show_error, None, "{}", family.label);
        assert_eq!(stored_cue_value(&startup), family.value);
    } else {
        let error = startup
            .active_show_error
            .clone()
            .unwrap_or_else(|| panic!("{} must not load at contract 0", family.label));
        assert!(
            error.contains("programming contract 1"),
            "{}: {error}",
            family.label
        );
        assert!(startup.engine.snapshot().fixtures.is_empty());
    }
    drop(startup);
    if family.requires != 0 {
        assert_eq!(
            show.bytes(),
            stable,
            "{}: contract 0 keeps the bytes",
            family.label
        );
    }
}

#[test]
fn contract_one_startup_loads_position_angles() {
    assert_family_round_trips(position_angles());
}

#[test]
fn contract_one_startup_loads_position_point_target() {
    assert_family_round_trips(position_target());
}

#[test]
fn contract_one_startup_loads_semantic_color() {
    assert_family_round_trips(color_semantic());
}

#[test]
fn contract_one_startup_loads_direct_color() {
    assert_family_round_trips(color_direct());
}

#[test]
fn contract_one_startup_loads_uv() {
    assert_family_round_trips(uv());
}

#[test]
fn contract_one_startup_loads_focus_and_contract_zero_still_does() {
    assert_family_round_trips(focus_family());
}

#[test]
fn contract_one_startup_loads_zoom() {
    assert_family_round_trips(zoom());
}

#[test]
fn contract_one_startup_loads_live_group_color() {
    assert_family_round_trips(live_group_color());
}

#[test]
fn contract_one_startup_loads_media_color() {
    assert_family_round_trips(media_color());
}

fn normalized(value: f32) -> serde_json::Value {
    serde_json::json!({"kind": "normalized", "value": value})
}

/// The demo generator's legacy shapes: Position preset `pan`/`tilt`, Cue `color.red` rows.
fn legacy_show(desk: &ContractDesk, name: &str) -> ContractShow {
    let show = desk.create_show(name);
    let fixture = fixture_for(&moving_head(), 0x56010);
    show.patch(&fixture);
    let id = fixture.fixture_id;
    show.put(
        "preset",
        "3.1",
        serde_json::json!({"family": "Position", "name": "Down", "number": 1,
            "group_values": {}, "values": {id.0.to_string(): {"pan": normalized(0.5), "tilt": normalized(0.25)}}}),
    );
    show.put(
        "cue_list",
        LIST,
        cue_list_body(
            serde_json::json!([
                {"fixture_id": id, "attribute": "intensity", "value": normalized(1.0), "automatic_restore": false},
                {"fixture_id": id, "attribute": "color.red", "value": normalized(1.0), "automatic_restore": false}
            ]),
            serde_json::json!([]),
        ),
    );
    show
}

const UNRELATED: (&str, &str) = ("tl560.unrelated_desk_preference", "kept");

#[tokio::test]
async fn legacy_show_is_rejected_at_contract_one_keeps_bytes_and_settings_and_a_new_show_opens() {
    let desk = ContractDesk::new("legacy");
    let show = legacy_show(&desk, "Legacy Tour");
    desk.activate(&show);
    desk.desk().set_setting(UNRELATED.0, UNRELATED.1).unwrap();
    let original = show.bytes();

    let startup = desk.load_at(SEMANTIC_CONTRACT);
    let error = startup
        .active_show_error
        .clone()
        .expect("visible rejection");
    for expected in [
        "The active show 'Legacy Tour' could not be loaded",
        "preset 3.1 pan (normalized Position)",
        "cue_list 00000000-0000-4004-8560-000000000001 color.red (Color component)",
        "never reinterpreted as degrees",
        "The show file was not changed",
        "Open or create a different show",
    ] {
        assert!(error.contains(expected), "{expected:?} missing: {error}");
    }
    assert!(
        startup.engine.snapshot().fixtures.is_empty(),
        "nothing loaded"
    );
    assert_eq!(
        startup
            .persistent
            .active_show
            .as_ref()
            .map(|entry| entry.id),
        Some(show.entry.id),
        "the operator still sees which show was rejected"
    );
    assert_eq!(show.bytes(), original, "original bytes preserved");
    let served = desk.serve(startup).await;
    let state = served.state.clone();
    assert_eq!(
        state.installation.setting(UNRELATED.0).unwrap().as_deref(),
        Some(UNRELATED.1),
        "unrelated desk settings are intact"
    );

    // A separate valid show still opens through the real show-library routes.
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let clean = create_show(&app, &token, "Clean after rejection").await;
    let response = app
        .clone()
        .oneshot(open_show_request(&token, clean["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state
            .active_show
            .current()
            .as_ref()
            .map(|entry| entry.id.0.to_string()),
        clean["id"].as_str().map(str::to_owned)
    );
    // Reopening the legacy show is refused the same way, without touching it.
    let response = app
        .clone()
        .oneshot(open_show_request(&token, show.entry.id.0))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("holds programming from before semantic programming contract 1"),
        "{body}"
    );
    assert_eq!(
        state
            .active_show
            .current()
            .as_ref()
            .map(|entry| entry.id.0.to_string()),
        clean["id"].as_str().map(str::to_owned),
        "the clean show stays active"
    );
    drop(app);
    served.close().await;
    drop(state);
    assert_eq!(show.bytes(), original, "still the original bytes");
    assert_eq!(
        desk.desk().setting(UNRELATED.0).unwrap().as_deref(),
        Some(UNRELATED.1)
    );
}

#[test]
fn legacy_show_still_loads_at_contract_zero_because_the_validator_is_dormant() {
    let desk = ContractDesk::new("legacy-zero");
    let show = legacy_show(&desk, "Legacy Zero");
    desk.activate(&show);
    let startup = desk.load_at(0);
    assert_eq!(startup.active_show_error, None);
    assert_eq!(startup.engine.snapshot().fixtures.len(), 1);
    assert_eq!(startup.engine.supported_programming_contract(), 0);
}

#[test]
fn malformed_or_newer_shows_are_rejected_at_contract_one_without_blocking_startup() {
    let desk = ContractDesk::new("malformed");
    // A newer contract marker on otherwise semantic content.
    let family = color_semantic();
    let show = semantic_show(&desk, &family);
    show.store()
        .set_metadata_values(&[(light_show::PROGRAMMING_CONTRACT_METADATA_KEY, "2")])
        .unwrap();
    let original = show.bytes();
    let startup = desk.load_at(SEMANTIC_CONTRACT);
    let error = startup.active_show_error.clone().unwrap();
    assert!(
        error.contains("written for programming contract 2; this desk supports 1"),
        "{error}"
    );
    drop(startup);
    assert_eq!(show.bytes(), original);
    // Contract 0 ignores the marker; the content gate still applies there.
    let startup = desk.load_at(0);
    assert!(
        startup
            .active_show_error
            .as_deref()
            .is_some_and(|error| error.contains("programming contract 1"))
    );
    drop(startup);

    // A file that is not a show at all.
    let broken = desk.create_show("Broken");
    std::fs::write(&broken.path, b"not a show").unwrap();
    desk.activate(&broken);
    let startup = desk.load_at(SEMANTIC_CONTRACT);
    let error = startup.active_show_error.clone().unwrap();
    assert!(
        error.contains("might be corrupted or incompatible"),
        "{error}"
    );
    assert!(startup.engine.snapshot().fixtures.is_empty());
    drop(startup);
    assert_eq!(std::fs::read(&broken.path).unwrap(), b"not a show");
}

#[test]
fn the_packaged_default_show_is_semantic_after_the_tl552_swap() {
    // TL-552 inverted this test (it pinned the legacy demo before the swap). A fresh desk
    // provisions the packaged demo as its default show. It is the regenerated contract-1 demo:
    // marker 1, no legacy programming, and production startup loads it with its Presets intact.
    // An older contract-0 runtime refuses it visibly and keeps the file unchanged.
    let desk = ContractDesk::new("default-show");
    // Production: no harness override at all.
    let startup = startup_state::StartupState::load(desk.options()).unwrap();
    assert_eq!(startup.active_show_error, None);
    assert_eq!(
        startup.engine.supported_programming_contract(),
        SEMANTIC_CONTRACT
    );
    assert!(!startup.engine.snapshot().fixtures.is_empty());
    let default_path = startup
        .persistent
        .active_show
        .as_ref()
        .unwrap()
        .path
        .clone();
    drop(startup);
    let report = light_show::inspect_show_programming_contract(&default_path).unwrap();
    assert!(report.legacy.is_empty(), "{:?}", report.legacy);
    assert_eq!(
        report.marker,
        light_show::ProgrammingContractMarker::Declared(SEMANTIC_CONTRACT)
    );
    let startup = desk.load_at(SEMANTIC_CONTRACT);
    assert_eq!(startup.active_show_error, None);
    drop(startup);

    // Every startup re-stamps the default show's desk identity, so the bytes are not a stable
    // comparison point here (the TL-560 legacy version of this test did not compare them either).
    let startup = desk.load_at(0);
    let error = startup.active_show_error.clone().unwrap();
    assert!(error.contains("programming contract 1"), "{error}");
    drop(startup);
    assert!(
        std::path::Path::new(&default_path).exists(),
        "an older runtime keeps the default show"
    );
    assert!(
        !desk.data_dir.join("backups").exists()
            || std::fs::read_dir(desk.data_dir.join("backups"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("unloadable")),
        "the semantic default show is not treated as structurally invalid"
    );
    // The data-dir copy (never the committed asset) holds semantic Position and Color presets.
    let presets = ShowStore::open(&default_path)
        .unwrap()
        .objects("preset")
        .unwrap();
    let bodies = presets
        .iter()
        .map(|preset| preset.body.to_string())
        .collect::<Vec<_>>();
    assert!(
        bodies
            .iter()
            .any(|body| body.contains(r#""kind":"angles""#))
    );
    assert!(
        bodies
            .iter()
            .any(|body| body.contains(r#""kind":"color_program""#))
    );
}

fn programmer_with_legacy_pan(session: SessionId) -> String {
    let programmers = ProgrammerRegistry::default();
    programmers.start(session);
    programmers.set(
        session,
        FixtureId::new(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.5),
    );
    serde_json::to_string(&programmers.get(session).unwrap()).unwrap()
}

#[test]
fn legacy_programmer_is_preserved_for_recovery_at_contract_one_and_restored_at_zero() {
    for contract in [SEMANTIC_CONTRACT, 0] {
        let desk = ContractDesk::new("programmer");
        let session = SessionId::new();
        let original = programmer_with_legacy_pan(session);
        desk.desk()
            .save_session(&light_show::PersistedSession {
                id: session,
                token: "tl560-never-copied".into(),
                programmer_json: original.clone(),
                connected: false,
                updated_at: fixed_test_time().to_rfc3339(),
            })
            .unwrap();
        let startup = desk.load_at(contract);
        if contract == 0 {
            assert!(startup.programmers.get(session).is_some());
            continue;
        }
        let error = startup.active_show_error.clone().unwrap();
        assert!(error.contains("original data is preserved"), "{error}");
        assert!(startup.programmers.get(session).is_none());
        assert_eq!(
            startup.persistent.desk.persisted_sessions().unwrap()[0].programmer_json,
            original
        );
        let reports: Vec<_> = std::fs::read_dir(desk.data_dir.join("backups"))
            .unwrap()
            .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
            .filter(|report| report.contains("before semantic programming contract 1"))
            .collect();
        assert_eq!(
            reports.len(),
            1,
            "one recovery report names the legacy values"
        );
        assert!(reports[0].contains("pan"));
        assert!(!reports[0].contains("tl560-never-copied"));
    }
}

#[test]
fn the_startup_harness_is_scoped_to_its_call() {
    let desk = ContractDesk::new("scope");
    let show = semantic_show(&desk, &zoom());
    let at_one = load_startup_at(SEMANTIC_CONTRACT, desk.options()).unwrap();
    assert_eq!(
        at_one.engine.supported_programming_contract(),
        SEMANTIC_CONTRACT
    );
    drop(at_one);
    let at_zero = load_startup_at(0, desk.options()).unwrap();
    assert_eq!(at_zero.engine.supported_programming_contract(), 0);
    assert!(
        at_zero.active_show_error.is_some(),
        "zoom intent needs contract 1"
    );
    drop(at_zero);
    // Without the harness the test binary reports production's contract (TL-552: 1).
    let plain = startup_state::StartupState::load(desk.options()).unwrap();
    assert_eq!(
        plain.engine.supported_programming_contract(),
        SEMANTIC_CONTRACT
    );
    assert_eq!(
        plain.active_show_error, None,
        "production loads Zoom intent"
    );
    drop(plain);
    let _ = show;
}

/// The real Preset writer (`/api/v2/presets/record` → `ActiveShowService` → the server unit of
/// work) stamps the marker in the same commit at contract 1 and never at contract 0.
#[tokio::test]
async fn the_preset_writer_stamps_the_marker_only_at_contract_one() {
    for contract in [SEMANTIC_CONTRACT, 0] {
        let (state, data_dir) =
            test_state_with_programming_contract(ProgrammerRegistry::default(), None, contract);
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, "Marker writer").await;
        let show_id = show["id"].as_str().unwrap().to_owned();
        let response = app
            .clone()
            .oneshot(open_show_request(&token, &show_id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let fixture = schema_v2_direct_fixture().0;
        let fixture_id = fixture.fixture_id;
        state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture].into(),
                revision: 1,
                ..EngineSnapshot::default()
            })
            .unwrap();
        let post = |path: &str, body: serde_json::Value| {
            Request::post(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.as_str())
                .body(Body::from(body.to_string()))
                .unwrap()
        };
        let set = serde_json::json!({
            "request_id": "tl560-marker-value", "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {"type": "set_fixture", "fixture_id": fixture_id.0, "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5}}
        });
        let response = app
            .clone()
            .oneshot(post("/api/v2/programmer/values/actions", set))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let entry = state.active_show.current().clone().unwrap();
        let marker = || {
            ShowStore::open(&entry.path)
                .unwrap()
                .programming_contract_marker()
                .unwrap()
        };
        assert_eq!(marker(), light_show::ProgrammingContractMarker::Absent);
        let record = serde_json::json!({
            "request_id": "tl560-marker-record", "address": {"family": "mixed", "number": 7},
            "name": "Marker", "mode": "overwrite", "expected_object_revision": 0
        });
        let response = app
            .clone()
            .oneshot(post("/api/v2/presets/record", record))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "contract {contract}");
        let expected = if contract == 0 {
            light_show::ProgrammingContractMarker::Absent
        } else {
            light_show::ProgrammingContractMarker::Declared(SEMANTIC_CONTRACT)
        };
        assert_eq!(marker(), expected, "contract {contract}");
        if let Some(cached) = state.active_show.document_cache().snapshot() {
            assert_eq!(
                cached.programming_contract_marker(),
                expected,
                "the retained document matches the file"
            );
        }
        light_show::validate_show_programming_contract(&entry.path, contract).unwrap();
        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
