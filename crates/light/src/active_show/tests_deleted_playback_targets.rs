use super::*;
use light_playback::{PlaybackDefinition, PlaybackTarget};

fn assigned_targets() -> Vec<(
    ActiveShowObjectKind,
    &'static str,
    String,
    Value,
    PlaybackTarget,
)> {
    let id = Uuid::from_u128(0x714de1);
    vec![
        (
            ActiveShowObjectKind::Group,
            "group",
            "front".into(),
            serde_json::to_value(light_programmer::GroupDefinition {
                id: "front".into(),
                name: "Front".into(),
                ..Default::default()
            })
            .unwrap(),
            PlaybackTarget::Group {
                group_id: "front".into(),
                initial_master: None,
            },
        ),
        (
            ActiveShowObjectKind::CueList,
            "cue_list",
            id.to_string(),
            cue_list_body(light_core::CueListId(id), "Main"),
            PlaybackTarget::CueList {
                cue_list_id: light_core::CueListId(id),
            },
        ),
        (
            ActiveShowObjectKind::Macro,
            "macro",
            id.to_string(),
            macro_body(id, 1, "Main", "GROUP 1"),
            PlaybackTarget::Macro { macro_id: id },
        ),
        (
            ActiveShowObjectKind::Timecode,
            "timecode",
            id.to_string(),
            timecode_body(id, 1, "Main"),
            PlaybackTarget::Timecode {
                timecode_id: light_playback::TimecodeId(id),
            },
        ),
    ]
}

fn seed_assignment(rig: &TestRig, target: PlaybackTarget) {
    let definition = PlaybackDefinition {
        name: "Assigned".into(),
        buttons: PlaybackDefinition::default_buttons(&target),
        has_fader: !matches!(
            target,
            PlaybackTarget::Macro { .. } | PlaybackTarget::Timecode { .. }
        ),
        target,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: Default::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    rig.seed_object(
        "playback_page",
        "1",
        json!({"number":1,"name":"Main","assignments":{"2":definition}}),
    );
    let document = rig.document();
    let (_, snapshot) = crate::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    *rig.ports.installed.lock() = Some(snapshot);
}

fn deletion(kind: ActiveShowObjectKind, id: String) -> ActiveShowObjectMutation {
    ActiveShowObjectMutation {
        kind,
        object_id: id,
        expected_object_revision: 1,
        mutation: ActiveShowObjectMutationKind::Delete,
    }
}

#[test]
fn assigned_target_deletion_is_rejected_before_persistence_runtime_or_events() {
    for (kind, stored_kind, id, body, target) in assigned_targets() {
        let rig = TestRig::new();
        rig.seed_object(stored_kind, &id, body.clone());
        seed_assignment(&rig, target);
        let original = fs::read(&rig.ports.path).unwrap();
        let revision = rig.installed_revision();
        let pages = rig.installed_snapshot().unwrap().playback_pages;
        let error = rig
            .service
            .mutate_objects(
                rig.object_action(vec![deletion(kind, id.clone())]),
                &rig.ports,
            )
            .expect_err(
                "deletion must not leave a configured Playback pointing at an absent target",
            );
        assert_eq!(error.kind, ActionErrorKind::Conflict);
        assert!(error.message.contains("1.2"), "{}", error.message);
        assert_eq!(rig.object_body(stored_kind, &id), body);
        assert_eq!(fs::read(&rig.ports.path).unwrap(), original);
        assert_eq!(rig.installed_revision(), revision);
        assert_eq!(rig.installed_snapshot().unwrap().playback_pages, pages);
        assert_eq!(rig.steps(), ["begin"]);
        assert_eq!(rig.service.events().latest_sequence(), 0);
    }
}

#[test]
fn clearing_assignment_and_deleting_target_in_same_transaction_is_allowed() {
    for (kind, stored_kind, id, body, target) in assigned_targets() {
        let rig = TestRig::new();
        rig.seed_object(stored_kind, &id, body);
        seed_assignment(&rig, target);
        let clear = ActiveShowObjectMutation {
            kind: ActiveShowObjectKind::PlaybackPage,
            object_id: "1".into(),
            expected_object_revision: 1,
            mutation: ActiveShowObjectMutationKind::Put {
                body: Box::new(typed(
                    ActiveShowObjectKind::PlaybackPage,
                    json!({"number":1,"name":"Main","assignments":{}}),
                )),
            },
        };
        rig.service
            .mutate_objects(
                rig.object_action(vec![deletion(kind, id.clone()), clear]),
                &rig.ports,
            )
            .unwrap();
        assert!(rig.document().object(stored_kind, &id).is_none());
        assert_eq!(
            rig.object_body("playback_page", "1")["assignments"],
            json!({})
        );
        assert_eq!(
            rig.steps().iter().filter(|step| **step == "commit").count(),
            1
        );
    }
}

#[test]
fn every_preparation_lane_rejects_deleted_assigned_targets() {
    for (_, stored_kind, id, body, target) in assigned_targets() {
        let rig = TestRig::new();
        rig.seed_object(stored_kind, &id, body);
        seed_assignment(&rig, target);
        let document = rig.document();
        let mut transaction = document.transaction();
        transaction.delete(stored_kind, &id);
        let previous = rig.installed_snapshot().unwrap();
        let results = [
            crate::prepare_show_candidate(&document, transaction.clone()),
            crate::show_compiler::prepare_normalized_show_candidate_incremental(
                &document,
                transaction.clone(),
                &previous,
            ),
            crate::show_compiler::prepare_show_candidate_exact_transaction(
                &document,
                transaction.clone(),
            ),
            crate::show_compiler::prepare_show_candidate_preserving_object(
                &document,
                transaction,
                "playback_page",
                "1",
            ),
        ];
        for result in results {
            let error = result
                .err()
                .expect("all preparation lanes must protect configured targets");
            assert_eq!(error.kind, ActionErrorKind::Conflict);
            assert!(error.message.contains("1.2"), "{}", error.message);
        }
        assert!(rig.document().object(stored_kind, &id).is_some());
        assert!(rig.steps().is_empty());
        assert_eq!(rig.service.events().latest_sequence(), 0);
    }
}
