//! TL-567: requested semantic intent through real selective-import planning and apply.
//!
//! Complements `programming.rs` (Position Point/Dynamic remapping) and `programming_native.rs`
//! (Direct native identity). Here the stored Color/UV/relativeOutput, Position and independent
//! Focus/Zoom intents are compared as typed values after identities change, while the persisted
//! bodies are checked for remapped nested identities and the absence of native substitution.
use super::support::*;
use crate::programming::semantic_intent_cases::*;
use crate::selective_import::*;
use crate::{ActiveShowObjectBody, ActiveShowObjectKind};
use light_core::{AttributeKey, AttributeValue, FixtureId, programming::*};
use light_dynamics::*;
use light_playback::{CueChange, CueList, GroupCueChange};
use light_programmer::{
    DerivedGroup, FrozenGroup, GroupDefinition, GroupFixtureSource, GroupReference, Preset,
    PresetFamily, SelectionRule,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, atomic::Ordering},
};
use uuid::Uuid;

pub(super) const CUE_LIST_ID: &str = "00000000-0000-0000-0000-00000000c567";
const GROUP: &str = "semantic-front";

pub(super) fn key_of(owner: ProgrammingOwner) -> AttributeKey {
    owner.key()
}

pub(super) fn color_preset(fixture: FixtureId) -> Preset {
    Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        instance_id: Some(Uuid::from_u128(fixture.0.as_u128() ^ (2_u128 << 96))),
        name: "Semantic color".into(),
        family: PresetFamily::Color,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(key_of(ProgrammingOwner::Color), color(warm_white_3200()))]),
        )]),
        group_values: HashMap::from([(
            GROUP.to_owned(),
            HashMap::from([(
                key_of(ProgrammingOwner::Color),
                group_family(
                    ProgrammingOwner::Color,
                    color(uv_only_black()),
                    [(fixture.0, color(magenta_with_uv()))],
                ),
            )]),
        )]),
        universal_values: HashMap::from([(key_of(ProgrammingOwner::Color), color(magenta()))]),
        aim_at_fixture_number: None,
    }
}

pub(super) fn position_preset(fixture: FixtureId, point: Uuid) -> Preset {
    Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        instance_id: Some(Uuid::from_u128(fixture.0.as_u128() ^ (3_u128 << 96))),
        name: "Semantic position".into(),
        family: PresetFamily::Position,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(key_of(ProgrammingOwner::Position), point_target(point))]),
        )]),
        group_values: HashMap::from([(
            GROUP.to_owned(),
            HashMap::from([(
                key_of(ProgrammingOwner::Position),
                group_family(
                    ProgrammingOwner::Position,
                    angles(),
                    [(fixture.0, point_target(point))],
                ),
            )]),
        )]),
        universal_values: HashMap::from([(key_of(ProgrammingOwner::Position), angles())]),
        aim_at_fixture_number: None,
    }
}

/// Focus and Zoom are separate owners: the fixture stores Focus only, universal stores Zoom only.
pub(super) fn beam_preset(fixture: FixtureId) -> Preset {
    Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        instance_id: Some(Uuid::from_u128(fixture.0.as_u128() ^ (4_u128 << 96))),
        name: "Semantic beam".into(),
        family: PresetFamily::Beam,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(key_of(ProgrammingOwner::Focus), focus())]),
        )]),
        group_values: HashMap::from([(
            GROUP.to_owned(),
            HashMap::from([(
                key_of(ProgrammingOwner::Zoom),
                group_family(ProgrammingOwner::Zoom, zoom(), []),
            )]),
        )]),
        universal_values: HashMap::from([(key_of(ProgrammingOwner::Zoom), zoom())]),
        aim_at_fixture_number: None,
    }
}

pub(super) fn cue_changes(
    fixture: FixtureId,
    point: Uuid,
) -> (Vec<CueChange>, Vec<GroupCueChange>) {
    let fixture_change =
        |owner: ProgrammingOwner, value| CueChange::set(fixture, key_of(owner), value);
    let group_change = |owner: ProgrammingOwner, value| GroupCueChange {
        replacement_projections: Default::default(),
        preset_reference: None,
        group_id: GROUP.into(),
        attribute: key_of(owner),
        value: Some(value),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    };
    (
        vec![
            fixture_change(ProgrammingOwner::Color, color(warm_white_zero_output())),
            fixture_change(ProgrammingOwner::Position, point_target(point)),
            fixture_change(ProgrammingOwner::Zoom, zoom()),
        ],
        vec![
            group_change(
                ProgrammingOwner::Color,
                group_family(
                    ProgrammingOwner::Color,
                    color(uv_only_black()),
                    [(fixture.0, color(magenta_with_uv()))],
                ),
            ),
            group_change(ProgrammingOwner::Focus, focus()),
        ],
    )
}

