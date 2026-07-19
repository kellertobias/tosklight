use light_core::FixtureId;
use light_fixture::{
    FixtureHead, FixtureProfile, MultiPatchInstance, PatchPolicy, PatchedFixture,
    PatchedFixtureProfileReference, PatchedHead,
};
use light_show::{FixtureProfileRevision, PortableShowDocument, ShowStore};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(super) fn portable_fixture() -> (
    FixtureProfileRevision,
    PatchedFixture,
    PatchedFixtureProfileReference,
) {
    portable_fixture_with_policy(PatchPolicy::Dmx, 9_000)
}

pub(super) fn portable_fixture_with_policy(
    policy: PatchPolicy,
    identity_base: u128,
) -> (
    FixtureProfileRevision,
    PatchedFixture,
    PatchedFixtureProfileReference,
) {
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(Uuid::from_u128(identity_base));
    profile.revision = 7;
    profile.manufacturer = "Acme".into();
    profile.name = "Portable".into();
    profile.short_name = "Port".into();
    profile.patch_policy = policy;
    profile.modes[0].id = Uuid::from_u128(identity_base + 1);
    if policy == PatchPolicy::VisualOnly {
        for split in &mut profile.modes[0].splits {
            split.footprint = 0;
        }
        profile.modes[0].channels.clear();
    }
    profile.modes[0].heads[0].id = Uuid::from_u128(identity_base + 2);
    profile.modes[0].heads.push(FixtureHead {
        id: Uuid::from_u128(identity_base + 3),
        name: "Cell".into(),
        master_shared: false,
    });
    let mode_id = profile.modes[0].id;
    let profile_id = profile.id;
    let mut raw_profile = serde_json::to_value(&profile).unwrap();
    raw_profile["future_profile"] = json!({"kept": [2, 1]});
    let stored = FixtureProfileRevision::from_profile(raw_profile).unwrap();
    let fixture_id = FixtureId(Uuid::from_u128(identity_base + 10));
    let retained_head_id = FixtureId(Uuid::from_u128(identity_base + 11));
    let fixture = PatchedFixture {
        fixture_id,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Fixture 1".into(),
        definition: profile.resolved_definition(mode_id).unwrap(),
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![
            PatchedHead {
                profile_head_id: None,
                head_index: 1,
                fixture_id: retained_head_id,
            },
            PatchedHead {
                profile_head_id: None,
                head_index: 99,
                fixture_id: FixtureId(Uuid::from_u128(identity_base + 12)),
            },
        ],
        multipatch: Vec::<MultiPatchInstance>::new(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    (
        stored,
        fixture,
        PatchedFixtureProfileReference {
            profile_id,
            profile_revision: 7,
            mode_id,
        },
    )
}

pub(super) fn profile_head_id(profile: &FixtureProfileRevision) -> Uuid {
    Uuid::parse_str(
        profile.profile()["modes"][0]["heads"][1]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

pub(super) fn document_with_objects(
    objects: &[(&str, &str, Value)],
) -> (ShowStore, PortableShowDocument) {
    let (store, _) = ShowStore::create(":memory:", "Compiler migration").unwrap();
    for (kind, id, body) in objects {
        store.put_object(kind, id, body, 0).unwrap();
    }
    let document = store.portable_document().unwrap();
    (store, document)
}

pub(super) fn stored_body(store: &ShowStore, kind: &str, id: &str) -> Value {
    store
        .objects(kind)
        .unwrap()
        .into_iter()
        .find(|object| object.id == id)
        .unwrap()
        .body
}

pub(super) fn snapshot_without_revision(mut snapshot: light_engine::EngineSnapshot) -> Value {
    snapshot.revision = 0;
    serde_json::to_value(snapshot).unwrap()
}
