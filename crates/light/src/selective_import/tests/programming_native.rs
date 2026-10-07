use super::support::*;
use crate::selective_import::*;
use light_core::{AttributeKey, FixtureId, NativeColorIdentity, programming::ColorIntent};
use light_fixture::{
    ChannelFunction, ColorPhysicalModel, FixtureProfile, HeadOpticalPath, OpticalSource,
};
use light_show::FixtureProfileRevision;
use serde_json::{Value, json};
use uuid::Uuid;

const CUE_LIST_ID: &str = "00000000-0000-0000-0000-0000000d9877";

pub(super) struct NativeFixture {
    pub(super) revision: FixtureProfileRevision,
    pub(super) identity: NativeColorIdentity,
    pub(super) fixture_id: FixtureId,
    pub(super) fixture_body: Value,
    pub(super) channels: Value,
}

pub(super) fn native_fixture() -> NativeFixture {
    let record = portable_fixture_record(890_000, 801);
    let mut profile: FixtureProfile =
        serde_json::from_value(record.profile.profile().clone()).unwrap();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    mode.channels = ["color.wheel.1", "color.uv"]
        .map(|attribute| {
            serde_json::from_value(json!({
                "id": Uuid::new_v4(), "head_id": head, "split": 1,
                "fixture_attribute": attribute, "attribute": attribute,
                "resolution": if attribute == "color.uv" { "u32" } else { "u8" },
                "secondary_slots": if attribute == "color.uv" { vec![3, 4, 5] } else { vec![] },
                "default_raw": 0,
                "highlight_raw": if attribute == "color.uv" { u32::MAX } else { 255 },
                "functions": [ChannelFunction::continuous(
                    attribute, AttributeKey(attribute.into()),
                    if attribute == "color.uv" { u32::MAX } else { 255 }
                )]
            }))
            .unwrap()
        })
        .into();
    mode.splits[0].footprint = 5;
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id: head,
            controls: mode.channels.iter().map(|channel| channel.id).collect(),
            source: OpticalSource::Unknown,
            filters: vec![],
            measurements: vec![],
        }],
    });
    let mode_id = mode.id;
    let identity = profile.native_color_identity(mode_id, head).unwrap();
    let channels = json!(profile.modes[0]
        .channels
        .iter()
        .map(|channel| json!({
            "channel_id": channel.id,
            "function_id": channel.functions[0].id,
            "raw": if channel.fixture_attribute.0.as_ref() == "color.uv" { u32::MAX - 1 } else { 132 }
        }))
        .collect::<Vec<_>>());
    NativeFixture {
        revision: FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
            .unwrap(),
        identity,
        fixture_id: record.fixture_id,
        fixture_body: record.body,
        channels,
    }
}

fn direct_color(source: &NativeColorIdentity, channels: &Value) -> Value {
    json!({
        "kind": "color_program",
        "value": {
            "kind": "direct",
            "recipe": {"source": source, "channels": channels, "spreads": []},
            "portable": {
                "model_revision": 1, "visible": null,
                "uv": {"amount": 0.7, "quality": "estimated"},
                "quality": "estimated", "limitations": ["UV response unknown"]
            }
        }
    })
}

fn semantic_wheel(source: &NativeColorIdentity, channel: &Value) -> Value {
    let mut intent = serde_json::to_value(ColorIntent::default()).unwrap();
    intent["wheel_constraints"] = json!([{"source": source, "value": channel}]);
    json!({"kind":"color_program", "value":{"kind":"semantic", "intent":intent}})
}

fn fix_at_cue(id: FixtureId, source: &NativeColorIdentity, channels: &Value) -> Value {
    json!({
        "id": CUE_LIST_ID, "name": "Native", "priority": 10,
        "mode": "sequence", "looped": false,
        "cues": [{
            "id": Uuid::new_v4(), "number": "1", "name": "Direct",
            "changes": [], "fade_millis": 0, "delay_millis": 0,
            "trigger": {"type":"manual"},
            "dynamic_changes": [{
                "fixture_id": id, "attribute": "color",
                "value": {
                    "type": "programming_fix_at",
                    "mask": {
                        "address": {
                            "representation": {"kind":"direct_color", "source":source},
                            "component": null
                        },
                        "family": direct_color(source, channels)
                    },
                    "timing": {}
                }
            }]
        }],
        "future": {"profile_id":"leave-extension-alone"}
    })
}

pub(super) fn profile_key(source: &NativeFixture) -> ImportProfileKey {
    ImportProfileKey {
        profile_id: source.revision.id().profile_id(),
        revision: source.revision.id().revision(),
    }
}

pub(super) fn conflicting_revision(source: &NativeFixture) -> FixtureProfileRevision {
    let mut body = source.revision.profile().clone();
    body["manufacturer"] = json!("Different optical model");
    FixtureProfileRevision::from_profile(body).unwrap()
}

