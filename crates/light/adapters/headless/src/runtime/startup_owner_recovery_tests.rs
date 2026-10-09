//! Real startup restore/resources/final-owner boundary, before rendering starts.
use super::*;
use crate::runtime::{
    PersistedOutputRuntime, active_playbacks_setting, output_runtime_setting, startup_options,
    startup_state,
};
use light_core::{AttributeKey, FixtureId, SessionId};
use light_dynamics::*;
use light_programmer::ProgrammerRegistry;
use std::path::{Path, PathBuf};

fn options(path: &Path) -> startup_options::StartupOptions {
    startup_options::StartupOptions {
        data_dir: path.into(),
        show_file: None,
        fixture_package_dir: None,
        extensions_dir: Some(path.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: Some("127.0.0.1:0".parse().unwrap()),
        output_bind_override: Some("127.0.0.1".parse().unwrap()),
    }
}
fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").expect("canonical test temporary directory"))
        .join(format!("startup-owner-recovery-{}", Uuid::new_v4()))
}
struct Saved {
    dir: PathBuf,
    session: SessionId,
    show: light_core::ShowId,
    programmer: String,
    playback: String,
    output: String,
}
fn seed(invalid: bool, missing_group: bool, random: bool) -> Saved {
    let dir = directory();
    let initial = StartupState::load(options(&dir)).unwrap();
    let show = initial.persistent.active_show.as_ref().unwrap().id;
    let registry = ProgrammerRegistry::with_clock(Arc::new(light_core::ManualClock::new(
        crate::runtime::fixed_test_time(),
    )));
    let session = SessionId::new();
    let mut programmer = registry.start(session);
    let mut definition = definition();
    if missing_group {
        definition.target_binding = DynamicTargetBinding::LiveGroup {
            group_id: "removed-group".into(),
        };
    }
    if random {
        let id = Uuid::new_v4();
        definition.lanes[0].legacy_mut().unwrap().mode = DynamicLaneMode::Random;
        definition.lanes[0].random_group_id = Some(id);
        definition.random_groups.push(DynamicRandomGroup {
            id,
            seed: 17,
            range: DynamicRandomRange::LegacyScalar {
                low: ScalarSource::Value { value: 0. },
                high: ScalarSource::Value { value: 1. },
            },
            decision_interval_millis: 100,
            start_probability: 1.,
            mean_duration_millis: 250,
            duration_spread_millis: 30,
            attack_ratio: 0.1,
            decay_ratio: 0.5,
        });
    }
    programmer.dynamic_values = Arc::new(vec![DynamicAddressValue {
        fixture_id: FixtureId::new(),
        attribute: AttributeKey::intensity(),
        value: DynamicSemanticValue::DynamicOn {
            instance_link: Uuid::new_v4(),
            dynamic: DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: 1,
                embedded_fallback: DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            lane_id: definition.lanes[0].id,
            overrides: DynamicInstanceOverrides {
                size: if invalid { -1. } else { 1. },
                speed_multiplier: Rational::ONE,
                phase_offset_degrees: 0.,
            },
            timing: Default::default(),
        },
        programmer_order: 1,
        changed_at_millis: crate::runtime::fixed_test_time().timestamp_millis() as u64,
    }]);
    assert_eq!(programmer.required_programming_contract(), 0);
    programmer.validate_programming().unwrap(); // strict owner validation must catch invalid overrides later
    let programmer = format!("  {}\n", serde_json::to_string_pretty(&programmer).unwrap());
    initial
        .persistent
        .desk
        .save_session(&light_show::PersistedSession {
            id: session,
            token: "AUTH-MUST-NOT-ENTER-RECOVERY-REPORT".into(),
            programmer_json: programmer.clone(),
            connected: false,
            updated_at: crate::runtime::fixed_test_time().to_rfc3339(),
        })
        .unwrap();
    // Legacy transition ordinal omission requires normalization. A direct Cuelist avoids
    // coupling the test to a particular saved physical/virtual Playback assignment.
    let cue_list = light_core::CueListId::new();
    let cue = light_playback::CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
        id: cue_list,
        name: "Startup owner recovery".into(),
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
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.,
        cues: vec![light_playback::Cue::new("1".parse().unwrap())],
    };
    light_show::ShowStore::open(&initial.persistent.active_show.as_ref().unwrap().path)
        .unwrap()
        .put_object(
            "cue_list",
            &cue_list.0.to_string(),
            &serde_json::to_value(cue).unwrap(),
            0,
        )
        .unwrap();
    let playback = format!(
        " {}\n",
        serde_json::json!([{
            "cue_list_id": cue_list, "cue_index": 0, "previous_index": null,
            "paused": true, "activated_at": crate::runtime::fixed_test_time(),
            "paused_at": crate::runtime::fixed_test_time()
        }])
    );
    let output = format!(
        "{}\n",
        serde_json::to_string_pretty(&PersistedOutputRuntime {
            grand_master: 0.61,
            blackout: true,
            ..Default::default()
        })
        .unwrap()
    );
    initial
        .persistent
        .desk
        .set_setting(&active_playbacks_setting(show), &playback)
        .unwrap();
    initial
        .persistent
        .desk
        .set_setting(&output_runtime_setting(show), &output)
        .unwrap();
    drop(initial);
    Saved {
        dir,
        session,
        show,
        programmer,
        playback,
        output,
    }
}
async fn loaded(
    saved: &Saved,
) -> (
    AppState,
    RuntimeResources,
    startup_state::OriginalStartupOwners,
) {
    let mut startup = StartupState::load(options(&saved.dir)).unwrap();
    assert!(
        startup.active_show_error.is_none(),
        "{:?}",
        startup.active_show_error
    );
    let originals = startup_state::OriginalStartupOwners::capture(&startup.persistent).unwrap();
    let resources = RuntimeResources::start(&mut startup).await.unwrap();
    let state = build_app_state(startup, &resources).unwrap();
    (state, resources, originals)
}
async fn close(state: &AppState, resources: RuntimeResources) {
    // These tests intentionally stop before supervisors take ownership of the queue.
    discard_unstarted_runtime(state, resources).await;
}
fn report(saved: &Saved) -> serde_json::Value {
    let files: Vec<_> = std::fs::read_dir(saved.dir.join("backups"))
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("runtime-recovery-")
        })
        .collect();
    assert_eq!(files.len(), 1);
    let raw = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(!raw.contains("AUTH-MUST-NOT-ENTER-RECOVERY-REPORT"));
    let report: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(report["kind"], "Dynamic owners");
    serde_json::from_str(report["serialized"].as_str().unwrap()).unwrap()
}
fn stored_unchanged(state: &AppState, saved: &Saved) {
    assert_eq!(
        state
            .installation
            .setting(&active_playbacks_setting(saved.show))
            .unwrap()
            .as_deref(),
        Some(saved.playback.as_str())
    );
    assert_eq!(
        state
            .installation
            .setting(&output_runtime_setting(saved.show))
            .unwrap()
            .as_deref(),
        Some(saved.output.as_str())
    );
    assert_eq!(
        state
            .installation
            .persisted_sessions()
            .unwrap()
            .iter()
            .find(|s| s.id == saved.session)
            .unwrap()
            .programmer_json,
        saved.programmer
    );
}

