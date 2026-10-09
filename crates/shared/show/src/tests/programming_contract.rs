//! TL-560: programming-contract marker and the legacy-programming validator on real show files.
use super::temporary;
use crate::{
    LegacyProgrammingFamily, PROGRAMMING_CONTRACT_METADATA_KEY, ProgrammingContractMarker,
    ShowStore, inspect_show_programming_contract, legacy_programming_attributes,
    validate_show_programming_contract,
};
use light_core::{
    AttributeValue,
    programming::{ColorIntent, ColorProgram, PositionIntent, ScalarIntent, ZoomIntent},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, sync::Arc};

const FIXTURE: &str = "00000000-0000-4001-8000-000000000065";

fn normalized(value: f32) -> Value {
    json!({"kind": "normalized", "value": value})
}

fn semantic_values() -> Value {
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let position = AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25)));
    let zoom = AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(23.5),
        convention: light_core::OpeningConvention::Beam,
    }));
    json!({
        "color": color, "position": position, "zoom": zoom,
        // Non-family and native addresses stay valid at every contract.
        "focus": normalized(0.37), "intensity": normalized(1.0), "gobo.1": normalized(0.2),
        "color.wheel.1": normalized(0.5), "color.tint": normalized(0.4),
        "position.movement": normalized(0.1), "shutter": {"kind": "discrete", "value": "open"},
    })
}

fn legacy_preset() -> Value {
    json!({"family": "Position", "name": "Down", "number": 1, "group_values": {},
        "values": {FIXTURE: {"pan": normalized(0.5), "tilt": normalized(0.25)}}})
}

fn create(name: &str) -> (std::path::PathBuf, ShowStore) {
    let path = temporary(name);
    let (store, _) = ShowStore::create(&path, name).unwrap();
    (path, store)
}

fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

fn bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

#[test]
fn every_legacy_programming_shape_is_found_and_semantic_or_non_family_values_are_not() {
    let semantic = json!({"family": "Color", "values": {FIXTURE: semantic_values()},
        "universal_values": semantic_values(), "group_values": {"front": semantic_values()}});
    assert_eq!(legacy_programming_attributes(&semantic), BTreeSet::new());

    let found = |value: Value| legacy_programming_attributes(&value);
    let set = |items: &[&str]| items.iter().map(|item| (*item).to_owned()).collect();
    assert_eq!(found(legacy_preset()), set(&["pan", "tilt"]));
    assert_eq!(
        found(json!({"universal_values": {"color.red": normalized(1.0)}})),
        set(&["color.red"])
    );
    assert_eq!(
        found(
            json!({"group_values": {"g": {"color.uv": {"kind": "spread", "value": [0.0, 1.0]}}}})
        ),
        set(&["color.uv"])
    );
    // Cue change rows and legacy scalar Dynamic lanes are addressed by `attribute`.
    assert_eq!(
        found(json!({"cues": [{"changes": [
            {"attribute": "intensity", "fixture_id": FIXTURE, "value": normalized(1.0)},
            {"attribute": "color.cyan", "fixture_id": FIXTURE, "value": normalized(1.0)}]}]})),
        set(&["color.cyan"])
    );
    assert_eq!(
        found(json!({"lanes": [{"attribute": "tilt", "mode": "max_min"},
            {"address": {"owner": "position"}, "configuration": {}}]})),
        set(&["tilt"])
    );
    // A legacy-named key whose value is not an authored value (for example a lane label map)
    // is not programming.
    assert_eq!(found(json!({"labels": {"pan": "Pan"}})), BTreeSet::new());
}

#[test]
fn the_validator_is_dormant_at_contract_zero_and_never_opens_the_file() {
    let missing = temporary("tl560-missing");
    assert!(!missing.exists());
    validate_show_programming_contract(&missing, 0).unwrap();
    assert!(
        !missing.exists(),
        "contract 0 must not create or touch the file"
    );
    assert!(validate_show_programming_contract(&missing, 1).is_err());
    assert!(
        !missing.exists(),
        "the read-only inspection never creates a file"
    );

    let (path, store) = create("tl560-dormant");
    store
        .put_object("preset", "3.1", &legacy_preset(), 0)
        .unwrap();
    store
        .set_metadata_values(&[(PROGRAMMING_CONTRACT_METADATA_KEY, "nonsense")])
        .unwrap();
    drop(store);
    validate_show_programming_contract(&path, 0).unwrap();
    let report = inspect_show_programming_contract(&path).unwrap();
    report.check(0).unwrap();
    assert!(report.check(1).is_err());
    remove(&path);
}

