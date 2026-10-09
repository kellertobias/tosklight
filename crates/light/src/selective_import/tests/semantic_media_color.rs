//! TL-560 column "Media color", row "Selective import".
//!
//! The source show patches the shipped Media Server (2-layer personality, each layer its own
//! logical head) and stores Media colour against its layers: a Color Preset with a per-layer
//! value and a live-Group family template whose member exception names layer 1, the live Group
//! of both layers, and a Cue with a per-layer change plus the same Group change. A real selective
//! import (`AddToEnd`) duplicates the Media Server: the root and both layer heads get new
//! identities. Every stored Media colour must arrive as the exact requested intent, rebound to
//! the duplicated layers (map keys and nested member exceptions), with no native substitution
//! and no reference left to a source layer, and the target must compile through the show-open
//! reader with the duplicated layers as heads of the duplicated root.
use super::semantic_intent::key_of;
use super::support::*;
use crate::programming::semantic_intent_cases::*;
use crate::selective_import::*;
use crate::{ActiveShowObjectBody, ActiveShowObjectKind};
use light_core::{AttributeValue, FixtureId, programming::ProgrammingOwner};
use light_fixture::{FixtureProfile, PatchedFixture, PatchedHead, PortablePatchedFixtureRecord};
use light_playback::{CueChange, CueList, GroupCueChange};
use light_programmer::{GroupDefinition, Preset, PresetFamily};
use light_show::FixtureProfileRevision;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

const GROUP: &str = "media-layers";
const CUE_LIST_ID: &str = "00000000-0000-0000-0000-0000000c560e";

fn shipped_media_server() -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fixture-library/tosklight--media-server.toskfixture");
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

fn media_server(
    profile: &FixtureProfile,
    root: FixtureId,
    layers: [FixtureId; 2],
) -> PatchedFixture {
    let mode = &profile.modes[0];
    let definition = profile.resolved_definition(mode.id).unwrap();
    let mut ids = layers.iter();
    let logical_heads = mode
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .map(|(index, head)| PatchedHead {
            profile_head_id: Some(head.id),
            head_index: definition.heads[index].index,
            fixture_id: *ids.next().unwrap(),
        })
        .collect();
    PatchedFixture {
        model_scale: None,
        scenery_options: Default::default(),
        scenery_size_metres: None,
        fixture_id: root,
        fixture_number: Some(560),
        virtual_fixture_number: None,
        name: "Media Server".into(),
        definition,
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        note: None,
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads,
        multipatch: Vec::new(),
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
        freeze: Default::default(),
    }
}

/// The live-Group Media colour: magenta for the Group, warm white for layer 1.
fn group_color(layer: Uuid) -> AttributeValue {
    group_family(
        ProgrammingOwner::Color,
        color(magenta()),
        [(layer, color(warm_white_3200()))],
    )
}

fn preset(layers: [FixtureId; 2]) -> Preset {
    Preset {
        instance_id: None,
        name: "Media colour".into(),
        family: PresetFamily::Color,
        number: 1,
        values: HashMap::from([(
            layers[1],
            HashMap::from([(key_of(ProgrammingOwner::Color), color(magenta_with_uv()))]),
        )]),
        group_values: HashMap::from([(
            GROUP.to_owned(),
            HashMap::from([(key_of(ProgrammingOwner::Color), group_color(layers[0].0))]),
        )]),
        universal_values: HashMap::new(),
        aim_at_fixture_number: None,
    }
}