pub(super) fn cue_list(fixture: FixtureId, point: Uuid) -> Value {
    let (changes, group_changes) = cue_changes(fixture, point);
    json!({
        "id": CUE_LIST_ID, "name": "Semantic", "priority": 10,
        "mode": "sequence", "looped": false,
        "cues": [{
            "id": Uuid::from_u128(0x567c1), "number": "1", "name": "Intent",
            "changes": changes, "group_changes": group_changes,
            "fade_millis": 0, "delay_millis": 0, "trigger": {"type": "manual"}
        }]
    })
}

fn color_address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        component: None,
    }
}

/// A whole-Color keyframe sourced from the Color preset, with a retained template, a bounded
/// fallback and a last-valid value per target.
fn color_dynamic(id: Uuid, preset_id: &str, fixture: FixtureId) -> Value {
    let group = group_family(
        ProgrammingOwner::Color,
        color(uv_only_black()),
        [(fixture.0, color(magenta_with_uv()))],
    );
    let source = DynamicValueSource::Preset {
        preset_id: preset_id.into(),
        address: color_address(),
        last_valid_by_target: vec![DynamicValueFallback {
            target: fixture,
            value: DynamicValue::Family(color(magenta_with_uv())),
        }],
        retained: Some(Arc::new(DynamicPresetTemplate {
            universal: Some(color(magenta())),
            groups: vec![DynamicPresetGroupTemplate {
                group_id: GROUP.into(),
                value: group,
            }],
            fixtures: vec![DynamicPresetFixtureTemplate {
                fixture_id: fixture,
                value: color(warm_white_3200()),
            }],
            fallback: Some(Box::new(DynamicPresetTemplate {
                universal: Some(color(uv_only_black())),
                ..Default::default()
            })),
        })),
    };
    let mut body = dynamic_with_dependencies(id, "unused", "unused");
    body["target_binding"] = json!({"type": "targetless"});
    body["lanes"] = json!([{
        "id": Uuid::from_u128(0x5671a),
        "programming": {
            "address": color_address(),
            "configuration": {"mode": "keyframes", "configuration": {
                "points": [
                    {"position": 0.0, "source": source, "interpolation": "linear"},
                    {"position": 0.5, "source": {"kind": "current"}, "interpolation": "linear"}
                ],
                "size": 1.0
            }}
        },
        "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
    }]);
    body
}

pub(super) struct SourceShow {
    pub fixture: PortableFixtureTestRecord,
    pub point: PortableFixtureTestRecord,
    pub dynamic: Uuid,
}

fn seed_source(rig: &TestRig) -> SourceShow {
    seed_source_with(rig, true)
}

fn seed_source_with(rig: &TestRig, color_preset_present: bool) -> SourceShow {
    let fixture = portable_fixture_record(567_000, 1);
    let point = portable_fixture_record(568_000, 2);
    for record in [&fixture, &point] {
        rig.source_profile(&record.profile);
        rig.source_object(
            "patched_fixture",
            &record.fixture_id.0.to_string(),
            record.body.clone(),
        );
    }
    rig.source_object(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "Front".into(),
            fixtures: vec![fixture.fixture_id],
            ..Default::default()
        })
        .unwrap(),
    );
    let id = fixture.fixture_id;
    for (key, preset) in [
        ("2.1", color_preset(id)),
        ("3.1", position_preset(id, point.fixture_id.0)),
        ("4.1", beam_preset(id)),
    ] {
        if key != "2.1" || color_preset_present {
            rig.source_object("preset", key, serde_json::to_value(preset).unwrap());
        }
    }
    rig.source_object("cue_list", CUE_LIST_ID, cue_list(id, point.fixture_id.0));
    let dynamic = Uuid::from_u128(0x567d);
    rig.source_object(
        "dynamic",
        &dynamic.to_string(),
        color_dynamic(dynamic, "2.1", id),
    );
    SourceShow {
        fixture,
        point,
        dynamic,
    }
}

fn bundle(rig: &TestRig, source: &SourceShow) -> SelectiveShowImportRequest {
    SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [
            key("cue_list", CUE_LIST_ID),
            key("preset", "2.1"),
            key("preset", "3.1"),
            key("preset", "4.1"),
            key("dynamic", &source.dynamic.to_string()),
        ],
    )
}

struct Destinations<'a>(&'a SelectiveShowImportPreview);

impl Destinations<'_> {
    fn id(&self, kind: &str, source: &str) -> String {
        self.0
            .objects
            .iter()
            .find(|entry| entry.source == key(kind, source))
            .unwrap_or_else(|| panic!("{kind}/{source} is not planned"))
            .destination
            .id()
            .to_owned()
    }

    fn fixture(&self, record: &PortableFixtureTestRecord) -> Uuid {
        let id = record.fixture_id.0.to_string();
        Uuid::parse_str(&self.id("patched_fixture", &id)).unwrap()
    }
}

