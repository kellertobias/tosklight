//! Migration write-back observability tests (chunks 02/02c): committed compatibility
//! migrations riding object and route mutations are reported and published, and the
//! cue_list zero-xfade echo is stripped at merge time.

use super::*;

#[test]
fn cue_list_mutation_drops_the_stored_zero_chaser_xfade_echo() {
    let rig = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0x801));
    let storage_id = cue_list_id.0.to_string();
    let mut legacy = cue_list_body(cue_list_id, "Legacy");
    legacy
        .as_object_mut()
        .unwrap()
        .insert("chaser_xfade_millis".into(), json!(0));
    rig.seed_object("cue_list", &storage_id, legacy);

    let result = rig
        .service
        .mutate_objects(
            rig.object_action(vec![ActiveShowObjectMutation {
                kind: ActiveShowObjectKind::CueList,
                object_id: storage_id.clone(),
                expected_object_revision: 1,
                mutation: ActiveShowObjectMutationKind::Put {
                    body: typed(
                        ActiveShowObjectKind::CueList,
                        cue_list_body(cue_list_id, "Edited"),
                    ),
                },
            }]),
            &rig.ports,
        )
        .unwrap();

    assert_eq!(result.changes.len(), 1);
    let body = rig.object_body("cue_list", &storage_id);
    // The canonical model treats a zero chaser_xfade_millis as absent, so the merge must not
    // re-persist the stored raw echo; otherwise the next migration pass silently rewrites it.
    assert!(
        !body
            .as_object()
            .unwrap()
            .contains_key("chaser_xfade_millis"),
        "merged cue_list body must not echo the skip-serialized zero"
    );
    let merged = result.changes[0].body.as_ref().unwrap().encode();
    assert!(
        !merged
            .as_object()
            .unwrap()
            .contains_key("chaser_xfade_millis")
    );
}

#[test]
fn committed_migration_write_backs_are_published_as_object_changes() {
    let rig = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0x901));
    let storage_id = cue_list_id.0.to_string();
    let mut legacy = cue_list_body(cue_list_id, "Legacy");
    legacy
        .as_object_mut()
        .unwrap()
        .insert("chaser_xfade_millis".into(), json!(0));
    rig.seed_object("cue_list", &storage_id, legacy);

    let result = rig
        .service
        .mutate_objects(
            rig.object_action(vec![ActiveShowObjectMutation {
                kind: ActiveShowObjectKind::Group,
                object_id: "9".into(),
                expected_object_revision: 0,
                mutation: ActiveShowObjectMutationKind::Put {
                    body: typed(
                        ActiveShowObjectKind::Group,
                        json!({"id":"9","name":"Unrelated","fixtures":[]}),
                    ),
                },
            }]),
            &rig.ports,
        )
        .unwrap();

    // The requested change set stays exactly the request.
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].kind, ActiveShowObjectKind::Group);
    // The cue_list migration rode along in the same commit and bumped the object revision.
    let document = rig.document();
    let migrated = document.object("cue_list", &storage_id).unwrap();
    assert_eq!(migrated.revision(), 2);
    assert!(
        !migrated
            .body()
            .as_object()
            .unwrap()
            .contains_key("chaser_xfade_millis")
    );
    // The migration write-back is reported alongside the requested change.
    assert_eq!(result.migration_changes.len(), 1);
    assert_eq!(
        result.migration_changes[0].kind,
        ActiveShowObjectKind::CueList
    );
    assert_eq!(result.migration_changes[0].object_id, storage_id);
    assert_eq!(result.migration_changes[0].object_revision, 2);
    // And the one published event carries both changes so no revision bump stays silent.
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained object-change event")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &events[0].payload else {
        panic!("expected an ObjectsChanged event")
    };
    assert_eq!(change.changes.len(), 2);
    assert!(
        change
            .changes
            .iter()
            .any(|entry| entry.kind == ActiveShowObjectKind::CueList && entry.object_revision == 2)
    );
}

#[test]
fn route_mutation_publishes_migration_write_backs() {
    let rig = TestRig::new();
    let cue_list_id = light_core::CueListId(Uuid::from_u128(0xA01));
    let storage_id = cue_list_id.0.to_string();
    let mut legacy = cue_list_body(cue_list_id, "Legacy");
    legacy
        .as_object_mut()
        .unwrap()
        .insert("chaser_xfade_millis".into(), json!(0));
    rig.seed_object("cue_list", &storage_id, legacy);
    // A legacy route missing its explicit destination/delivery_mode is migration-pending.
    rig.seed_route(
        "legacy",
        json!({
            "protocol": "art_net",
            "logical_universe": 2,
            "destination_universe": 2,
            "enabled": true,
            "minimum_slots": 512
        }),
    );

    let result = rig
        .service
        .mutate_output_route(
            rig.action(
                "main",
                0,
                OutputRouteMutation::Put {
                    body: typed_route(json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 1,
                        "delivery_mode": "broadcast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 512
                    })),
                },
            ),
            &rig.ports,
        )
        .unwrap();

    // The cue_list migration rode along and is reported plus published.
    assert_eq!(result.migration_changes.len(), 1);
    assert_eq!(
        result.migration_changes[0].kind,
        ActiveShowObjectKind::CueList
    );
    assert_eq!(result.migration_changes[0].object_id, storage_id);
    // The legacy route's migration is reported as its own route change.
    assert_eq!(result.migrated_routes.len(), 1);
    assert_eq!(result.migrated_routes[0].route_id, "legacy");
    assert!(!result.migrated_routes[0].deleted);
    assert!(result.migrated_routes[0].route.is_some());
    let document = rig.document();
    assert!(
        !document
            .object("cue_list", &storage_id)
            .unwrap()
            .body()
            .as_object()
            .unwrap()
            .contains_key("chaser_xfade_millis")
    );
    assert!(
        document
            .object("route", "legacy")
            .unwrap()
            .body()
            .as_object()
            .unwrap()
            .contains_key("destination")
    );
    // Events: the requested route change first, then the object riders and route rider.
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained events")
    };
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0].payload,
        ApplicationEvent::Show(ShowEvent::OutputRouteChanged(change)) if change.route_id == "main"
    ));
    assert!(matches!(
        &events[1].payload,
        ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) if change.changes.len() == 1
    ));
    assert!(matches!(
        &events[2].payload,
        ApplicationEvent::Show(ShowEvent::OutputRouteChanged(change)) if change.route_id == "legacy"
    ));
}
