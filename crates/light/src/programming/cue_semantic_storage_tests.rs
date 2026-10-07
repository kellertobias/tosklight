//! TL-606: requested semantic intent through the actual Cue recording writer and show storage.
//!
//! These regressions reuse the real `cue_active_show_tests` rig: `commit_programming_cue`
//! prepares the candidate with `prepare_show_candidate`, the rig's ports validate and install the
//! compiled `EngineSnapshot`, and every assertion that names storage reopens the SQLite file with
//! `ShowStore::open`. The capture-lane tests additionally drive
//! `ProgrammingService::handle_cue_recording`, whose `ProgrammerRegistry::capture_cue_recording`
//! produces the Normal, PendingPreload and ActivePreload captures that are committed here.
//!
//! Payload equality proves retained authoring only. Nothing here encodes output, fits a fixture,
//! activates startup support or claims TL-560.
use super::*;
use crate::programming::semantic_intent_cases::{
    assert_semantic_value, color, focus, group_family, magenta_with_uv, point_target,
    uv_only_black, warm_white_3200, warm_white_zero_output, zoom,
};
use crate::{
    ActionEnvelope, CueNumber, PlaybackCueReference, ProgrammingCueActivationCompletion,
    ProgrammingCueCommitResult, ProgrammingCueRecordOutcome, ProgrammingCueRecordingPorts,
    ProgrammingService, prepare_show_candidate,
};
use light_core::programming::{
    ColorProgram, NativeColorRecipe, PortableColorEstimate, PortableUv, PortableVisibleColor,
    PositionIntent, ProgrammingOwner, ScalarIntent, TargetReference,
};
use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality, SessionId, Xyz};
use light_playback::{CueChange, GroupCueChange};
use light_programmer::{
    CueRecordingCapturedSource, CueRecordingGroupValue, GroupDefinition, HighlightRegistry,
    ProgrammerRegistry,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const GROUP: &str = "tl606-front";
const POINT: Uuid = Uuid::from_u128(0x0606_0f0f);

fn fixture_a() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0606_00a1))
}
fn fixture_b() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0606_00b1))
}
/// Unpatched and outside the Group's membership: a dormant fixture row and Group member exception.
fn dormant() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0606_00d1))
}

fn key(owner: ProgrammingOwner) -> AttributeKey {
    owner.key()
}

/// Angles beyond one turn. Storage must not wrap them into a canonical range.
fn unwrapped_angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(540.0, -200.5)))
}