fn decoded_preset(body: &Value) -> Preset {
    let decoded = ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, body.clone()).unwrap();
    decoded.preset().expect("typed Preset").typed().clone()
}

/// Remaps fixture/Group map keys and nested Point/member identities; nothing else.
fn remapped(preset: &Preset, fixtures: &BTreeMap<Uuid, Uuid>, group: &str) -> Preset {
    let values = |map: &HashMap<AttributeKey, AttributeValue>| {
        map.iter()
            .map(|(key, value)| (key.clone(), remap(value, fixtures)))
            .collect::<HashMap<_, _>>()
    };
    Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        values: preset
            .values
            .iter()
            .map(|(fixture, map)| (FixtureId(fixtures[&fixture.0]), values(map)))
            .collect(),
        group_values: preset
            .group_values
            .values()
            .map(|map| (group.to_owned(), values(map)))
            .collect(),
        universal_values: values(&preset.universal_values),
        ..preset.clone()
    }
}

#[test]
fn semantic_presets_cues_and_dynamic_sources_survive_import_with_remapped_identities() {
    let rig = TestRig::new();
    let source = seed_source(&rig);
    let preview = rig.preview(bundle(&rig, &source).with_mode(ImportLoadMode::AddToEnd));
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    rig.apply(&preview).unwrap();

    let ids = Destinations(&preview);
    let fixture = ids.fixture(&source.fixture);
    let point = ids.fixture(&source.point);
    assert_ne!(fixture, source.fixture.fixture_id.0, "identity must change");
    assert_ne!(
        point, source.point.fixture_id.0,
        "Point identity must change"
    );
    let group = ids.id("group", GROUP);
    assert_ne!(group, GROUP, "Group identity must change");
    let fixtures = BTreeMap::from([
        (source.fixture.fixture_id.0, fixture),
        (source.point.fixture_id.0, point),
    ]);
    let document = rig.target_document();

    for (key, original) in [
        ("2.1", color_preset(source.fixture.fixture_id)),
        (
            "3.1",
            position_preset(source.fixture.fixture_id, source.point.fixture_id.0),
        ),
        ("4.1", beam_preset(source.fixture.fixture_id)),
    ] {
        let destination = ids.id("preset", key);
        let body = document.object("preset", &destination).unwrap().body();
        let loaded = decoded_preset(body);
        let expected = remapped(&original, &fixtures, &group);
        assert_eq!(loaded.values, expected.values, "{key}");
        assert_eq!(loaded.group_values, expected.group_values, "{key}");
        assert_eq!(loaded.universal_values, expected.universal_values, "{key}");
        let allowed = ["color", "position", "focus", "zoom"];
        assert_semantic_attribute_map(&body["values"][fixture.to_string()], &allowed);
        assert_semantic_attribute_map(&body["group_values"][&group], &allowed);
        assert_semantic_attribute_map(&body["universal_values"], &allowed);
        assert!(
            body["values"]
                .get(source.fixture.fixture_id.0.to_string())
                .is_none()
        );
    }
    assert_persisted_color_fields(&document, &ids.id("preset", "2.1"), fixture, &group);
    assert_persisted_position_fields(&document, &ids.id("preset", "3.1"), fixture, point);
    assert_independent_focus_and_zoom(&document, &ids.id("preset", "4.1"), fixture, &group);
    assert_loaded_cue_list(&rig, &source, &fixtures, &group);
    assert_imported_dynamic(&document, &ids, &source, fixture, &group, true);
}

fn assert_persisted_color_fields(
    document: &light_show::PortableShowDocument,
    id: &str,
    fixture: Uuid,
    group: &str,
) {
    let body = document.object("preset", id).unwrap().body();
    let warm = &body["values"][fixture.to_string()]["color"]["value"]["intent"];
    assert_eq!(warm["white_target"]["kelvin"], json!(3200.0));
    assert_eq!(warm["white_blend"], json!(0.85_f32));
    assert_eq!(warm["allocation"], "prefer_white");
    let family = &body["group_values"][group]["color"]["value"];
    let template = &family["template"]["value"]["intent"];
    assert_eq!(template["relative_output"], json!(0.0));
    assert_eq!(template["uv"]["amount"], json!(0.9_f32));
    let member = &family["members"][fixture.to_string()]["value"]["intent"];
    assert_eq!(member["uv"]["amount"], json!(0.45_f32));
    let universal = &body["universal_values"]["color"]["value"]["intent"];
    assert_eq!(universal["uv"]["amount"], json!(0.0));
    assert_eq!(universal["white_target"]["duv"], json!(-0.004_f32));
}

