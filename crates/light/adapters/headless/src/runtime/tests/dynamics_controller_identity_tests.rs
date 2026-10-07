//! Programmer, Preload and external control surfaces share one authored Dynamic identity.
use super::*;
use light_application::{DynamicControllerUpdate, DynamicOffCommand, DynamicStartCommand};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_dynamics::{DynamicControllerSource, DynamicInstanceSnapshot, DynamicSemanticValue};

struct IdentityDesk {
    state: AppState,
    session: Session,
    clock: Arc<ManualClock>,
    data_dir: PathBuf,
    fixture: FixtureId,
    dynamic_id: Uuid,
}

impl IdentityDesk {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(fixed_test_time()));
        let (state, data_dir) = test_state_with_clock(clock.clone());
        let session = Session {
            capability: light_core::SurfaceCapability::Programming,
            id: SessionId::new(),
            token: "dynamic-controller-identity".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programming.start(session.id);
        state.sessions.insert_session(session.clone());
        attach_session_command_context(&state, &session);
        let fixture = FixtureId::new();
        let dynamic_id = Uuid::new_v4();
        state.programming.select(session.id, [fixture]);
        state
            .output
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![operational_fixture(fixture)].into(),
                dynamics: vec![command_test_dynamic(dynamic_id, 1)].into(),
                ..Default::default()
            })
            .unwrap();
        let path = data_dir.join("shows/dynamic-controller-identity.show");
        let store = light_show::ShowStore::create(&path, "Dynamic controller identity")
            .unwrap()
            .0;
        let show = state
            .installation
            .upsert_show(
                "Dynamic controller identity",
                &path.display().to_string(),
                false,
            )
            .unwrap();
        store.set_identity(show.id, &show.name, None).unwrap();
        state.active_show.replace_current(Some(show));
        Self {
            state,
            session,
            clock,
            data_dir,
            fixture,
            dynamic_id,
        }
    }

    fn ports(&self) -> crate::runtime::dynamics_adapter::ServerDynamicsPorts<'_> {
        crate::runtime::dynamics_adapter::ServerDynamicsPorts {
            state: &self.state,
            session: &self.session,
        }
    }

    fn start(&self) -> light_application::DynamicStartOutcome {
        self.state
            .dynamics
            .start(
                &operator_action_context(&self.session, light_application::ActionSource::Http),
                DynamicStartCommand {
                    dynamic_id: self.dynamic_id,
                    targets: vec![self.fixture],
                    overrides: light_dynamics::DynamicInstanceOverrides {
                        size: 1.0,
                        speed_multiplier: light_dynamics::Rational::ONE,
                        phase_offset_degrees: 0.0,
                    },
                    timing: Default::default(),
                    undo_group: None,
                },
                &self.ports(),
            )
            .unwrap()
    }

    fn update_size(&self, controller_id: Uuid, size: f32) {
        self.state
            .dynamics
            .update_controller(
                &operator_action_context(&self.session, light_application::ActionSource::Http),
                DynamicControllerUpdate {
                    controller_id,
                    size: Some(size),
                    speed_multiplier: None,
                    phase_offset_degrees: None,
                    undo_group: None,
                },
                &self.ports(),
            )
            .unwrap();
    }

    fn one_instance(&self) -> DynamicInstanceSnapshot {
        let runtime = self.state.output.dynamic_runtime_snapshot();
        assert_eq!(runtime.instances.len(), 1);
        let instance = runtime.instances.into_iter().next().unwrap();
        assert_eq!(instance.controllers.len(), 1);
        instance
    }

    fn raw_link(&self, pending: bool) -> Uuid {
        let programmer = self.state.programming.get(self.session.id).unwrap();
        let rows = if pending {
            &programmer.preload_dynamic_pending
        } else {
            &programmer.dynamic_values
        };
        assert_eq!(rows.len(), 1);
        let DynamicSemanticValue::DynamicOn { instance_link, .. } = rows[0].value else {
            panic!("test must retain the authored DynamicOn row");
        };
        instance_link
    }

    fn projected(
        &self,
    ) -> (
        light_engine::ResolvedValues,
        light_dynamics::DynamicRuntimeSnapshot,
    ) {
        let programmer = self.state.programming.get(self.session.id).unwrap();
        let pending = programmer
            .preload_dynamic_pending
            .iter()
            .cloned()
            .map(|value| (programmer.id.0, programmer.priority, value))
            .collect::<Vec<_>>();
        let (values, runtime, _) = self
            .state
            .output
            .visualization_dynamic_projection(&pending, true);
        (values, runtime)
    }

    fn intensity(&self, values: &light_engine::ResolvedValues) -> f32 {
        let Some(AttributeValue::Normalized(value)) =
            values.get(&(self.fixture, AttributeKey::intensity()))
        else {
            panic!("expected a sampled Intensity value");
        };
        *value
    }

    fn assert_identity(
        &self,
        instance: &DynamicInstanceSnapshot,
        raw_link: Uuid,
        controller_id: Uuid,
    ) {
        let programmer = self.state.programming.get(self.session.id).unwrap();
        assert_eq!(
            controller_id,
            light_dynamics::programmer_dynamic_controller_id(programmer.id, raw_link)
        );
        assert_ne!(
            controller_id, raw_link,
            "the stored link must not become the runtime identity"
        );
        assert_eq!(instance.controllers[0].id, controller_id);
        assert_eq!(
            instance.controllers[0].source,
            DynamicControllerSource::Programmer {
                programmer_id: programmer.id.0,
                instance_link: Some(raw_link),
            }
        );
    }
}

