//! TL-572: Cue `dynamic_changes` of type `static` carrying requested semantic intent through the
//! real selective-import preview/apply path when source and destination identities collide.
//!
//! TL-567 (`semantic_intent.rs`) covers ordinary Cue `changes`/`group_changes` after AddToEnd and
//! `programming_native.rs` covers Direct FixAT masks. Here only the Static Dynamic value path is
//! added: Color/UV (including zero relative output), Angles, a tagged Point target and independent
//! Focus and Zoom. Values are compared as typed intent after decoding the stored body, never as
//! fitted channels; nested fixture/Point/Group identities must follow the planner's policy.
use super::semantic_intent::key_of;
use super::support::*;
use crate::programming::semantic_intent_cases::*;
use crate::selective_import::*;
use crate::{ActiveShowObjectBody, ActiveShowObjectKind};
use light_core::{AttributeValue, FixtureId, programming::ProgrammingOwner};
use light_dynamics::{DynamicSemanticValue, DynamicValueTiming};
use light_playback::{CueDynamicChange, CueList, GroupCueChange};
use light_programmer::GroupDefinition;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::atomic::Ordering};
use uuid::Uuid;

const CUE_LIST_ID: &str = "00000000-0000-0000-0000-00000000c572";
const GROUP: &str = "static-front";
const SOURCE_NAME: &str = "Semantic static";
const DESTINATION_NAME: &str = "Destination static";

fn timing() -> DynamicValueTiming {
    DynamicValueTiming {
        fade_millis: Some(1_250),
        delay_millis: Some(300),
    }
}

fn static_change(
    fixture: FixtureId,
    owner: ProgrammingOwner,
    value: AttributeValue,
) -> CueDynamicChange {
    CueDynamicChange {
        fixture_id: fixture,
        attribute: key_of(owner),
        value: DynamicSemanticValue::Static {
            value,
            timing: timing(),
        },
        automatic_restore: false,
    }
}

/// Fixture carries whole Color with UV, a Point target at `point`, and separate Focus and Zoom.
/// The Point fixture itself carries a UV-only zero-output Color and Angles.
fn static_changes(fixture: FixtureId, point: FixtureId) -> Vec<CueDynamicChange> {
    vec![
        static_change(fixture, ProgrammingOwner::Color, color(magenta_with_uv())),
        static_change(fixture, ProgrammingOwner::Position, point_target(point.0)),
        static_change(fixture, ProgrammingOwner::Focus, focus()),
        static_change(fixture, ProgrammingOwner::Zoom, zoom()),
        static_change(point, ProgrammingOwner::Color, color(uv_only_black())),
        static_change(point, ProgrammingOwner::Position, angles()),
    ]
}

