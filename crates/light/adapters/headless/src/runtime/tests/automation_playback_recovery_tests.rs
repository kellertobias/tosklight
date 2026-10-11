use super::*;

fn automation_page(family: &str, id: Uuid) -> serde_json::Value {
    let target = if family == "macro" {
        light_playback::PlaybackTarget::Macro { macro_id: id }
    } else {
        light_playback::PlaybackTarget::Timecode {
            timecode_id: light_playback::TimecodeId(id),
        }
    };
    let definition = light_playback::PlaybackDefinition {
        name: format!("{family} control"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        fader: light_playback::PlaybackDefinition::default_fader(&target),
        target,
        has_fader: false,
        button_count: 3,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    serde_json::to_value(light_playback::PlaybackPage {
        number: 1,
        name: "Automation".into(),
        assignments: HashMap::from([(103, definition)]),
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automation_fader_recovery_rejects_saved_controls_without_rewriting_or_switching_show() {
    for family in ["macro", "timecode"] {
        for omitted in [false, true] {
            let desk = ContractDesk::new("automation-fader-recovery");
            let prior = desk.create_show("Previous valid show");
            let candidate = desk.create_show("Automation candidate");
            let target_id = Uuid::new_v4();
            put_automation_target(&candidate, family, target_id);
            let mut page = automation_page(family, target_id);
            eprintln!("{family}, omitted={omitted}: valid startup");
            candidate.put("playback_page", "1", page.clone());
            desk.activate(&candidate);
            let valid = desk.load_at(SEMANTIC_CONTRACT);
            assert!(valid.active_show_error.is_none(), "valid {family} control");
            assert!(!valid.engine.snapshot().playback_pages[0].assignments[&103].has_fader);
            drop(valid);

            // Only the capability flag changes. Simulate an already saved incompatible file,
            // without teaching current guarded writers to accept an invalid assignment.
            let definition = page["assignments"]["103"].as_object_mut().unwrap();
            if omitted {
                definition.remove("has_fader");
            } else {
                definition.insert("has_fader".into(), serde_json::json!(true));
            }
            let connection = rusqlite::Connection::open(&candidate.path).unwrap();
            connection
                .execute(
                    "UPDATE objects SET body_json=?1 WHERE kind='playback_page' AND id='1'",
                    [page.to_string()],
                )
                .unwrap();
            connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .unwrap();
            drop(connection);
            let original = candidate.bytes();

            desk.activate(&prior);
            eprintln!("{family}, omitted={omitted}: serve prior");
            let served = tokio::time::timeout(
                Duration::from_secs(10),
                desk.serve(desk.load_at(SEMANTIC_CONTRACT)),
            )
            .await
            .expect("served recovery runtime must initialize");
            let state = served.state.clone();
            let snapshot = state.output.snapshot();
            let prior_bytes = prior.bytes();
            let previous = state
                .installation
                .setting("previous_active_show_id")
                .unwrap();
            let app = router(state.clone());
            let (token, _) =
                tokio::time::timeout(Duration::from_secs(10), login(&app, "Recovery operator"))
                    .await
                    .expect("recovery login must complete");
            eprintln!("{family}, omitted={omitted}: invalid open");
            let response = tokio::time::timeout(
                Duration::from_secs(20),
                app.clone()
                    .oneshot(open_show_request(&token, candidate.entry.id.0)),
            )
            .await
            .expect("invalid show admission must finish")
            .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{family}, omitted={omitted}"
            );
            let body = json(response).await;
            assert!(
                body["error"]
                    .as_str()
                    .unwrap()
                    .contains("playback layout is incompatible with its function"),
                "{body}"
            );
            assert_eq!(state.active_show.current().unwrap().id, prior.entry.id);
            assert_eq!(
                state.installation.active_show().unwrap().unwrap().id,
                prior.entry.id
            );
            assert_eq!(
                state
                    .installation
                    .setting("previous_active_show_id")
                    .unwrap(),
                previous
            );
            assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
            assert_eq!(candidate.bytes(), original);
            assert_eq!(prior.bytes(), prior_bytes);
            drop(app);
            drop(state);
            eprintln!("{family}, omitted={omitted}: close served runtime");
            tokio::time::timeout(Duration::from_secs(10), served.close())
                .await
                .expect("served recovery runtime must shut down");

            desk.activate(&candidate);
            eprintln!("{family}, omitted={omitted}: invalid startup");
            let startup = desk.load_at(SEMANTIC_CONTRACT);
            let error = startup
                .active_show_error
                .as_ref()
                .expect("visible startup rejection");
            assert!(error.contains("Automation candidate"), "{error}");
            assert!(
                error.contains("playback layout is incompatible with its function"),
                "{error}"
            );
            assert!(
                startup
                    .engine
                    .snapshot()
                    .playback_pages
                    .iter()
                    .all(|page| page.assignments.is_empty())
            );
            assert_eq!(
                startup.persistent.active_show.as_ref().unwrap().id,
                candidate.entry.id
            );
            assert_eq!(candidate.bytes(), original);
            drop(startup);

            // Recovery is a separate valid show, never conversion of the rejected file.
            desk.activate(&prior);
            assert!(desk.load_at(SEMANTIC_CONTRACT).active_show_error.is_none());
            assert_eq!(candidate.bytes(), original);
            eprintln!(
                "{family}, omitted={omitted}: PASS valid control, open rejection, startup rejection, byte preservation, recovery"
            );
        }
    }
}

fn put_automation_target(show: &ContractShow, family: &str, id: Uuid) {
    let body = if family == "macro" {
        serde_json::to_value(light_application::CommandMacroDefinition {
            id,
            number: 103,
            name: "Recovery Macro".into(),
            source: "Clear".into(),
            presentation: Default::default(),
        })
        .unwrap()
    } else {
        serde_json::to_value(light_playback::TimecodeDefinition {
            id: light_playback::TimecodeId(id),
            number: 103,
            name: "Recovery Timecode".into(),
            duration: Some(light_playback::TimecodeFrame(440)),
            transport_offset: light_playback::TimecodeFrame::ZERO,
            auto_start: false,
            audio: None,
            markers: Vec::new(),
            lanes: Vec::new(),
        })
        .unwrap()
    };
    show.put(family, &id.to_string(), body);
}
