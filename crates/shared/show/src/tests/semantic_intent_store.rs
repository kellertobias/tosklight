//! TL-567: the portable show store retains typed semantic intent across its own boundaries.
//!
//! This covers the storage codec only: transaction commit, file reopen, backup copy and prepared
//! undo. Application writers, the show-open compiler and selective import are covered by
//! `light-application` `selective_import::tests::semantic_intent*`.
use super::temporary;
use crate::ShowStore;
use light_core::{
    AttributeValue, OpeningConvention, Xyz,
    programming::{
        ColorIntent, ColorProgram, PositionIntent, ScalarIntent, TargetReference, UvIntent,
        VirtualColorAuthoringV1, VirtualColorRecipe, WhiteTarget, ZoomIntent,
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, sync::Arc};
use uuid::Uuid;

fn color(rgb: [f32; 3], relative_output: f32, uv: f32) -> AttributeValue {
    let recipe = VirtualColorRecipe {
        rgb,
        ..VirtualColorRecipe::default()
    };
    let intent = ColorIntent {
        base_xyz: VirtualColorAuthoringV1::recipe_xyz(&recipe).unwrap(),
        recipe,
        white_blend: 0.85,
        white_target: WhiteTarget {
            kelvin: 3200.0,
            duv: 0.0035,
        },
        relative_output,
        uv: UvIntent { amount: uv },
        ..ColorIntent::default()
    };
    let program = ColorProgram::Semantic { intent };
    program.validate().unwrap();
    AttributeValue::ColorProgram(Arc::new(program))
}

fn intents() -> BTreeMap<&'static str, AttributeValue> {
    BTreeMap::from([
        ("magenta", color([1.0, 0.0, 1.0], 0.8, 0.0)),
        ("warm_zero_output", color([1.0, 0.72, 0.42], 0.0, 0.0)),
        ("uv_only_black", color([0.0, 0.0, 0.0], 0.0, 0.9)),
        (
            "angles",
            AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25))),
        ),
        (
            "target",
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Point {
                    point_id: Uuid::from_u128(0x567),
                },
                [0.5, -1.25, 2.0],
            ))),
        ),
        ("focus", AttributeValue::Normalized(0.37)),
        (
            "zoom",
            AttributeValue::Zoom(Arc::new(ZoomIntent {
                opening_degrees: ScalarIntent::Value(23.5),
                convention: OpeningConvention::Beam,
            })),
        ),
    ])
}

fn body(values: &BTreeMap<&'static str, AttributeValue>) -> Value {
    json!({"name": "Semantic", "values": values, "future": {"kept": true}})
}

fn decoded(body: &Value) -> BTreeMap<String, AttributeValue> {
    serde_json::from_value(body["values"].clone()).unwrap()
}

fn expected(values: &BTreeMap<&'static str, AttributeValue>) -> BTreeMap<String, AttributeValue> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[test]
fn committed_semantic_intent_reopens_backs_up_and_undoes_to_identical_typed_values() {
    let path = temporary("semantic-intent");
    let backup = temporary("semantic-intent-backup");
    let (show, _) = ShowStore::create(&path, "Semantic").unwrap();
    let original = intents();
    let mut transaction = show.portable_document().unwrap().transaction();
    transaction.put("preset", "2.1", body(&original));
    show.apply_portable_transaction(transaction).unwrap();
    drop(show);

    let show = ShowStore::open(&path).unwrap();
    let document = show.portable_document().unwrap();
    let stored = document.object("preset", "2.1").unwrap().body().clone();
    assert_eq!(decoded(&stored), expected(&original));
    assert_eq!(stored["future"]["kept"], true);
    let zero = &stored["values"]["warm_zero_output"]["value"]["intent"];
    assert_eq!(
        zero["relative_output"],
        json!(0.0),
        "zero output must stay explicit"
    );
    assert_eq!(zero["uv"]["amount"], json!(0.0));
    assert_eq!(
        stored["values"]["target"]["value"]["reference"],
        json!({"kind": "point", "point_id": Uuid::from_u128(0x567)})
    );

    show.backup_to(&backup).unwrap();
    let copied = ShowStore::open(&backup)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(copied.object("preset", "2.1").unwrap().body(), &stored);

    let mut changed = original.clone();
    changed.insert("magenta", color([0.0, 1.0, 0.0], 1.0, 0.2));
    changed.remove("zoom");
    show.put_object("preset", "2.1", &body(&changed), 1)
        .unwrap();
    let undo = show.prepare_object_undo("preset", "2.1", 2).unwrap();
    let mut transaction = show.portable_document().unwrap().transaction();
    transaction.undo_object(undo);
    show.apply_portable_transaction(transaction).unwrap();
    drop(show);

    let restored = ShowStore::open(&path).unwrap().portable_document().unwrap();
    let restored = restored.object("preset", "2.1").unwrap().body().clone();
    assert_eq!(decoded(&restored), expected(&original));
    assert!(
        restored["values"].get("zoom").is_some(),
        "Zoom must be restored"
    );
    remove(&path);
    remove(&backup);
}

/// Guards TL-571 (TL-567 defect D2). The store writes `body_json` as text and reparses it; the
/// workspace enables `serde_json`'s `float_roundtrip`, so the reparsed `f64` is bit-identical
/// (0.22964008152484894 no longer reads back as 0.22964008152484897) and lossless no-change
/// checks compare equal bodies.
#[test]
fn reopened_semantic_body_is_bitwise_identical_to_the_committed_body() {
    let path = temporary("semantic-intent-float");
    let (show, _) = ShowStore::create(&path, "Semantic").unwrap();
    let written = json!({"base_xyz": Xyz { x: 0.0, y: 0.0, z: 0.229_640_08_f32 }});
    show.put_object("preset", "2.1", &written, 0).unwrap();
    drop(show);
    let reopened = ShowStore::open(&path).unwrap().portable_document().unwrap();
    let stored = reopened.object("preset", "2.1").unwrap().body().clone();
    remove(&path);
    assert_eq!(stored, written);
}
