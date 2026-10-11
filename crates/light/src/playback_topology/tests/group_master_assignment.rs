use super::*;
use light_programmer::GroupDefinition;

fn group_action(
    id: &str,
    group_revision: u64,
    page: u8,
    position: u16,
    revision: u64,
    page_id: Option<&str>,
) -> PlaybackTopologyAction {
    PlaybackTopologyAction::AssignGroupMaster {
        group_object_id: id.into(),
        expected_group_revision: group_revision,
        address: GroupMasterPlaybackAddress {
            address: address(page, position),
            expected_page_revision: revision,
            expected_page_object_id: page_id.map(str::to_owned),
        },
    }
}

#[test]
fn group_assignment_uses_storage_identity_and_group_presentation_for_a_new_assignment() {
    let rig = TestRig::new();
    let raw = serde_json::to_value(group(
        "non-authoritative-body-id",
        "Front",
        "#123456",
        "front-icon",
    ))
    .unwrap();
    rig.seed("group", "front", &raw);
    let result = rig
        .handle(
            "assign",
            rig.show_revision(),
            group_action("front", 1, 1, 401, 0, None),
        )
        .unwrap();
    let stored = assigned(&rig, 1, 401);
    assert_eq!(
        stored.target,
        PlaybackTarget::Group {
            group_id: "front".into(),
            initial_master: None
        }
    );
    assert_eq!(stored.name, "Front");
    assert_eq!(stored.color, "#123456");
    assert_eq!(stored.presentation_icon.as_deref(), Some("front-icon"));
    assert_eq!(
        rig.document().object("group", "front").unwrap().body(),
        &raw
    );
    assert_eq!(result.outcome.objects().len(), 1);
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);
}

#[test]
fn group_retarget_preserves_assignment_local_presentation_and_controls() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "old",
        &serde_json::to_value(group("old", "Old", "#112233", "old-icon")).unwrap(),
    );
    rig.seed(
        "group",
        "new",
        &serde_json::to_value(group("new", "New", "#abcdef", "new-icon")).unwrap(),
    );
    let mut existing = playback(0, "Local label");
    existing.target = PlaybackTarget::Group {
        group_id: "old".into(),
        initial_master: Some(0.0),
    };
    existing.color = "#fedcba".into();
    existing.presentation_icon = Some("local-icon".into());
    existing.protect_from_swap = true;
    existing.buttons = PlaybackDefinition::default_buttons(&existing.target);
    existing.fader = PlaybackDefinition::default_fader(&existing.target);
    rig.seed(
        "playback_page",
        "two",
        &json!({"number":2,"name":"Two","assignments":{"3":existing}}),
    );
    rig.handle(
        "retarget",
        rig.show_revision(),
        group_action("new", 1, 2, 3, 1, Some("two")),
    )
    .unwrap();
    let stored = assigned(&rig, 2, 3);
    assert_eq!(stored.name, existing.name);
    assert_eq!(stored.color, existing.color);
    assert_eq!(stored.presentation_icon, existing.presentation_icon);
    assert_eq!(stored.buttons, existing.buttons);
    assert_eq!(stored.fader, existing.fader);
    assert!(stored.protect_from_swap);
    assert_eq!(
        stored.target,
        PlaybackTarget::Group {
            group_id: "new".into(),
            initial_master: None
        }
    );
}

#[test]
fn same_group_reassignment_preserves_zero_seed_and_is_no_change() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "front",
        &serde_json::to_value(group("front", "Front", "#123456", "front-icon")).unwrap(),
    );
    let mut existing = playback(0, "Front");
    existing.target = PlaybackTarget::Group {
        group_id: "front".into(),
        initial_master: Some(0.0),
    };
    existing.buttons = PlaybackDefinition::default_buttons(&existing.target);
    existing.fader = PlaybackDefinition::default_fader(&existing.target);
    rig.seed(
        "playback_page",
        "one",
        &json!({"number":1,"name":"One","assignments":{"1000":existing}}),
    );
    rig.handle(
        "same",
        rig.show_revision(),
        group_action("front", 1, 1, 1000, 1, Some("one")),
    )
    .unwrap();
    assert_eq!(assigned(&rig, 1, 1000).target, existing.target);
}

#[test]
fn missing_or_stale_group_and_stale_page_fail_before_mutation() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "front",
        &serde_json::to_value(group("front", "Front", "#123456", "front-icon")).unwrap(),
    );
    rig.seed(
        "playback_page",
        "one",
        &json!({"number":1,"name":"One","assignments":{}}),
    );
    let before = rig.show_revision();
    for (id, group_revision, page_revision, kind) in [
        ("missing", 1, 1, ActionErrorKind::NotFound),
        ("front", 0, 1, ActionErrorKind::Conflict),
        ("front", 1, 0, ActionErrorKind::Conflict),
    ] {
        let error = rig
            .handle(
                &format!("{id}-{group_revision}-{page_revision}"),
                before,
                group_action(id, group_revision, 1, 1, page_revision, Some("one")),
            )
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(rig.show_revision(), before);
    }
    assert_one_event(&rig, 0);
}

#[test]
fn group_replay_fingerprint_captures_identity_revision_and_address() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "front",
        &serde_json::to_value(group("front", "Front", "#123456", "front-icon")).unwrap(),
    );
    let before = rig.show_revision();
    let action = group_action("front", 1, 99, 1000, 0, None);
    rig.handle("same", before, action.clone()).unwrap();
    assert!(rig.handle("same", before, action).unwrap().replayed);
    for changed in [
        group_action("other", 1, 99, 1000, 0, None),
        group_action("front", 2, 99, 1000, 0, None),
        group_action("front", 1, 98, 1000, 0, None),
        group_action("front", 1, 99, 999, 0, None),
    ] {
        assert_eq!(
            rig.handle("same", before, changed).unwrap_err().kind,
            ActionErrorKind::Conflict
        );
    }
    assert_one_event(&rig, 1);
}

fn group(id: &str, name: &str, color: &str, icon: &str) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: name.into(),
        color: Some(color.into()),
        icon: Some(icon.into()),
        ..GroupDefinition::default()
    }
}