impl Drop for IdentityDesk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

#[test]
fn immediate_start_and_scheduler_reconciliation_preserve_controller_instance_and_clock() {
    let desk = IdentityDesk::new();
    let started = desk.start();
    let raw_link = desk.raw_link(false);
    let immediate = desk.one_instance();
    assert_eq!(immediate.id, started.runtime_instance_id);
    desk.assert_identity(&immediate, raw_link, started.controller_id);

    desk.clock.advance_millis(250);
    desk.state.output.dynamic_contributions_for_test();
    let reconciled = desk.one_instance();
    desk.assert_identity(&reconciled, raw_link, started.controller_id);
    assert_eq!(reconciled.id, immediate.id);
    assert_eq!(reconciled.started_at_millis, immediate.started_at_millis);
    assert_eq!(
        reconciled.controllers[0].activated_at_millis,
        immediate.controllers[0].activated_at_millis
    );
    let values = desk.state.output.visualization_dynamic_values(&[], false);
    assert!(
        (desk.intensity(&values) - 0.5).abs() < 0.0001,
        "reconciliation must not restart at the first keyframe"
    );

    desk.clock.advance_millis(125);
    desk.state.output.dynamic_contributions_for_test();
    assert_eq!(desk.one_instance().id, immediate.id);
    let values = desk.state.output.visualization_dynamic_values(&[], false);
    assert!((desk.intensity(&values) - 0.625).abs() < 0.0001);
    assert_eq!(desk.raw_link(false), raw_link);
}