#[tokio::test]
async fn malformed_saved_owner_recovers_after_real_restore_without_overwriting_sources() {
    let saved = seed(true, false, false);
    let (state, resources, originals) = loaded(&saved).await;
    assert!(
        !state.output.playback_runtime().is_empty(),
        "normalization must exercise a restored row"
    );
    assert!(
        state
            .output
            .playback_runtime()
            .iter()
            .any(|p| p.transition_ordinal == 0)
    );
    startup_state::finalize_restored_owners_for_startup(&state, originals).unwrap();
    assert!(
        state
            .active_show
            .error()
            .unwrap()
            .contains("original data is preserved")
    );
    assert!(state.programming.get(saved.session).is_none());
    assert!(state.output.snapshot().fixtures.is_empty());
    assert!(state.output.playback_runtime().is_empty());
    assert!(state.output.dynamic_runtime_snapshot().instances.is_empty());
    assert_eq!(state.output.control_projection().grand_master, 1.);
    assert!(!state.output.control_projection().blackout);
    let original = report(&saved);
    assert_eq!(
        original["programmers"][0]["session_id"],
        serde_json::to_value(saved.session).unwrap()
    );
    assert_eq!(
        original["programmers"][0]["programmer_json"],
        saved.programmer
    );
    assert_eq!(original["playback"], saved.playback);
    assert_eq!(original["output"], saved.output);
    state.output.reconcile_dynamic_runtime();
    assert!(state.output.dynamic_runtime_snapshot().instances.is_empty());
    state
        .output
        .apply_runtime_control(Some(0.2), Some(true))
        .unwrap();
    crate::runtime::persist_output_runtime(&state).unwrap();
    crate::runtime::persist_active_playbacks(&state).unwrap();
    stored_unchanged(&state, &saved);
    close(&state, resources).await;
    drop(state);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

#[tokio::test]
async fn recovery_report_io_error_does_not_commit_empty_state_or_checkpoint() {
    let saved = seed(true, false, false);
    let (state, resources, originals) = loaded(&saved).await;
    let before = state.output.snapshot();
    let backups = saved.dir.join("backups");
    if backups.exists() {
        std::fs::rename(&backups, saved.dir.join("prior-backups")).unwrap();
    }
    std::fs::write(&backups, b"blocked report destination").unwrap();
    assert!(startup_state::finalize_restored_owners_for_startup(&state, originals).is_err());
    assert!(state.active_show.error().is_none());
    assert!(Arc::ptr_eq(&before, &state.output.snapshot()));
    assert!(state.programming.get(saved.session).is_some());
    stored_unchanged(&state, &saved);
    close(&state, resources).await;
    drop(state);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

struct Current;
impl ScalarSourceResolver for Current {
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.37)
    }
}
#[tokio::test]
async fn valid_random_paused_and_held_checkpoint_survives_real_startup_finalization() {
    let mut saved = seed(false, false, true);
    let programmer: light_programmer::ProgrammerState =
        serde_json::from_str(&saved.programmer).unwrap();
    let row = &programmer.dynamic_values[0];
    let DynamicSemanticValue::DynamicOn {
        dynamic,
        instance_link,
        lane_id,
        ..
    } = &row.value
    else {
        panic!("saved fixture has an On owner")
    };
    let definition = dynamic.embedded_fallback.definition.clone();
    let now = row.changed_at_millis;
    let controller_id = programmer_dynamic_controller_id(programmer.id, *instance_link);
    let mut runtime = DynamicRuntime::with_programming_contract_support(0);
    runtime
        .install_definitions([(*definition).clone()])
        .unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: controller_id,
                source: DynamicControllerSource::Programmer {
                    programmer_id: programmer.id.0,
                    instance_link: Some(*instance_link),
                },
                priority: programmer.priority,
                activated_at_millis: now,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![row.fixture_id],
            },
            stage_positions: Default::default(),
            inherited_spatial_mapping: None,
            now_millis: now,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            controller_id,
            DynamicLaneSelection::for_recorded_values(
                dynamic,
                &definition,
                &[(row.fixture_id, *lane_id)],
            ),
        )
        .unwrap();
    let transport = [DynamicSpeedTransport {
        effective_bpm: 120.,
        phase_origin_millis: 0,
        phase_reference_millis: now,
        beat_phase: 0.,
        phase_advancing: true,
    }; 5];
    runtime.sample_all_addressed(now + 250, 25, &transport, &Current, None);
    runtime.set_global_paused(true, now + 250);
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.instances.len(), 1);
    assert!(!snapshot.instances[0].random_streams.is_empty());
    assert!(!snapshot.instances[0].last_sample_values.is_empty());
    assert!(snapshot.global_paused);
    saved.output = serde_json::to_string(&PersistedOutputRuntime {
        dynamic_runtime: Some(snapshot.clone()),
        dynamics_paused_at: Some(
            crate::runtime::fixed_test_time() + chrono::Duration::milliseconds(250),
        ),
        ..Default::default()
    })
    .unwrap();
    assert!(PersistedOutputRuntime::decode_for_support(&saved.output, 0).is_ok());
    let desk = light_show::DeskStore::open(saved.dir.join("desk.sqlite")).unwrap();
    desk.set_setting(&output_runtime_setting(saved.show), &saved.output)
        .unwrap();
    drop(desk);

    // The first real startup receives an actual saved checkpoint. No synthetic connection
    // or earlier finalizer invocation is used to create this history.
    let mut startup = StartupState::load(options(&saved.dir)).unwrap();
    assert!(startup.active_show_error.is_none());
    let speed_groups = Arc::clone(&startup.speed_groups);
    let originals = startup_state::OriginalStartupOwners::capture(&startup.persistent).unwrap();
    let resources = RuntimeResources::start(&mut startup).await.unwrap();
    let state = build_app_state(startup, &resources).unwrap();
    assert!(state.output.engine().dynamic_programmer_values().is_empty());
    assert_eq!(state.output.dynamic_runtime_snapshot(), snapshot);
    startup_state::finalize_restored_owners_for_startup(&state, originals).unwrap();
    assert!(state.active_show.error().is_none());
    assert_eq!(state.output.dynamic_runtime_snapshot(), snapshot);
    assert!(state.programming.get(saved.session).is_some());
    assert!(
        state.output.engine().dynamic_programmer_values().is_empty(),
        "cold retained-owner validation must not connect or activate the saved Programmer"
    );
    // The first ordinary disconnected tick obeys existing output membership rules.
    // Cold validation preserved history above; it did not make this owner active.
    let (contributions, samples) = crate::runtime::output_scheduler::dynamic_projection(
        state.output.engine(),
        &resources.dynamics,
        &speed_groups,
        &AtomicU16::new(40),
        &[],
    );
    assert!(
        contributions.is_empty(),
        "disconnected Programmer emits no Dynamic contribution"
    );
    assert!(samples.is_empty());
    assert!(
        state.output.dynamic_runtime_snapshot().instances.is_empty(),
        "the ordinary disconnected tick retires the retained Programmer controller"
    );
    assert!(state.output.engine().dynamic_programmer_values().is_empty());
    close(&state, resources).await;
    drop(state);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

