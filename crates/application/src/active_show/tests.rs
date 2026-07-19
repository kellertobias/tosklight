use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ApplicationEvent,
    EventBus, EventFilter, EventReplay, EventSource, ShowEvent,
};
use light_core::ShowId;
use light_engine::EngineSnapshot;
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore, StoreError,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

#[test]
fn route_update_prepares_before_one_backup_and_preserves_raw_extensions() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "art_net",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "broadcast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512,
            "future_server_field": {"kept": true}
        }),
    );
    let result = rig
        .service
        .mutate_output_route(
            rig.action(
                "main",
                1,
                OutputRouteMutation::Put {
                    body: json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 2,
                        "delivery_mode": "broadcast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 128,
                        "future_client_field": "accepted"
                    }),
                },
            ),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(
        rig.steps(),
        ["begin", "prepare", "backup", "commit", "install"]
    );
    assert_eq!(result.change.show_revision.value(), 2);
    assert_eq!(result.change.object_revision, 2);
    assert_eq!(result.event_sequence, 1);
    assert_eq!(
        result
            .route_to_terminate
            .as_ref()
            .map(|route| route.destination_universe),
        Some(1)
    );
    let stored = rig.route_body("main");
    assert_eq!(stored["future_server_field"], json!({"kept": true}));
    assert_eq!(stored["future_client_field"], "accepted");
    assert_eq!(stored["destination_universe"], 2);
    assert_eq!(rig.installed_routes(), 1);

    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained route event");
    };
    let event = events.first().unwrap();
    assert_eq!(
        event.object.as_ref().unwrap().id,
        format!("route:{}:main", rig.show_id.0)
    );
    assert!(matches!(
        &event.payload,
        ApplicationEvent::Show(ShowEvent::OutputRouteChanged(change))
            if change.route_id == "main" && !change.deleted
    ));
}

