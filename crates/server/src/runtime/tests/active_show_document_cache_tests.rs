#[tokio::test]
async fn active_show_document_cache_reuses_and_detects_out_of_band_writes() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Cache show").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    assert!(state.active_show_document.lock().is_none());
    let path = state.active_show.read().as_ref().unwrap().path.clone();

    // The first mutation loads the document and leaves it cached at the committed revision.
    assert_eq!(
        put_show_object(
            &app,
            &token,
            &show_id,
            "group",
            "1",
            serde_json::json!({"id":"1","name":"First","fixtures":[]}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let cached_revision = state
        .active_show_document
        .lock()
        .as_ref()
        .map(light_show::PortableShowDocument::revision);
    let store_revision = ShowStore::open(&path).unwrap().portable_revision().unwrap();
    assert_eq!(cached_revision, Some(store_revision));

    // The second mutation reuses the cache and stays byte-identical with the store.
    assert_eq!(
        put_show_object(
            &app,
            &token,
            &show_id,
            "group",
            "2",
            serde_json::json!({"id":"2","name":"Second","fixtures":[]}),
        )
        .await
        .status(),
        StatusCode::OK
    );

    // An out-of-band portable write bumps the store revision behind the cache's back.
    ShowStore::open(&path)
        .unwrap()
        .put_object(
            "user_layout",
            "operator",
            &serde_json::json!({"marker":"out-of-band"}),
            0,
        )
        .unwrap();

    // The next mutation must detect the stale cache, reload, and succeed with the fresh data.
    assert_eq!(
        put_show_object(
            &app,
            &token,
            &show_id,
            "group",
            "3",
            serde_json::json!({"id":"3","name":"Third","fixtures":[]}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    let document = state.active_show_document.lock().clone().unwrap();
    assert_eq!(
        document,
        ShowStore::open(&path).unwrap().portable_document().unwrap()
    );
    assert_eq!(
        document.object("user_layout", "operator").unwrap().body()["marker"],
        "out-of-band"
    );
    assert_eq!(document.object("group", "2").unwrap().body()["name"], "Second");
    let _ = std::fs::remove_dir_all(data_dir);
}