#[tokio::test]
async fn missing_saved_group_is_passive_through_real_startup() {
    let saved = seed(false, true, false);
    let (state, resources, originals) = loaded(&saved).await;
    startup_state::finalize_restored_owners_for_startup(&state, originals).unwrap();
    assert!(state.active_show.error().is_none());
    assert!(state.programming.get(saved.session).is_some());
    assert!(!state.output.snapshot().fixtures.is_empty());
    close(&state, resources).await;
    drop(state);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

#[tokio::test]
async fn malformed_saved_owner_does_not_block_running_server_start() {
    let saved = seed(true, false, false);
    let running = RunningServer::start(StartupState::load(options(&saved.dir)).unwrap())
        .await
        .unwrap();
    report(&saved);
    running.supervisors.shutdown().await.unwrap();
    drop(running.app);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Restored owners",
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "current"}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "current"}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

#[tokio::test]
async fn running_server_report_failure_drops_queued_state_and_unstarted_scheduler() {
    let saved = seed(true, false, false);
    let startup = StartupState::load(options(&saved.dir)).unwrap();
    assert!(startup.active_show_error.is_none());
    let engine = Arc::downgrade(&startup.engine);
    let dynamics = Arc::downgrade(&startup.dynamics);
    // Block only final-owner report writing, after normal source loading succeeds.
    let backups = saved.dir.join("backups");
    if backups.exists() {
        std::fs::rename(&backups, saved.dir.join("prior-backups")).unwrap();
    }
    std::fs::write(&backups, b"blocked report destination").unwrap();
    let result = RunningServer::start(startup).await;
    assert!(
        result.is_err(),
        "failed evidence preservation remains a startup I/O error"
    );
    drop(result);
    // Real build_app_state queues extension feedback. Its unpolled future captures
    // AppState -> LifecycleResource -> its own receiver. Cancellation alone cannot
    // release these weak owners; the queue itself must be dropped.
    assert!(
        engine.upgrade().is_none(),
        "queued startup AppState retained the Engine"
    );
    assert!(
        dynamics.upgrade().is_none(),
        "startup scheduler or queued state retained Dynamics"
    );
    let desk = light_show::DeskStore::open(saved.dir.join("desk.sqlite")).unwrap();
    assert_eq!(
        desk.setting(&active_playbacks_setting(saved.show))
            .unwrap()
            .as_deref(),
        Some(saved.playback.as_str())
    );
    assert_eq!(
        desk.setting(&output_runtime_setting(saved.show))
            .unwrap()
            .as_deref(),
        Some(saved.output.as_str())
    );
    assert_eq!(
        desk.persisted_sessions()
            .unwrap()
            .into_iter()
            .find(|s| s.id == saved.session)
            .unwrap()
            .programmer_json,
        saved.programmer
    );
    drop(desk);
    std::fs::remove_dir_all(saved.dir).unwrap();
}