fn group_change(group: &str, fixture: FixtureId) -> GroupCueChange {
    GroupCueChange {
        replacement_projections: Default::default(),
        preset_reference: None,
        group_id: group.into(),
        attribute: key_of(ProgrammingOwner::Color),
        value: Some(group_family(
            ProgrammingOwner::Color,
            color(warm_white_zero_output()),
            [(fixture.0, color(magenta()))],
        )),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn static_cue_list(
    name: &str,
    changes: Vec<CueDynamicChange>,
    group_changes: Vec<GroupCueChange>,
) -> Value {
    json!({
        "id": CUE_LIST_ID, "name": name, "priority": 10,
        "mode": "sequence", "looped": false,
        "cues": [{
            "id": Uuid::from_u128(0x572c1), "number": "1", "name": "Static intent",
            "changes": [], "group_changes": group_changes, "dynamic_changes": changes,
            "fade_millis": 0, "delay_millis": 0, "trigger": {"type": "manual"}
        }]
    })
}

struct Collision {
    fixture: PortableFixtureTestRecord,
    point: PortableFixtureTestRecord,
    /// Exact destination bodies that already occupy every source identity before import.
    destination: BTreeMap<(&'static str, String), Value>,
}

/// Seeds the source bundle and a destination that already owns the same fixture, Point, Group
/// and Cue list identities with different content, plus one unrelated destination Cue list.
fn seed_collision(rig: &TestRig) -> Collision {
    let fixture = portable_fixture_record(572_000, 1);
    let point = portable_fixture_record(573_000, 2);
    let mut destination = BTreeMap::new();
    for record in [&fixture, &point] {
        let id = record.fixture_id.0.to_string();
        rig.source_profile(&record.profile);
        rig.target_profile(&record.profile);
        rig.source_object("patched_fixture", &id, record.body.clone());
        let mut occupied = record.body.clone();
        occupied["name"] = json!(format!("Destination {id}"));
        rig.target_object("patched_fixture", &id, occupied.clone());
        destination.insert(("patched_fixture", id), occupied);
    }
    let group = |name: &str| {
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: name.into(),
            fixtures: vec![fixture.fixture_id],
            ..Default::default()
        })
        .unwrap()
    };
    rig.source_object("group", GROUP, group("Source front"));
    let occupied_group = group("Destination front");
    rig.target_object("group", GROUP, occupied_group.clone());
    destination.insert(("group", GROUP.into()), occupied_group);
    rig.source_object(
        "cue_list",
        CUE_LIST_ID,
        static_cue_list(
            SOURCE_NAME,
            static_changes(fixture.fixture_id, point.fixture_id),
            vec![group_change(GROUP, fixture.fixture_id)],
        ),
    );
    let occupied_cue = static_cue_list(
        DESTINATION_NAME,
        vec![static_change(
            fixture.fixture_id,
            ProgrammingOwner::Color,
            color(warm_white_3200()),
        )],
        vec![],
    );
    rig.target_object("cue_list", CUE_LIST_ID, occupied_cue.clone());
    destination.insert(("cue_list", CUE_LIST_ID.into()), occupied_cue);
    Collision {
        fixture,
        point,
        destination,
    }
}

fn destination_of(preview: &SelectiveShowImportPreview, kind: &str, source: &str) -> String {
    preview
        .objects
        .iter()
        .find(|entry| entry.source == key(kind, source))
        .unwrap_or_else(|| panic!("{kind}/{source} is not planned"))
        .destination
        .id()
        .to_owned()
}

fn decoded_cue_list(body: &Value) -> CueList {
    let decoded =
        ActiveShowObjectBody::decode(ActiveShowObjectKind::CueList, body.clone()).unwrap();
    let ActiveShowObjectBody::CueList(list) = decoded else {
        panic!("typed Cue list")
    };
    list.typed().clone()
}

/// The expected imported Cue: only fixture keys, Point targets and Group identities change.
fn expected_changes(
    collision: &Collision,
    ids: &BTreeMap<Uuid, Uuid>,
    group: &str,
) -> (Vec<CueDynamicChange>, Vec<GroupCueChange>) {
    let changes = static_changes(collision.fixture.fixture_id, collision.point.fixture_id)
        .into_iter()
        .map(|change| {
            let DynamicSemanticValue::Static { value, timing } = &change.value else {
                unreachable!()
            };
            CueDynamicChange {
                fixture_id: FixtureId(ids[&change.fixture_id.0]),
                value: DynamicSemanticValue::Static {
                    value: remap(value, ids),
                    timing: *timing,
                },
                ..change
            }
        })
        .collect();
    let group_change = group_change(GROUP, collision.fixture.fixture_id);
    let group_change = GroupCueChange {
        replacement_projections: Default::default(),
        group_id: group.into(),
        value: group_change.value.as_ref().map(|value| remap(value, ids)),
        ..group_change
    };
    (changes, vec![group_change])
}

fn assert_static_cue(
    rig: &TestRig,
    collision: &Collision,
    cue_list_id: &str,
    ids: &BTreeMap<Uuid, Uuid>,
    group: &str,
) {
    let document = rig.target_document();
    let body = document.object("cue_list", cue_list_id).unwrap().body();
    let (changes, group_changes) = expected_changes(collision, ids, group);
    let stored = decoded_cue_list(body);
    assert_eq!(stored.name, SOURCE_NAME);
    assert_eq!(stored.cues[0].dynamic_changes, changes);
    assert_eq!(stored.cues[0].group_changes, group_changes);
    assert!(stored.cues[0].changes.is_empty());
    for (index, change) in changes.iter().enumerate() {
        let persisted = &body["cues"][0]["dynamic_changes"][index];
        assert_eq!(persisted["value"]["type"], "static", "{persisted}");
        assert_eq!(persisted["fixture_id"], json!(change.fixture_id.0));
        assert_semantic_value(&persisted["value"]["value"]);
    }
    let point = &body["cues"][0]["dynamic_changes"][1]["value"]["value"]["value"];
    assert_eq!(point["kind"], "target");
    assert_eq!(
        point["reference"],
        json!({"kind": "point", "point_id": ids[&collision.point.fixture_id.0]})
    );
    let focus_zoom = changes
        .iter()
        .filter(|change| change.fixture_id.0 == ids[&collision.fixture.fixture_id.0])
        .map(|change| change.attribute.0.to_string())
        .collect::<Vec<_>>();
    assert_eq!(focus_zoom, ["color", "position", "focus", "zoom"]);
    let installed = rig.ports.installed.lock();
    let snapshot = installed.as_ref().expect("apply installs the candidate");
    let compiled = snapshot
        .cue_lists
        .iter()
        .find(|list| list.name == SOURCE_NAME)
        .expect("imported static Cue list is compiled");
    assert_eq!(compiled.cues[0].dynamic_changes, changes);
    assert_eq!(compiled.cues[0].group_changes, group_changes);
}

/// The occupied destination objects keep their identity and requested content. The import
/// transaction also normalizes existing destination bodies to their canonical shape (Group
/// `source`, Cue list defaults, fixture split/multipatch patches), which is existing compatibility
/// policy, so the comparison is on the owned content rather than on the seeded JSON text.
fn assert_destination_unchanged(rig: &TestRig, collision: &Collision) {
    let document = rig.target_document();
    for ((kind, id), expected) in &collision.destination {
        let body = document.object(kind, id).unwrap().body();
        match *kind {
            "cue_list" => {
                let [actual, expected] = [body, expected].map(decoded_cue_list);
                assert_eq!(
                    (actual.id, actual.name, actual.cues),
                    (expected.id, expected.name, expected.cues)
                );
            }
            "group" => {
                let [actual, expected] = [body, expected]
                    .map(|body| serde_json::from_value::<GroupDefinition>(body.clone()).unwrap());
                assert_eq!(
                    (actual.name, actual.fixtures, actual.programming),
                    (expected.name, expected.fixtures, expected.programming)
                );
            }
            _ => {
                for field in ["fixture_id", "fixture_number", "name", "future_fixture"] {
                    assert_eq!(body[field], expected[field], "{kind}/{id} {field}");
                }
            }
        }
    }
}

/// Both duplicate policies (AddToEnd without resolutions and ReplaceByPosition with explicit
/// Duplicate resolutions) allocate new identities for every colliding object. The Static values
/// then follow the new fixture, Point and Group identities with all other intent unchanged, and
/// the colliding destination objects stay exactly as they were.
#[test]
fn static_dynamic_cue_intent_follows_duplicated_identities_after_collisions() {
    for duplicate_by_resolution in [false, true] {
        let rig = TestRig::new();
        let collision = seed_collision(&rig);
        let mut request = rig.request("cue_list", CUE_LIST_ID);
        if duplicate_by_resolution {
            for (kind, id) in collision.destination.keys() {
                request = request.resolve(key(kind, id), ImportConflictResolution::Duplicate);
            }
        } else {
            request = request.with_mode(ImportLoadMode::AddToEnd);
        }
        let preview = rig.preview(request);
        assert!(
            preview.can_apply(),
            "{duplicate_by_resolution}: {:?}",
            preview.blockers
        );
        let fixture = collision.fixture.fixture_id.0;
        let point = collision.point.fixture_id.0;
        let ids = BTreeMap::from([
            (
                fixture,
                Uuid::parse_str(&destination_of(
                    &preview,
                    "patched_fixture",
                    &fixture.to_string(),
                ))
                .unwrap(),
            ),
            (
                point,
                Uuid::parse_str(&destination_of(
                    &preview,
                    "patched_fixture",
                    &point.to_string(),
                ))
                .unwrap(),
            ),
        ]);
        let group = destination_of(&preview, "group", GROUP);
        let cue_list = destination_of(&preview, "cue_list", CUE_LIST_ID);
        assert_ne!(ids[&fixture], fixture);
        assert_ne!(ids[&point], point);
        assert_ne!(group, GROUP);
        assert_ne!(cue_list, CUE_LIST_ID);
        rig.apply(&preview).unwrap();
        assert_static_cue(&rig, &collision, &cue_list, &ids, &group);
        assert_destination_unchanged(&rig, &collision);
    }
}

/// KeepDestination binds every nested reference to the destination identity (here the same
/// ids) while ReplaceDestination writes the source Cue intent over the occupied Cue list.
#[test]
fn static_dynamic_cue_intent_binds_to_kept_destination_identities_on_replace() {
    let rig = TestRig::new();
    let collision = seed_collision(&rig);
    let mut request = rig.request("cue_list", CUE_LIST_ID).resolve(
        key("cue_list", CUE_LIST_ID),
        ImportConflictResolution::ReplaceDestination,
    );
    for (kind, id) in collision.destination.keys() {
        if *kind != "cue_list" {
            request = request.resolve(key(kind, id), ImportConflictResolution::KeepDestination);
        }
    }
    let preview = rig.preview(request);
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert_eq!(preview.conflicts.len(), collision.destination.len());
    rig.apply(&preview).unwrap();
    let ids = [collision.fixture.fixture_id.0, collision.point.fixture_id.0]
        .map(|id| (id, id))
        .into();
    assert_static_cue(&rig, &collision, CUE_LIST_ID, &ids, GROUP);
    let document = rig.target_document();
    for (kind, id) in collision
        .destination
        .keys()
        .filter(|(k, _)| *k != "cue_list")
    {
        assert!(document.object(kind, id).is_some());
    }
    assert_eq!(document.objects_of_kind("cue_list").count(), 1);
    assert_eq!(document.objects_of_kind("group").count(), 1);
    assert_eq!(document.objects_of_kind("patched_fixture").count(), 2);
    let kept = Collision {
        destination: collision
            .destination
            .iter()
            .filter(|((kind, _), _)| *kind != "cue_list")
            .map(|(key, body)| (key.clone(), body.clone()))
            .collect(),
        ..collision
    };
    assert_destination_unchanged(&rig, &kept);
}

/// A Static Point target whose Point is in neither show blocks the whole import. Separately, a
/// commit and a runtime failure of a valid bundle leave every object and the show revision
/// untouched.
#[test]
fn static_dynamic_cue_failures_leave_the_destination_unchanged() {
    let rig = TestRig::new();
    let collision = seed_collision(&rig);
    let missing = FixtureId(Uuid::from_u128(0x572dead));
    rig.update_source_object(
        "cue_list",
        CUE_LIST_ID,
        static_cue_list(
            SOURCE_NAME,
            static_changes(collision.fixture.fixture_id, missing),
            vec![],
        ),
    );
    let preview = rig.preview(
        rig.request("cue_list", CUE_LIST_ID)
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(!preview.can_apply(), "a missing Static Point was accepted");
    assert!(
        format!("{:?}", preview.blockers).contains(&missing.0.to_string()),
        "{:?}",
        preview.blockers
    );
    let before = rig.target_document();
    rig.clear_steps();
    assert!(rig.apply(&preview).is_err());
    assert_eq!(rig.target_document(), before);
    assert!(!rig.steps().contains(&"commit"));

    rig.update_source_object(
        "cue_list",
        CUE_LIST_ID,
        static_cue_list(
            SOURCE_NAME,
            static_changes(collision.fixture.fixture_id, collision.point.fixture_id),
            vec![group_change(GROUP, collision.fixture.fixture_id)],
        ),
    );
    let preview = rig.preview(
        rig.request("cue_list", CUE_LIST_ID)
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    for flag in [&rig.ports.fail_prepare, rig.ports.fail_commit.as_ref()] {
        flag.store(true, Ordering::SeqCst);
        assert!(rig.apply(&preview).is_err());
        flag.store(false, Ordering::SeqCst);
        let after = rig.target_document();
        assert_eq!(after, before);
        assert_eq!(after.revision(), before.revision());
        assert!(rig.ports.installed.lock().is_none());
        assert!(rig.ports.reconciled.lock().is_empty());
    }
    assert_destination_unchanged(&rig, &collision);
}
