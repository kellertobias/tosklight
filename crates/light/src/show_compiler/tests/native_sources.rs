use super::super::{prepare_normalized_show_candidate_incremental, prepare_show_candidate};
use light_core::{
    AttributeKey, NativeColorBinding, NativeColorValue, PhysicalDataQuality, Xyz,
    programming::NativeColorRecipe,
};
use light_fixture::{
    ChannelFunction, ColorPhysicalModel, FixtureProfile, HeadOpticalPath, OpticalEmitter,
    OpticalEmitterBand, OpticalSource,
};
use light_show::{FixtureProfileRevision, ShowStore};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

fn original_profile() -> FixtureProfile {
    let mut p = FixtureProfile::blank();
    p.revision = 1;
    p.manufacturer = "Test".into();
    p.name = "Show retained source".into();
    let mode = &mut p.modes[0];
    let head_id = mode.heads[0].id;
    let channel_id = Uuid::new_v4();
    let function = ChannelFunction::continuous("Red", AttributeKey("color.red".into()), 255);
    let binding = NativeColorBinding {
        channel_id,
        function_id: function.id,
    };
    mode.channels = vec![
        serde_json::from_value(json!({
            "id":channel_id, "head_id":head_id, "split":1,
            "fixture_attribute":"color.red", "attribute":"color.red", "resolution":"u8",
            "default_raw":0, "highlight_raw":255, "functions":[function]
        }))
        .unwrap(),
    ];
    mode.splits[0].footprint = 1;
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id,
            controls: vec![channel_id],
            filters: vec![],
            measurements: vec![],
            source: OpticalSource::Additive {
                emitters: vec![OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: "Red".into(),
                    binding,
                    xyz: Some(Xyz {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Visible,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: light_fixture::OpticalProvenance {
                        quality: PhysicalDataQuality::Estimated,
                        ..Default::default()
                    },
                }],
            },
        }],
    });
    p
}

fn recipe(p: &FixtureProfile) -> NativeColorRecipe {
    let mode = &p.modes[0];
    let c = &mode.channels[0];
    NativeColorRecipe {
        source: p.native_color_identity(mode.id, mode.heads[0].id).unwrap(),
        channels: vec![NativeColorValue {
            channel_id: c.id,
            function_id: c.functions[0].id,
            raw: 255,
        }],
        spreads: vec![],
    }
}

#[test]
fn source_catalogue_survives_patch_absence_and_reuses_original_models_after_revision_insertion() {
    let p = original_profile();
    let original = recipe(&p);
    let (store, _) = ShowStore::create(":memory:", "Original Color sources").unwrap();
    store
        .insert_fixture_profile_revision(
            &FixtureProfileRevision::from_profile(serde_json::to_value(&p).unwrap()).unwrap(),
        )
        .unwrap();
    let document = store.portable_document().unwrap();
    let snapshot = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    assert!(
        snapshot.fixtures.is_empty(),
        "catalogue cannot depend on current patch membership"
    );
    assert!(snapshot.native_color_sources.is_prepared());
    let retained = snapshot
        .native_color_sources
        .resolve(&original.source)
        .unwrap();
    assert_eq!(
        retained.predict(&original).unwrap().visible.unwrap().xyz.x,
        1.
    );

    let mut replacement = p.clone();
    replacement.revision = 2;
    let OpticalSource::Additive { emitters } =
        &mut replacement.modes[0].color_physical.as_mut().unwrap().paths[0].source
    else {
        unreachable!()
    };
    emitters[0].xyz = Some(Xyz {
        x: 0.,
        y: 1.,
        z: 0.,
    });
    let newer = recipe(&replacement);
    let mut transaction = document.transaction();
    transaction
        .put_fixture_profile_revision(
            FixtureProfileRevision::from_profile(serde_json::to_value(replacement).unwrap())
                .unwrap(),
        )
        .unwrap();
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &snapshot)
        .unwrap()
        .into_parts()
        .1;
    assert!(!Arc::ptr_eq(
        &snapshot.native_color_sources,
        &next.native_color_sources
    ));
    assert!(Arc::ptr_eq(
        &retained,
        &next.native_color_sources.resolve(&original.source).unwrap()
    ));
    assert_eq!(
        next.native_color_sources
            .resolve(&newer.source)
            .unwrap()
            .predict(&newer)
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .y,
        1.
    );
    assert!(
        snapshot
            .native_color_sources
            .resolve(&newer.source)
            .is_err(),
        "old captured generation stays independent"
    );
}

