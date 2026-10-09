use super::super::replacement_projection::validate;
use super::support::document_with_objects;
use light_core::{FixtureId, ReplacementProgramProjection};
use light_core::{ReplacementHeadTarget, ReplacementProfileContext};
use serde_json::json;

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
