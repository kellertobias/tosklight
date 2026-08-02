use super::*;
use light_programmer::GroupDefinition;

#[test]
fn group_assignment_resolves_group_and_constructs_new_playback_atomically() {
    let rig = TestRig::new();
    let group = group("body-id-is-not-authority", "Front", "#123456", "front-icon");
    let raw_group = serde_json::to_value(&group).unwrap();
    rig.seed("group", "front", &raw_group);
    rig.seed(
        "playback_page",
        "page-one",
        &json!({"number":1,"name":"Main","slots":{},"virtual_playbacks":{}}),
    );

    let result = rig
        .handle(
            "assign-front",
            rig.show_revision(),
            assign_group_action("front", 1, 1, 4, 1, Some("page-one"), 0, None),
        )
        .unwrap();

    assert_eq!(
        result.outcome.resolution(),
        PlaybackTopologyResolution::PageSlot {
            page: 1,
            slot: 4,
            playback_number: Some(1),
        }
    );
    assert_eq!(result.outcome.objects().len(), 2);
    let document = rig.document();
    let playback: PlaybackDefinition =
        serde_json::from_value(document.object("playback", "1").unwrap().body().clone()).unwrap();
    assert_eq!(
        playback.target,
        PlaybackTarget::Group {
            group_id: "front".into()
        }
    );
    assert_eq!(playback.name, "Front");
    assert_eq!(playback.color, "#123456");
    assert_eq!(playback.presentation_icon.as_deref(), Some("front-icon"));
    assert_eq!(
        document.object("group", "front").unwrap().body(),
        &raw_group
    );
    assert_eq!(document.object("group", "front").unwrap().revision(), 1);
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 2);
}

#[test]
fn group_assignment_preserves_existing_assignment_local_presentation_and_layout() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "new-group",
        &serde_json::to_value(group("new-group", "New Group", "#abcdef", "new-icon")).unwrap(),
    );
    let mut existing = playback(7, "Local label");
    existing.target = PlaybackTarget::Group {
        group_id: "old-group".into(),
    };
    existing.buttons = [
        light_playback::PlaybackButtonAction::Flash,
        light_playback::PlaybackButtonAction::Select,
        light_playback::PlaybackButtonAction::None,
    ];
    existing.button_count = 2;
    existing.color = "#fedcba".into();
    existing.presentation_icon = Some("local-icon".into());
    existing.protect_from_swap = true;
    rig.seed(
        "playback",
        "legacy-seven",
        &serde_json::to_value(&existing).unwrap(),
    );
    rig.seed(
        "playback_page",
        "page-two",
        &json!({"number":2,"name":"Two","slots":{"3":7},"virtual_playbacks":{}}),
    );

    let result = rig
        .handle(
            "retarget-local",
            rig.show_revision(),
            assign_group_action(
                "new-group",
                1,
                2,
                3,
                1,
                Some("page-two"),
                1,
                Some("legacy-seven"),
            ),
        )
        .unwrap();

    let stored: PlaybackDefinition = serde_json::from_value(
        rig.document()
            .object("playback", "legacy-seven")
            .unwrap()
            .body()
            .clone(),
    )
    .unwrap();
    assert_eq!(stored.name, existing.name);
    assert_eq!(stored.color, existing.color);
    assert_eq!(stored.presentation_icon, existing.presentation_icon);
    assert_eq!(stored.buttons, existing.buttons);
    assert_eq!(stored.button_count, existing.button_count);
    assert_eq!(stored.fader, existing.fader);
    assert!(stored.protect_from_swap);
    assert_eq!(
        stored.target,
        PlaybackTarget::Group {
            group_id: "new-group".into()
        }
    );
    assert_eq!(result.outcome.objects().len(), 2);
    assert_one_event(&rig, 1);
}

#[test]
fn stale_or_missing_group_authority_rejects_without_mutation() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "front",
        &serde_json::to_value(group("front", "Front", "#123456", "front-icon")).unwrap(),
    );
    let before = rig.show_revision();

    let stale = rig
        .handle(
            "stale-group",
            before,
            assign_group_action("front", 2, 1, 1, 0, None, 0, None),
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_related_revision, Some(1));
    assert_eq!(rig.show_revision(), before);
    assert!(rig.document().objects_of_kind("playback").next().is_none());

    rig.clear_steps();
    let missing = rig
        .handle(
            "missing-group",
            before,
            assign_group_action("missing", 0, 1, 1, 0, None, 0, None),
        )
        .unwrap_err();
    assert_eq!(missing.kind, ActionErrorKind::NotFound);
    assert_eq!(rig.show_revision(), before);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn group_assignment_requires_exact_page_and_playback_authority() {
    let rig = TestRig::new();
    rig.seed(
        "group",
        "front",
        &serde_json::to_value(group("front", "Front", "#123456", "front-icon")).unwrap(),
    );
    rig.seed(
        "playback",
        "legacy-seven",
        &serde_json::to_value(playback(7, "Local")).unwrap(),
    );
    rig.seed(
        "playback_page",
        "page-one",
        &json!({"number":1,"name":"Main","slots":{"2":7},"virtual_playbacks":{}}),
    );
    let before = rig.show_revision();

    let stale_page = rig
        .handle(
            "stale-page",
            before,
            assign_group_action(
                "front",
                1,
                1,
                2,
                1,
                Some("replacement-page"),
                1,
                Some("legacy-seven"),
            ),
        )
        .unwrap_err();
    assert_eq!(stale_page.kind, ActionErrorKind::Conflict);

    rig.clear_steps();
    let stale_playback = rig
        .handle(
            "stale-playback",
            before,
            assign_group_action(
                "front",
                1,
                1,
                2,
                1,
                Some("page-one"),
                1,
                Some("replacement-seven"),
            ),
        )
        .unwrap_err();
    assert_eq!(stale_playback.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.show_revision(), before);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn group_assignment_replay_fingerprint_includes_group_identity_and_revision() {
    let rig = TestRig::new();
    for id in ["front", "back"] {
        rig.seed(
            "group",
            id,
            &serde_json::to_value(group(id, id, "#123456", "icon")).unwrap(),
        );
    }
    let show_revision = rig.show_revision();
    let action = assign_group_action("front", 1, 1, 1, 0, None, 0, None);
    let first = rig
        .handle("same-request", show_revision, action.clone())
        .unwrap();
    let replayed = rig.handle("same-request", show_revision, action).unwrap();
    assert_eq!(replayed.outcome, first.outcome);
    assert!(replayed.replayed);

    let conflict = rig
        .handle(
            "same-request",
            show_revision,
            assign_group_action("back", 1, 1, 1, 0, None, 0, None),
        )
        .unwrap_err();
    assert_eq!(conflict.kind, ActionErrorKind::Conflict);
}

fn assign_group_action(
    group_object_id: &str,
    expected_group_revision: u64,
    page: u8,
    slot: u8,
    expected_page_revision: u64,
    expected_page_object_id: Option<&str>,
    expected_playback_revision: u64,
    expected_playback_object_id: Option<&str>,
) -> PlaybackTopologyAction {
    PlaybackTopologyAction::AssignGroupMaster {
        group_object_id: group_object_id.into(),
        expected_group_revision,
        page,
        slot,
        expected_page_revision,
        expected_page_object_id: expected_page_object_id.map(str::to_owned),
        expected_playback_revision,
        expected_playback_object_id: expected_playback_object_id.map(str::to_owned),
    }
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
