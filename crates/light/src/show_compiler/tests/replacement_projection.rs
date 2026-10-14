use super::super::replacement_projection::validate;
use super::support::document_with_objects;
use light_core::{FixtureId, ReplacementProgramProjection};
use light_core::{ReplacementHeadTarget, ReplacementProfileContext};
use serde_json::json;

#[test]
fn universal_color_group_cue_keeps_captured_member_projection_and_dormancy() {
    use light_core::programming::{ColorIntent, ColorProgram};
    use light_core::{AttributeKey, AttributeValue, PresetValueOwner, PresetValueReference};
    use light_playback::{Cue, CueList, GroupCueChange};
    use light_programmer::Preset;
    use std::{collections::HashMap, sync::Arc};
    let root = FixtureId::new();
    let child = FixtureId::new();
    let context = ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: uuid::Uuid::new_v4(),
    };
    let projection = ReplacementProgramProjection {
        source_owner: root,
        source_profile: context.clone(),
        source_head_id: uuid::Uuid::new_v4(),
        target_profile: context,
        targets: vec![ReplacementHeadTarget {
            fixture_id: child,
            profile_head_id: uuid::Uuid::new_v4(),
        }],
    };
    let instance = uuid::Uuid::new_v4();
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let preset = Preset {
        instance_id: Some(instance),
        universal_values: HashMap::from([(AttributeKey::color(), value.clone())]),
        ..Default::default()
    };
    for dormant in [false, true] {
        let mut captured = projection.clone();
        if dormant {
            captured.targets.clear();
        }
        let mut cue = Cue::new(1_u16.into());
        cue.group_changes.push(GroupCueChange {
            group_id: "1".into(),
            attribute: AttributeKey::color(),
            value: Some(value.clone()),
            replacement_projections: HashMap::from([(root, captured.clone())]),
            preset_reference: Some(PresetValueReference {
                preset_instance_id: instance,
                source_owner: PresetValueOwner::Universal,
                source_attribute: AttributeKey::color(),
                sample_rank: None,
                member_fixture: None,
            }),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        });
        let list: CueList = serde_json::from_value(json!({
            "id": uuid::Uuid::new_v4(), "name": "Universal linked Group", "priority": 0,
            "mode": "sequence", "looped": false, "cues": [cue],
        }))
        .unwrap();
        let key = list.id.0.to_string();
        let (_store, document) = document_with_objects(&[
            ("preset", "2.1", serde_json::to_value(&preset).unwrap()),
            ("cue_list", &key, serde_json::to_value(&list).unwrap()),
        ]);
        let transaction = document.transaction();
        let decoded =
            super::super::objects::decode_cue_lists(document.candidate(&transaction).unwrap())
                .unwrap();
        assert_eq!(
            decoded[0].cues[0].group_changes[0].value,
            Some(value.clone())
        );
        assert_eq!(
            decoded[0].cues[0].group_changes[0].replacement_projections,
            HashMap::from([(root, captured)]),
            "universal live value must retain address correspondence, including explicit dormancy"
        );
        assert_eq!(
            document.object("cue_list", &key).unwrap().body(),
            &serde_json::to_value(&list).unwrap()
        );
    }
}

#[test]
fn replacement_portable_source_rejects_effective_child_and_foreign_member_addresses() {
    let root = FixtureId::new();
    let child = FixtureId::new();
    let context = ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: uuid::Uuid::new_v4(),
    };
    let mut projection = ReplacementProgramProjection {
        source_owner: root,
        source_profile: context.clone(),
        source_head_id: uuid::Uuid::new_v4(),
        target_profile: context,
        targets: vec![ReplacementHeadTarget {
            profile_head_id: uuid::Uuid::new_v4(),
            fixture_id: child,
        }],
    };
    let (_store, document) = document_with_objects(&[]);
    let mut transaction = document.transaction();
    transaction.put(
        "cue_list",
        "1",
        json!({"cues":[{"changes":[{"fixture_id":child,"replacement_projection":projection}]}]}),
    );
    assert!(validate(document.candidate(&transaction).unwrap()).is_err());
    transaction.put(
        "cue_list",
        "1",
        json!({"cues":[{"changes":[{"fixture_id":root,"replacement_projection":projection}]}]}),
    );
    assert!(validate(document.candidate(&transaction).unwrap()).is_ok());
    projection.targets.clear();
    transaction.put(
        "cue_list",
        "1",
        json!({"cues":[{"changes":[{"fixture_id":root,"replacement_projection":projection}]}]}),
    );
    assert!(validate(document.candidate(&transaction).unwrap()).is_ok());
    transaction.put(
        "group",
        "1",
        json!({"replacement_projections":{"intensity":{child.0.to_string():projection}}}),
    );
    assert!(validate(document.candidate(&transaction).unwrap()).is_err());
}
