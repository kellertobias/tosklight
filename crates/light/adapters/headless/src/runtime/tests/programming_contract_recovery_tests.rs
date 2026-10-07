use super::*;

fn contract_recovery_options(data_dir: &std::path::Path) -> startup_options::StartupOptions {
    startup_options::StartupOptions {
        data_dir: data_dir.to_owned(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        osc_bind_override: None,
        output_bind_override: None,
    }
}

/// TL-552: production supports contract 1, so these tests model the compatibility direction that
/// still needs recovery: an older contract-0 runtime meeting contract-1 runtime data. The guard is
/// the test-only startup harness and lasts until it is dropped.
fn older_contract_zero_runtime()
-> crate::runtime::e2e_semantic_contract::startup_contract_override::StartupContractGuard {
    crate::runtime::e2e_semantic_contract::startup_contract_override::at(0)
}

/// TL-552: a blank active show that loads at contract 0 and 1, so an older-runtime restore fails
/// on the runtime payload under test and not on the (contract-1) packaged demo show.
fn activate_blank_show(data_dir: &std::path::Path) {
    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let path = data_dir.join("shows/Recovery.show");
    let (store, _) = ShowStore::create(&path, "Recovery").unwrap();
    let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let entry = desk
        .upsert_show("Recovery", &path.display().to_string(), false)
        .unwrap();
    store.set_identity(entry.id, "Recovery", None).unwrap();
    desk.set_active_show(Some(entry.id)).unwrap();
}

fn preserved_runtime(data_dir: &std::path::Path) -> serde_json::Value {
    let reports: Vec<_> = std::fs::read_dir(data_dir.join("backups"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("runtime-recovery-")
        })
        .collect();
    assert_eq!(reports.len(), 1);
    serde_json::from_slice(&std::fs::read(&reports[0]).unwrap()).unwrap()
}

fn contract_programmer() -> (SessionId, serde_json::Value, serde_json::Value) {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    let empty = serde_json::to_value(programmers.start(session)).unwrap();
    programmers.set(
        session,
        light_core::FixtureId::new(),
        light_core::AttributeKey("position".into()),
        light_core::AttributeValue::Position(Arc::new(
            light_core::programming::PositionIntent::angles(720.0, -30.0),
        )),
    );
    let typed = serde_json::to_value(programmers.get(session).unwrap()).unwrap();
    (session, empty, typed)
}

#[test]
fn programming_contract_recovery_preserves_normal_preload_and_history_before_starting() {
    for lane in [
        "values",
        "preload_pending",
        "preload_active",
        "undo",
        "redo",
        "invalid_owner",
        "malformed",
    ] {
        let data_dir =
            std::env::temp_dir().join(format!("light-contract-recovery-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let (session, mut stored, typed) = contract_programmer();
        match lane {
            "undo" | "redo" => stored[lane] = serde_json::json!([{"values": typed["values"]}]),
            "invalid_owner" => {
                stored["values"] = typed["values"].clone();
                stored["values"][0]["attribute"] = "intensity".into();
            }
            "malformed" => {}
            _ => stored[lane] = typed["values"].clone(),
        }
        let original = if lane == "malformed" {
            "{broken programmer".to_owned()
        } else {
            stored.to_string()
        };
        let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
        desk.save_session(&light_show::PersistedSession {
            id: session,
            token: "never-copy-auth-token".into(),
            programmer_json: original.clone(),
            connected: false,
            updated_at: fixed_test_time().to_rfc3339(),
        })
        .unwrap();
        drop(desk);
        let older = older_contract_zero_runtime();
        let startup =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        drop(older);
        let error = startup
            .active_show_error
            .as_deref()
            .expect("desk starts in recoverable state");
        assert!(
            error.contains("original data is preserved"),
            "{lane}: {error}"
        );
        assert!(startup.programmers.get(session).is_none());
        assert!(startup.engine.snapshot().fixtures.is_empty());
        assert_eq!(
            startup.persistent.desk.persisted_sessions().unwrap()[0].programmer_json,
            original
        );
        let report = preserved_runtime(&data_dir);
        assert_eq!(report["serialized"], original);
        assert!(!report.to_string().contains("never-copy-auth-token"));
        drop(startup);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

#[test]
fn programming_contract_recovery_preserves_deleted_playback_sources_and_malformed_runtime() {
    for lane in [
        "deleted_cue_hold",
        "deleted_cue_transition_source",
        "malformed",
    ] {
        let data_dir = std::env::temp_dir().join(format!(
            "light-playback-contract-recovery-{}",
            Uuid::new_v4()
        ));
        activate_blank_show(&data_dir);
        let initial =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        assert!(
            initial.active_show_error.is_none(),
            "clean startup remains supported"
        );
        let show_id = initial.persistent.active_show.as_ref().unwrap().id;
        let (_, _, typed) = contract_programmer();
        let mut playback = serde_json::json!({
            "cue_list_id": Uuid::new_v4(), "cue_index":0, "previous_index":null,
            "paused":false, "activated_at":fixed_test_time(), "paused_at":null,
        });
        playback[lane] = if lane == "deleted_cue_hold" {
            serde_json::json!({"deleted_number":"1", "previous_number":null, "next_number":null, "contributions":typed["values"]})
        } else {
            typed["values"].clone()
        };
        let original = if lane == "malformed" {
            "{broken playback".to_owned()
        } else {
            serde_json::json!([playback]).to_string()
        };
        initial
            .persistent
            .desk
            .set_setting(&active_playbacks_setting(show_id), &original)
            .unwrap();
        drop(initial);
        let older = older_contract_zero_runtime();
        let startup =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        drop(older);
        let error = startup.active_show_error.as_deref().unwrap();
        assert!(
            error.contains("original data is preserved"),
            "{lane}: {error}"
        );
        if lane != "malformed" {
            assert!(error.contains("programming contract 1"), "{lane}: {error}");
        }
        assert_eq!(
            startup
                .persistent
                .desk
                .setting(&active_playbacks_setting(show_id))
                .unwrap()
                .unwrap(),
            original
        );
        assert_eq!(preserved_runtime(&data_dir)["serialized"], original);
        drop(startup);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

#[test]
fn retained_playback_evidence_reader_accepts_legacy_rows_and_preserves_invalid_runtime() {
    use light_core::programming::{ProgrammingFieldScope, ProgrammingTraceField};
    use light_playback::{
        PlaybackFamilyEntry, PlaybackFamilyEvidence, PlaybackFamilyFootprint, PlaybackFamilyRole,
        PlaybackRetainedValue, PlaybackSourceOccurrence, SequenceMasterSource,
    };

    for case in ["valid", "legacy", "bad_scope", "bad_cue_id"] {
        let data_dir = std::env::temp_dir().join(format!(
            "light-playback-evidence-recovery-{}",
            Uuid::new_v4()
        ));
        let initial =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        assert!(initial.active_show_error.is_none());
        let show_id = initial.persistent.active_show.as_ref().unwrap().id;
        let list_id = light_core::CueListId::new();
        let row = PlaybackRetainedValue {
            timed: light_core::TimedValue {
                fixture_id: light_core::FixtureId::new(),
                attribute: light_core::AttributeKey("focus".into()),
                value: light_core::AttributeValue::Normalized(0.5),
                priority: 100,
                changed_at: fixed_test_time(),
                programmer_order: 0,
                merge_mode: light_core::MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
            family_evidence: Some(Arc::new(
                PlaybackFamilyEvidence::try_new(vec![PlaybackFamilyEntry {
                    occurrence: PlaybackSourceOccurrence {
                        source: SequenceMasterSource {
                            playback_number: None,
                            playback_identity: None,
                            cue_list_id: list_id,
                            temporary: false,
                        },
                        action_changed_at: fixed_test_time(),
                        action_ordinal: 7,
                        authored_cue_id: Some(Uuid::new_v4()),
                    },
                    footprint: PlaybackFamilyFootprint::Whole,
                    role: PlaybackFamilyRole::Authored,
                    effective_fields: ProgrammingFieldScope::new([ProgrammingTraceField::Focus]),
                }])
                .unwrap(),
            )),
            pending_transition: None,
        };
        let mut row = serde_json::to_value(row).unwrap();
        match case {
            "legacy" => {
                row.as_object_mut().unwrap().remove("family_evidence");
            }
            "bad_scope" => {
                row["family_evidence"]["entries"][0]["effective_fields"] =
                    serde_json::to_value(ProgrammingFieldScope::new([ProgrammingTraceField::Pan]))
                        .unwrap()
            }
            "bad_cue_id" => {
                row["family_evidence"]["entries"][0]["occurrence"]["authored_cue_id"] =
                    serde_json::json!(Uuid::nil())
            }
            _ => {}
        }
        let original = serde_json::json!([{
            "cue_list_id":list_id, "cue_index":0, "previous_index":null,
            "paused":false, "activated_at":fixed_test_time(), "paused_at":null,
            "deleted_cue_transition_source":[row]
        }])
        .to_string();
        let key = active_playbacks_setting(show_id);
        initial
            .persistent
            .desk
            .set_setting(&key, &original)
            .unwrap();
        drop(initial);
        let reopened =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        if case.starts_with("bad_") {
            assert!(
                reopened
                    .active_show_error
                    .as_deref()
                    .is_some_and(|error| error.contains("original data is preserved")),
                "{case}"
            );
            assert_eq!(
                reopened.persistent.desk.setting(&key).unwrap().as_deref(),
                Some(original.as_str())
            );
            assert_eq!(preserved_runtime(&data_dir)["serialized"], original);
        } else {
            assert!(
                reopened.active_show_error.is_none(),
                "{case}: {:?}",
                reopened.active_show_error
            );
        }
        drop(reopened);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

#[tokio::test]
async fn programming_contract_recovery_keeps_persisted_runtime_when_global_controls_change() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Recovery control check").await;
    let response = app
        .clone()
        .oneshot(open_show_request(&token, show["id"].as_str().unwrap()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let show = state.active_show.current().unwrap();
    let output_key = output_runtime_setting(show.id);
    let playback_key = active_playbacks_setting(show.id);
    state
        .installation
        .set_setting(&output_key, "retained output checkpoint")
        .unwrap();
    state
        .installation
        .set_setting(&playback_key, "retained playback checkpoint")
        .unwrap();
    state
        .active_show
        .set_error(Some("stored runtime unavailable".into()));
    let response = app
        .oneshot(
            Request::post("/api/v2/output-runtime/global-master/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"grand_master":0.4,"blackout":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.output.control_projection().blackout);
    persist_active_playbacks(&state).unwrap();
    assert_eq!(
        state.installation.setting(&output_key).unwrap().as_deref(),
        Some("retained output checkpoint")
    );
    assert_eq!(
        state
            .installation
            .setting(&playback_key)
            .unwrap()
            .as_deref(),
        Some("retained playback checkpoint")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn programming_contract_recovery_preserves_deleted_typed_dynamic_and_malformed_output() {
    for malformed in [false, true] {
        let data_dir =
            std::env::temp_dir().join(format!("light-output-contract-recovery-{}", Uuid::new_v4()));
        activate_blank_show(&data_dir);
        let initial =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        let show_id = initial.persistent.active_show.as_ref().unwrap().id;
        let mut definition = command_test_dynamic(Uuid::new_v4(), 1);
        definition.lanes[0].body =
            light_dynamics::DynamicLaneBody::Programming(light_dynamics::ProgrammingLaneBody {
                address: light_dynamics::DynamicValueAddress {
                    representation: light_dynamics::DynamicFamilyRepresentation::Angles,
                    component: Some(light_core::programming::ProgrammingComponent::Pan),
                },
                configuration: light_dynamics::ProgrammingLaneConfiguration::MaxMin(
                    light_dynamics::MaxMinConfiguration {
                        minimum: light_dynamics::DynamicValueSource::Value {
                            value: light_dynamics::DynamicValue::Scalar(-90.0),
                        },
                        maximum: light_dynamics::DynamicValueSource::Value {
                            value: light_dynamics::DynamicValue::Scalar(90.0),
                        },
                        function: light_dynamics::PeriodicFunction::Sinus,
                        size: 1.0,
                        pwm: Default::default(),
                    },
                ),
            });
        let definition_id = definition.id;
        let mut runtime = light_dynamics::DynamicRuntime::default();
        runtime.install_definitions([definition]).unwrap();
        runtime
            .start(light_dynamics::DynamicStartRequest {
                definition_id,
                controller: light_dynamics::DynamicController {
                    id: Uuid::new_v4(),
                    source: light_dynamics::DynamicControllerSource::Programmer {
                        programmer_id: Uuid::new_v4(),
                        instance_link: None,
                    },
                    priority: 0,
                    activated_at_millis: 0,
                    size: 1.0,
                    speed_multiplier: 1.0,
                    phase_offset_degrees: 0.0,
                    paused: false,
                },
                target_scope: light_dynamics::DynamicTargetScope {
                    ordered_targets: vec![light_core::FixtureId::new()],
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: 0,
                activation_delay_millis: 0,
                activation_duration_millis: 0,
                activation_policy_override: None,
                reuse_matching_targetless: false,
            })
            .unwrap();
        let original = if malformed {
            "{broken output".to_owned()
        } else {
            serde_json::to_string(&PersistedOutputRuntime {
                dynamic_runtime: Some(runtime.snapshot()),
                ..Default::default()
            })
            .unwrap()
        };
        initial
            .persistent
            .desk
            .set_setting(&output_runtime_setting(show_id), &original)
            .unwrap();
        drop(initial);
        let older = older_contract_zero_runtime();
        let startup =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        drop(older);
        let error = startup.active_show_error.as_deref().unwrap();
        assert!(error.contains("original data is preserved"), "{error}");
        if !malformed {
            assert!(error.contains("programming contract 1"), "{error}");
        }
        assert!(startup.output_runtime.dynamic_runtime.is_none());
        assert_eq!(
            startup
                .persistent
                .desk
                .setting(&output_runtime_setting(show_id))
                .unwrap()
                .unwrap(),
            original
        );
        assert_eq!(preserved_runtime(&data_dir)["serialized"], original);
        drop(startup);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

fn legacy_programmer_dynamic_checkpoint(
    explicit_collision: bool,
) -> (
    SessionId,
    String,
    light_dynamics::DynamicRuntimeSnapshot,
    light_core::ProgrammerId,
    Uuid,
) {
    let programmers =
        ProgrammerRegistry::with_clock(Arc::new(light_core::ManualClock::new(fixed_test_time())));
    let session = SessionId::new();
    let owner = programmers.start(session).id;
    let target = light_core::FixtureId::new();
    let link = Uuid::new_v4();
    let definition = command_test_dynamic(Uuid::new_v4(), 1);
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: target,
            attribute: light_core::AttributeKey::intensity(),
            value: light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link: link,
                dynamic: light_dynamics::DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone()),
                    },
                },
                lane_id: definition.lanes[0].id,
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 0.75,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 45.0,
                },
                timing: Default::default(),
            },
        }],
        None,
    ));
    let programmer_json = serde_json::to_string(&programmers.get(session).unwrap()).unwrap();
    let mut runtime = light_dynamics::DynamicRuntime::with_programming_contract_support(0);
    runtime.install_definitions([definition.clone()]).unwrap();
    let started_at = u64::try_from(fixed_test_time().timestamp_millis()).unwrap() - 1_000;
    for explicit in [false, true]
        .into_iter()
        .take(if explicit_collision { 2 } else { 1 })
    {
        let controller_id = if explicit {
            light_dynamics::programmer_dynamic_controller_id(owner, link)
        } else {
            link
        };
        let instance_id = runtime
            .start(light_dynamics::DynamicStartRequest {
                definition_id: definition.id,
                controller: light_dynamics::DynamicController {
                    id: controller_id,
                    source: light_dynamics::DynamicControllerSource::Programmer {
                        programmer_id: owner.0,
                        instance_link: explicit.then_some(link),
                    },
                    priority: 100,
                    activated_at_millis: started_at,
                    size: 0.75,
                    speed_multiplier: 1.0,
                    phase_offset_degrees: 45.0,
                    paused: false,
                },
                target_scope: light_dynamics::DynamicTargetScope {
                    ordered_targets: vec![target],
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: started_at,
                activation_delay_millis: 17,
                activation_duration_millis: 250,
                activation_policy_override: None,
                reuse_matching_targetless: false,
            })
            .unwrap();
        runtime
            .set_controller_paused(instance_id, controller_id, true, started_at + 500)
            .unwrap();
    }
    (session, programmer_json, runtime.snapshot(), owner, link)
}

#[test]
fn programming_contract_recovery_normalizes_legacy_dynamic_identity_without_restarting_clock() {
    let data_dir = std::env::temp_dir().join(format!("light-dynamic-identity-{}", Uuid::new_v4()));
    let initial = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    assert!(initial.active_show_error.is_none());
    let show_id = initial.persistent.active_show.as_ref().unwrap().id;
    let (session, programmer_json, snapshot, owner, link) =
        legacy_programmer_dynamic_checkpoint(false);
    assert_eq!(snapshot.instances.len(), 1);
    assert!(snapshot.instances[0].paused_at_millis.is_some());
    let original = serde_json::to_string(&PersistedOutputRuntime {
        dynamic_runtime: Some(snapshot.clone()),
        ..Default::default()
    })
    .unwrap();
    assert!(PersistedOutputRuntime::decode_for_support(&original, 0).is_ok());
    initial
        .persistent
        .desk
        .save_session(&light_show::PersistedSession {
            id: session,
            token: "never-copy-auth-token".into(),
            programmer_json: programmer_json.clone(),
            connected: false,
            updated_at: fixed_test_time().to_rfc3339(),
        })
        .unwrap();
    initial
        .persistent
        .desk
        .set_setting(&output_runtime_setting(show_id), &original)
        .unwrap();
    drop(initial);

    let startup = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    assert!(
        startup.active_show_error.is_none(),
        "{:?}",
        startup.active_show_error
    );
    let mut expected = snapshot;
    let scoped = light_dynamics::programmer_dynamic_controller_id(owner, link);
    let instance = &mut expected.instances[0];
    instance.controllers[0].id = scoped;
    instance.controllers[0].source = light_dynamics::DynamicControllerSource::Programmer {
        programmer_id: owner.0,
        instance_link: Some(link),
    };
    for selection in &mut instance.lane_selections {
        selection.controller_id = scoped;
    }
    for transition in &mut instance.controller_transitions {
        transition.controller_id = scoped;
    }
    assert_eq!(
        startup.output_runtime.dynamic_runtime.as_ref(),
        Some(&expected),
        "normalization changes controller identity only, retaining instance, pause, phase and activation clocks"
    );
    let retained = startup.programmers.retained_dynamic_source().unwrap();
    assert_eq!(retained.0, owner.0);
    assert!(matches!(retained.2[0].value,
        light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. } if instance_link == link));
    assert_eq!(
        startup
            .persistent
            .desk
            .setting(&output_runtime_setting(show_id))
            .unwrap()
            .as_deref(),
        Some(original.as_str())
    );
    assert_eq!(
        startup.persistent.desk.persisted_sessions().unwrap()[0].programmer_json,
        programmer_json
    );
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn programming_contract_recovery_preserves_output_when_legacy_dynamic_identity_collides() {
    let data_dir = std::env::temp_dir().join(format!(
        "light-dynamic-identity-collision-{}",
        Uuid::new_v4()
    ));
    let initial = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    assert!(initial.active_show_error.is_none());
    let show_id = initial.persistent.active_show.as_ref().unwrap().id;
    let (session, programmer_json, snapshot, owner, _) = legacy_programmer_dynamic_checkpoint(true);
    assert_eq!(snapshot.instances.len(), 2);
    let original = serde_json::to_string(&PersistedOutputRuntime {
        dynamic_runtime: Some(snapshot),
        ..Default::default()
    })
    .unwrap();
    // This is a valid domain checkpoint. Only the restored authored Programmer link makes
    // the legacy and explicit controllers ambiguous; generic malformed-data recovery is insufficient.
    assert!(PersistedOutputRuntime::decode_for_support(&original, 0).is_ok());
    initial
        .persistent
        .desk
        .save_session(&light_show::PersistedSession {
            id: session,
            token: "never-copy-auth-token".into(),
            programmer_json: programmer_json.clone(),
            connected: false,
            updated_at: fixed_test_time().to_rfc3339(),
        })
        .unwrap();
    initial
        .persistent
        .desk
        .set_setting(&output_runtime_setting(show_id), &original)
        .unwrap();
    drop(initial);

    let startup = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    let error = startup
        .active_show_error
        .as_deref()
        .expect("identity collision is recoverable startup failure");
    assert!(
        error.contains("legacy and explicit controllers claim the same Programmer link"),
        "{error}"
    );
    assert!(error.contains("original data is preserved"), "{error}");
    assert!(startup.output_runtime.dynamic_runtime.is_none());
    assert_eq!(
        startup.programmers.retained_dynamic_source().unwrap().0,
        owner.0
    );
    assert_eq!(
        startup
            .persistent
            .desk
            .setting(&output_runtime_setting(show_id))
            .unwrap()
            .as_deref(),
        Some(original.as_str())
    );
    assert_eq!(
        startup.persistent.desk.persisted_sessions().unwrap()[0].programmer_json,
        programmer_json
    );
    let report = preserved_runtime(&data_dir);
    assert_eq!(report["serialized"], original);
    assert!(!report.to_string().contains("never-copy-auth-token"));
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn startup_preserves_output_when_dynamic_source_catalogue_is_missing_or_dangling() {
    use crate::runtime::dynamic_source_origins::{
        DynamicProgrammerSourceLane, DynamicSourceBinding, DynamicSourceOrigin,
        DynamicSourceOrigins,
    };

    let temporary = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR")
            .expect("repository test runner sets the canonical LIGHT_TMP_DIR"),
    );
    for case in ["missing", "dangling"] {
        let data_dir = temporary.join(format!(
            "light-dynamic-source-recovery-{case}-{}",
            Uuid::new_v4()
        ));
        let initial =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        let show_id = initial.persistent.active_show.as_ref().unwrap().id;
        let original = if case == "missing" {
            let (_, _, mut snapshot, _, _) = legacy_programmer_dynamic_checkpoint(false);
            let instance = &mut snapshot.instances[0];
            instance
                .last_sample_values
                .push(light_dynamics::DynamicHeldSampleSnapshot {
                    controller_id: instance.controllers[0].id,
                    target: instance.targets[0],
                    lane_id: instance.definition.lanes[0].id,
                    payload: light_dynamics::DynamicHeldPayload::Expression {
                        expression: light_dynamics::DynamicSampleExpression::LegacyScalar {
                            attribute: light_core::AttributeKey::intensity(),
                            value: 0.5,
                            occurrence: Some(
                                light_dynamics::DynamicSourceOccurrenceId::new(Uuid::new_v4())
                                    .unwrap(),
                            ),
                            dependency_occurrence: None,
                        },
                    },
                });
            serde_json::to_string(&PersistedOutputRuntime {
                dynamic_runtime: Some(snapshot),
                ..Default::default()
            })
            .unwrap()
        } else {
            let mut catalogue = DynamicSourceOrigins::default();
            let programmer_id = light_core::ProgrammerId::new();
            let instance_link = Uuid::new_v4();
            let binding = DynamicSourceBinding::Authored {
                instance_id: Uuid::new_v4(),
                controller_id: light_dynamics::programmer_dynamic_controller_id(
                    programmer_id,
                    instance_link,
                ),
                target: light_core::FixtureId::new(),
                lane_id: Uuid::new_v4(),
            };
            catalogue
                .bind(
                    binding,
                    DynamicSourceOrigin::Programmer {
                        programmer_id,
                        lane: DynamicProgrammerSourceLane::Live,
                        instance_link,
                        changed_at_millis: 100,
                        programmer_order: 1,
                    },
                )
                .unwrap();
            let mut origins = catalogue.snapshot();
            origins.records.clear();
            serde_json::to_string(&PersistedOutputRuntime {
                dynamic_runtime: Some(Default::default()),
                dynamic_source_origins: Some(origins),
                ..Default::default()
            })
            .unwrap()
        };
        initial
            .persistent
            .desk
            .set_setting(&output_runtime_setting(show_id), &original)
            .unwrap();
        drop(initial);

        let startup =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        let error = startup.active_show_error.as_deref().unwrap();
        assert!(error.contains("original data is preserved"), "{error}");
        assert_eq!(startup.output_runtime.dynamic_runtime, None);
        assert_eq!(preserved_runtime(&data_dir)["serialized"], original);
        assert_eq!(
            startup
                .persistent
                .desk
                .setting(&output_runtime_setting(show_id))
                .unwrap()
                .as_deref(),
            Some(original.as_str())
        );
        drop(startup);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

/// TL-558: a saved Programmer with independent typed Focus and Zoom round-trips exactly through
/// the persisted JSON (Undo/Redo are live-session only by design and are not written). A contract-0 runtime (production before TL-552) keeps
/// the original JSON in recovery instead of reinterpreting it.
#[test]
fn typed_focus_and_zoom_programmer_round_trips_and_production_preserves_it() {
    use light_core::programming::{ProgrammingOwner, ScalarIntent, ZoomIntent};
    let zoom = |degrees| {
        light_core::AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(degrees),
            convention: light_core::OpeningConvention::Beam,
        }))
    };
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let fixture = light_core::FixtureId::new();
    let focus = light_core::AttributeValue::Normalized(0.4);
    programmers.set(
        session,
        fixture,
        ProgrammingOwner::Focus.key(),
        focus.clone(),
    );
    programmers.set(session, fixture, ProgrammingOwner::Zoom.key(), zoom(20.));
    programmers.set(session, fixture, ProgrammingOwner::Zoom.key(), zoom(35.));
    let saved = programmers.get(session).unwrap();
    let json = serde_json::to_string(&saved).unwrap();

    let reopened: light_programmer::ProgrammerState = serde_json::from_str(&json).unwrap();
    reopened.validate_programming().unwrap();
    assert_eq!(reopened.values, saved.values);
    assert!(
        !saved.undo.is_empty() && reopened.undo.is_empty(),
        "Undo is not persisted"
    );
    let value = |state: &light_programmer::ProgrammerState, owner: ProgrammingOwner| {
        state
            .values
            .iter()
            .find(|v| v.fixture_id == fixture && v.attribute == owner.key())
            .map(|v| v.value.clone())
    };
    assert_eq!(value(&reopened, ProgrammingOwner::Zoom), Some(zoom(35.)));
    assert_eq!(value(&reopened, ProgrammingOwner::Focus), Some(focus));
    let restored = ProgrammerRegistry::default();
    restored.restore(reopened);
    let live = restored.get(session).unwrap();
    assert_eq!(value(&live, ProgrammingOwner::Zoom), Some(zoom(35.)));
    assert_eq!(
        value(&live, ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );

    let stored_desk = || {
        let data_dir =
            std::env::temp_dir().join(format!("light-optics-recovery-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
        desk.save_session(&light_show::PersistedSession {
            id: session,
            token: "never-copy-auth-token".into(),
            programmer_json: json.clone(),
            connected: false,
            updated_at: fixed_test_time().to_rfc3339(),
        })
        .unwrap();
        drop(desk);
        data_dir
    };

    // TL-552 production startup (contract 1): the typed Programmer restores exactly.
    let data_dir = stored_desk();
    let startup = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    assert_eq!(startup.active_show_error, None);
    let live = startup.programmers.get(session).unwrap();
    assert_eq!(value(&live, ProgrammingOwner::Zoom), Some(zoom(35.)));
    assert_eq!(
        value(&live, ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();

    // An older contract-0 runtime: preserved byte-for-byte, never reinterpreted.
    let data_dir = stored_desk();
    let older = older_contract_zero_runtime();
    let startup = startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
    drop(older);
    assert!(startup.active_show_error.is_some());
    assert!(startup.programmers.get(session).is_none());
    assert_eq!(preserved_runtime(&data_dir)["serialized"], json);
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();
}

/// Programmer values holding a pre-cutover legacy normalized `pan`.
fn legacy_programmer_values() -> serde_json::Value {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    programmers.set(
        session,
        light_core::FixtureId::new(),
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.5),
    );
    serde_json::to_value(programmers.get(session).unwrap()).unwrap()["values"].clone()
}

/// TL-552 owner decision: Playback and Output runtime restore apply the legacy-key validator at
/// contract 1, exactly like the Programmer. A pre-cutover payload (a Cue hold of normalized
/// `pan`, a checkpointed scalar `pan` Dynamic lane) is never restored: the desk starts in
/// recovery, the original JSON stays in the desk setting and in the recovery report.
#[test]
fn legacy_playback_and_output_runtime_payloads_are_preserved_for_recovery_at_contract_one() {
    for payload in ["playback", "output"] {
        let data_dir = std::env::temp_dir().join(format!(
            "light-legacy-runtime-recovery-{payload}-{}",
            Uuid::new_v4()
        ));
        activate_blank_show(&data_dir);
        let initial =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        assert!(initial.active_show_error.is_none());
        let show_id = initial.persistent.active_show.as_ref().unwrap().id;
        let (setting, original) = if payload == "playback" {
            let playback = serde_json::json!({
                "cue_list_id": Uuid::new_v4(), "cue_index":0, "previous_index":null,
                "paused":false, "activated_at":fixed_test_time(), "paused_at":null,
                "deleted_cue_hold": {"deleted_number":"1", "previous_number":null,
                    "next_number":null, "contributions": legacy_programmer_values()},
            });
            (
                active_playbacks_setting(show_id),
                serde_json::json!([playback]).to_string(),
            )
        } else {
            let definition = command_test_dynamic(Uuid::new_v4(), 1);
            let definition_id = definition.id;
            let mut runtime = light_dynamics::DynamicRuntime::default();
            runtime.install_definitions([definition]).unwrap();
            runtime
                .start(light_dynamics::DynamicStartRequest {
                    definition_id,
                    controller: light_dynamics::DynamicController {
                        id: Uuid::new_v4(),
                        source: light_dynamics::DynamicControllerSource::Programmer {
                            programmer_id: Uuid::new_v4(),
                            instance_link: None,
                        },
                        priority: 0,
                        activated_at_millis: 0,
                        size: 1.0,
                        speed_multiplier: 1.0,
                        phase_offset_degrees: 0.0,
                        paused: false,
                    },
                    target_scope: light_dynamics::DynamicTargetScope {
                        ordered_targets: vec![light_core::FixtureId::new()],
                    },
                    stage_positions: HashMap::new(),
                    inherited_spatial_mapping: None,
                    now_millis: 0,
                    activation_delay_millis: 0,
                    activation_duration_millis: 0,
                    activation_policy_override: None,
                    reuse_matching_targetless: false,
                })
                .unwrap();
            let serialized = serde_json::to_string(&PersistedOutputRuntime {
                dynamic_runtime: Some(runtime.snapshot()),
                ..Default::default()
            })
            .unwrap();
            // As an older desk checkpointed it: the same Dynamic driving a scalar `pan` lane.
            assert!(serialized.contains(r#""attribute":"intensity""#));
            (
                output_runtime_setting(show_id),
                serialized.replace(r#""attribute":"intensity""#, r#""attribute":"pan""#),
            )
        };
        initial
            .persistent
            .desk
            .set_setting(&setting, &original)
            .unwrap();
        drop(initial);
        let startup =
            startup_state::StartupState::load(contract_recovery_options(&data_dir)).unwrap();
        let error = startup.active_show_error.as_deref().unwrap();
        assert!(
            error.contains("original data is preserved"),
            "{payload}: {error}"
        );
        let report = preserved_runtime(&data_dir);
        assert_eq!(report["serialized"], original);
        assert!(
            report
                .to_string()
                .contains("before semantic programming contract 1 (pan)"),
            "{payload}: {report}"
        );
        assert_eq!(
            startup.persistent.desk.setting(&setting).unwrap().unwrap(),
            original
        );
        if payload == "output" {
            assert!(startup.output_runtime.dynamic_runtime.is_none());
        }
        drop(startup);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}