#[test]
fn legacy_shows_are_rejected_read_only_with_an_actionable_message_at_contract_one() {
    let (path, store) = create("tl560-legacy");
    store
        .put_object("preset", "3.1", &legacy_preset(), 0)
        .unwrap();
    store
        .put_object(
            "dynamic",
            "d1",
            &json!({"lanes": [{"attribute": "color.red", "mode": "max_min"}]}),
            0,
        )
        .unwrap();
    // A fixture profile/patch record legitimately names `pan` and `color.red` and is ignored.
    store
        .put_object(
            "patched_fixture",
            FIXTURE,
            &json!({"channels": [{"attribute": "pan"}, {"attribute": "color.red"}]}),
            0,
        )
        .unwrap();
    // Older layout: the read-only inspection must not migrate it in place.
    store
        .conn
        .execute("UPDATE schema_info SET version=8", [])
        .unwrap();
    store
        .conn
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(store);
    let before = bytes(&path);

    let report = inspect_show_programming_contract(&path).unwrap();
    assert_eq!(report.marker, ProgrammingContractMarker::Absent);
    let summary = report
        .legacy
        .iter()
        .map(|finding| {
            (
                finding.kind.as_str(),
                finding.attribute.as_str(),
                finding.family,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        [
            ("dynamic", "color.red", LegacyProgrammingFamily::Color),
            ("preset", "pan", LegacyProgrammingFamily::Position),
            ("preset", "tilt", LegacyProgrammingFamily::Position),
        ]
    );
    let error = validate_show_programming_contract(&path, 1)
        .unwrap_err()
        .to_string();
    for expected in [
        "tl560-legacy",
        "preset 3.1 pan (normalized Position)",
        "dynamic d1 color.red (Color component)",
        "never reinterpreted as degrees",
        "The show file was not changed",
        "Open or create a different show",
        "desk's settings are unaffected",
    ] {
        assert!(
            error.contains(expected),
            "{expected:?} missing from {error}"
        );
    }
    assert_eq!(bytes(&path), before, "original bytes are preserved");
    let schema: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(schema, 8, "inspection never migrates");
    remove(&path);
}

#[test]
fn the_marker_rejects_newer_or_unreadable_contracts_and_accepts_semantic_shows() {
    let (path, store) = create("tl560-marker");
    store
        .put_object(
            "preset",
            "2.1",
            &json!({"family": "Color", "values": {FIXTURE: semantic_values()}}),
            0,
        )
        .unwrap();
    drop(store);
    validate_show_programming_contract(&path, 1).unwrap();
    for (value, expected) in [
        ("1", None),
        (
            "2",
            Some("written for programming contract 2; this desk supports 1"),
        ),
        ("one", Some("unreadable programming contract marker")),
    ] {
        ShowStore::open(&path)
            .unwrap()
            .set_metadata_values(&[(PROGRAMMING_CONTRACT_METADATA_KEY, value)])
            .unwrap();
        let result = validate_show_programming_contract(&path, 1);
        match expected {
            None => result.unwrap(),
            Some(message) => assert!(result.unwrap_err().to_string().contains(message)),
        }
        validate_show_programming_contract(&path, 0).unwrap();
    }
    remove(&path);
}

#[test]
fn malformed_shows_fail_inspection_instead_of_passing_silently() {
    let not_sqlite = temporary("tl560-not-sqlite");
    fs::write(&not_sqlite, b"this is not a show").unwrap();
    assert!(validate_show_programming_contract(&not_sqlite, 1).is_err());
    assert_eq!(bytes(&not_sqlite), b"this is not a show");
    remove(&not_sqlite);

    let (path, store) = create("tl560-malformed-body");
    store
        .conn
        .execute(
            "INSERT INTO objects(kind,id,body_json,revision,updated_at) VALUES('preset','3.9','{broken',1,'now')",
            [],
        )
        .unwrap();
    drop(store);
    assert!(validate_show_programming_contract(&path, 1).is_err());
    validate_show_programming_contract(&path, 0).unwrap();
    remove(&path);

    let (path, store) = create("tl560-no-objects");
    store.conn.execute_batch("DROP TABLE objects").unwrap();
    drop(store);
    assert!(validate_show_programming_contract(&path, 1).is_err());
    remove(&path);
}

#[test]
fn writers_stamp_the_marker_only_at_contract_one_and_only_for_programming_changes() {
    let (path, store) = create("tl560-writer");
    let preset = json!({"family": "Color", "values": {FIXTURE: semantic_values()}});

    let mut document = store.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("preset", "2.1", preset.clone());
    assert!(!transaction.stamp_programming_contract(0));
    let commit = store.apply_portable_transaction(transaction).unwrap();
    document.apply_commit(&commit);
    assert_eq!(
        store.programming_contract_marker().unwrap(),
        ProgrammingContractMarker::Absent
    );

    let mut transaction = document.transaction();
    transaction.put("patched_fixture", FIXTURE, json!({"name": "Spot"}));
    assert!(!transaction.stamp_programming_contract(1));
    let commit = store.apply_portable_transaction(transaction).unwrap();
    document.apply_commit(&commit);
    assert!(commit.metadata_changes().is_empty());

    let mut transaction = document.transaction();
    transaction.put("preset", "2.2", preset);
    assert!(transaction.stamp_programming_contract(1));
    let revision = document.revision().value();
    let commit = store.apply_portable_transaction(transaction).unwrap();
    document.apply_commit(&commit);
    assert_eq!(commit.revision().value(), revision + 1, "one atomic commit");
    assert_eq!(
        document.programming_contract_marker(),
        ProgrammingContractMarker::Declared(1)
    );
    drop(store);
    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(
        reopened.programming_contract_marker().unwrap(),
        ProgrammingContractMarker::Declared(1)
    );
    assert_eq!(reopened.portable_document().unwrap(), document);
    validate_show_programming_contract(&path, 1).unwrap();
    remove(&path);
}

/// TL-552 owner decision: a percentage at `zoom` is legacy (plan §13 "normalized-position/zoom")
/// and is rejected at contract 1; semantic Zoom in degrees at the same address is not.
#[test]
fn percentage_zoom_is_legacy_and_zoom_in_degrees_is_not() {
    let found = |value: Value| legacy_programming_attributes(&value);
    let zoom = || BTreeSet::from(["zoom".to_owned()]);
    let degrees = semantic_values()["zoom"].clone();
    assert_eq!(degrees["kind"], "zoom");
    // Value maps (Preset, Group, Programmer).
    assert_eq!(
        found(json!({"values": {FIXTURE: {"zoom": normalized(0.4)}}})),
        zoom()
    );
    assert_eq!(
        found(json!({"group_values": {"g": {"zoom": {"kind": "spread", "value": [0.0, 1.0]}}}})),
        zoom()
    );
    assert_eq!(
        found(json!({"values": {FIXTURE: {"zoom": degrees.clone()}}})),
        BTreeSet::new()
    );
    // Cue change rows carry the value next to the attribute.
    assert_eq!(
        found(
            json!({"changes": [{"attribute": "zoom", "fixture_id": FIXTURE, "value": normalized(0.4)}]})
        ),
        zoom()
    );
    assert_eq!(
        found(json!({"changes": [{"attribute": "zoom", "fixture_id": FIXTURE, "value": degrees}]})),
        BTreeSet::new()
    );
    // A legacy scalar Dynamic lane on `zoom` is percentage-domain.
    assert_eq!(
        found(json!({"lanes": [{"attribute": "zoom", "mode": "max_min"}]})),
        zoom()
    );
    // Focus stays a plain percentage and is never legacy.
    assert_eq!(
        found(json!({"values": {FIXTURE: {"focus": normalized(0.4)}}})),
        BTreeSet::new()
    );

    let (path, store) = create("tl552-percent-zoom");
    store
        .put_object(
            "preset",
            "4.1",
            &json!({"family": "Beam", "values": {FIXTURE: {"zoom": normalized(0.4)}}}),
            0,
        )
        .unwrap();
    drop(store);
    let before = bytes(&path);
    validate_show_programming_contract(&path, 0).unwrap();
    let error = validate_show_programming_contract(&path, 1)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("preset 4.1 zoom (percentage Zoom)"),
        "{error}"
    );
    assert!(error.contains("The show file was not changed"), "{error}");
    assert_eq!(bytes(&path), before);
    remove(&path);
}

/// TL-552 owner decision: direct object writers (the legacy object routes, Preload Store on an
/// inactive show) stamp the marker atomically when a contract ≥ 1 runtime writes or deletes
/// authored programming, and never for other kinds or at contract 0.
#[test]
fn direct_object_writes_stamp_the_marker_only_for_programming_at_contract_one() {
    use crate::{AtomicObjectDelete, AtomicObjectWrite};
    let (path, store) = create("tl552-direct-writers");
    let marker = |store: &ShowStore| store.programming_contract_marker().unwrap();
    let semantic = json!({"family": "Color", "values": {FIXTURE: semantic_values()}});
    // Contract 0 and non-programming kinds never stamp.
    store
        .put_object_at_contract("preset", "2.1", &semantic, 0, 0)
        .unwrap();
    store
        .put_object_at_contract("route", "r1", &json!({"protocol": "artnet"}), 0, 1)
        .unwrap();
    store.delete_object_at_contract("route", "r1", 1).unwrap();
    assert_eq!(marker(&store), ProgrammingContractMarker::Absent);
    // A contract-1 put of a programming kind stamps in the same commit.
    let before = store.portable_revision().unwrap();
    store
        .put_object_at_contract("preset", "2.1", &semantic, 1, 1)
        .unwrap();
    assert_eq!(marker(&store), ProgrammingContractMarker::Declared(1));
    assert_eq!(
        store.portable_revision().unwrap().value(),
        before.value() + 1,
        "one atomic revision"
    );
    drop(store);

    for (label, write) in [
        (
            "atomic write",
            Box::new(|store: &ShowStore| {
                store
                    .mutate_objects_atomically_at_contract(
                        &[AtomicObjectWrite {
                            kind: "group",
                            id: "1",
                            body: &json!({"id": "1", "fixtures": []}),
                            expected: 0,
                        }],
                        &[],
                        1,
                    )
                    .map(|_| ())
            }) as Box<dyn Fn(&ShowStore) -> Result<(), crate::StoreError>>,
        ),
        (
            "atomic delete",
            Box::new(|store: &ShowStore| {
                store.put_object("cue_list", "c1", &json!({"cues": []}), 0)?;
                store
                    .mutate_objects_atomically_at_contract(
                        &[],
                        &[AtomicObjectDelete {
                            kind: "cue_list",
                            id: "c1",
                            expected: 1,
                        }],
                        1,
                    )
                    .map(|_| ())
            }),
        ),
        (
            "delete",
            Box::new(|store: &ShowStore| {
                store.put_object("dynamic", "d1", &json!({"lanes": []}), 0)?;
                store
                    .delete_object_at_contract("dynamic", "d1", 1)
                    .map(|_| ())
            }),
        ),
    ] {
        let (path, store) = create(&format!("tl552-direct-{}", label.replace(' ', "-")));
        write(&store).unwrap();
        assert_eq!(
            marker(&store),
            ProgrammingContractMarker::Declared(1),
            "{label}"
        );
        drop(store);
        remove(&path);
    }
    // The plain (non-runtime) API keeps its old behaviour.
    let (plain, store) = create("tl552-direct-plain");
    store.put_object("preset", "2.1", &semantic, 0).unwrap();
    assert_eq!(marker(&store), ProgrammingContractMarker::Absent);
    drop(store);
    remove(&plain);
    remove(&path);
}

/// TL-552 follow-up: at contract ≥ 1 every writer refuses legacy programming before storing it,
/// so an accepted write can never make the file fail the load-time validator. Contract 0 (an
/// older runtime, tools and tests) is unchanged, and non-programming kinds are never inspected.
#[test]
fn contract_one_writers_refuse_legacy_programming_before_storing_it() {
    use crate::{
        AtomicObjectWrite, StoreError, check_programming_object_writes, legacy_attribute_value,
        legacy_programming_address,
    };
    let (path, store) = create("tl552-followup-write-gate");
    let revision = store.portable_revision().unwrap();
    fn rejected<T: std::fmt::Debug>(result: Result<T, StoreError>) -> String {
        match result {
            Err(StoreError::Invalid(message)) => message,
            other => panic!("expected an actionable refusal, got {other:?}"),
        }
    }
    let message = rejected(store.put_object_at_contract("preset", "3.1", &legacy_preset(), 0, 1));
    assert!(
        message.contains("preset 3.1 pan (normalized Position)"),
        "{message}"
    );
    assert!(message.contains("nothing was changed"), "{message}");
    let message = rejected(store.mutate_objects_atomically_at_contract(
        &[
            AtomicObjectWrite {
                kind: "group",
                id: "1",
                body: &json!({"id": "1", "fixtures": []}),
                expected: 0,
            },
            AtomicObjectWrite {
                kind: "cue_list",
                id: "c1",
                body: &json!({"cues": [{"changes": [{"fixture_id": FIXTURE,
                    "attribute": "color.red", "value": normalized(1.0)}]}]}),
                expected: 0,
            },
        ],
        &[],
        1,
    ));
    assert!(
        message.contains("cue_list c1 color.red (Color component)"),
        "{message}"
    );
    // Atomic: the semantic Group in the same write was not stored either.
    assert_eq!(store.portable_revision().unwrap().value(), revision.value());
    assert!(store.objects("group").unwrap().is_empty());
    assert_eq!(marker_of(&store), ProgrammingContractMarker::Absent);
    // Semantic programming and non-programming kinds still write at contract 1.
    let semantic = json!({"family": "Color", "values": {FIXTURE: semantic_values()}});
    store
        .put_object_at_contract("preset", "2.1", &semantic, 0, 1)
        .unwrap();
    store
        .put_object_at_contract("route", "r1", &json!({"attribute": "pan"}), 0, 1)
        .unwrap();
    // Contract 0 keeps the old behaviour.
    store
        .put_object_at_contract("preset", "3.1", &legacy_preset(), 0, 0)
        .unwrap();
    drop(store);
    remove(&path);

    // The transaction gate (unit-of-work commits) and the live-value rule share the stored rule.
    let mut transaction = crate::PortableShowTransaction::new(revision);
    transaction.put("preset", "2.1", semantic);
    assert!(transaction.check_programming_contract(1).is_ok());
    transaction.put("dynamic", "d1", json!({"lanes": [{"attribute": "tilt"}]}));
    let message = transaction
        .check_programming_contract(1)
        .unwrap_err()
        .message;
    assert!(message.contains("dynamic d1 tilt"), "{message}");
    assert!(transaction.check_programming_contract(0).is_ok());
    assert!(
        check_programming_object_writes(1, [("route", "r", &json!({"pan": normalized(0.1)}))])
            .is_ok()
    );
    for (attribute, value, family) in [
        (
            "pan",
            AttributeValue::Normalized(0.5),
            Some(LegacyProgrammingFamily::Position),
        ),
        (
            "tilt",
            AttributeValue::RawDmx(12),
            Some(LegacyProgrammingFamily::Position),
        ),
        (
            "color.red",
            AttributeValue::Spread(vec![0.0, 1.0]),
            Some(LegacyProgrammingFamily::Color),
        ),
        (
            "zoom",
            AttributeValue::Normalized(0.5),
            Some(LegacyProgrammingFamily::Zoom),
        ),
        ("zoom", AttributeValue::RawDmx(10), None),
        ("color.wheel.1", AttributeValue::Normalized(0.5), None),
        ("intensity", AttributeValue::Normalized(0.5), None),
    ] {
        assert_eq!(
            legacy_attribute_value(attribute, &value),
            family,
            "{attribute}"
        );
    }
    assert_eq!(
        legacy_programming_address("pan", Some("release")),
        Some(LegacyProgrammingFamily::Position)
    );
    assert_eq!(legacy_programming_address("zoom", Some("zoom")), None);
    // A Release of the Zoom owner or a running typed Zoom Dynamic shares the address and is not
    // legacy; a FixAT/static percentage or a scalar lane at `zoom` is.
    let found = |value: Value| legacy_programming_attributes(&value);
    for row in [
        json!({"changes": [{"attribute": "zoom", "fixture_id": FIXTURE, "value": null}]}),
        json!({"group_release_values": [{"group_id": "g", "attribute": "zoom"}]}),
        json!({"dynamic_values": [{"fixture_id": FIXTURE, "attribute": "zoom", "value": {"type": "release"}}]}),
        json!({"dynamic_values": [{"fixture_id": FIXTURE, "attribute": "zoom", "value": {"type": "static", "value": semantic_values()["zoom"]}}]}),
    ] {
        assert_eq!(found(row.clone()), BTreeSet::new(), "{row}");
    }
    for row in [
        json!({"dynamic_values": [{"fixture_id": FIXTURE, "attribute": "zoom", "value": {"type": "fix_at", "value": 0.4}}]}),
        json!({"dynamic_values": [{"fixture_id": FIXTURE, "attribute": "zoom", "value": {"type": "static", "value": normalized(0.4)}}]}),
        json!({"lanes": [{"attribute": "zoom", "mode": "keyframes"}]}),
        json!({"changes": [{"attribute": "pan", "fixture_id": FIXTURE, "value": null}]}),
    ] {
        assert_eq!(found(row.clone()).len(), 1, "{row}");
    }
}

fn marker_of(store: &ShowStore) -> ProgrammingContractMarker {
    store.programming_contract_marker().unwrap()
}

#[test]
fn feature_two_only_for_live_references_and_never_downgrades_after_reopen() {
    let (path, store) = create("live-preset-contract");
    let mut document = store.portable_document().unwrap();
    let mut literal = document.transaction();
    literal.put("cue_list", "literal", json!({"cues": [{"changes": []}]}));
    literal.check_programming_contract(2).unwrap();
    literal.stamp_programming_contract(2);
    let commit = store.apply_portable_transaction(literal).unwrap();
    document.apply_commit(&commit);
    assert_eq!(
        document.programming_contract_marker(),
        ProgrammingContractMarker::Declared(1)
    );
    validate_show_programming_contract(&path, 1).unwrap();

    let reference = json!({"preset_instance_id": uuid::Uuid::new_v4(), "source_owner": {"type": "universal"}, "source_attribute": "intensity"});
    let mut linked = document.transaction();
    linked.put("cue_list", "linked", json!({"cues": [{"changes": [{"attribute":"intensity", "value":normalized(0.5), "preset_reference":reference}]}]}));
    assert!(linked.check_programming_contract(1).is_err());
    linked.check_programming_contract(2).unwrap();
    linked.stamp_programming_contract(2);
    let commit = store.apply_portable_transaction(linked).unwrap();
    document.apply_commit(&commit);
    assert_eq!(
        document.programming_contract_marker(),
        ProgrammingContractMarker::Declared(2)
    );
    assert!(validate_show_programming_contract(&path, 1).is_err());
    validate_show_programming_contract(&path, 2).unwrap();
    drop(store);
    let reopened = ShowStore::open(&path).unwrap();
    let mut document = reopened.portable_document().unwrap();
    let mut later = document.transaction();
    later.put("preset", "1.9", json!({"values":{}, "universal_values":{}}));
    later.stamp_programming_contract(2);
    let commit = reopened.apply_portable_transaction(later).unwrap();
    document.apply_commit(&commit);
    assert_eq!(
        document.programming_contract_marker(),
        ProgrammingContractMarker::Declared(2)
    );
    drop(reopened);
    remove(&path);
}