#[tokio::test]
async fn visualizer_preview_real_startup_publishes_frames_without_any_delivery_path() {
    struct CancelOnDrop(CancellationToken);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.cancel();
        }
    }
    let directory = directory();
    let mut preview_options = options(&directory);
    preview_options.visualizer_preview = true;
    preview_options.test_bench = false;
    let startup = StartupState::load(preview_options).unwrap();
    let originals = startup_state::OriginalStartupOwners::capture(&startup.persistent).unwrap();
    let ServedStartupForTests {
        state,
        mut resources,
    } = ServedStartupForTests::start(startup).await;
    startup_state::finalize_restored_owners_for_startup(&state, originals).unwrap();
    let art = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let sacn = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let routes = [
        (light_output::Protocol::ArtNet, art.local_addr().unwrap()),
        (light_output::Protocol::Sacn, sacn.local_addr().unwrap()),
    ]
    .map(|(protocol, destination)| light_output::OutputRoute {
        target: Default::default(),
        protocol,
        logical_universe: 1,
        destination_universe: 1,
        delivery_mode: Some(light_output::DeliveryMode::Unicast),
        destination: Some(destination),
        enabled: true,
        minimum_slots: 512,
    });
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.routes = Arc::new(routes.to_vec());
    state.output.replace_snapshot(snapshot).unwrap();
    let _cancel_on_drop = CancelOnDrop(resources.cancellation.clone());
    resources.scheduler.start_rendering().unwrap();
    let supervisors = CapabilitySupervisors::start(
        resources.cancellation,
        resources.output_cancellation,
        resources.scheduler,
        &state,
    );
    assert!(!state.output.permits_external_delivery());
    let preview_tasks = supervisors.runtime_task_count();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Some(frame) = state.output.latest_visualization_frame() {
                break frame;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(frame.source_snapshot.routes.as_ref(), &routes);
    assert!(
        !frame.source_snapshot.fixtures.is_empty(),
        "supported default show fixtures must remain available to the visualizer"
    );
    assert!(
        frame.sequence > 0,
        "ordinary scheduler must keep publishing semantic preview frames"
    );
    assert_eq!(
        state
            .output
            .send_network_routes(
                &routes,
                &HashMap::from([(1, [77; 512])]),
                &HashMap::from([(1, 512)])
            )
            .await
            .unwrap(),
        0
    );
    assert_eq!(state.output.send_retained_output().await.unwrap(), 0);
    state.output.terminate_routes(&routes).await;
    supervisors.shutdown().await.unwrap();
    drop(state);
    let mut packet = [0; 1200];
    for receiver in [&art, &sacn] {
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(30),
                receiver.recv_from(&mut packet)
            )
            .await
            .is_err(),
            "neither ordinary, retained, explicit, safe-shutdown nor termination delivery may escape preview authority"
        );
    }
    let live_directory = directory.join("normal-live");
    let live = StartupState::load(options(&live_directory)).unwrap();
    let ServedStartupForTests {
        state: live_state,
        mut resources,
    } = ServedStartupForTests::start(live).await;
    assert!(live_state.output.permits_external_delivery());
    let mut live_snapshot = (*live_state.output.snapshot()).clone();
    live_snapshot.fixtures = Arc::new(
        live_snapshot
            .fixtures
            .iter()
            .cloned()
            .map(|mut fixture| {
                fixture.direct_control = None;
                fixture
            })
            .collect(),
    );
    live_snapshot.routes = Arc::new(Vec::new());
    live_state.output.replace_snapshot(live_snapshot).unwrap();
    resources.scheduler.start_rendering().unwrap();
    let live_supervisors = CapabilitySupervisors::start(
        resources.cancellation,
        resources.output_cancellation,
        resources.scheduler,
        &live_state,
    );
    assert_eq!(
        live_supervisors.runtime_task_count(),
        preview_tasks + 2,
        "Live must retain both media identity and Speed Group publisher tasks, preview must spawn neither"
    );
    live_supervisors.shutdown().await.unwrap();
    drop(live_state);
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn visualizer_preview_http_refuses_external_media_writes_but_keeps_reads_and_live_policy() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let root = directory();
    for preview in [true, false] {
        let mut startup_options = options(&root.join(if preview { "preview" } else { "live" }));
        startup_options.visualizer_preview = preview;
        let served =
            ServedStartupForTests::start(StartupState::load(startup_options).unwrap()).await;
        let app = router(served.state.clone());
        let login = app
            .clone()
            .oneshot(
                Request::post("/api/v2/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"Operator"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login.status(), StatusCode::OK);
        let bytes = login.into_body().collect().await.unwrap().to_bytes();
        let login: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = login["token"].as_str().unwrap();
        // No endpoint lookup is allowed for a preview write, even when the target is missing.
        // A normal desk must keep its ordinary missing-fixture response instead of this refusal.
        let fixture = FixtureId::new().0;
        for (suffix, body) in [
            (
                "layers/0/effects/update",
                serde_json::json!({"request_id":"isolated-effect", "control_id":"opacity", "number_value":1}),
            ),
            (
                "text/1/1/update",
                serde_json::json!({"request_id":"isolated-text", "text":"isolated test"}),
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(format!("/api/v2/media-servers/{fixture}/native/{suffix}"))
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if preview {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::NOT_FOUND
                }
            );
            if preview {
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                assert!(body["error"].as_str().unwrap().contains("Light controller"));
            }
        }
        let read = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v2/media-servers/{fixture}/native"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            read.status(),
            StatusCode::NOT_FOUND,
            "read/query paths must remain available under preview policy"
        );
        drop(app);
        served.close().await;
    }
    std::fs::remove_dir_all(root).unwrap();
}