#[test]
fn invalid_route_stops_before_backup_commit_install_and_event() {
    let rig = TestRig::new();
    let error = rig
        .service
        .mutate_output_route(
            rig.action(
                "broken",
                0,
                OutputRouteMutation::Put {
                    body: json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 1,
                        "delivery_mode": "unicast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 512
                    }),
                },
            ),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.steps(), ["begin"]);
    assert!(rig.document().object("route", "broken").is_none());
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn stale_object_revision_stops_before_candidate_preparation_or_side_effects() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "art_net",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "broadcast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512
        }),
    );

    let error = rig
        .service
        .mutate_output_route(
            rig.action("main", 0, OutputRouteMutation::Delete),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(rig.steps(), ["begin"]);
    assert!(rig.document().object("route", "main").is_some());
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn route_delete_uses_the_same_prepared_atomic_boundary() {
    let rig = TestRig::new();
    rig.seed_route(
        "main",
        json!({
            "protocol": "sacn",
            "logical_universe": 1,
            "destination_universe": 1,
            "delivery_mode": "multicast",
            "destination": null,
            "enabled": true,
            "minimum_slots": 512,
            "future": true
        }),
    );

    let result = rig
        .service
        .mutate_output_route(
            rig.action("main", 1, OutputRouteMutation::Delete),
            &rig.ports,
        )
        .unwrap();

    assert!(result.change.deleted);
    assert!(result.change.route.is_none());
    assert!(result.route_to_terminate.is_some());
    assert_eq!(result.change.object_revision, 2);
    assert!(rig.document().object("route", "main").is_none());
    assert_eq!(rig.installed_routes(), 0);
    assert_eq!(
        rig.steps(),
        ["begin", "prepare", "backup", "commit", "install"]
    );
}

#[test]
fn group_batch_preserves_extensions_empty_state_and_ordered_membership() {
    let rig = TestRig::new();
    let first = light_core::FixtureId(Uuid::from_u128(11));
    let second = light_core::FixtureId(Uuid::from_u128(12));
    let mut existing = serde_json::to_value(light_programmer::GroupDefinition {
        id: "wrong-body-id".into(),
        name: "Existing".into(),
        fixtures: vec![first],
        ..Default::default()
    })
    .unwrap();
    existing["future_server_field"] = json!({"retained": true});
    rig.seed_object("group", "7", existing);

    let mut ordered = serde_json::to_value(light_programmer::GroupDefinition {
        id: "ignored".into(),
        name: "Ordered".into(),
        fixtures: vec![second, first],
        ..Default::default()
    })
    .unwrap();
    ordered["future_client_field"] = json!("accepted");
    let empty = serde_json::to_value(light_programmer::GroupDefinition {
        id: "8".into(),
        name: "Stored empty".into(),
        fixtures: Vec::new(),
        ..Default::default()
    })
    .unwrap();
    let result = rig
        .service
        .mutate_objects(
            rig.object_action(vec![
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Group,
                    object_id: "7".into(),
                    expected_object_revision: 1,
                    mutation: ActiveShowObjectMutationKind::Put { body: ordered },
                },
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Group,
                    object_id: "8".into(),
                    expected_object_revision: 0,
                    mutation: ActiveShowObjectMutationKind::Put { body: empty },
                },
            ]),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(
        rig.steps(),
        [
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
    assert_eq!(result.show_revision.value(), 2);
    assert_eq!(result.changes[0].object_revision, 2);
    assert_eq!(result.changes[1].object_revision, 1);
    assert_eq!(result.event_sequence, 1);
    assert_eq!(rig.installed_revision(), Some(2));
    let stored = rig.object_body("group", "7");
    assert_eq!(stored["id"], "7");
    assert_eq!(stored["fixtures"], json!([second, first]));
    assert_eq!(stored["future_server_field"], json!({"retained": true}));
    assert_eq!(stored["future_client_field"], "accepted");
    assert_eq!(rig.object_body("group", "8")["fixtures"], json!([]));

    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained show-object event");
    };
    let event = events.first().unwrap();
    assert_eq!(
        event.object.as_ref().unwrap().id,
        format!("objects:{}", rig.show_id.0)
    );
    assert!(matches!(
        &event.payload,
        ApplicationEvent::Show(ShowEvent::ObjectsChanged(change))
            if change.show_revision.value() == 2 && change.changes.len() == 2
    ));
}

#[test]
fn cue_list_mutation_uses_one_prepared_boundary_and_keeps_action_context() {
    let rig = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0x601));
    let mut cue = light_playback::Cue::new(1.0);
    cue.id = Uuid::from_u128(0x602);
    let body = serde_json::to_value(light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    })
    .unwrap();
    let action = rig.object_action(vec![ActiveShowObjectMutation {
        kind: ActiveShowObjectKind::CueList,
        object_id: cue_list_id.0.to_string(),
        expected_object_revision: 0,
        mutation: ActiveShowObjectMutationKind::Put { body },
    }]);
    let correlation_id = action.context.correlation_id;

    let result = rig.service.mutate_objects(action, &rig.ports).unwrap();

    assert_eq!(
        rig.steps(),
        [
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
    assert_eq!(result.event_sequence, 1);
    assert_eq!(result.changes[0].kind, ActiveShowObjectKind::CueList);
    assert_eq!(
        rig.object_body("cue_list", &cue_list_id.0.to_string())["id"],
        cue_list_id.0.to_string()
    );
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained CueList event");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].correlation_id, Some(correlation_id));
    assert_eq!(events[0].source, EventSource::Action(ActionSource::Http));
}

#[test]
fn playback_and_page_batch_prepares_backs_up_commits_and_installs_once() {
    let rig = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0x701));
    rig.seed_object(
        "cue_list",
        &cue_list_id.0.to_string(),
        cue_list_body(cue_list_id, "Main"),
    );
    let target = light_playback::PlaybackTarget::CueList { cue_list_id };
    let mut playback = serde_json::to_value(light_playback::PlaybackDefinition {
        number: 99,
        name: "Main".into(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        target,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    })
    .unwrap();
    playback["future_playback"] = json!({"kept": true});
    let page = json!({
        "number": 9,
        "name": "Main",
        "slots": {"1": 1},
        "future_page": "kept"
    });

    let result = rig
        .service
        .mutate_objects(
            rig.object_action(vec![
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Playback,
                    object_id: "1".into(),
                    expected_object_revision: 0,
                    mutation: ActiveShowObjectMutationKind::Put { body: playback },
                },
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::PlaybackPage,
                    object_id: "1".into(),
                    expected_object_revision: 0,
                    mutation: ActiveShowObjectMutationKind::Put { body: page },
                },
            ]),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(
        rig.steps(),
        [
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
    assert_eq!(result.show_revision.value(), 2);
    assert_eq!(result.event_sequence, 1);
    assert_eq!(result.changes.len(), 2);
    assert_eq!(rig.object_body("playback", "1")["number"], 1);
    assert_eq!(
        rig.object_body("playback", "1")["future_playback"]["kept"],
        true
    );
    assert_eq!(rig.object_body("playback_page", "1")["number"], 1);
    assert_eq!(rig.object_body("playback_page", "1")["future_page"], "kept");
    let installed = rig.installed_snapshot().unwrap();
    assert_eq!(installed.revision, 2);
    assert_eq!(installed.playbacks.len(), 1);
    assert_eq!(installed.playback_pages.len(), 1);
}

#[test]
fn preset_address_is_validated_before_backup_and_commit() {
    let rig = TestRig::new();
    let body = serde_json::to_value(light_programmer::Preset {
        name: "Wrong pool".into(),
        family: light_programmer::PresetFamily::Position,
        number: 1,
        ..Default::default()
    })
    .unwrap();
    let error = rig
        .service
        .mutate_objects(
            rig.object_action(vec![ActiveShowObjectMutation {
                kind: ActiveShowObjectKind::Preset,
                object_id: "2.1".into(),
                expected_object_revision: 0,
                mutation: ActiveShowObjectMutationKind::Put { body },
            }]),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.steps(), ["begin"]);
    assert!(rig.document().object("preset", "2.1").is_none());
}

#[test]
fn stale_member_of_a_batch_leaves_every_group_and_preset_unchanged() {
    let rig = TestRig::new();
    let group = serde_json::to_value(light_programmer::GroupDefinition {
        id: "1".into(),
        name: "Before".into(),
        ..Default::default()
    })
    .unwrap();
    let preset = serde_json::to_value(light_programmer::Preset {
        name: "Before".into(),
        family: light_programmer::PresetFamily::Color,
        number: 1,
        ..Default::default()
    })
    .unwrap();
    rig.seed_object("group", "1", group.clone());
    rig.seed_object("preset", "2.1", preset.clone());

    let error = rig
        .service
        .mutate_objects(
            rig.object_action(vec![
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Group,
                    object_id: "1".into(),
                    expected_object_revision: 1,
                    mutation: ActiveShowObjectMutationKind::Put {
                        body: json!({"name":"After"}),
                    },
                },
                ActiveShowObjectMutation {
                    kind: ActiveShowObjectKind::Preset,
                    object_id: "2.1".into(),
                    expected_object_revision: 0,
                    mutation: ActiveShowObjectMutationKind::Delete,
                },
            ]),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(rig.steps(), ["begin"]);
    assert_eq!(rig.object_body("group", "1"), group);
    assert_eq!(rig.object_body("preset", "2.1"), preset);
}

#[test]
fn object_undo_commits_exact_history_through_one_prepared_boundary() {
    let rig = TestRig::new();
    let original = json!({
        "id":"historic-id",
        "name":"Original",
        "fixtures":[],
        "future_extension":{"nested":[3, 1, 2]}
    });
    rig.seed_object("group", "7", original.clone());
    rig.write_object(
        "group",
        "7",
        json!({"id":"7","name":"Changed","fixtures":[]}),
        1,
    );
    let action = rig.undo_action(ActiveShowObjectKind::Group, "7", 2);
    let correlation_id = action.context.correlation_id;

    let result = rig.service.undo_object(action, &rig.ports).unwrap();

    assert_eq!(
        rig.steps(),
        [
            "begin",
            "history",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );
    assert_eq!(result.show_revision.value(), 3);
    assert_eq!(result.change.object_revision, 3);
    assert_eq!(result.change.body, Some(original.clone()));
    assert_eq!(result.event_sequence, 1);
    assert_eq!(rig.object_body("group", "7"), original);
    assert_eq!(rig.installed_revision(), Some(3));
    assert!(matches!(
        ShowStore::open(&rig.ports.path)
            .unwrap()
            .prepare_object_undo("group", "7", 3),
        Err(StoreError::Invalid(message)) if message == "object has no undo history"
    ));

    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained object Undo event");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].correlation_id, Some(correlation_id));
    assert_eq!(events[0].source, EventSource::Action(ActionSource::Http));
    assert!(matches!(
        &events[0].payload,
        ApplicationEvent::Show(ShowEvent::ObjectsChanged(change))
            if change.show_revision.value() == 3
                && change.changes == vec![result.change]
    ));
}

#[test]
fn object_undo_commits_pending_migrations_in_the_same_compiled_candidate() {
    let rig = TestRig::new();
    let original = json!({
        "id":"7",
        "name":"Original",
        "fixtures":[],
        "future_extension":{"kept":true}
    });
    rig.seed_object("group", "7", original.clone());
    rig.write_object(
        "group",
        "7",
        json!({"id":"7","name":"Changed","fixtures":[]}),
        1,
    );
    rig.seed_object(
        "group",
        "8",
        json!({"id":"legacy-wrong-id","name":"Pending migration","fixtures":[]}),
    );

    let result = rig
        .service
        .undo_object(
            rig.undo_action(ActiveShowObjectKind::Group, "7", 2),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(result.show_revision.value(), 4);
    let document = rig.document();
    assert_eq!(document.revision().value(), 4);
    assert_eq!(document.object("group", "7").unwrap().body(), &original);
    assert_eq!(document.object("group", "8").unwrap().body()["id"], "8");
    assert_eq!(document.object("group", "8").unwrap().revision(), 2);

    let prepared = crate::prepare_show_candidate(&document, document.transaction()).unwrap();
    let (transaction, recompiled) = prepared.into_parts();
    assert!(transaction.is_empty());
    let installed = rig.installed_snapshot().unwrap();
    assert_eq!(installed.revision, recompiled.revision);
    assert_eq!(
        serde_json::to_value(&installed.groups).unwrap(),
        serde_json::to_value(&recompiled.groups).unwrap()
    );
}

#[test]
fn object_undo_accepts_cue_list_and_preset_families() {
    let cue_list = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0x701));
    let mut original_cues = cue_list_body(cue_list_id, "Original CueList");
    original_cues["future_extension"] = json!({"nested":{"kept":true}});
    let changed_cues = cue_list_body(cue_list_id, "Changed CueList");
    let storage_id = cue_list_id.0.to_string();
    cue_list.seed_object("cue_list", &storage_id, original_cues.clone());
    cue_list.write_object("cue_list", &storage_id, changed_cues, 1);

    let result = cue_list
        .service
        .undo_object(
            cue_list.undo_action(ActiveShowObjectKind::CueList, &storage_id, 2),
            &cue_list.ports,
        )
        .unwrap();

    assert_eq!(result.change.kind, ActiveShowObjectKind::CueList);
    assert_eq!(cue_list.object_body("cue_list", &storage_id), original_cues);

    let preset = TestRig::new();
    let original_preset = json!({
        "name":"Original Preset",
        "family":"Color",
        "number":3,
        "future_extension":{"kept":true}
    });
    let changed_preset = json!({"name":"Changed Preset","family":"Color","number":3});
    preset.seed_object("preset", "2.3", original_preset.clone());
    preset.write_object("preset", "2.3", changed_preset, 1);

    let result = preset
        .service
        .undo_object(
            preset.undo_action(ActiveShowObjectKind::Preset, "2.3", 2),
            &preset.ports,
        )
        .unwrap();

    assert_eq!(result.change.kind, ActiveShowObjectKind::Preset);
    assert_eq!(preset.object_body("preset", "2.3"), original_preset);
}

#[test]
fn stale_or_missing_history_undo_has_no_side_effects_and_preserves_history() {
    let stale = TestRig::new();
    let original = json!({"id":"1","name":"Original","fixtures":[]});
    let changed = json!({"id":"1","name":"Changed","fixtures":[]});
    stale.seed_object("group", "1", original.clone());
    stale.write_object("group", "1", changed.clone(), 1);

    let error = stale
        .service
        .undo_object(
            stale.undo_action(ActiveShowObjectKind::Group, "1", 1),
            &stale.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(2));
    assert_eq!(stale.steps(), ["begin"]);
    assert_eq!(stale.object_body("group", "1"), changed);
    assert_eq!(stale.installed_revision(), None);
    assert_eq!(stale.service.events().latest_sequence(), 0);
    assert_eq!(
        ShowStore::open(&stale.ports.path)
            .unwrap()
            .prepare_object_undo("group", "1", 2)
            .unwrap()
            .body(),
        &original
    );

    let no_history = TestRig::new();
    let only = json!({"name":"Only version","family":"Color","number":1});
    no_history.seed_object("preset", "2.1", only.clone());
    let error = no_history
        .service
        .undo_object(
            no_history.undo_action(ActiveShowObjectKind::Preset, "2.1", 1),
            &no_history.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert!(error.message.contains("no undo history"));
    assert_eq!(no_history.steps(), ["begin", "history"]);
    assert_eq!(no_history.object_body("preset", "2.1"), only);
    assert_eq!(no_history.installed_revision(), None);
    assert_eq!(no_history.service.events().latest_sequence(), 0);
}

#[test]
fn invalid_historical_candidate_stops_before_backup_and_keeps_history() {
    let rig = TestRig::new();
    let invalid = json!({
        "id":"9",
        "name":"Invalid historic Group",
        "fixtures":"not-an-array",
        "future_extension":{"must":"survive rejection"}
    });
    let current = json!({"id":"9","name":"Current","fixtures":[]});
    rig.seed_object("group", "9", invalid.clone());
    rig.write_object("group", "9", current.clone(), 1);

    let error = rig
        .service
        .undo_object(
            rig.undo_action(ActiveShowObjectKind::Group, "9", 2),
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.steps(), ["begin", "history"]);
    assert_eq!(rig.object_body("group", "9"), current);
    assert_eq!(rig.installed_revision(), None);
    assert_eq!(rig.service.events().latest_sequence(), 0);
    assert_eq!(
        ShowStore::open(&rig.ports.path)
            .unwrap()
            .prepare_object_undo("group", "9", 2)
            .unwrap()
            .body(),
        &invalid
    );
}

struct TestRig {
    service: ActiveShowService,
    ports: TestPorts,
    show_id: ShowId,
}

impl TestRig {
    fn new() -> Self {
        let path = temporary_show_path();
        let (store, show_id) = ShowStore::create(&path, "Active show route test").unwrap();
        drop(store);
        let events = EventBus::new(16);
        Self {
            service: ActiveShowService::new(events),
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
                installed: Arc::default(),
            },
            show_id,
        }
    }

    fn seed_route(&self, id: &str, body: Value) {
        self.seed_object("route", id, body);
    }

    fn seed_object(&self, kind: &str, id: &str, body: Value) {
        self.write_object(kind, id, body, 0);
    }

    fn write_object(&self, kind: &str, id: &str, body: Value, expected: u64) {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(kind, id, &body, expected)
            .unwrap();
    }

    fn action(
        &self,
        route_id: &str,
        expected_object_revision: u64,
        mutation: OutputRouteMutation,
    ) -> ActionEnvelope<MutateOutputRouteCommand> {
        ActionEnvelope {
            context: ActionContext::operator(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                Uuid::from_u128(3),
                ActionSource::Http,
            ),
            command: MutateOutputRouteCommand {
                show_id: self.show_id,
                route_id: route_id.into(),
                expected_object_revision,
                mutation,
            },
        }
    }

    fn route_body(&self, id: &str) -> Value {
        self.object_body("route", id)
    }

    fn object_action(
        &self,
        mutations: Vec<ActiveShowObjectMutation>,
    ) -> ActionEnvelope<MutateActiveShowObjectsCommand> {
        ActionEnvelope {
            context: ActionContext::operator(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                Uuid::from_u128(3),
                ActionSource::Http,
            ),
            command: MutateActiveShowObjectsCommand {
                show_id: self.show_id,
                mutations,
            },
        }
    }

    fn undo_action(
        &self,
        kind: ActiveShowObjectKind,
        object_id: &str,
        expected_object_revision: u64,
    ) -> ActionEnvelope<UndoActiveShowObjectCommand> {
        ActionEnvelope {
            context: ActionContext::operator(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                Uuid::from_u128(3),
                ActionSource::Http,
            ),
            command: UndoActiveShowObjectCommand {
                show_id: self.show_id,
                kind,
                object_id: object_id.into(),
                expected_object_revision,
            },
        }
    }

    fn object_body(&self, kind: &str, id: &str) -> Value {
        self.document().object(kind, id).unwrap().body().clone()
    }

    fn document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn installed_routes(&self) -> usize {
        self.ports
            .installed
            .lock()
            .as_ref()
            .map_or(0, |snapshot| snapshot.routes.len())
    }

    fn installed_revision(&self) -> Option<u64> {
        self.ports
            .installed
            .lock()
            .as_ref()
            .map(|snapshot| snapshot.revision)
    }

    fn installed_snapshot(&self) -> Option<EngineSnapshot> {
        self.ports.installed.lock().clone()
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        remove_sqlite_files(&self.ports.path);
    }
}

struct TestPorts {
    path: PathBuf,
    show_id: ShowId,
    steps: Arc<Mutex<Vec<&'static str>>>,
    installed: Arc<Mutex<Option<EngineSnapshot>>>,
}

struct TestUnitOfWork {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
}

impl ActiveShowUnitOfWork for TestUnitOfWork {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        self.steps.lock().push("backup");
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.steps.lock().push("commit");
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))
    }
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnitOfWork;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.steps.lock().push("begin");
        if show_id != self.show_id {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "requested show is not active",
            ));
        }
        let store = ShowStore::open(&self.path)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        let document = store
            .portable_document()
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        Ok(TestUnitOfWork {
            store,
            document,
            steps: Arc::clone(&self.steps),
        })
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.steps.lock().push("history");
        unit.store
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(test_store_error)
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<Self::PreparedRuntime, ActionError> {
        self.steps.lock().push("prepare");
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, prepared: Self::PreparedRuntime) {
        self.steps.lock().push("install");
        *self.installed.lock() = Some(prepared);
    }

    fn reconcile_object_changes(&self, _changes: &[ActiveShowObjectChange]) {
        self.steps.lock().push("reconcile");
    }
}

fn test_store_error(error: StoreError) -> ActionError {
    match error {
        StoreError::RevisionConflict { current, .. } => {
            ActionError::new(ActionErrorKind::Conflict, error.to_string()).at_revision(current)
        }
        StoreError::Invalid(_) | StoreError::Uuid(_) | StoreError::Json(_) => {
            ActionError::new(ActionErrorKind::Invalid, error.to_string())
        }
        _ => ActionError::new(ActionErrorKind::Internal, error.to_string()),
    }
}

fn cue_list_body(id: light_core::CueListId, name: &str) -> Value {
    serde_json::to_value(light_playback::CueList {
        id,
        name: name.into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0)],
    })
    .unwrap()
}

fn temporary_show_path() -> PathBuf {
    std::env::temp_dir().join(format!("light-active-show-route-{}.sqlite", Uuid::new_v4()))
}

fn remove_sqlite_files(path: &PathBuf) {
    let _ = fs::remove_file(path);
    for suffix in ["-shm", "-wal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}
