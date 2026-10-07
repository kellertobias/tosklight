use super::*;

fn activation_entry(state: &AppState, data_dir: &FsPath, name: &str) -> ShowEntry {
    let path = data_dir.join(format!("{name}.show"));
    initialise_show(&path, name).unwrap();
    let entry = state
        .installation
        .upsert_show(name, &path.display().to_string(), false)
        .unwrap();
    ActiveShowRepository::open(&path)
        .unwrap()
        .set_identity(entry.id, name, None)
        .unwrap();
    entry
}

fn seed_activation_runtime(state: &AppState) {
    let definition = command_test_dynamic(Uuid::new_v4(), 1);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            revision: 7,
            dynamics: vec![definition.clone()].into(),
            ..Default::default()
        })
        .unwrap();
    state
        .output
        .start_dynamic(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: Uuid::new_v4(),
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: Some(Uuid::new_v4()),
                },
                priority: 0,
                activated_at_millis: 100,
                size: 0.6,
                speed_multiplier: 1.0,
                phase_offset_degrees: 45.0,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: vec![light_core::FixtureId::new()],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 100,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
}

#[tokio::test]
async fn activation_rejects_invalid_controls_before_transition_or_show_effects() {
    for transition in [
        Transition::SafeBlackout,
        Transition::TimedFade,
        Transition::HoldCurrent,
    ] {
        let (state, data_dir) = test_state();
        let previous = activation_entry(&state, &data_dir, "Prior activation");
        let destination = activation_entry(&state, &data_dir, "Rejected activation");
        state.active_show.replace_current(Some(previous.clone()));
        seed_activation_runtime(&state);
        state
            .output
            .apply_runtime_control(Some(0.37), Some(false))
            .unwrap();
        let highlighted = light_core::FixtureId::new();
        state.output.set_highlighted_fixtures(vec![highlighted]);
        state.output.record_output_health(3, 0);
        let snapshot = state.output.snapshot();
        let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
        let controls = state.output.control_projection();
        let health = state.output.health_snapshot();
        let watermark = state.output.engine().playback_source_occurrence_watermark();
        let prepared = prepare_show_activation_for_runtime(&state, &destination).unwrap();
        let context = light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::System,
        );
        let error = activate_prepared_show(
            &state,
            prepared,
            &context,
            &transition,
            Some(100),
            destination,
            PersistedOutputRuntime {
                grand_master: -0.1,
                ..Default::default()
            },
            ActivationCompletion::Open {
                previous: Some(previous.clone()),
            },
            state.active_show.acquire_show_change().await,
        )
        .await
        .unwrap_err();
        assert!(
            error.message.contains("invalid controls"),
            "{}",
            error.message
        );
        assert!(Arc::ptr_eq(&state.output.snapshot(), &snapshot));
        assert_eq!(state.active_show.current().unwrap().id, previous.id);
        assert_eq!(
            state.output.dynamic_source_checkpoint().unwrap(),
            checkpoint
        );
        assert_eq!(
            state.output.engine().playback_source_occurrence_watermark(),
            watermark
        );
        assert_eq!(state.output.highlighted_fixtures(), vec![highlighted]);
        let after = state.output.control_projection();
        assert_eq!(after.revision, controls.revision);
        assert_eq!(after.grand_master, controls.grand_master);
        assert_eq!(after.blackout, controls.blackout);
        assert_eq!(
            state.output.health_snapshot().frames_sent,
            health.frames_sent
        );
        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[tokio::test]
async fn activation_installs_valid_empty_checkpoint_and_destination_together() {
    let (state, data_dir) = test_state();
    let previous = activation_entry(&state, &data_dir, "Prior valid activation");
    let destination = activation_entry(&state, &data_dir, "Valid activation");
    state.active_show.replace_current(Some(previous));
    seed_activation_runtime(&state);
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    let definition = command_test_dynamic(Uuid::new_v4(), 2);
    let definition_id = definition.id;
    let store = light_show::ShowStore::open(&destination.path).unwrap();
    store
        .put_object(
            "dynamic",
            &definition_id.to_string(),
            &serde_json::to_value(&definition).unwrap(),
            0,
        )
        .unwrap();
    let prepared = prepare_show_activation_for_runtime(&state, &destination).unwrap();
    let revision = prepared.runtime().snapshot().revision;
    let context = light_application::ActionContext::system(
        Uuid::nil(),
        light_application::ActionSource::System,
    );
    activate_prepared_show(
        &state,
        prepared,
        &context,
        &Transition::HoldCurrent,
        None,
        destination.clone(),
        PersistedOutputRuntime::default(),
        ActivationCompletion::Open { previous: None },
        state.active_show.acquire_show_change().await,
    )
    .await
    .unwrap();
    assert_eq!(state.active_show.current().unwrap().id, destination.id);
    let snapshot = state.output.snapshot();
    assert_eq!(snapshot.revision, revision);
    assert_eq!(snapshot.dynamics.len(), 1);
    assert_eq!(snapshot.dynamics[0].id, definition_id);
    assert!(state.output.dynamic_runtime_snapshot().instances.is_empty());
    assert!(
        state
            .output
            .dynamic_source_checkpoint()
            .unwrap()
            .runtime
            .instances
            .is_empty()
    );
    assert_eq!(state.output.control_projection().grand_master, 1.0);
    assert!(!state.output.control_projection().blackout);
    // Exercise the paired publication boundary after the transition releases Hold.
    state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            state.output.render_options(),
        )
        .unwrap();
    drop(state);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// TL-589: the completion descriptor alone decides the durable previous-active write and the
/// single event. RevisionCopy self-reference and MVR must both preserve the stored setting.
#[tokio::test]
async fn completion_descriptors_apply_their_previous_id_policy_and_one_event() {
    let source = |show: &ShowEntry| light_show::RevisionCopySource {
        show_id: show.id,
        show_name: show.name.clone(),
        revision: 3,
        revision_name: "Checkpoint".into(),
        copied_at: "2026-09-30T00:00:00Z".into(),
    };
    for case in 0..6 {
        let (state, data_dir) = test_state();
        let sentinel = activation_entry(&state, &data_dir, "Sentinel previous");
        let previous = activation_entry(&state, &data_dir, "Outgoing activation");
        let destination = activation_entry(&state, &data_dir, "Descriptor destination");
        state.active_show.replace_current(Some(previous.clone()));
        state
            .installation
            .set_setting("previous_active_show_id", &sentinel.id.0.to_string())
            .unwrap();
        let (completion, kind, expected_previous) = match case {
            0 => (
                ActivationCompletion::Open {
                    previous: Some(previous.clone()),
                },
                "show_opened",
                previous.id,
            ),
            1 => (
                ActivationCompletion::CleanDefault {
                    previous: Some(previous.clone()),
                },
                "show_opened",
                previous.id,
            ),
            2 => (
                ActivationCompletion::Rollback {
                    previous: Some(previous.clone()),
                },
                "show_rolled_back",
                previous.id,
            ),
            3 => (
                ActivationCompletion::RevisionCopy {
                    previous: Some(previous.clone()),
                    source: source(&previous),
                },
                "show_opened",
                previous.id,
            ),
            // A revision copy never records itself as its own rollback target.
            4 => (
                ActivationCompletion::RevisionCopy {
                    previous: Some(destination.clone()),
                    source: source(&previous),
                },
                "show_opened",
                sentinel.id,
            ),
            _ => (
                ActivationCompletion::Mvr {
                    imported: 2,
                    unresolved: 1,
                },
                "mvr_imported",
                sentinel.id,
            ),
        };
        let watermark = state
            .events
            .audit_events()
            .iter()
            .map(|event| event.revision)
            .max()
            .unwrap_or(0);
        let prepared = prepare_show_activation_for_runtime(&state, &destination).unwrap();
        let context = light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::System,
        );
        activate_prepared_show(
            &state,
            prepared,
            &context,
            &Transition::HoldCurrent,
            None,
            destination.clone(),
            PersistedOutputRuntime::default(),
            completion,
            state.active_show.acquire_show_change().await,
        )
        .await
        .unwrap();
        let durable = state.installation.active_show().unwrap().unwrap();
        assert_eq!(durable.id, destination.id, "case {case}");
        assert_eq!(
            state
                .installation
                .setting("previous_active_show_id")
                .unwrap(),
            Some(expected_previous.0.to_string()),
            "case {case}"
        );
        let events: Vec<_> = state
            .events
            .audit_events()
            .into_iter()
            .filter(|event| event.revision > watermark)
            .filter(|event| {
                matches!(
                    event.kind.as_str(),
                    "show_opened" | "show_rolled_back" | "mvr_imported"
                )
            })
            .collect();
        assert_eq!(events.len(), 1, "case {case}");
        assert_eq!(events[0].kind, kind, "case {case}");
        let payload = &events[0].payload;
        assert_eq!(payload["show"]["id"], destination.id.0.to_string());
        match case {
            0 | 1 => assert_eq!(payload["previous_show"]["id"], previous.id.0.to_string()),
            3 | 4 => {
                assert!(payload.get("previous_show").is_none());
                assert_eq!(payload["revision_copy"]["revision"], 3);
            }
            5 => {
                assert_eq!(
                    (payload["fixtures"].clone(), payload["unresolved"].clone()),
                    (2.into(), 1.into())
                );
                assert!(payload.get("previous_show").is_none());
            }
            _ => assert!(payload.get("previous_show").is_none()),
        }
        assert_eq!(payload.get("source").is_some(), case == 1, "case {case}");
        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[tokio::test]
async fn activation_copy_cleanup_preserves_files_when_storage_checks_fail() {
    for fail_identity_read in [true, false] {
        let (state, data_dir) = test_state();
        let entry = activation_entry(&state, &data_dir, "Cleanup retry");
        let database = rusqlite::Connection::open(data_dir.join("desk.sqlite")).unwrap();
        if fail_identity_read {
            state
                .installation
                .set_setting("active_show_id", "invalid-show-id")
                .unwrap();
            assert!(state.installation.active_show().is_err());
        } else {
            database.execute_batch("CREATE TRIGGER reject_cleanup BEFORE DELETE ON show_library BEGIN SELECT RAISE(ABORT, 'cleanup failure'); END;").unwrap();
        }
        crate::runtime::show_activation::discard_unactivated_destination(&state, &entry).await;
        assert!(std::path::Path::new(&entry.path).is_file());
        let remaining: i64 = database
            .query_row(
                "SELECT count(*) FROM show_library WHERE id = ?1",
                [entry.id.0.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        if fail_identity_read {
            state.installation.set_active_show(None).unwrap();
        } else {
            database
                .execute_batch("DROP TRIGGER reject_cleanup;")
                .unwrap();
        }
        crate::runtime::show_activation::discard_unactivated_destination(&state, &entry).await;
        assert!(!std::path::Path::new(&entry.path).exists());
        let remaining: i64 = database
            .query_row(
                "SELECT count(*) FROM show_library WHERE id = ?1",
                [entry.id.0.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
        drop(database);
        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[tokio::test]
async fn activation_copy_cleanup_preserves_live_or_durably_active_destinations() {
    for durable in [true, false] {
        let (state, data_dir) = test_state();
        let entry = activation_entry(&state, &data_dir, "Active cleanup protection");
        if durable {
            state.installation.set_active_show(Some(entry.id)).unwrap();
        } else {
            state.active_show.replace_current(Some(entry.clone()));
        }
        crate::runtime::show_activation::discard_unactivated_destination(&state, &entry).await;
        assert!(std::path::Path::new(&entry.path).is_file());
        let database = rusqlite::Connection::open(data_dir.join("desk.sqlite")).unwrap();
        let remaining: i64 = database
            .query_row(
                "SELECT count(*) FROM show_library WHERE id = ?1",
                [entry.id.0.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        drop(database);
        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
