use super::support::{fixture, profile};
use crate::{PatchedFixturePatch, PortablePatchError, PortablePatchedFixtureRecord};
use serde_json::{Value, json};

#[test]
fn new_write_contains_only_profile_reference_and_patch_owned_fields() {
    let profile = profile();
    let fixture = fixture(&profile);
    let record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let serialized = serde_json::to_value(&record).unwrap();
    assert_eq!(&serialized, record.body());
    let body = &serialized;

    assert_eq!(body["profile_id"], json!(profile.id));
    assert_eq!(body["patch_record_schema"], 1);
    assert_eq!(body["profile_revision"], profile.revision);
    assert_eq!(body["mode_id"], json!(profile.modes[0].id));
    assert_eq!(body["fixture_id"], json!(fixture.fixture_id));
    assert_eq!(body["logical_heads"], json!(fixture.logical_heads));
    assert_eq!(body["multipatch"], json!(fixture.multipatch));
    assert!(body.get("definition").is_none());
    assert!(!contains_key(body, "profile_snapshot"));
}

#[test]
fn profile_head_identity_is_optional_for_legacy_records_and_persisted_when_known() {
    let profile = profile();
    let mut fixture = fixture(&profile);
    let legacy = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    assert!(
        legacy.body()["logical_heads"][0]
            .get("profile_head_id")
            .is_none()
    );
    assert_eq!(
        legacy.patch().unwrap().logical_heads[0].profile_head_id,
        None
    );

    let profile_head_id = profile.modes[0].heads[1].id;
    fixture.logical_heads[0].profile_head_id = Some(profile_head_id);
    let identified = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();

    assert_eq!(
        identified.body()["logical_heads"][0]["profile_head_id"],
        json!(profile_head_id)
    );
    assert_eq!(
        identified.patch().unwrap().logical_heads[0].profile_head_id,
        Some(profile_head_id)
    );
}

#[test]
fn ambiguous_unversioned_record_cannot_bypass_profile_resolution() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = serde_json::to_value(fixture).unwrap();
    body["profile_id"] = json!(profile.id);
    body["profile_revision"] = json!(profile.revision);
    body["mode_id"] = json!(profile.modes[0].id);

    assert!(matches!(
        PortablePatchedFixtureRecord::decode(body),
        Err(PortablePatchError::AmbiguousRepresentation)
    ));
}

#[test]
fn versioned_reference_record_retains_an_unknown_definition_extension() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
        .unwrap()
        .into_body();
    body["definition"] = json!({"future_extension": true});

    let record = PortablePatchedFixtureRecord::decode(body).unwrap();

    assert!(!record.is_legacy_inline());
    assert_eq!(record.body()["definition"]["future_extension"], true);
    assert!(record.profile_reference().unwrap().is_some());
}

#[test]
fn typed_patch_update_retains_unknown_fields_and_legacy_inline_definition() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = serde_json::to_value(&fixture).unwrap();
    body["future_binding"] = json!({"adapter":"future","opaque":[3,1,4]});
    body["multipatch"][0]["future_instance"] = json!({"calibration":0.75});
    body["definition"]["future_definition"] = json!({"do_not_normalize":true});
    let original_definition = body["definition"].clone();
    let mut record = serde_json::from_value::<PortablePatchedFixtureRecord>(body).unwrap();
    let mut patch: PatchedFixturePatch = record.patch().unwrap();
    patch.name = "Updated name".into();
    patch.multipatch[0].name = "Updated instance".into();
    record.update_patch(&patch).unwrap();

    assert!(record.is_legacy_inline());
    assert_eq!(record.body()["name"], "Updated name");
    assert_eq!(record.body()["multipatch"][0]["name"], "Updated instance");
    assert_eq!(
        record.body()["multipatch"][0]["future_instance"],
        json!({"calibration":0.75})
    );
    assert_eq!(
        record.body()["future_binding"],
        json!({"adapter":"future","opaque":[3,1,4]})
    );
    assert_eq!(record.body()["definition"], original_definition);
}

#[test]
fn typed_patch_update_retains_unknown_fields_in_reference_records() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
        .unwrap()
        .into_body();
    body["future_binding"] = json!({"transport":"future"});
    body["logical_heads"][0]["future_head"] = json!({"pixel_order":[2,1]});
    let mut record = PortablePatchedFixtureRecord::decode(body).unwrap();
    let mut patch = record.patch().unwrap();
    patch.name = "Reference update".into();
    patch.logical_heads[0].head_index = 2;
    record.update_patch(&patch).unwrap();

    assert!(!record.is_legacy_inline());
    assert_eq!(record.body()["name"], "Reference update");
    assert_eq!(
        record.body()["future_binding"],
        json!({"transport":"future"})
    );
    assert_eq!(
        record.body()["logical_heads"][0]["future_head"],
        json!({"pixel_order":[2,1]})
    );
    assert!(record.body().get("definition").is_none());
}

