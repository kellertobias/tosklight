use super::show_object_intents_v2_route_tests::{create_seeded_show, dynamic_definition_json};
use super::*;

async fn dynamic_pool_command_desk() -> (AppState, Session, PathBuf, Uuid) {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let body = dynamic_definition_json(32);
    let id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    create_seeded_show(
        &state,
        &app,
        &token,
        "Dynamic commands",
        &[("dynamic", &id.to_string(), body)],
    )
    .await;
    let session = state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == token)
        .unwrap();
    state
        .programming
        .select(session.id, [light_core::FixtureId::new()]);
    (state, session, data_dir, id)
}

#[tokio::test]
async fn dynamic_pool_command_set_opens_exact_editor_without_programming() {
    let (state, session, data_dir, id) = dynamic_pool_command_desk().await;
    let before = state.programming.get(session.id).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let baseline = state.events.audit_events().len();
    assert_eq!(
        execute_programmer_command_from(&state, &session, "SET DYNAMIC 32", &context).unwrap(),
        0
    );
    let after = state.programming.get(session.id).unwrap();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(after.undo.len(), before.undo.len());
    assert!(state.output.dynamic_runtime_snapshot().instances.is_empty());
    assert!(
        state.events.audit_events()[baseline..]
            .iter()
            .any(|event| event.kind == "desk_action"
                && event.payload["action"] == "open-object-editor"
                && event.payload["control"] == "dynamic"
                && event.payload["value"] == id.to_string()
                && event.payload["desk_id"] == session.desk.id.to_string())
    );
    for invalid in ["SET DYNAMIC 0", "SET DYNAMIC 9999", "SET DYNAMIC 32 AT 33"] {
        assert!(execute_programmer_command_from(&state, &session, invalid, &context).is_err());
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_pool_command_copy_aliases_preserve_source_and_reject_occupied_atomically() {
    let (state, session, data_dir, source_id) = dynamic_pool_command_desk().await;
    let entry = state.active_show.current().unwrap();
    let store = ActiveShowRepository::open(&entry.path).unwrap();
    let original = store
        .object_with_portable_revision("dynamic", &source_id.to_string())
        .unwrap()
        .1
        .unwrap();
    let before = state.programming.get(session.id).unwrap();
    for (alias, destination) in [("COPY", 33), ("CPY", 34)] {
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        assert_eq!(
            execute_programmer_command_from(
                &state,
                &session,
                &format!("{alias} DYNAMIC 32 AT DYNAMIC {destination}"),
                &context
            )
            .unwrap(),
            1
        );
        let copy = store
            .objects_with_portable_revision("dynamic")
            .unwrap()
            .1
            .into_iter()
            .find(|object| object.body["pool_number"] == destination)
            .unwrap();
        assert_ne!(copy.id, source_id.to_string());
        assert_eq!(copy.revision, 1);
        // Authored lanes retain identity/content; legacy omitted fields may normalize.
        let copied_definition: light_dynamics::DynamicDefinition =
            serde_json::from_value(copy.body.clone()).unwrap();
        let source_definition: light_dynamics::DynamicDefinition =
            serde_json::from_value(original.body.clone()).unwrap();
        assert_eq!(copied_definition.lanes, source_definition.lanes);
        assert_eq!(
            copy.body["future_definition_field"],
            original.body["future_definition_field"]
        );
        assert_eq!(copy.body["target_binding"], original.body["target_binding"]);
        assert_eq!(copy.body["speed"], original.body["speed"]);
    }
    let snapshot = store.objects_with_portable_revision("dynamic").unwrap();
    for command in [
        "COPY DYNAMIC 32 AT DYNAMIC 33",
        "CPY DYNAMIC 32 AT DYNAMIC 32",
    ] {
        let context = operator_action_context(&session, light_application::ActionSource::Http);
        let error =
            execute_programmer_command_from(&state, &session, command, &context).unwrap_err();
        assert!(error.contains("already occupied"), "{error}");
    }
    let unchanged = store.objects_with_portable_revision("dynamic").unwrap();
    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        serde_json::to_value(unchanged).unwrap()
    );
    let source = store
        .object_with_portable_revision("dynamic", &source_id.to_string())
        .unwrap()
        .1
        .unwrap();
    assert_eq!(source.body, original.body);
    assert_eq!(source.revision, original.revision);
    let after = state.programming.get(session.id).unwrap();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(after.undo.len(), before.undo.len());
    assert!(state.output.dynamic_runtime_snapshot().instances.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