fn assert_persisted_position_fields(
    document: &light_show::PortableShowDocument,
    id: &str,
    fixture: Uuid,
    point: Uuid,
) {
    let body = document.object("preset", id).unwrap().body();
    let target = &body["values"][fixture.to_string()]["position"]["value"];
    assert_eq!(target["kind"], "target");
    assert_eq!(
        target["reference"],
        json!({"kind": "point", "point_id": point})
    );
    assert_eq!(
        target["offset_metres"][1],
        json!({"kind": "value", "value": -1.25})
    );
    let family = &body["universal_values"]["position"]["value"];
    assert_eq!(family["kind"], "angles");
    assert!(family.get("reference").is_none(), "angles gained a target");
}

fn assert_independent_focus_and_zoom(
    document: &light_show::PortableShowDocument,
    id: &str,
    fixture: Uuid,
    group: &str,
) {
    let body = document.object("preset", id).unwrap().body();
    let keys = |value: &Value| {
        let mut keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        keys
    };
    assert_eq!(keys(&body["values"][fixture.to_string()]), ["focus"]);
    assert_eq!(keys(&body["group_values"][group]), ["zoom"]);
    assert_eq!(keys(&body["universal_values"]), ["zoom"]);
}

fn assert_loaded_cue_list(
    rig: &TestRig,
    source: &SourceShow,
    fixtures: &BTreeMap<Uuid, Uuid>,
    group: &str,
) {
    let installed = rig.ports.installed.lock();
    let snapshot = installed.as_ref().expect("apply installs the candidate");
    let list: &CueList = snapshot
        .cue_lists
        .iter()
        .find(|list| list.name == "Semantic")
        .expect("imported cue list is compiled");
    let cue = &list.cues[0];
    let (changes, group_changes) =
        cue_changes(source.fixture.fixture_id, source.point.fixture_id.0);
    let fixture = FixtureId(fixtures[&source.fixture.fixture_id.0]);
    let expected = changes
        .into_iter()
        .map(|change| CueChange {
            fixture_id: fixture,
            value: change.value.as_ref().map(|value| remap(value, fixtures)),
            ..change
        })
        .collect::<Vec<_>>();
    assert_eq!(cue.changes, expected);
    let expected = group_changes
        .into_iter()
        .map(|change| GroupCueChange {
            replacement_projections: Default::default(),
            group_id: group.into(),
            value: change.value.as_ref().map(|value| remap(value, fixtures)),
            ..change
        })
        .collect::<Vec<_>>();
    assert_eq!(cue.group_changes, expected);
}