/// A tagged Direct Color with a pinned native identity, exact raw channels and a portable estimate.
fn direct(raw: u32) -> AttributeValue {
    let source = NativeColorIdentity {
        profile_id: Uuid::from_u128(0x0006_0601),
        profile_revision: 4,
        profile_digest: "tl606-profile-digest".into(),
        mode_id: Uuid::from_u128(0x0006_0602),
        head_id: Uuid::from_u128(0x0006_0603),
        path_id: Uuid::from_u128(0x0006_0604),
        model_revision: 2,
        native_layout_signature: "tl606-layout".into(),
    };
    let program = ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source,
            channels: (0..3u128)
                .map(|index| NativeColorValue {
                    channel_id: Uuid::from_u128(0x0006_0610 + index),
                    function_id: Uuid::from_u128(0x0006_0620 + index),
                    raw: raw + index as u32,
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 2,
            visible: Some(PortableVisibleColor {
                xyz: Xyz {
                    x: 0.11,
                    y: 0.07,
                    z: 0.02,
                },
                relative_output: 1.0,
            }),
            uv: Some(PortableUv {
                amount: 0.4,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["recorded".into()],
        },
    };
    program.validate().unwrap();
    AttributeValue::ColorProgram(Arc::new(program))
}

/// Requested Programmer intent for one recording, in Programmer order.
#[derive(Clone)]
struct Intent {
    fixtures: Vec<(FixtureId, AttributeKey, AttributeValue)>,
    groups: Vec<(String, AttributeKey, AttributeValue)>,
}

fn group_color() -> AttributeValue {
    group_family(
        ProgrammingOwner::Color,
        color(uv_only_black()),
        [
            (fixture_a().0, color(magenta_with_uv())),
            (dormant().0, color(warm_white_zero_output())),
        ],
    )
}

fn group_position() -> AttributeValue {
    group_family(
        ProgrammingOwner::Position,
        unwrapped_angles(),
        [(fixture_b().0, point_target(POINT))],
    )
}

fn full_intent() -> Intent {
    Intent {
        fixtures: vec![
            (
                fixture_a(),
                key(ProgrammingOwner::Color),
                color(magenta_with_uv()),
            ),
            (
                fixture_a(),
                key(ProgrammingOwner::Position),
                unwrapped_angles(),
            ),
            (fixture_a(), key(ProgrammingOwner::Focus), focus()),
            (fixture_a(), key(ProgrammingOwner::Zoom), zoom()),
            (fixture_b(), key(ProgrammingOwner::Color), direct(1000)),
            (
                fixture_b(),
                key(ProgrammingOwner::Position),
                point_target(POINT),
            ),
            (
                dormant(),
                key(ProgrammingOwner::Color),
                color(uv_only_black()),
            ),
        ],
        groups: vec![
            (GROUP.into(), key(ProgrammingOwner::Color), group_color()),
            (
                GROUP.into(),
                key(ProgrammingOwner::Position),
                group_position(),
            ),
            (GROUP.into(), key(ProgrammingOwner::Focus), focus()),
            (
                GROUP.into(),
                key(ProgrammingOwner::Zoom),
                group_family(ProgrammingOwner::Zoom, zoom(), []),
            ),
        ],
    }
}

fn semantic_capture(intent: &Intent) -> CueRecordingCapture {
    let mut order = 0;
    let mut next = || {
        order += 1;
        order
    };
    CueRecordingCapture {
        source: CueRecordingCapturedSource::Normal,
        fixture_values: intent
            .fixtures
            .iter()
            .map(|(fixture_id, attribute, value)| CueRecordingFixtureValue {
                fixture_id: *fixture_id,
                attribute: attribute.clone(),
                value: value.clone(),
                programmer_order: next(),
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect(),
        group_values: intent
            .groups
            .iter()
            .map(|(group_id, attribute, value)| CueRecordingGroupValue {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: value.clone(),
                programmer_order: next(),
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect(),
        group_release_values: Vec::new(),
        dynamic_values: Vec::new(),
    }
}

fn sorted_changes(changes: &[CueChange]) -> Vec<CueChange> {
    let mut changes = changes.to_vec();
    changes.sort_by_key(|change| (change.fixture_id.0, change.attribute.0.to_string()));
    changes
}

fn sorted_group_changes(changes: &[GroupCueChange]) -> Vec<GroupCueChange> {
    let mut changes = changes.to_vec();
    changes.sort_by_key(|change| (change.group_id.clone(), change.attribute.0.to_string()));
    changes
}

fn expected_changes(intent: &Intent) -> Vec<CueChange> {
    sorted_changes(
        &intent
            .fixtures
            .iter()
            .map(|(fixture_id, attribute, value)| {
                CueChange::set(*fixture_id, attribute.clone(), value.clone())
            })
            .collect::<Vec<_>>(),
    )
}

fn expected_group_changes(intent: &Intent) -> Vec<GroupCueChange> {
    sorted_group_changes(
        &intent
            .groups
            .iter()
            .map(|(group_id, attribute, value)| GroupCueChange {
                group_id: group_id.clone(),
                attribute: attribute.clone(),
                value: Some(value.clone()),
                automatic_restore: false,
                fade_millis: None,
                delay_millis: None,
            })
            .collect::<Vec<_>>(),
    )
}

fn seed_group(rig: &TestRig) {
    rig.seed(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "TL-606 front".into(),
            fixtures: vec![fixture_a(), fixture_b()],
            ..Default::default()
        })
        .unwrap(),
    );
}

/// Finds one stored row by its exact target and attribute.
fn stored_row<'a>(rows: &'a Value, target_field: &str, target: &str, attribute: &str) -> &'a Value {
    let matches = rows
        .as_array()
        .expect("stored change rows")
        .iter()
        .filter(|row| row[target_field] == target && row["attribute"] == attribute)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{target}/{attribute} in {rows}");
    matches[0]
}

/// Exact authored payloads in one stored Cue body: every row is present once, its stored value is
/// the JSON of the requested value with no numeric drift, semantic rows carry no native recipe,
/// estimate or wheel substitution, and the tagged Direct row is the exact tagged program.
fn assert_stored_cue(cue: &Value, intent: &Intent) {
    let changes = &cue["changes"];
    assert_eq!(
        changes.as_array().unwrap().len(),
        intent.fixtures.len(),
        "{changes}"
    );
    for (fixture_id, attribute, value) in &intent.fixtures {
        let row = stored_row(
            changes,
            "fixture_id",
            &fixture_id.0.to_string(),
            &attribute.0,
        );
        assert_eq!(row["value"], serde_json::to_value(value).unwrap(), "{row}");
        if row["value"]["value"]["kind"] == "direct" {
            assert_eq!(row["value"], serde_json::to_value(direct(1000)).unwrap());
        } else {
            assert_semantic_value(&row["value"]);
        }
    }
    let groups = &cue["group_changes"];
    assert_eq!(
        groups.as_array().unwrap().len(),
        intent.groups.len(),
        "{groups}"
    );
    for (group_id, attribute, value) in &intent.groups {
        let row = stored_row(groups, "group_id", group_id, &attribute.0);
        assert_eq!(row["value"], serde_json::to_value(value).unwrap(), "{row}");
        assert_semantic_value(&row["value"]);
    }
}

/// Literal persisted facts the acceptance criteria name, read back from the stored JSON body.
fn assert_named_semantic_facts(cue: &Value) {
    let changes = &cue["changes"];
    let a = fixture_a().0.to_string();
    let b = fixture_b().0.to_string();
    let d = dormant().0.to_string();
    let color_key = key(ProgrammingOwner::Color).0.to_string();
    let position_key = key(ProgrammingOwner::Position).0.to_string();

    let a_color = &stored_row(changes, "fixture_id", &a, &color_key)["value"]["value"]["intent"];
    assert_eq!(a_color["uv"]["amount"].as_f64().unwrap() as f32, 0.45);
    let uv_only = &stored_row(changes, "fixture_id", &d, &color_key)["value"]["value"]["intent"];
    assert_eq!(uv_only["uv"]["amount"].as_f64().unwrap() as f32, 0.9);
    assert_eq!(uv_only["base_xyz"]["y"], json!(0.0), "zero-Y kept");
    assert_eq!(
        uv_only["relative_output"],
        json!(0.0),
        "zero output stored explicitly"
    );
    let b_color = &stored_row(changes, "fixture_id", &b, &color_key)["value"]["value"];
    assert_eq!(b_color["kind"], "direct");
    assert_eq!(
        b_color["recipe"]["source"]["profile_digest"],
        "tl606-profile-digest"
    );
    assert_eq!(
        b_color["portable"]["uv"]["amount"].as_f64().unwrap() as f32,
        0.4
    );

    let decode = |row: &Value| -> AttributeValue {
        serde_json::from_value(row["value"].clone()).expect("typed stored value")
    };
    let AttributeValue::Position(angles) =
        decode(stored_row(changes, "fixture_id", &a, &position_key))
    else {
        panic!("Angles row")
    };
    assert_eq!(
        *angles,
        PositionIntent::Angles {
            pan_degrees: ScalarIntent::Value(540.0),
            tilt_degrees: ScalarIntent::Value(-200.5),
        },
        "unwrapped Angles are not wrapped"
    );
    let AttributeValue::Position(target) =
        decode(stored_row(changes, "fixture_id", &b, &position_key))
    else {
        panic!("Target row")
    };
    assert_eq!(
        *target,
        PositionIntent::Target {
            reference: TargetReference::Point { point_id: POINT },
            offset_metres: [0.5, -1.25, 2.0].map(ScalarIntent::Value),
        }
    );
    for owner in [ProgrammingOwner::Focus, ProgrammingOwner::Zoom] {
        stored_row(changes, "fixture_id", &a, &owner.key().0);
    }
    assert_ne!(
        key(ProgrammingOwner::Focus),
        key(ProgrammingOwner::Zoom),
        "Focus and Zoom are independent rows"
    );

    let group_color = &stored_row(&cue["group_changes"], "group_id", GROUP, &color_key)["value"];
    assert_eq!(
        group_color["value"]["template"],
        serde_json::to_value(color(uv_only_black())).unwrap()
    );
    let members = group_color["value"]["members"].as_object().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(
        members[&d],
        serde_json::to_value(color(warm_white_zero_output())).unwrap(),
        "dormant member exception kept"
    );
    assert_eq!(
        members[&a],
        serde_json::to_value(color(magenta_with_uv())).unwrap()
    );
}

/// The Cue with `cue_id` in one compiled Cuelist, compared as typed requested intent.
fn assert_compiled_cue(
    snapshot: &EngineSnapshot,
    cue_list_id: CueListId,
    cue_id: Uuid,
    intent: &Intent,
) {
    let list = snapshot
        .cue_lists
        .iter()
        .find(|list| list.id == cue_list_id)
        .expect("compiled Cuelist");
    let cue = list
        .cues
        .iter()
        .find(|cue| cue.id == cue_id)
        .expect("compiled Cue");
    assert_eq!(sorted_changes(&cue.changes), expected_changes(intent));
    assert_eq!(
        sorted_group_changes(&cue.group_changes),
        expected_group_changes(intent)
    );
}

/// Stored body, installed snapshot and a fresh show-open compile all hold the same exact intent.
fn assert_recorded_everywhere(
    rig: &TestRig,
    result: &ProgrammingCueCommitResult,
    intent: &Intent,
) -> CueListId {
    let object_id = result.projections.cue_list.object_id.clone();
    let cue_list_id = CueListId(object_id.parse().unwrap());
    let cue_id = result.recorded_cue.id;
    let document = rig.document();
    let body = document.object("cue_list", &object_id).unwrap().body();
    let cue = body["cues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cue| cue["id"] == cue_id.to_string())
        .unwrap();
    assert_stored_cue(cue, intent);
    assert_eq!(
        body,
        result.projections.cue_list.raw_body.as_ref(),
        "the committed projection is the reopened body"
    );

    let installed = rig
        .ports
        .installed
        .lock()
        .clone()
        .expect("installed runtime");
    assert_eq!(installed.revision, document.revision().value());
    assert_compiled_cue(&installed, cue_list_id, cue_id, intent);

    let (_, reopened) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    assert_compiled_cue(&reopened, cue_list_id, cue_id, intent);
    let installed_list = installed
        .cue_lists
        .iter()
        .find(|list| list.id == cue_list_id);
    let reopened_list = reopened
        .cue_lists
        .iter()
        .find(|list| list.id == cue_list_id);
    assert_eq!(installed_list, reopened_list);
    cue_list_id
}

/// Byte-level and revision fingerprint of the stored show, read from a fresh `ShowStore::open`.
#[derive(Debug, PartialEq)]
struct StorageFingerprint {
    revision: u64,
    objects: Vec<(String, String, u64, Vec<u8>)>,
}

fn fingerprint(rig: &TestRig) -> StorageFingerprint {
    let document = rig.document();
    let mut objects = document
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.revision(),
                serde_json::to_vec(object.body()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    objects.sort();
    StorageFingerprint {
        revision: document.revision().value(),
        objects,
    }
}

fn record(
    rig: &TestRig,
    cue_list_id: CueListId,
    operation: ProgrammingCueRecordOperation,
    capture: CueRecordingCapture,
) -> Result<ProgrammingCueCommitResult, ActionError> {
    let commit = rig.commit(
        ProgrammingCueRecordTarget::CueList { cue_list_id },
        ProgrammingCueResolvedTarget::CueList { cue_list_id },
        operation,
        Some(1.0),
        capture,
    );
    rig.service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
}

/// A rig whose show already holds the Group and one Cuelist recorded from `intent`.
fn recorded_rig(intent: &Intent) -> (TestRig, CueListId) {
    recorded_rig_with(TestRig::new(), intent)
}

fn recorded_rig_with(rig: TestRig, intent: &Intent) -> (TestRig, CueListId) {
    seed_group(&rig);
    let commit = rig.commit(
        ProgrammingCueRecordTarget::Pool { playback_number: 6 },
        ProgrammingCueResolvedTarget::Playback {
            playback_number: 6,
            page_slot: None,
        },
        ProgrammingCueRecordOperation::Overwrite,
        Some(1.0),
        semantic_capture(intent),
    );
    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();
    assert!(result.changed && result.created_topology);
    let cue_list_id = assert_recorded_everywhere(&rig, &result, intent);
    rig.ports.steps.lock().clear();
    (rig, cue_list_id)
}

#[test]
fn recorded_semantic_cue_reopens_with_exact_fixture_group_and_direct_payloads() {
    let intent = full_intent();
    let (rig, cue_list_id) = recorded_rig(&intent);

    let document = rig.document();
    let body = document
        .object("cue_list", &cue_list_id.0.to_string())
        .unwrap()
        .body();
    assert_named_semantic_facts(&body["cues"][0]);
    assert_eq!(rig.service.events().latest_sequence(), 1);
    assert_eq!(document.objects_of_kind("playback").count(), 1);
}

#[test]
fn merge_replaces_only_captured_rows_and_keeps_every_other_intent_exact() {
    let original = full_intent();
    let (rig, cue_list_id) = recorded_rig(&original);
    let revision = rig.document().revision();

    let merge = Intent {
        fixtures: vec![(
            fixture_a(),
            key(ProgrammingOwner::Color),
            color(warm_white_3200()),
        )],
        groups: vec![(
            GROUP.into(),
            key(ProgrammingOwner::Position),
            group_family(
                ProgrammingOwner::Position,
                point_target(POINT),
                [(dormant().0, unwrapped_angles())],
            ),
        )],
    };
    let result = record(
        &rig,
        cue_list_id,
        ProgrammingCueRecordOperation::Merge,
        semantic_capture(&merge),
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(result.recorded_cue.id, rig_cue_id(&rig, cue_list_id));
    assert_eq!(result.show_revision.value(), revision.value() + 1);

    let mut expected = original;
    for (fixture_id, attribute, value) in merge.fixtures {
        let row = expected
            .fixtures
            .iter_mut()
            .find(|row| row.0 == fixture_id && row.1 == attribute)
            .unwrap();
        row.2 = value;
    }
    for (group_id, attribute, value) in merge.groups {
        let row = expected
            .groups
            .iter_mut()
            .find(|row| row.0 == group_id && row.1 == attribute)
            .unwrap();
        row.2 = value;
    }
    assert_recorded_everywhere(&rig, &result, &expected);
    assert_eq!(
        rig.steps(),
        [
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
}

#[test]
fn overwrite_replaces_the_cue_with_exactly_the_new_requested_intent() {
    let (rig, cue_list_id) = recorded_rig(&full_intent());

    let overwrite = Intent {
        fixtures: vec![
            (
                dormant(),
                key(ProgrammingOwner::Color),
                color(uv_only_black()),
            ),
            (fixture_b(), key(ProgrammingOwner::Color), direct(1000)),
            (fixture_b(), key(ProgrammingOwner::Zoom), zoom()),
        ],
        groups: vec![(GROUP.into(), key(ProgrammingOwner::Color), group_color())],
    };
    let result = record(
        &rig,
        cue_list_id,
        ProgrammingCueRecordOperation::Overwrite,
        semantic_capture(&overwrite),
    )
    .unwrap();
    assert!(result.changed);
    assert_eq!(result.recorded_cue.id, rig_cue_id(&rig, cue_list_id));
    assert_recorded_everywhere(&rig, &result, &overwrite);
}

#[test]
fn identical_overwrite_and_merge_after_reopen_are_verified_no_changes() {
    let intent = full_intent();
    let (rig, cue_list_id) = recorded_rig(&intent);
    let before = fingerprint(&rig);
    let installed = installed_json(&rig);
    let events = rig.service.events().latest_sequence();

    for operation in [
        ProgrammingCueRecordOperation::Overwrite,
        ProgrammingCueRecordOperation::Merge,
        ProgrammingCueRecordOperation::AddMissing,
    ] {
        // Every attempt opens the store again, so the comparison reads reopened storage.
        let result = record(&rig, cue_list_id, operation, semantic_capture(&intent)).unwrap();
        assert!(!result.changed, "{operation:?} of identical intent changed");
        assert_eq!(result.event_sequence, None);
        assert_eq!(result.show_revision.value(), before.revision);
        assert_eq!(rig.steps(), ["begin"], "{operation:?}");
        rig.ports.steps.lock().clear();
    }
    assert_eq!(fingerprint(&rig), before);
    assert_eq!(rig.service.events().latest_sequence(), events);
    assert_eq!(installed_json(&rig), installed);
}

#[test]
fn failed_candidate_preparation_preserves_bytes_revision_runtime_and_events() {
    // A stored independent Color component beside the recorded rows: merging a complete Color
    // for the same fixture is rejected by candidate compilation, not by the planner.
    let intent = full_intent();
    let (rig, cue_list_id) = recorded_rig(&intent);
    let object_id = cue_list_id.0.to_string();
    let mut body = rig
        .document()
        .object("cue_list", &object_id)
        .unwrap()
        .body()
        .clone();
    body["cues"][0]["changes"].as_array_mut().unwrap().push(
        serde_json::to_value(CueChange::set(
            fixture_b(),
            AttributeKey(Arc::from("color.red")),
            AttributeValue::Normalized(0.25),
        ))
        .unwrap(),
    );
    // Remove B's complete Color so the seeded body itself is valid.
    let color_key = key(ProgrammingOwner::Color).0.to_string();
    let b = fixture_b().0.to_string();
    body["cues"][0]["changes"]
        .as_array_mut()
        .unwrap()
        .retain(|row| !(row["fixture_id"] == b && row["attribute"] == color_key));
    let revision = rig
        .document()
        .object("cue_list", &object_id)
        .unwrap()
        .revision();
    ShowStore::open(&rig.ports.path)
        .unwrap()
        .put_object("cue_list", &object_id, &body, revision)
        .unwrap();
    let installed = installed_json(&rig);
    let events = rig.service.events().latest_sequence();
    let before = fingerprint(&rig);
    let file_before = show_files(&rig);

    let conflicting = Intent {
        fixtures: vec![(fixture_b(), key(ProgrammingOwner::Color), direct(1000))],
        groups: Vec::new(),
    };
    let fixture_group_family = Intent {
        fixtures: vec![(fixture_a(), key(ProgrammingOwner::Color), group_color())],
        groups: Vec::new(),
    };
    for (name, candidate) in [
        ("complete Color beside a stored component", conflicting),
        ("Group family in fixture scope", fixture_group_family),
    ] {
        let error = record(
            &rig,
            cue_list_id,
            ProgrammingCueRecordOperation::Merge,
            semantic_capture(&candidate),
        )
        .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid, "{name}: {error:?}");
        assert!(
            error.message.contains("invalid cue list"),
            "{name}: candidate compilation must reject it: {error:?}"
        );
        assert_eq!(rig.steps(), ["begin"], "{name}");
        rig.ports.steps.lock().clear();
        assert_eq!(fingerprint(&rig), before, "{name}");
        assert_eq!(show_files(&rig), file_before, "{name}");
        assert_eq!(rig.service.events().latest_sequence(), events, "{name}");
        assert_eq!(installed_json(&rig), installed, "{name}");
    }
}

#[test]
fn failed_runtime_preparation_of_a_semantic_candidate_leaves_storage_unchanged() {
    let intent = full_intent();
    let (rig, cue_list_id) = recorded_rig(&intent);
    let failing = TestRig {
        service: ActiveShowService::new(EventBus::new(16)),
        ports: TestPorts {
            path: rig.ports.path.clone(),
            show_id: rig.show_id,
            steps: Arc::default(),
            installed: Arc::default(),
            fail_prepare: true,
        },
        show_id: rig.show_id,
    };
    let before = fingerprint(&rig);
    let file_before = show_files(&rig);

    let error = record(
        &failing,
        cue_list_id,
        ProgrammingCueRecordOperation::Overwrite,
        semantic_capture(&Intent {
            fixtures: vec![(
                fixture_a(),
                key(ProgrammingOwner::Color),
                color(warm_white_3200()),
            )],
            groups: Vec::new(),
        }),
    )
    .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Unavailable);
    assert_eq!(failing.steps(), ["begin", "prepare"]);
    assert_eq!(fingerprint(&rig), before);
    assert_eq!(show_files(&rig), file_before);
    assert_eq!(failing.service.events().latest_sequence(), 0);
    assert!(failing.ports.installed.lock().is_none());
    // Both rigs share one file; only the original rig removes it.
    std::mem::forget(failing);
}

/// Raw SQLite main and WAL file bytes, so a rejected recording is proven not to touch storage.
fn show_files(rig: &TestRig) -> Vec<Option<Vec<u8>>> {
    ["", "-wal"]
        .into_iter()
        .map(|suffix| std::fs::read(format!("{}{suffix}", rig.ports.path.display())).ok())
        .collect()
}

/// The installed runtime as JSON (EngineSnapshot has no PartialEq); `None` when nothing installed.
fn installed_json(rig: &TestRig) -> Option<Value> {
    rig.ports
        .installed
        .lock()
        .as_ref()
        .map(|snapshot| serde_json::to_value(snapshot).unwrap())
}

fn rig_cue_id(rig: &TestRig, cue_list_id: CueListId) -> Uuid {
    let document = rig.document();
    let body = document
        .object("cue_list", &cue_list_id.0.to_string())
        .unwrap()
        .body();
    body["cues"][0]["id"].as_str().unwrap().parse().unwrap()
}

// --- Real capture-mode service lanes -------------------------------------------------------------

/// Delegates the service's commit port to the real `commit_programming_cue` on the rig's store.
struct ServicePorts<'a> {
    rig: &'a TestRig,
    playback_number: u16,
    commits: AtomicUsize,
    activations: AtomicUsize,
}

impl ProgrammingCueRecordingPorts for ServicePorts<'_> {
    fn authorize_cue_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn cue_recording_environment(
        &self,
        _context: &ActionContext,
        _request: &ProgrammingCueRecordRequest,
    ) -> Result<ProgrammingCueRecordingEnvironment, ActionError> {
        Ok(ProgrammingCueRecordingEnvironment {
            target: ProgrammingCueResolvedTarget::Playback {
                playback_number: self.playback_number,
                page_slot: None,
            },
            active_cue: None::<PlaybackCueReference>,
            cuelist_auto_off_at_zero_default: false,
            cuelist_auto_off_flash_release_default: false,
            start_after_first_recording: false,
        })
    }

    fn commit_cue(
        &self,
        context: &ActionContext,
        commit: &ProgrammingCueCommit,
    ) -> Result<ProgrammingCueCommitResult, ActionError> {
        self.commits.fetch_add(1, Ordering::Relaxed);
        self.rig
            .service
            .commit_programming_cue(context, commit, &self.rig.ports)
    }

    fn activate_recorded_cue(
        &self,
        _context: &ActionContext,
        _playback_number: u16,
        _cue_number: CueNumber,
    ) -> Option<ProgrammingCueActivationCompletion> {
        self.activations.fetch_add(1, Ordering::Relaxed);
        None
    }
}

#[derive(Clone, Copy, Debug)]
enum Lane {
    Normal,
    PendingPreload,
    ActivePreload,
}

/// Puts `intent` into the Programmer lane through the registry's operator setters.
fn programmer_for(lane: Lane, intent: &Intent) -> (ProgrammerRegistry, SessionId) {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    if !matches!(lane, Lane::Normal) {
        registry.arm_preload(session, true);
    }
    for (fixture_id, attribute, value) in &intent.fixtures {
        registry.set(session, *fixture_id, attribute.clone(), value.clone());
    }
    for (group_id, attribute, value) in &intent.groups {
        assert!(registry.set_group(session, group_id.clone(), attribute.clone(), value.clone()));
    }
    if matches!(lane, Lane::ActivePreload) {
        assert!(registry.activate_preload(session));
    }
    (registry, session)
}

#[test]
fn every_capture_lane_records_the_same_exact_intent_through_the_programming_service() {
    let intent = full_intent();
    for (lane, policy, expected_source) in [
        (
            Lane::Normal,
            ProgrammingCueCapturePolicy::CurrentCapture,
            CueRecordingCapturedSource::Normal,
        ),
        (
            Lane::PendingPreload,
            ProgrammingCueCapturePolicy::CurrentCapture,
            CueRecordingCapturedSource::PendingPreload,
        ),
        (
            Lane::ActivePreload,
            ProgrammingCueCapturePolicy::PendingOrActivePreload,
            CueRecordingCapturedSource::ActivePreload,
        ),
    ] {
        let rig = TestRig::new();
        seed_group(&rig);
        let (registry, session) = programmer_for(lane, &intent);
        let service = ProgrammingService::new(
            registry.clone(),
            EventBus::default(),
            Arc::new(HighlightRegistry::default()),
        );
        let ports = ServicePorts {
            rig: &rig,
            playback_number: 9,
            commits: AtomicUsize::new(0),
            activations: AtomicUsize::new(0),
        };
        let envelope = ActionEnvelope {
            context: ActionContext::operator(
                Uuid::from_u128(0x0606_de5c),
                session.0,
                ActionSource::UserInterface,
            )
            .with_request_id(format!("tl606-{lane:?}")),
            command: ProgrammingCueRecordRequest {
                show_id: rig.show_id,
                target: ProgrammingCueRecordTarget::Pool { playback_number: 9 },
                operation: ProgrammingCueRecordOperation::Overwrite,
                cue_number: Some(CueNumber::try_from_legacy_f64(1.0).unwrap()),
                timing: ProgrammingCueRecordTiming::default(),
                cue_only: false,
                name: None,
                capture_policy: policy,
                activation_policy: ProgrammingCueActivationPolicy::Hold,
                expected_show_revision: ProgrammingCueShowRevisionExpectation::Current,
            },
        };

        let result = service.handle_cue_recording(envelope, &ports).unwrap();

        assert_eq!(result.captured_source, expected_source, "{lane:?}");
        assert_eq!(ports.commits.load(Ordering::Relaxed), 1, "{lane:?}");
        assert_eq!(ports.activations.load(Ordering::Relaxed), 0, "{lane:?}");
        let ProgrammingCueRecordOutcome::Changed {
            projections,
            recorded_cue,
            show_revision,
            ..
        } = &result.outcome
        else {
            panic!("{lane:?}: expected a changed recording")
        };
        let committed = ProgrammingCueCommitResult {
            changed: true,
            created_topology: true,
            projections: projections.as_ref().clone(),
            recorded_cue: recorded_cue.clone(),
            show_revision: *show_revision,
            event_sequence: Some(1),
            concrete_playback_number: Some(9),
        };
        let cue_list_id = assert_recorded_everywhere(&rig, &committed, &intent);
        assert_named_semantic_facts(
            &rig.document()
                .object("cue_list", &cue_list_id.0.to_string())
                .unwrap()
                .body()["cues"][0],
        );
        if matches!(lane, Lane::ActivePreload) {
            assert!(
                registry.get(session).unwrap().preload_active.is_empty(),
                "accepted ActivePreload fallback is released"
            );
        }
    }
}
