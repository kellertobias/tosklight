use super::*;

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
                    body: typed_route(json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 2,
                        "delivery_mode": "broadcast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 128,
                        "future_client_field": "accepted"
                    })),
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
                    body: typed_route(json!({
                        "protocol": "art_net",
                        "logical_universe": 1,
                        "destination_universe": 1,
                        "delivery_mode": "unicast",
                        "destination": null,
                        "enabled": true,
                        "minimum_slots": 512
                    })),
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
fn output_route_range_is_one_atomic_persist_and_runtime_install() {
    let rig = TestRig::new();
    let result = rig
        .service
        .create_output_route_range(rig.range_action(8, 108), &rig.ports)
        .unwrap();

    assert_eq!(
        rig.steps(),
        ["begin", "prepare", "backup", "commit", "install"]
    );
    assert_eq!(result.changes.len(), 8);
    assert!(result.migration_changes.is_empty());
    assert!(result.migrated_routes.is_empty());
    assert_eq!(result.event_sequence, 8);
    assert!(
        result
            .changes
            .iter()
            .all(|change| change.show_revision.value() == 1 && change.object_revision == 1)
    );
    assert_eq!(rig.document().objects_of_kind("route").count(), 8);
    assert_eq!(rig.installed_routes(), 8);
    for (offset, change) in result.changes.iter().enumerate() {
        let route = change.route.as_ref().unwrap();
        assert_eq!(route.logical_universe, offset as u16 + 1);
        assert_eq!(route.destination_universe, offset as u16 + 101);
    }
}

#[test]
fn invalid_output_route_range_has_no_partial_side_effects() {
    let rig = TestRig::new();
    let error = rig
        .service
        .create_output_route_range(rig.range_action(8, 107), &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.steps(), ["begin"]);
    assert_eq!(rig.document().objects_of_kind("route").count(), 0);
    assert_eq!(rig.installed_routes(), 0);
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn output_route_range_rejects_descending_and_more_than_128_routes() {
    for (logical_end, destination_end, message) in
        [(0, 100, "ascending"), (129, 229, "more than 128")]
    {
        let rig = TestRig::new();
        let error = rig
            .service
            .create_output_route_range(rig.range_action(logical_end, destination_end), &rig.ports)
            .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid);
        assert!(error.message.contains(message));
        assert_eq!(rig.steps(), ["begin"]);
        assert_eq!(rig.document().objects_of_kind("route").count(), 0);
    }
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
