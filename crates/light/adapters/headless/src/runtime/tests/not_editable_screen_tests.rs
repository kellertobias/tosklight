use super::*;

/// Store a screen and return its id. `not_editable` decides whether it may change programming.
fn install_screen(scenario: &CommandHttpScenario, name: &str, not_editable: bool) -> Uuid {
    let screen = light_show::ScreenConfiguration {
        id: Uuid::new_v4(),
        name: name.into(),
        layout: serde_json::json!({"desks": [], "activeDeskId": ""}),
        show_dock: true,
        show_playbacks: true,
        playback_count: 8,
        playback_rows: 1,
        first_playback_slot: 1,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: light_show::ScreenContent::default(),
        not_editable,
    };
    scenario
        .state
        .installation
        .put_screen(screen)
        .expect("the screen should store")
        .id
}

async fn values_action_from_screen(
    scenario: &CommandHttpScenario,
    screen_id: Option<Uuid>,
    request_id: &str,
    expected_revision: u64,
    fixture: Uuid,
) -> Response {
    let mut request = Request::post(format!(
        "/api/v2/users/{}/programmer-values/actions",
        scenario.session.user.id.0
    ))
    .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
    .header(header::CONTENT_TYPE, "application/json");
    if let Some(screen_id) = screen_id {
        request = request.header("x-tosk-screen", screen_id.to_string());
    }
    scenario
        .app
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    fixture_set_request(request_id, expected_revision, fixture, 0.4).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn a_not_editable_screen_cannot_change_programming_but_an_ordinary_one_can() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let ordinary = install_screen(&scenario, "Front of house", false);
    let read_only = install_screen(&scenario, "Foyer repeater", true);

    // The same session, the same operator, the same Programmer — only the screen differs.
    let refused =
        values_action_from_screen(&scenario, Some(read_only), "from-read-only", 0, fixture.0).await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(
        json(refused).await["error"]
            .as_str()
            .unwrap()
            .contains("Not Editable")
    );

    let allowed =
        values_action_from_screen(&scenario, Some(ordinary), "from-ordinary", 0, fixture.0).await;
    assert_eq!(allowed.status(), StatusCode::OK);

    // No screen named at all is the main window, which programs.
    let main_window =
        values_action_from_screen(&scenario, None, "from-main", 1, fixture.0).await;
    assert_eq!(main_window.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn a_not_editable_screen_still_reads_the_desk_and_operates_playback() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let read_only = install_screen(&scenario, "Foyer repeater", true);
    assert_eq!(
        values_action_from_screen(&scenario, None, "seed", 0, fixture.0)
            .await
            .status(),
        StatusCode::OK
    );

    // Reading is presentation, not programming: a Not Editable screen shows the fixture sheet,
    // the Stage and the desk's values exactly as any other screen does.
    let snapshot = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/users/{}/programmer-values/snapshot",
                scenario.session.user.id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
            .header("x-tosk-screen", read_only.to_string())
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(
        snapshot["projection"]["fixture_values"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Playback is operation, not programming, so it is accepted from the same screen.
    let playback = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/output-runtime/global-master/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-screen", read_only.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"grand_master":0.5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(playback.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn an_unknown_screen_header_does_not_grant_or_remove_programming() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();

    // A screen id that names nothing is not a Not Editable screen, so it changes nothing. The
    // header can only ever take capability away, never hand it out.
    let unknown =
        values_action_from_screen(&scenario, Some(Uuid::new_v4()), "unknown", 0, fixture.0).await;
    assert_eq!(unknown.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn the_not_editable_flag_survives_being_stored_and_read_back() {
    let scenario = CommandHttpScenario::new().await;
    let screen_id = install_screen(&scenario, "Foyer repeater", true);

    let stored = scenario
        .state
        .installation
        .screen(screen_id)
        .unwrap()
        .expect("the screen should still be there");
    assert!(stored.not_editable);
    assert_eq!(stored.page_mode, "follow_main", "screen-local presentation configuration is untouched");

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