#[test]
fn restoring_a_verified_legacy_programmer_key_preserves_the_live_dynamic_clock() {
    let desk = IdentityDesk::new();
    let started = desk.start();
    let raw_link = desk.raw_link(false);
    desk.clock.advance_millis(250);
    desk.state.output.dynamic_contributions_for_test();
    let before = desk.state.output.dynamic_runtime_snapshot();
    let mut legacy = before.clone();
    let instance = &mut legacy.instances[0];
    instance.controllers[0].id = raw_link;
    let DynamicControllerSource::Programmer { instance_link, .. } =
        &mut instance.controllers[0].source
    else {
        panic!("Programmer controller");
    };
    *instance_link = None;
    for selection in &mut instance.lane_selections {
        selection.controller_id = raw_link;
    }
    for transition in &mut instance.controller_transitions {
        transition.controller_id = raw_link;
    }
    for sample in instance
        .last_sample_values
        .iter_mut()
        .chain(instance.synchronized_hold_values.iter_mut())
    {
        sample.controller_id = raw_link;
    }
    let show_id = desk.state.active_show.current().as_ref().unwrap().id;
    restore_output_runtime_for_show(
        &desk.state,
        show_id,
        PersistedOutputRuntime {
            dynamic_runtime: Some(legacy),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(desk.state.output.dynamic_runtime_snapshot(), before);
    desk.clock.advance_millis(125);
    desk.state.output.dynamic_contributions_for_test();
    let restored = desk.one_instance();
    desk.assert_identity(&restored, raw_link, started.controller_id);
    assert_eq!(restored.id, before.instances[0].id);
    assert_eq!(
        restored.started_at_millis,
        before.instances[0].started_at_millis
    );
    let values = desk.state.output.visualization_dynamic_values(&[], false);
    assert!((desk.intensity(&values) - 0.625).abs() < 0.0001);
}

#[test]
fn pending_size_uses_same_controller_only_in_preview_then_go_keeps_live_phase() {
    let desk = IdentityDesk::new();
    // Native channel defaults are applied after semantic composition; they do not supply
    // Dynamic Current. Size scales around an actual static Programmer/Cue value.
    assert!(
        !desk
            .state
            .output
            .resolved_values()
            .contains_key(&(desk.fixture, AttributeKey::intensity())),
        "the fixture's native default must not stand in for an authored Current value"
    );
    desk.state.programming.set(
        desk.session.id,
        desk.fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    );
    assert_eq!(
        desk.state
            .output
            .resolved_values()
            .get(&(desk.fixture, AttributeKey::intensity())),
        Some(&AttributeValue::Normalized(0.0))
    );
    // Keep the setup underlay older than the Dynamic; this test checks controller identity and
    // phase, independently of the legacy static/Dynamic equal-timestamp arbitration rule.
    desk.clock.advance_millis(1);
    let started = desk.start();
    let raw_link = desk.raw_link(false);
    desk.clock.advance_millis(250);
    desk.state.output.dynamic_contributions_for_test();
    let before = desk.state.output.dynamic_runtime_snapshot();
    assert!(desk.state.programming.arm_preload(desk.session.id, true));
    desk.update_size(started.controller_id, 0.4);
    assert_eq!(desk.raw_link(true), raw_link);
    assert_eq!(desk.state.output.dynamic_runtime_snapshot(), before);

    let (values, preview) = desk.projected();
    assert_eq!(preview.instances.len(), 1);
    desk.assert_identity(&preview.instances[0], raw_link, started.controller_id);
    assert_eq!(preview.instances[0].id, before.instances[0].id);
    assert_eq!(
        preview.instances[0].started_at_millis,
        before.instances[0].started_at_millis
    );
    assert_eq!(preview.instances[0].controllers[0].size, 0.4);
    assert!(
        (desk.intensity(&values) - 0.2).abs() < 0.0001,
        "preview Size 0.4 at phase 0.25: expected 0.2 from static Current 0.0, got {:?}",
        values.get(&(desk.fixture, AttributeKey::intensity()))
    );
    assert_eq!(
        desk.state.output.dynamic_runtime_snapshot(),
        before,
        "preview sampling is observational"
    );
    let live = desk.state.output.visualization_dynamic_values(&[], false);
    assert!(
        (desk.intensity(&live) - 0.5).abs() < 0.0001,
        "Live must keep Size1 while Size0.4 is pending: got {:?}; controller Size {}",
        live.get(&(desk.fixture, AttributeKey::intensity())),
        desk.one_instance().controllers[0].size
    );

    desk.clock.advance_millis(125);
    commit_preload(&desk.state, &desk.session).unwrap();
    let committed = desk.one_instance();
    desk.assert_identity(&committed, raw_link, started.controller_id);
    assert_eq!(committed.id, before.instances[0].id);
    assert_eq!(
        committed.started_at_millis,
        before.instances[0].started_at_millis
    );
    assert_eq!(committed.controllers[0].size, 0.4);
    let live = desk.state.output.visualization_dynamic_values(&[], false);
    assert!(
        (desk.intensity(&live) - 0.25).abs() < 0.0001,
        "GO changes Size at the running phase: expected 0.25, got {:?}",
        live.get(&(desk.fixture, AttributeKey::intensity()))
    );

    desk.state.programming.arm_preload(desk.session.id, false);
    desk.update_size(raw_link, 0.6);
    desk.state.output.dynamic_contributions_for_test();
    let edited = desk.one_instance();
    assert_eq!(edited.id, committed.id);
    assert_eq!(edited.started_at_millis, committed.started_at_millis);
    assert_eq!(
        edited.controllers[0].size, 0.6,
        "raw-link edits must address the same committed controller"
    );
}

#[test]
fn staged_off_preview_and_go_mask_the_controller_until_preload_release() {
    let desk = IdentityDesk::new();
    let started = desk.start();
    let raw_link = desk.raw_link(false);
    desk.clock.advance_millis(250);
    desk.state.output.dynamic_contributions_for_test();
    let before = desk.state.output.dynamic_runtime_snapshot();
    assert!(desk.state.programming.arm_preload(desk.session.id, true));
    let stopped = desk
        .state
        .dynamics
        .off(
            &operator_action_context(&desk.session, light_application::ActionSource::Http),
            DynamicOffCommand {
                controller_id: raw_link,
                timing: Default::default(),
            },
            &desk.ports(),
        )
        .unwrap();
    assert_eq!(stopped.controller_id, started.controller_id);
    assert!(!stopped.started);
    assert_eq!(desk.state.output.dynamic_runtime_snapshot(), before);
    let (_, preview) = desk.projected();
    assert!(
        preview.instances[0].controller_transitions[0]
            .output_gate
            .is_some_and(|gate| gate.to == 0.0),
        "pending Off masks that source only in the preview branch"
    );
    assert_eq!(desk.state.output.dynamic_runtime_snapshot(), before);
    let live = desk.state.output.visualization_dynamic_values(&[], false);
    assert!((desk.intensity(&live) - 0.5).abs() < 0.0001);

    commit_preload(&desk.state, &desk.session).unwrap();
    desk.state.output.dynamic_contributions_for_test();
    let muted = desk.one_instance();
    assert_eq!(muted.id, started.runtime_instance_id);
    assert_eq!(
        muted.started_at_millis,
        before.instances[0].started_at_millis
    );
    assert!(
        muted.controller_transitions[0]
            .output_gate
            .is_some_and(|gate| gate.to == 0.0)
    );
    let programmer = desk.state.programming.get(desk.session.id).unwrap();
    assert!(light_application::effective_programmer_dynamic_controllers(&programmer).is_empty());

    // Several complete cycles still sample the retained controller. Releasing Preload reveals
    // its continuing phase, rather than starting a new instance at the first keyframe.
    desk.clock.advance_millis(2_125);
    let batches = desk.state.output.dynamic_contributions_for_test();
    assert!(
        batches.is_empty(),
        "a muted source must not cast an LTP vote"
    );
    assert_eq!(desk.one_instance().id, started.runtime_instance_id);
    assert!(desk.state.programming.release_preload(desk.session.id));
    desk.state.output.dynamic_contributions_for_test();
    let revealed = desk.one_instance();
    assert_eq!(revealed.id, started.runtime_instance_id);
    assert_eq!(
        revealed.started_at_millis,
        before.instances[0].started_at_millis
    );
    assert!(revealed.controller_transitions[0].output_gate.is_none());
    let values = desk.state.output.visualization_dynamic_values(&[], false);
    assert!((desk.intensity(&values) - 0.625).abs() < 0.0001);
}

#[tokio::test]
async fn preload_started_controller_remains_editable_after_go_by_runtime_http_and_osc() {
    let desk = IdentityDesk::new();
    assert!(desk.state.programming.arm_preload(desk.session.id, true));
    let started = desk.start();
    let raw_link = desk.raw_link(true);
    assert!(
        desk.state
            .output
            .dynamic_runtime_snapshot()
            .instances
            .is_empty()
    );
    desk.update_size(started.controller_id, 0.8);
    assert_eq!(desk.raw_link(true), raw_link);
    commit_preload(&desk.state, &desk.session).unwrap();
    desk.state.programming.arm_preload(desk.session.id, false);
    let committed = desk.one_instance();
    desk.assert_identity(&committed, raw_link, started.controller_id);
    assert_eq!(committed.controllers[0].size, 0.8);
    assert!(
        desk.state
            .programming
            .get(desk.session.id)
            .unwrap()
            .dynamic_values
            .is_empty(),
        "controller lookup must find the committed Preload source without a Live authored row"
    );

    desk.clock.advance_millis(250);
    let source: SocketAddr = "127.0.0.1:19171".parse().unwrap();
    desk.state.integrations.register_osc_subscriber(
        "dynamic-controller-identity".into(),
        OscSubscriber {
            capability: light_core::SurfaceCapability::Programming,
            path: "desk".into(),
            target: "127.0.0.1:19172".parse().unwrap(),
            command_source: source,
            session_id: desk.session.id,
            last_seen: Instant::now(),
            shifted: false,
            shift_held: false,
            update_record_started: None,
            update_first_release: None,
            last_highlight_action: None,
        },
    );
    assert!(handle_dynamics_osc(
        &desk.state,
        &format!("/light/desk/dynamic/instance/{}/size", committed.id),
        &[OscArgument::Float(0.6)],
        Some(&source.to_string()),
    ));
    desk.state.output.dynamic_contributions_for_test();
    let osc_edited = desk.one_instance();
    desk.assert_identity(&osc_edited, raw_link, started.controller_id);
    assert_eq!(osc_edited.id, committed.id);
    assert_eq!(osc_edited.started_at_millis, committed.started_at_millis);
    assert_eq!(osc_edited.controllers[0].size, 0.6);
    assert!(
        !desk
            .state
            .integrations
            .captured_osc_feedback()
            .iter()
            .any(|(_, address, _)| address.ends_with("/error"))
    );

    let app = router(desk.state.clone());
    let show_id = desk.state.active_show.current().unwrap().id;
    for (action, body) in [
        ("size", serde_json::json!({"value": 0.3})),
        ("off", serde_json::json!({"timing": {}})),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/dynamic-instances/{}/{action}",
                    committed.id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", desk.session.token),
                )
                .header("x-tosk-show", show_id.0.to_string())
                .body(Body::from(body.to_string()))
                .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["controller_id"], started.controller_id.to_string());
        desk.state.output.dynamic_contributions_for_test();
        if action == "size" {
            let updated = desk.one_instance();
            assert_eq!(updated.id, committed.id);
            assert_eq!(updated.started_at_millis, committed.started_at_millis);
            assert_eq!(updated.controllers[0].size, 0.3);
            assert_eq!(desk.raw_link(false), raw_link);
        } else {
            let covered = desk.one_instance();
            assert_eq!(covered.id, committed.id);
            assert!(
                covered.controller_transitions[0]
                    .output_gate
                    .is_some_and(|gate| gate.to == 0.0)
            );
            assert!(
                desk.state
                    .output
                    .dynamic_contributions_for_test()
                    .is_empty()
            );
            let programmer = desk.state.programming.get(desk.session.id).unwrap();
            assert!(
                light_application::effective_programmer_dynamic_controllers(&programmer).is_empty()
            );
            let response = app
                .clone()
                .oneshot(
                    Request::get("/api/v2/dynamics/runtime")
                        .header(
                            header::AUTHORIZATION,
                            format!("Bearer {}", desk.session.token),
                        )
                        .header("x-tosk-show", show_id.0.to_string())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let projection = json(response).await;
            assert_eq!(projection["programmer_id"], programmer.id.0.to_string());
            assert!(
                projection["instances"].as_array().unwrap().is_empty(),
                "retained masked history must not offer an invalid editable controller"
            );
        }
    }
}