fn assert_imported_dynamic(
    document: &light_show::PortableShowDocument,
    ids: &Destinations<'_>,
    source: &SourceShow,
    fixture: Uuid,
    group: &str,
    live_preset_imported: bool,
) {
    let body = document
        .object("dynamic", &ids.id("dynamic", &source.dynamic.to_string()))
        .unwrap()
        .body();
    let definition: DynamicDefinition = serde_json::from_value(body.clone()).unwrap();
    validate_definition(&definition).unwrap();
    let DynamicLaneBody::Programming(lane) = &definition.lanes[0].body else {
        panic!("programming lane")
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &lane.configuration else {
        panic!("keyframes")
    };
    let DynamicValueSource::Preset {
        preset_id,
        last_valid_by_target,
        retained,
        ..
    } = &config.points[0].source
    else {
        panic!("Preset source")
    };
    let expected_preset = if live_preset_imported {
        ids.id("preset", "2.1")
    } else {
        "2.1".into()
    };
    assert_eq!(preset_id, &expected_preset);
    assert_eq!(
        last_valid_by_target,
        &vec![DynamicValueFallback {
            target: FixtureId(fixture),
            value: DynamicValue::Family(color(magenta_with_uv())),
        }]
    );
    let retained = retained.as_ref().expect("retained template survives");
    assert_eq!(retained.universal, Some(color(magenta())));
    assert_eq!(retained.groups[0].group_id, group);
    assert_eq!(
        retained.groups[0].value,
        group_family(
            ProgrammingOwner::Color,
            color(uv_only_black()),
            [(fixture, color(magenta_with_uv()))],
        )
    );
    assert_eq!(retained.fixtures[0].fixture_id, FixtureId(fixture));
    assert_eq!(retained.fixtures[0].value, color(warm_white_3200()));
    let fallback = retained.fallback.as_deref().expect("fallback survives");
    if live_preset_imported {
        // The candidate's retention stage re-derives the generation from the imported live
        // Preset; its fallback may only hold remapped semantic values from the authored set.
        let known = [
            magenta(),
            magenta_with_uv(),
            warm_white_3200(),
            uv_only_black(),
        ]
        .map(color);
        for value in fallback
            .universal
            .iter()
            .chain(fallback.fixtures.iter().map(|f| &f.value))
        {
            assert!(known.contains(value), "{value:?}");
        }
        assert!(fallback.fixtures.iter().all(|f| f.fixture_id.0 == fixture));
        assert!(fallback.groups.iter().all(|g| g.group_id == group));
    } else {
        assert_eq!(fallback.universal, Some(color(uv_only_black())));
        assert!(fallback.groups.is_empty() && fallback.fixtures.is_empty());
    }
}

/// With its live Preset absent from both shows, the Dynamic keeps the authored retained
/// generation and bounded fallback exactly, only remapping fixture and Group identities.
#[test]
fn dynamic_without_its_live_preset_keeps_authored_retained_and_fallback_intent() {
    let rig = TestRig::new();
    let source = seed_source_with(&rig, false);
    let preview = rig.preview(
        rig.request("dynamic", &source.dynamic.to_string())
            .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(
        !preview
            .dependencies
            .iter()
            .any(|dependency| dependency.dependency.kind() == "preset")
    );
    rig.apply(&preview).unwrap();
    let ids = Destinations(&preview);
    let fixture = ids.fixture(&source.fixture);
    assert_ne!(fixture, source.fixture.fixture_id.0);
    let group = ids.id("group", GROUP);
    let document = rig.target_document();
    assert_imported_dynamic(&document, &ids, &source, fixture, &group, false);
    let body = document
        .object("dynamic", &ids.id("dynamic", &source.dynamic.to_string()))
        .unwrap()
        .body();
    let point = "/lanes/0/programming/configuration/configuration/points/0/source";
    assert_eq!(
        body.pointer(&format!("{point}/preset_id")),
        Some(&json!("2.1"))
    );
}

#[test]
fn conflicting_destination_preset_keeps_or_replaces_its_complete_intent() {
    for resolution in [
        ImportConflictResolution::KeepDestination,
        ImportConflictResolution::ReplaceDestination,
    ] {
        let rig = TestRig::new();
        let source = seed_source(&rig);
        let destination = Preset {
            universal_values: HashMap::from([(
                key_of(ProgrammingOwner::Color),
                color(warm_white_zero_output()),
            )]),
            values: HashMap::new(),
            group_values: HashMap::new(),
            ..color_preset(source.fixture.fixture_id)
        };
        let destination_body = serde_json::to_value(&destination).unwrap();
        rig.target_object("preset", "2.1", destination_body.clone());
        let preview = rig.preview(
            rig.request("dynamic", &source.dynamic.to_string())
                .resolve(key("preset", "2.1"), resolution),
        );
        assert!(
            preview.can_apply(),
            "{resolution:?}: {:?}",
            preview.blockers
        );
        rig.apply(&preview).unwrap();
        let document = rig.target_document();
        let body = document.object("preset", "2.1").unwrap().body();
        let dynamic = document
            .object("dynamic", &source.dynamic.to_string())
            .unwrap()
            .body();
        let preset_id = dynamic
            .pointer("/lanes/0/programming/configuration/configuration/points/0/source/preset_id")
            .unwrap();
        assert_eq!(preset_id, &json!("2.1"), "{resolution:?}");
        let loaded = decoded_preset(body);
        match resolution {
            ImportConflictResolution::KeepDestination => {
                assert_persisted_eq(body, &destination_body);
                assert_eq!(loaded.universal_values, destination.universal_values);
            }
            _ => {
                let fixtures =
                    BTreeMap::from([(source.fixture.fixture_id.0, source.fixture.fixture_id.0)]);
                let expected = remapped(&color_preset(source.fixture.fixture_id), &fixtures, GROUP);
                assert_eq!(loaded.universal_values, expected.universal_values);
                assert_eq!(loaded.values, expected.values);
                assert_eq!(loaded.group_values, expected.group_values);
            }
        }
    }
}

#[test]
fn semantic_bundle_commit_and_runtime_failures_leave_the_target_unchanged() {
    for failure in ["prepare", "commit"] {
        let rig = TestRig::new();
        let source = seed_source(&rig);
        let preview = rig.preview(bundle(&rig, &source).with_mode(ImportLoadMode::AddToEnd));
        assert!(preview.can_apply(), "{:?}", preview.blockers);
        let before = rig.target_document();
        let flag = match failure {
            "prepare" => &rig.ports.fail_prepare,
            _ => rig.ports.fail_commit.as_ref(),
        };
        flag.store(true, Ordering::SeqCst);
        assert!(rig.apply(&preview).is_err(), "{failure}");
        assert_eq!(rig.target_document(), before, "{failure}");
        assert!(rig.ports.installed.lock().is_none(), "{failure}");
        assert!(rig.ports.reconciled.lock().is_empty(), "{failure}");
        flag.store(false, Ordering::SeqCst);
        rig.apply(&preview).unwrap();
        assert!(rig.target_document().objects_of_kind("preset").count() == 3);
    }
}

#[test]
fn unresolved_point_target_blocks_the_whole_import() {
    let rig = TestRig::new();
    let source = seed_source(&rig);
    let missing = Uuid::from_u128(0x567dead);
    rig.update_source_object(
        "cue_list",
        CUE_LIST_ID,
        cue_list(source.fixture.fixture_id, missing),
    );
    let preview = rig.preview(bundle(&rig, &source).with_mode(ImportLoadMode::AddToEnd));
    assert!(!preview.can_apply(), "a missing Point target was accepted");
    assert!(
        format!("{:?}", preview.blockers).contains(&missing.to_string()),
        "{:?}",
        preview.blockers
    );
    let before = rig.target_document();
    rig.clear_steps();
    assert!(rig.apply(&preview).is_err());
    assert_eq!(rig.target_document(), before);
    assert!(rig.ports.installed.lock().is_none());
    assert!(!rig.steps().contains(&"commit"));
}

const PARENT: &str = "semantic-parent";

/// Every stored Group membership shape that can carry live-Group programming.
#[derive(Clone, Copy, Debug)]
enum Membership {
    Legacy,
    Explicit,
    Derived,
    Frozen,
}

struct LiveGroupShow {
    first: PortableFixtureTestRecord,
    second: PortableFixtureTestRecord,
    template_point: PortableFixtureTestRecord,
    member_point: PortableFixtureTestRecord,
    group: GroupDefinition,
}

impl LiveGroupShow {
    fn records(&self) -> [&PortableFixtureTestRecord; 4] {
        [
            &self.first,
            &self.second,
            &self.template_point,
            &self.member_point,
        ]
    }
}

/// Position template and member exception each target a different Point; Color keeps UV and a
/// member exception; Focus is direct and Zoom a template-only Group family.
fn live_group_programming(
    first: Uuid,
    second: Uuid,
    template_point: Uuid,
    member_point: Uuid,
) -> HashMap<AttributeKey, AttributeValue> {
    HashMap::from([
        (
            key_of(ProgrammingOwner::Position),
            group_family(
                ProgrammingOwner::Position,
                point_target(template_point),
                [(first, point_target(member_point)), (second, angles())],
            ),
        ),
        (
            key_of(ProgrammingOwner::Color),
            group_family(
                ProgrammingOwner::Color,
                color(uv_only_black()),
                [(first, color(magenta_with_uv()))],
            ),
        ),
        (key_of(ProgrammingOwner::Focus), focus()),
        (
            key_of(ProgrammingOwner::Zoom),
            group_family(ProgrammingOwner::Zoom, zoom(), []),
        ),
    ])
}

/// Seeds a live Group whose stored programming references two members and two Points. With
/// `collide`, the destination already holds every source identity (fixtures, Group and parent).
fn seed_live_group(rig: &TestRig, membership: Membership, collide: bool) -> LiveGroupShow {
    let first = portable_fixture_record(569_000, 1);
    let template_point = portable_fixture_record(570_000, 2);
    let second = portable_fixture_record(571_000, 3);
    let member_point = portable_fixture_record(572_000, 4);
    for record in [&first, &template_point, &second, &member_point] {
        let id = record.fixture_id.0.to_string();
        rig.source_profile(&record.profile);
        rig.source_object("patched_fixture", &id, record.body.clone());
        if collide {
            rig.target_profile(&record.profile);
            rig.target_object("patched_fixture", &id, record.body.clone());
        }
    }
    // Deliberately not source order, so a reordering rewrite is visible.
    let ordered = vec![second.fixture_id, first.fixture_id];
    let mut group = GroupDefinition {
        id: GROUP.into(),
        name: "Front".into(),
        fixtures: ordered.clone(),
        programming: live_group_programming(
            first.fixture_id.0,
            second.fixture_id.0,
            template_point.fixture_id.0,
            member_point.fixture_id.0,
        ),
        ..Default::default()
    };
    match membership {
        Membership::Legacy => {}
        Membership::Explicit => {
            group.source = Some(GroupFixtureSource::Explicit {
                fixture_ids: ordered,
            })
        }
        Membership::Derived => {
            group.derived_from = Some(DerivedGroup {
                source_group_id: PARENT.into(),
                rule: SelectionRule::All,
            })
        }
        Membership::Frozen => {
            group.frozen_from = Some(FrozenGroup {
                source_group_id: PARENT.into(),
                source_revision: 3,
                captured_at: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            })
        }
    }
    if matches!(membership, Membership::Derived | Membership::Frozen) {
        let parent = GroupDefinition {
            id: PARENT.into(),
            name: "Parent".into(),
            fixtures: vec![first.fixture_id, second.fixture_id],
            ..Default::default()
        };
        rig.source_object("group", PARENT, serde_json::to_value(parent).unwrap());
    }
    if collide {
        for (id, name) in [(GROUP, "Destination front"), (PARENT, "Destination parent")] {
            rig.target_object("group", id, json!({"id": id, "name": name, "fixtures": []}));
        }
    }
    rig.source_object("group", GROUP, serde_json::to_value(&group).unwrap());
    LiveGroupShow {
        first,
        second,
        template_point,
        member_point,
        group,
    }
}

fn decoded_group(body: &Value) -> GroupDefinition {
    serde_json::from_value(body.clone()).expect("typed Group")
}

/// Live-Group stored programming (`GroupDefinition::programming`) is validated and compiled as
/// LiveGroup intent, so its nested Point and member identities need the same remapping.
///
/// TL-570 (D1 from TL-567): with every source identity already taken in the destination, an
/// AddToEnd import must plan both Points and both members into the closure, rewrite the member
/// keys and the template/member `point_id`s, and keep ordered membership, the explicit/derived/
/// frozen source and every authored nonidentity value.
#[test]
fn live_group_stored_programming_remaps_nested_point_and_member_identities() {
    for membership in [
        Membership::Legacy,
        Membership::Explicit,
        Membership::Derived,
        Membership::Frozen,
    ] {
        let rig = TestRig::new();
        let show = seed_live_group(&rig, membership, true);
        let preview = rig.preview(
            rig.request("group", GROUP)
                .with_mode(ImportLoadMode::AddToEnd),
        );
        assert!(
            preview.can_apply(),
            "{membership:?}: {:?}",
            preview.blockers
        );
        for record in show.records() {
            let dependency = key("patched_fixture", &record.fixture_id.0.to_string());
            assert!(
                preview
                    .dependencies
                    .iter()
                    .any(|entry| entry.dependency == dependency),
                "{membership:?}: {dependency:?} missing from closure: {:?}",
                preview.dependencies
            );
        }
        rig.apply(&preview).unwrap();

        let ids = Destinations(&preview);
        let fixtures = show
            .records()
            .map(|record| (record.fixture_id.0, ids.fixture(record)))
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        for (source, destination) in &fixtures {
            assert_ne!(
                source, destination,
                "{membership:?}: collision kept an identity"
            );
        }
        let id = |record: &PortableFixtureTestRecord| fixtures[&record.fixture_id.0];
        let group_id = ids.id("group", GROUP);
        assert_ne!(group_id, GROUP);
        let document = rig.target_document();
        assert_eq!(
            document.object("group", GROUP).unwrap().body()["name"],
            json!("Destination front"),
            "{membership:?}: the colliding destination Group changed"
        );
        let body = document.object("group", &group_id).unwrap().body();
        let imported = decoded_group(body);

        let ordered = vec![FixtureId(id(&show.second)), FixtureId(id(&show.first))];
        // Existing policy: a canonical source wins and the legacy `fixtures` projection beside it
        // stays lossless (unscanned); without a canonical source `fixtures` is the authority.
        let legacy_fixtures = match membership {
            Membership::Explicit => show.group.fixtures.clone(),
            _ => ordered.clone(),
        };
        assert_eq!(imported.fixtures, legacy_fixtures, "{membership:?}");
        match membership {
            // The store canonicalizes legacy membership into the equivalent explicit source.
            Membership::Legacy | Membership::Explicit => {
                assert_eq!(
                    imported.source,
                    Some(GroupFixtureSource::Explicit {
                        fixture_ids: ordered
                    }),
                    "{membership:?}"
                );
                assert!(imported.derived_from.is_none() && imported.frozen_from.is_none());
            }
            Membership::Derived => {
                let derived = imported.derived_from.as_ref().expect("derived source");
                assert_eq!(derived.source_group_id, ids.id("group", PARENT));
                assert_ne!(derived.source_group_id, PARENT);
                assert_eq!(derived.rule, SelectionRule::All);
                // The store canonicalizes the legacy derivation into the equivalent reference.
                assert_eq!(
                    imported.source,
                    Some(GroupFixtureSource::References {
                        references: vec![GroupReference {
                            group_id: ids.id("group", PARENT),
                            rule: SelectionRule::All,
                        }]
                    })
                );
                assert!(imported.frozen_from.is_none());
            }
            Membership::Frozen => {
                let frozen = imported.frozen_from.as_ref().expect("frozen source");
                let authored = show.group.frozen_from.as_ref().unwrap();
                assert_eq!(frozen.source_group_id, ids.id("group", PARENT));
                assert_ne!(frozen.source_group_id, PARENT);
                assert_eq!(frozen.source_revision, authored.source_revision);
                assert_eq!(frozen.captured_at, authored.captured_at);
                assert_eq!(
                    imported.source,
                    Some(GroupFixtureSource::Explicit {
                        fixture_ids: ordered
                    })
                );
            }
        }

        // Typed equality after rewriting identities only: Color/UV, Point offsets, angles,
        // Focus and Zoom all survive exactly.
        let expected = show
            .group
            .programming
            .iter()
            .map(|(key, value)| (key.clone(), remap(value, &fixtures)))
            .collect::<HashMap<_, _>>();
        assert_eq!(imported.programming, expected, "{membership:?}");

        let position = &body["programming"]["position"]["value"];
        assert_eq!(
            position["template"]["value"]["reference"]["point_id"],
            json!(id(&show.template_point)),
            "{membership:?}: template Point target was not remapped"
        );
        let members = position["members"].as_object().unwrap();
        let mut keys = members.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        let mut expected_keys = [id(&show.first), id(&show.second)].map(|id| id.to_string());
        expected_keys.sort();
        assert_eq!(keys, expected_keys, "{membership:?}: member keys");
        assert_eq!(
            members[&id(&show.first).to_string()]["value"]["reference"]["point_id"],
            json!(id(&show.member_point)),
            "{membership:?}: member Point exception was not remapped"
        );
        let color_members = body["programming"]["color"]["value"]["members"]
            .as_object()
            .unwrap();
        assert_eq!(
            color_members.keys().collect::<Vec<_>>(),
            vec![&id(&show.first).to_string()]
        );
        for value in body["programming"].as_object().unwrap().values() {
            assert_semantic_value(value);
        }
    }
}

/// A colliding live Group follows the existing Keep/Replace policy: Keep leaves the destination
/// body byte-for-byte; Replace installs the complete source programming at unchanged identities.
#[test]
fn conflicting_live_group_keeps_or_replaces_its_stored_programming() {
    for resolution in [
        ImportConflictResolution::KeepDestination,
        ImportConflictResolution::ReplaceDestination,
    ] {
        let rig = TestRig::new();
        let show = seed_live_group(&rig, Membership::Explicit, true);
        let destination = rig
            .target_document()
            .object("group", GROUP)
            .unwrap()
            .body()
            .clone();
        let preview = rig.preview(
            rig.request("group", GROUP)
                .resolve(key("group", GROUP), resolution),
        );
        assert!(
            preview.can_apply(),
            "{resolution:?}: {:?}",
            preview.blockers
        );
        rig.apply(&preview).unwrap();
        let document = rig.target_document();
        let body = document.object("group", GROUP).unwrap().body();
        match resolution {
            ImportConflictResolution::KeepDestination => assert_eq!(body, &destination),
            _ => {
                let imported = decoded_group(body);
                assert_eq!(imported.fixtures, show.group.fixtures);
                assert_eq!(imported.source, show.group.source);
                assert_eq!(imported.programming, show.group.programming);
            }
        }
    }
}

/// An unresolved Point inside Group programming is a missing reference under the existing policy:
/// preview blocks, apply refuses, and destination contents and revision stay unchanged.
#[test]
fn unresolved_live_group_point_blocks_without_changing_the_destination() {
    for mode in [ImportLoadMode::AddToEnd, ImportLoadMode::ReplaceByPosition] {
        let rig = TestRig::new();
        let show = seed_live_group(&rig, Membership::Explicit, true);
        let missing = Uuid::from_u128(0x570dead);
        let group = GroupDefinition {
            programming: live_group_programming(
                show.first.fixture_id.0,
                show.second.fixture_id.0,
                show.template_point.fixture_id.0,
                missing,
            ),
            ..show.group.clone()
        };
        rig.update_source_object("group", GROUP, serde_json::to_value(group).unwrap());
        let request = || rig.request("group", GROUP).with_mode(mode);
        let preview = rig.preview(request());
        assert!(
            !preview.can_apply(),
            "{mode:?}: a missing Point was accepted"
        );
        assert!(
            format!("{:?}", preview.blockers).contains(&missing.to_string()),
            "{mode:?}: {:?}",
            preview.blockers
        );
        let before = rig.target_document();
        rig.clear_steps();
        assert!(rig.apply(&preview).is_err(), "{mode:?}");
        assert_eq!(rig.target_document(), before, "{mode:?}");
        assert_eq!(
            rig.preview(request()).target_revision,
            preview.target_revision,
            "{mode:?}: destination revision moved"
        );
        assert!(rig.ports.installed.lock().is_none());
        assert!(!rig.steps().contains(&"commit"));
    }
}