#[test]
fn unrelated_edits_share_catalogue_and_snapshot_json_cannot_claim_prepared_models() {
    let p = original_profile();
    let original = recipe(&p);
    let (store, _) = ShowStore::create(":memory:", "Catalogue reuse").unwrap();
    store
        .insert_fixture_profile_revision(
            &FixtureProfileRevision::from_profile(serde_json::to_value(p).unwrap()).unwrap(),
        )
        .unwrap();
    let document = store.portable_document().unwrap();
    let snapshot = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut transaction = document.transaction();
    transaction.put(
        "preset",
        "1.1",
        json!({"number":1,"decimal":1,"name":"Dim","family":"Intensity","values":{}}),
    );
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &snapshot)
        .unwrap()
        .into_parts()
        .1;
    assert!(Arc::ptr_eq(
        &snapshot.native_color_sources,
        &next.native_color_sources
    ));
    let json = serde_json::to_value(&next).unwrap();
    assert!(json.get("native_color_sources").is_none());
    let deserialized: light_engine::EngineSnapshot = serde_json::from_value(json).unwrap();
    assert!(!deserialized.native_color_sources.is_prepared());
    assert!(
        deserialized
            .native_color_sources
            .resolve(&original.source)
            .is_err()
    );
    let rebuilt = prepare_normalized_show_candidate_incremental(
        &document,
        document.transaction(),
        &deserialized,
    )
    .unwrap()
    .into_parts()
    .1;
    assert!(rebuilt.native_color_sources.is_prepared());
    assert!(
        rebuilt
            .native_color_sources
            .resolve(&original.source)
            .is_ok()
    );
}

#[test]
fn unsupported_unused_optical_data_does_not_prevent_show_open() {
    let mut p = original_profile();
    let source = recipe(&p).source;
    p.modes[0].color_physical.as_mut().unwrap().version = 999;
    let (store, _) = ShowStore::create(":memory:", "Unavailable original source").unwrap();
    store
        .insert_fixture_profile_revision(
            &FixtureProfileRevision::from_profile(serde_json::to_value(p).unwrap()).unwrap(),
        )
        .unwrap();
    let document = store.portable_document().unwrap();
    let snapshot = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    assert!(snapshot.native_color_sources.is_prepared());
    assert!(snapshot.native_color_sources.resolve(&source).is_err());
}

#[test]
fn renaming_or_deleting_a_patch_does_not_rebuild_original_source_models() {
    let (profile, fixture, _) = super::support::portable_fixture();
    let id = fixture.fixture_id.0.to_string();
    let mut body = serde_json::to_value(fixture).unwrap();
    body["definition"]["profile_snapshot"] = profile.profile().clone();
    let (store, document) =
        super::support::document_with_objects(&[("patched_fixture", &id, body)]);
    let (normalization, _) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    store.apply_portable_transaction(normalization).unwrap();
    let document = store.portable_document().unwrap();
    let snapshot = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut changed = document
        .object("patched_fixture", &id)
        .unwrap()
        .body()
        .clone();
    changed["name"] = json!("Moved fixture");
    let mut transaction = document.transaction();
    transaction.put("patched_fixture", &id, changed);
    let renamed = prepare_normalized_show_candidate_incremental(&document, transaction, &snapshot)
        .unwrap()
        .into_parts()
        .1;
    assert!(!Arc::ptr_eq(&renamed.fixtures, &snapshot.fixtures));
    assert!(Arc::ptr_eq(
        &renamed.native_color_sources,
        &snapshot.native_color_sources
    ));
    let mut transaction = document.transaction();
    transaction.delete("patched_fixture", &id);
    let deleted = prepare_normalized_show_candidate_incremental(&document, transaction, &snapshot)
        .unwrap()
        .into_parts()
        .1;
    assert!(deleted.fixtures.is_empty());
    assert!(Arc::ptr_eq(
        &deleted.native_color_sources,
        &snapshot.native_color_sources
    ));
}