#[test]
fn duplicate_rebinds_fix_at_recipe_wheel_and_group_member_to_complete_native_identity() {
    let rig = TestRig::new();
    let source = native_fixture();
    rig.source_profile(&source.revision);
    rig.target_profile(&conflicting_revision(&source));
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.fixture_body.clone(),
    );
    rig.source_object(
        "cue_list",
        CUE_LIST_ID,
        fix_at_cue(source.fixture_id, &source.identity, &source.channels),
    );
    rig.source_object(
        "group",
        "native-group",
        serde_json::to_value(light_programmer::GroupDefinition {
            id: "native-group".into(),
            name: "Native group".into(),
            fixtures: vec![source.fixture_id],
            ..Default::default()
        })
        .unwrap(),
    );
    let semantic = semantic_wheel(&source.identity, &source.channels[0]);
    let direct = direct_color(&source.identity, &source.channels);
    let mut members = serde_json::Map::new();
    members.insert(source.fixture_id.0.to_string(), direct);
    rig.source_object(
        "preset",
        "2.7",
        json!({
            "name":"Native color", "family":"Color", "number":7,
            "values":{},
            "universal_values":{"color":semantic.clone()},
            "group_values":{"native-group":{"color":{
                "kind":"group_family", "value":{
                    "owner":"color", "template":semantic,
                    "members":members
                }
            }}},
            "future":{"profile_id":"leave-extension-alone"}
        }),
    );
    let preview = rig.preview(
        SelectiveShowImportRequest::new(
            rig.source_id,
            rig.target_id,
            [key("cue_list", CUE_LIST_ID), key("preset", "2.7")],
        )
        .resolve_profile(
            profile_key(&source),
            ImportProfileConflictResolution::Duplicate,
        ),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    let destination = preview
        .profiles
        .iter()
        .find(|p| p.source == profile_key(&source))
        .unwrap()
        .destination;
    assert_ne!(destination.profile_id, profile_key(&source).profile_id);
    rig.apply(&preview).unwrap();

    let target = rig.target_document();
    let duplicate = target
        .fixture_profile_revision(destination.profile_id, destination.revision)
        .unwrap();
    let profile: FixtureProfile = serde_json::from_value(duplicate.profile().clone()).unwrap();
    let expected = serde_json::to_value(
        profile
            .native_color_identity(source.identity.mode_id, source.identity.head_id)
            .unwrap(),
    )
    .unwrap();
    assert_ne!(
        expected["profile_digest"],
        json!(source.identity.profile_digest)
    );
    let cue = target.object("cue_list", CUE_LIST_ID).unwrap().body();
    let mask = cue.pointer("/cues/0/dynamic_changes/0/value/mask").unwrap();
    assert_eq!(
        mask.pointer("/address/representation/source").unwrap(),
        &expected
    );
    assert_eq!(
        mask.pointer("/family/value/recipe/source").unwrap(),
        &expected
    );
    assert_eq!(
        mask.pointer("/family/value/recipe/channels").unwrap(),
        &source.channels
    );
    assert_eq!(
        mask.pointer("/family/value/portable/uv/amount").unwrap(),
        &json!(0.7)
    );
    assert_eq!(cue["future"]["profile_id"], "leave-extension-alone");
    let preset = target.object("preset", "2.7").unwrap().body();
    assert_eq!(
        preset
            .pointer("/universal_values/color/value/intent/wheel_constraints/0/source")
            .unwrap(),
        &expected
    );
    assert_eq!(preset.pointer("/group_values/native-group/color/value/template/value/intent/wheel_constraints/0/source").unwrap(), &expected);
    let member = preset
        .pointer(&format!(
            "/group_values/native-group/color/value/members/{}/value",
            source.fixture_id.0
        ))
        .unwrap();
    assert_eq!(&member["recipe"]["source"], &expected);
    assert_eq!(&member["recipe"]["channels"], &source.channels);
    assert_eq!(member["portable"]["uv"]["amount"], 0.7);
    assert_eq!(preset["future"]["profile_id"], "leave-extension-alone");
}

#[test]
fn incompatible_keep_destination_and_forged_native_digest_block_preview_and_apply() {
    let source = native_fixture();
    let rig = TestRig::new();
    rig.source_profile(&source.revision);
    rig.target_profile(&conflicting_revision(&source));
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.fixture_body.clone(),
    );
    rig.source_object(
        "cue_list",
        CUE_LIST_ID,
        fix_at_cue(source.fixture_id, &source.identity, &source.channels),
    );
    let keep = rig.preview(rig.request("cue_list", CUE_LIST_ID).resolve_profile(
        profile_key(&source),
        ImportProfileConflictResolution::KeepDestination,
    ));
    assert!(!keep.can_apply(), "incompatible native model was accepted");
    assert!(keep.blockers.contains(&ImportBlocker::ReferenceRewrite {
        owner: key("cue_list", CUE_LIST_ID),
        message: "destination profile changes the pinned native Color model; duplicate the source profile to preserve its recipe".into(),
    }), "{:?}", keep.blockers);
    assert!(rig.apply(&keep).is_err());
    assert!(
        rig.target_document()
            .object("cue_list", CUE_LIST_ID)
            .is_none()
    );

    let mut forged = fix_at_cue(source.fixture_id, &source.identity, &source.channels);
    forged["cues"][0]["dynamic_changes"][0]["value"]["mask"]["family"]["value"]["recipe"]["source"]
        ["profile_digest"] = json!("forged-source-digest");
    rig.update_source_object("cue_list", CUE_LIST_ID, forged);
    let preview = rig.preview(rig.request("cue_list", CUE_LIST_ID).resolve_profile(
        profile_key(&source),
        ImportProfileConflictResolution::Duplicate,
    ));
    assert!(!preview.can_apply(), "forged immutable source was accepted");
    assert!(
        preview.blockers.contains(&ImportBlocker::ReferenceRewrite {
            owner: key("cue_list", CUE_LIST_ID),
            message: "pinned native Color source does not match its original immutable profile"
                .into(),
        }),
        "{:?}",
        preview.blockers
    );
    assert!(rig.apply(&preview).is_err());
    assert!(
        rig.target_document()
            .object("cue_list", CUE_LIST_ID)
            .is_none()
    );
}