fn cue_list(layers: [FixtureId; 2]) -> Value {
    let changes = vec![CueChange::set(
        layers[1],
        key_of(ProgrammingOwner::Color),
        color(warm_white_3200()),
    )];
    let group_changes = vec![GroupCueChange {
        preset_reference: None,
        group_id: GROUP.into(),
        attribute: key_of(ProgrammingOwner::Color),
        value: Some(group_color(layers[0].0)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    }];
    json!({
        "id": CUE_LIST_ID, "name": "Media", "priority": 10,
        "mode": "sequence", "looped": false,
        "cues": [{
            "id": Uuid::from_u128(0x560c1), "number": "1", "name": "Media",
            "changes": changes, "group_changes": group_changes,
            "fade_millis": 0, "delay_millis": 0, "trigger": {"type": "manual"}
        }]
    })
}

#[test]
fn media_layer_color_survives_selective_import_with_duplicated_layer_identities() {
    let rig = TestRig::new();
    let profile = shipped_media_server();
    let root = FixtureId(Uuid::from_u128(0x560_e000));
    let layers = [
        FixtureId(Uuid::from_u128(0x560_e001)),
        FixtureId(Uuid::from_u128(0x560_e002)),
    ];
    rig.source_profile(
        &FixtureProfileRevision::from_profile(serde_json::to_value(&profile).unwrap()).unwrap(),
    );
    let body =
        PortablePatchedFixtureRecord::from_runtime_fixture(&media_server(&profile, root, layers))
            .unwrap()
            .into_body();
    rig.source_object("patched_fixture", &root.0.to_string(), body);
    rig.source_object(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "Layers".into(),
            fixtures: layers.to_vec(),
            ..Default::default()
        })
        .unwrap(),
    );
    rig.source_object(
        "preset",
        "2.1",
        serde_json::to_value(preset(layers)).unwrap(),
    );
    rig.source_object("cue_list", CUE_LIST_ID, cue_list(layers));

    let preview = rig.preview(
        SelectiveShowImportRequest::new(
            rig.source_id,
            rig.target_id,
            [key("cue_list", CUE_LIST_ID), key("preset", "2.1")],
        )
        .with_mode(ImportLoadMode::AddToEnd),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    rig.apply(&preview).unwrap();
    let destination = |kind: &str, source: &str| {
        preview
            .objects
            .iter()
            .find(|entry| entry.source == key(kind, source))
            .unwrap_or_else(|| panic!("{kind}/{source} is not planned"))
            .destination
            .id()
            .to_owned()
    };
    let document = rig.target_document();

    // The duplicated Media Server and its layer heads.
    let new_root = destination("patched_fixture", &root.0.to_string());
    assert_ne!(new_root, root.0.to_string(), "the root identity changes");
    let fixture = document
        .object("patched_fixture", &new_root)
        .unwrap()
        .body();
    let new_layers: Vec<Uuid> = fixture["logical_heads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|head| Uuid::parse_str(head["fixture_id"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(new_layers.len(), 2);
    for (old, new) in layers.iter().zip(&new_layers) {
        assert_ne!(old.0, *new, "every layer head identity changes");
    }
    let ids = BTreeMap::from([(layers[0].0, new_layers[0]), (layers[1].0, new_layers[1])]);
    let group = destination("group", GROUP);
    let imported_group: GroupDefinition =
        serde_json::from_value(document.object("group", &group).unwrap().body().clone()).unwrap();
    assert_eq!(
        imported_group.fixtures,
        new_layers
            .iter()
            .copied()
            .map(FixtureId)
            .collect::<Vec<_>>(),
        "the live Group holds the duplicated layers in order"
    );

    // The Preset: exact intent rebound to the duplicated layer and Group.
    let preset_id = destination("preset", "2.1");
    let preset_body = document.object("preset", &preset_id).unwrap().body();
    let decoded = ActiveShowObjectBody::decode(ActiveShowObjectKind::Preset, preset_body.clone())
        .unwrap()
        .preset()
        .expect("typed Preset")
        .typed()
        .clone();
    assert_eq!(
        decoded.values,
        HashMap::from([(
            FixtureId(new_layers[1]),
            HashMap::from([(key_of(ProgrammingOwner::Color), color(magenta_with_uv()))]),
        )])
    );
    assert_eq!(
        decoded.group_values,
        HashMap::from([(
            group.clone(),
            HashMap::from([(
                key_of(ProgrammingOwner::Color),
                remap(&group_color(layers[0].0), &ids)
            )]),
        )])
    );
    assert_semantic_attribute_map(
        &preset_body["values"][new_layers[1].to_string()],
        &["color"],
    );
    assert_semantic_attribute_map(&preset_body["group_values"][&group], &["color"]);

    // The Cue list: the per-layer change and the Group change, rebound and exact.
    let list_id = destination("cue_list", CUE_LIST_ID);
    let list: CueList = serde_json::from_value(
        document
            .object("cue_list", &list_id)
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    let cue = &list.cues[0];
    assert_eq!(cue.changes.len(), 1);
    assert_eq!(cue.changes[0].fixture_id, FixtureId(new_layers[1]));
    assert_eq!(cue.changes[0].value, Some(color(warm_white_3200())));
    assert_eq!(cue.group_changes.len(), 1);
    assert_eq!(cue.group_changes[0].group_id, group);
    assert_eq!(
        cue.group_changes[0].value,
        Some(remap(&group_color(layers[0].0), &ids))
    );

    // No stored reference to a source identity survives anywhere in the target.
    let text =
        serde_json::to_string(&[preset_body.clone(), json!(list), json!(imported_group)]).unwrap();
    for old in [root.0, layers[0].0, layers[1].0] {
        assert!(!text.contains(&old.to_string()), "{old} leaked: {text}");
    }

    // The target compiles with the duplicated layers as heads of the duplicated root.
    let (_, snapshot) = crate::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    let compiled = snapshot
        .fixtures
        .iter()
        .find(|f| f.fixture_id.0.to_string() == new_root)
        .expect("the duplicated Media Server");
    assert_eq!(
        compiled
            .logical_heads
            .iter()
            .map(|head| head.fixture_id.0)
            .collect::<Vec<_>>(),
        new_layers
    );
    assert!(
        snapshot.required_programming_contract >= 1,
        "semantic Media colour requires contract 1"
    );
}
