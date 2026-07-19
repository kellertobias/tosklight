#[tokio::test]
async fn programmer_values_snapshot_returns_authenticated_projection_and_safe_cursor() {
    let scenario = CommandHttpScenario::new().await;
    let fixture_id = scenario.install_direct_fixture();
    let response = scenario
        .execute("values-snapshot", Some("GROUP 1 AT 50"))
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let expected_cursor = scenario.state.application_events.latest_sequence();
    let response = scenario.values_snapshot().await;
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot: light_wire::v2::programming::ProgrammingValuesSnapshot =
        serde_json::from_value(json(response).await).unwrap();

    assert_eq!(snapshot.cursor.sequence, expected_cursor);
    assert_eq!(snapshot.projection.user_id, scenario.session.user.id.0);
    assert_eq!(snapshot.projection.revision, 1);
    assert!(snapshot.projection.fixture_values.is_empty());
    assert_eq!(snapshot.projection.group_values.len(), 1);
    let value = &snapshot.projection.group_values[0];
    assert_eq!(value.group_id, "1");
    assert_eq!(value.attribute, "intensity");
    assert_eq!(
        value.value,
        light_wire::v2::programming::ProgrammingAttributeValue::Normalized(0.5)
    );
    assert_eq!(
        scenario
            .state
            .programmers
            .get(scenario.session.id)
            .unwrap()
            .selected,
        vec![fixture_id]
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn programmer_values_snapshot_rejects_foreign_user_and_missing_authentication() {
    let scenario = CommandHttpScenario::new().await;

    let response = scenario
        .values_snapshot_for(Uuid::new_v4(), Some(&scenario.token))
        .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = scenario
        .values_snapshot_for(scenario.session.user.id.0, None)
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