#[test]
fn keyed_array_updates_keep_unknown_fields_with_their_stable_identity() {
    let profile = profile();
    let mut fixture = fixture(&profile);
    let mut second = fixture.multipatch[0].clone();
    second.id = uuid::Uuid::new_v4();
    second.name = "Second".into();
    fixture.multipatch.push(second);
    let mut body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
        .unwrap()
        .into_body();
    body["multipatch"][0]["future_data"] = json!("first");
    body["multipatch"][1]["future_data"] = json!("second");
    let mut record = PortablePatchedFixtureRecord::decode(body).unwrap();
    let mut patch = record.patch().unwrap();
    patch.multipatch.reverse();
    patch.multipatch[0].name = "Moved second".into();
    record.update_patch(&patch).unwrap();

    assert_eq!(record.body()["multipatch"][0]["future_data"], "second");
    assert_eq!(record.body()["multipatch"][1]["future_data"], "first");
    assert_eq!(record.body()["multipatch"][0]["name"], "Moved second");
}

#[test]
fn typed_patch_update_rejects_a_fixture_identity_change() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let mut patch = record.patch().unwrap();
    patch.fixture_id = light_core::FixtureId::new();

    assert!(matches!(
        record.update_patch(&patch),
        Err(PortablePatchError::FixtureIdentityChanged { .. })
    ));
    assert_eq!(record.patch().unwrap().fixture_id, fixture.fixture_id);
}

#[test]
fn nested_identity_changes_require_an_explicit_topology_update() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let mut patch = record.patch().unwrap();
    patch.multipatch[0].id = uuid::Uuid::new_v4();

    assert!(matches!(
        record.update_patch(&patch),
        Err(PortablePatchError::NestedIdentityChanged {
            collection: "multipatch"
        })
    ));
    record
        .update_patch_allowing_identity_changes(&patch)
        .unwrap();
    assert_eq!(
        record.patch().unwrap().multipatch[0].id,
        patch.multipatch[0].id
    );
}

#[test]
fn profile_reference_update_retains_patch_and_unknown_data() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
        .unwrap()
        .into_body();
    body["future_binding"] = json!({"retain": true});
    let mut record = PortablePatchedFixtureRecord::decode(body).unwrap();
    let updated = crate::PatchedFixtureProfileReference {
        profile_id: light_core::FixtureId::new(),
        profile_revision: 19,
        mode_id: uuid::Uuid::new_v4(),
    };

    record.update_profile_reference(updated).unwrap();

    assert_eq!(record.profile_reference().unwrap(), Some(updated));
    assert_eq!(record.body()["future_binding"]["retain"], true);
    assert_eq!(record.patch().unwrap().fixture_id, fixture.fixture_id);
}

#[test]
fn legacy_migration_removes_inline_definition_and_retains_unknown_fields() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = serde_json::to_value(&fixture).unwrap();
    body["future_binding"] = json!({"retain": true});
    body["definition"]["future_definition"] = json!({"opaque": 7});
    body["definition"]["heads"][0]["future_head"] = json!("head-data");
    body["definition"]["heads"][0]["parameters"][0]["future_parameter"] = json!([1, 2, 3]);
    let mut record = PortablePatchedFixtureRecord::decode(body).unwrap();
    let reference = crate::PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id: profile.modes[0].id,
    };

    record
        .migrate_legacy_to_profile_reference(reference)
        .unwrap();

    assert!(!record.is_legacy_inline());
    assert_eq!(record.profile_reference().unwrap(), Some(reference));
    assert!(record.body().get("definition").is_none());
    assert!(!contains_key(record.body(), "profile_snapshot"));
    assert_eq!(record.body()["future_binding"]["retain"], true);
    let retained = record.body()[crate::RETAINED_LEGACY_DEFINITION_FIELDS]
        .as_array()
        .unwrap();
    assert_retained(retained, "/future_definition", json!({"opaque": 7}));
    assert_retained(retained, "/heads/0/future_head", json!("head-data"));
    assert_retained(
        retained,
        "/heads/0/parameters/0/future_parameter",
        json!([1, 2, 3]),
    );
}

fn assert_retained(retained: &[Value], pointer: &str, expected: Value) {
    let field = retained
        .iter()
        .find(|field| field["json_pointer"] == pointer)
        .unwrap_or_else(|| panic!("missing retained field at {pointer}"));
    assert_eq!(field["value"], expected);
}

fn contains_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Array(values) => values.iter().any(|value| contains_key(value, key)),
        Value::Object(values) => {
            values.contains_key(key) || values.values().any(|value| contains_key(value, key))
        }
        _ => false,
    }
}
