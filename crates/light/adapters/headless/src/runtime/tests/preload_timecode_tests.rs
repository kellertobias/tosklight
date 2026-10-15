//! Incoming Timecode actions must participate in the same prepared Preload transaction.
use super::*;
use light_playback::{PlaybackAddress, TimecodeId, TimecodeTransportState};

struct TransportRig {
    state: AppState,
    app: axum::Router,
    token: String,
    session: Session,
    data_dir: PathBuf,
    timecode: TimecodeId,
    clock: Arc<ManualClock>,
    fixture: light_core::FixtureId,
    address: PlaybackAddress,
}

impl Drop for TransportRig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

impl TransportRig {
    async fn new() -> Self {
        let clock = Arc::new(ManualClock::new(fixed_test_time()));
        let (state, data_dir) = test_state_with_clock(clock.clone());
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = state
            .sessions
            .sessions()
            .into_iter()
            .find(|value| value.token == token)
            .unwrap();
        let show = create_show(&app, &token, "Incoming Preload transport").await;
        assert_eq!(
            app.clone()
                .oneshot(open_show_request(&token, show["id"].as_str().unwrap()))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let fixture = light_core::FixtureId::new();
        let pending_fixture = light_core::FixtureId::new();
        let mut second = operational_fixture(pending_fixture);
        second.address = Some(2);
        let mut list = operational_cue_list(light_core::CueListId::new(), fixture);
        list.cues[0].fade_millis = 0;
        list.cues[0].changes[0].value = Some(light_core::AttributeValue::Normalized(0.75));
        list.cues[0].changes[0].fade_millis = Some(0);
        list.cues[0].changes[0].delay_millis = Some(0);
        let timecode = TimecodeId(Uuid::new_v4());
        let definition: light_playback::TimecodeDefinition = serde_json::from_value(serde_json::json!({
            "id":timecode,"number":3,"name":"Incoming transport","duration":440,
            "transport_offset":0,"auto_start":false,"markers":[],
            "lanes":[{"id":Uuid::new_v4(),"name":"Held Cue","kind":"cue_list","cue_list_id":list.id,
                "clips":[{"id":Uuid::new_v4(),"start_frame":0,"end_frame":44,
                    "start_cue_id":list.cues[0].id,"end_cue_id":list.cues[0].id,
                    "start_behavior":"state","end_behavior":"hold","cue_starts":[],"in_fade_frames":0,"out_fade_frames":0}]}]
        })).unwrap();
        let address = PlaybackAddress::new(3, 1).unwrap();
        let playback = serde_json::from_value(serde_json::json!({
            "name":"Timecode3","target":{"type":"timecode","timecode_id":timecode},"has_fader":false,"auto_off":false
        })).unwrap();
        let snapshot = EngineSnapshot {
            fixtures: vec![operational_fixture(fixture), second].into(),
            cue_lists: vec![list].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 3,
                name: "Transport".into(),
                assignments: HashMap::from([(1, playback)]),
            }]
            .into(),
            ..EngineSnapshot::default()
        };
        state.output.replace_snapshot(snapshot).unwrap();
        let active = state.active_show.current().clone().unwrap();
        ShowStore::open(&active.path)
            .unwrap()
            .put_object(
                "timecode",
                &timecode.0.to_string(),
                &serde_json::to_value(&definition).unwrap(),
                0,
            )
            .unwrap();
        state.timecodes.install(definition, None).unwrap();
        state
            .installation
            .update_configuration(|config| config.programmer_fade_millis = 0);
        let rig = Self {
            state,
            app,
            token,
            session,
            data_dir,
            timecode,
            clock,
            fixture,
            address,
        };
        let entered=rig.post("/api/v2/programmer/preload/actions",serde_json::json!({
            "request_id":"transport-enter","expected_capture_mode_revision":0,"expected_values_revision":0,
            "expected_queue_revision":0,"expected_selection_revision":0,"action":{"type":"enter"}
        })).await;
        assert_eq!(entered.status(), StatusCode::OK, "{}", json(entered).await);
        let seeded=rig.post("/api/v2/programmer/preload-values/actions",serde_json::json!({
            "request_id":"transport-pending","expected_revision":0,"expected_capture_mode_revision":1,
            "action":{"type":"set_fixture","fixture_id":pending_fixture,"attribute":"intensity","value":{"kind":"normalized","value":0.4}}
        })).await;
        assert_eq!(seeded.status(), StatusCode::OK, "{}", json(seeded).await);
        rig
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(path)
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .header("x-tosk-desk", self.session.desk.id.to_string())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn capture(&self, osc: bool) {
        let transport_before = self.state.timecodes.snapshot(self.timecode).ok();
        if osc {
            handle_control_event(
                &self.state,
                ControlEvent::Osc {
                    address: "/light/subscribe".into(),
                    arguments: vec![
                        OscArgument::String("preload-transport-hardware".into()),
                        OscArgument::String("desk".into()),
                        OscArgument::Int(19142),
                    ],
                    source: Some("127.0.0.1:19141".into()),
                },
            );
            handle_control_event(
                &self.state,
                ControlEvent::Osc {
                    address: "/light/desk/playback/3/1/go".into(),
                    arguments: vec![OscArgument::Bool(true), OscArgument::Int(21204)],
                    source: Some("127.0.0.1:19141".into()),
                },
            );
        } else {
            let captured=self.post("/api/v2/playback-actions",serde_json::json!({
                "address":{"kind":"playback","page":3,"position":1},"action":{"type":"go","pressed":true},
                "control_source":{"surface_id":"main","control_id":21204}
            })).await;
            assert_eq!(
                captured.status(),
                StatusCode::OK,
                "{}",
                json(captured).await
            );
        }
        let queue = self
            .state
            .programming
            .preload_playback_actions(self.session.id)
            .unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].address, self.address);
        assert_eq!(queue[0].control_source.control_id, 21204);
        assert_eq!(
            queue[0].control_source.surface_id.starts_with("hardware:"),
            osc
        );
        assert_eq!(
            self.state.timecodes.snapshot(self.timecode).ok(),
            transport_before
        );
    }

    async fn go(&self) -> Response {
        let show = self.state.active_show.current().clone().unwrap();
        let revision = ShowStore::open(&show.path)
            .unwrap()
            .portable_revision()
            .unwrap()
            .value();
        self.post("/api/v2/programmer/preload/actions",serde_json::json!({
            "request_id":"transport-preload-go","expected_capture_mode_revision":1,"expected_values_revision":1,
            "expected_queue_revision":1,"expected_selection_revision":0,
            "action":{"type":"go","show_id":show.id,"expected_show_revision":revision,
                "expected_playback_event_sequence":self.state.events.latest_sequence()}
        })).await
    }
}

async fn assert_incoming_transport_commit(osc: bool) {
    let rig = TransportRig::new().await;
    rig.capture(osc).await;
    let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
    assert_eq!(
        &rig.state.output.dmx_snapshot().0.universes[0].slots[..2],
        &[0, 0]
    );
    let result = rig.go().await;
    let status = result.status();
    let body = json(result).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["commit"]["executed_playback_actions"], 1);
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.timecode)
            .unwrap()
            .transport,
        TimecodeTransportState::Playing
    );
    assert!(
        rig.state
            .programming
            .preload_playback_actions(rig.session.id)
            .unwrap()
            .is_empty()
    );
    let light_application::EventReplay::Events(events) = rig
        .state
        .events
        .replay(0, &light_application::EventFilter::default())
    else {
        panic!("retained events")
    };
    assert!(events.iter().any(|event| matches!(&event.payload,
        light_application::ApplicationEvent::Desk(light_application::DeskEvent::TimecodeRuntimeChanged(change))
        if change.snapshot.timecode_id == rig.timecode && change.snapshot.transport == TimecodeTransportState::Playing
        && change.snapshot.cue_list_clips.iter().any(|clip|clip.kind==light_playback::TimecodeCueListClipExecutionKind::Active))), "accepted feedback must describe the final candidate transport and clip ownership");
    let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
    assert_eq!(
        &rig.state.output.dmx_snapshot().0.universes[0].slots[..2],
        &[191, 102]
    );
    assert!(
        rig.state
            .output
            .engine()
            .playback_contributions_at(fixed_test_time())
            .iter()
            .any(|row| row.value.fixture_id == rig.fixture)
    );
    if osc {
        handle_control_event(
            &rig.state,
            ControlEvent::Osc {
                address: "/light/unsubscribe".into(),
                arguments: vec![OscArgument::String("preload-transport-hardware".into())],
                source: Some("127.0.0.1:19141".into()),
            },
        );
    }
}

#[tokio::test]
async fn preload_transport_commit_typed_capture_and_revisioned_go_are_atomic() {
    assert_incoming_transport_commit(false).await;
}
#[tokio::test]
async fn preload_transport_commit_supported_osc_capture_and_typed_go_are_atomic() {
    assert_incoming_transport_commit(true).await;
}

fn queue_transport(
    rig: &TransportRig,
    address: PlaybackAddress,
    action: light_programmer::PreloadPlaybackQueueAction,
) {
    assert!(
        rig.state
            .programming
            .queue_preload_playback_action_with_origin(
                rig.session.id,
                address,
                action,
                light_playback::PlaybackControlSource {
                    surface_id: "pane:transport-test".into(),
                    control_id: 796
                },
                Some(rig.session.desk.id)
            )
    );
}

fn add_solo_cue(rig: &TransportRig) -> PlaybackAddress {
    let peer = PlaybackAddress::new(3, 2).unwrap();
    let mut snapshot = rig.state.output.snapshot().as_ref().clone();
    let mut list = operational_cue_list(light_core::CueListId::new(), rig.fixture);
    list.cues[0].fade_millis = 0;
    list.cues[0].changes[0].value = Some(light_core::AttributeValue::Normalized(0.25));
    list.cues[0].changes[0].fade_millis = Some(0);
    list.cues[0].changes[0].delay_millis = Some(0);
    let definition=serde_json::from_value(serde_json::json!({"name":"Solo Cue","target":{"type":"cue_list","cue_list_id":list.id},"auto_off":false})).unwrap();
    Arc::make_mut(&mut snapshot.cue_lists).push(list);
    Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .insert(2, definition);
    snapshot.playback_solo_regions = vec![light_playback::PlaybackSoloRegion {
        id: Uuid::new_v4(),
        name: "Exact transport members".into(),
        members: vec![rig.address, peer],
    }]
    .into();
    rig.state.output.replace_snapshot(snapshot).unwrap();
    peer
}

#[tokio::test]
async fn preload_transport_commit_preserves_mixed_solo_order_and_off_releases_held_clip() {
    use light_programmer::PreloadPlaybackQueueAction as Action;
    for final_transport in [false, true] {
        let rig = TransportRig::new().await;
        let peer = add_solo_cue(&rig);
        rig.capture(false).await;
        queue_transport(&rig, peer, Action::Go);
        if final_transport {
            queue_transport(&rig, rig.address, Action::Go);
        }
        commit_preload(&rig.state, &rig.session).unwrap();
        assert_eq!(
            rig.state
                .timecodes
                .snapshot(rig.timecode)
                .unwrap()
                .transport,
            if final_transport {
                TimecodeTransportState::Playing
            } else {
                TimecodeTransportState::Stopped
            }
        );
        assert_eq!(
            rig.state.output.engine().permanent_playback_enabled(peer),
            !final_transport
        );
        let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
        assert_eq!(
            rig.state.output.dmx_snapshot().0.universes[0].slots[0],
            if final_transport { 191 } else { 64 }
        );
        if final_transport {
            rig.clock.advance_millis(2_000);
            rig.state.timecodes.tick();
            rig.state
                .timecodes
                .reconcile_cue_lists(rig.state.output.engine());
            assert!(rig.state.timecodes.snapshot(rig.timecode).unwrap().cue_list_clips.iter().any(|clip|clip.kind==light_playback::TimecodeCueListClipExecutionKind::Held));
            assert!(rig.state.programming.arm_preload(rig.session.id, true));
            queue_transport(&rig, rig.address, Action::Off);
            commit_preload(&rig.state, &rig.session).unwrap();
            let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
            assert_eq!(
                rig.state
                    .timecodes
                    .snapshot(rig.timecode)
                    .unwrap()
                    .transport,
                TimecodeTransportState::Stopped
            );
            assert_eq!(rig.state.output.dmx_snapshot().0.universes[0].slots[0], 0);
        }
    }
}

#[tokio::test]
async fn preload_transport_commit_late_invalid_step_preserves_every_live_authority_and_pending_work()
 {
    let rig = TransportRig::new().await;
    rig.capture(true).await;
    queue_transport(
        &rig,
        rig.address,
        light_programmer::PreloadPlaybackQueueAction::Back,
    );
    let before = serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap();
    let transport = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    let playback = serde_json::to_value(rig.state.output.playback_runtime()).unwrap();
    let dynamics = rig.state.output.dynamic_runtime_snapshot();
    let cursor = rig.state.events.latest_sequence();
    let error = commit_preload(&rig.state, &rig.session).unwrap_err();
    assert!(error.contains("incompatible with a Timecode"), "{error}");
    assert_eq!(
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap(),
        before
    );
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap(),
        transport
    );
    assert_eq!(
        serde_json::to_value(rig.state.output.playback_runtime()).unwrap(),
        playback
    );
    assert_eq!(rig.state.output.dynamic_runtime_snapshot(), dynamics);
    assert_eq!(rig.state.events.latest_sequence(), cursor);
}

#[tokio::test]
async fn preload_transport_commit_rejects_a_changed_current_show_definition_without_refreshing_live_cache()
 {
    let rig = TransportRig::new().await;
    rig.capture(false).await;
    let show = rig.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&show.path).unwrap();
    let (_, Some(mut object)) = store
        .object_with_portable_revision("timecode", &rig.timecode.0.to_string())
        .unwrap()
    else {
        panic!("seeded object")
    };
    object.body["name"] = serde_json::json!("Changed after capture");
    store
        .put_object("timecode", &object.id, &object.body, object.revision)
        .unwrap();
    let before = serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap();
    let transport = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    let cursor = rig.state.events.latest_sequence();
    let error = commit_preload(&rig.state, &rig.session).unwrap_err();
    assert!(error.contains("definition changed"), "{error}");
    assert_eq!(
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap(),
        before
    );
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap(),
        transport
    );
    assert_eq!(rig.state.events.latest_sequence(), cursor);
}

#[tokio::test]
async fn preload_transport_commit_installs_a_never_opened_current_show_timecode_only_on_acceptance()
{
    let rig = TransportRig::new().await;
    rig.state.timecodes.reset_for_show_activation();
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    rig.capture(false).await;
    let result = rig.go().await;
    let status = result.status();
    let body = json(result).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let snapshot = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    assert_eq!(snapshot.transport, TimecodeTransportState::Playing);
    assert!(
        snapshot
            .cue_list_clips
            .iter()
            .any(|clip| clip.kind == light_playback::TimecodeCueListClipExecutionKind::Active)
    );
    let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
    assert_eq!(
        &rig.state.output.dmx_snapshot().0.universes[0].slots[..2],
        &[191, 102]
    );
}

struct TransportPublisher(EventResource);
impl light_application::timeline::TimecodeChangePublisher for TransportPublisher {
    fn publish(&self, change: &light_application::timeline::TimecodeRuntimeChange) {
        self.0
            .publish(light_application::EventDraft::timecode_runtime_changed(
                change.clone(),
            ));
    }
}

#[derive(Default)]
struct RejectTransportBatch {
    calls: std::sync::atomic::AtomicU64,
}
impl light_application::TimecodeAudioOutput for RejectTransportBatch {
    fn output_latency_micros(&self) -> u64 {
        0
    }
    fn apply(&self, _: light_application::TimecodeAudioCommand) -> Result<(), String> {
        Ok(())
    }
    fn apply_batch(&self, _: &[light_application::TimecodeAudioCommand]) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err("injected atomic audio refusal".into())
    }
}

#[tokio::test]
async fn preload_transport_commit_audio_refusal_rolls_back_pending_normal_queue_and_all_runtimes() {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(RejectTransportBatch::default());
    rig.state.timecodes = light_application::timeline::TimecodeRuntimeService::new(
        crate::runtime::timecode_clock::runtime_clock(Some(&rig.clock)),
        Arc::new(TransportPublisher(rig.state.events.clone())),
        light_playback::TimecodeFrameRate::whole_frames(44).unwrap(),
    )
    .with_audio(Arc::new(light_application::TimecodeAudioService::new(
        output.clone(),
    )));
    let show = rig.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&show.path).unwrap();
    let (_, Some(object)) = store
        .object_with_portable_revision("timecode", &rig.timecode.0.to_string())
        .unwrap()
    else {
        panic!("seed")
    };
    rig.state
        .timecodes
        .install(serde_json::from_value(object.body).unwrap(), None)
        .unwrap();
    rig.state
        .timecodes
        .prepare_audio(
            rig.timecode,
            light_application::AssetReference {
                id: light_application::AssetId(Uuid::new_v4()),
                revision: light_application::AssetRevision(1),
            },
            light_application::WavMetadata {
                encoding: light_application::WavEncoding::PcmInteger,
                channels: 1,
                sample_rate: 44,
                bits_per_sample: 16,
                data_bytes: 880,
                sample_frames: 440,
            },
            true,
        )
        .unwrap();
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let programmer =
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap();
    let transport = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    let playback = serde_json::to_value(rig.state.output.playback_runtime()).unwrap();
    let dynamics = rig.state.output.dynamic_runtime_snapshot();
    let cursor = rig.state.events.latest_sequence();
    let result = rig.go().await;
    let status = result.status();
    let body = json(result).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("audio refusal"),
        "{body}"
    );
    assert_eq!(output.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap(),
        programmer
    );
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap(),
        transport
    );
    assert_eq!(
        serde_json::to_value(rig.state.output.playback_runtime()).unwrap(),
        playback
    );
    assert_eq!(rig.state.output.dynamic_runtime_snapshot(), dynamics);
    assert_eq!(rig.state.events.latest_sequence(), cursor);
}

#[tokio::test]
async fn preload_transport_commit_go_resumes_paused_frame_without_restarting_the_timeline() {
    let rig = TransportRig::new().await;
    rig.state
        .timecodes
        .handle_playback_go(rig.timecode)
        .unwrap();
    rig.clock.advance_millis(500);
    rig.state.timecodes.tick();
    rig.state
        .timecodes
        .handle(rig.timecode, light_playback::TimecodeTransportAction::Pause)
        .unwrap();
    let before = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    assert!(before.frame.0 > 0);
    rig.capture(false).await;
    commit_preload(&rig.state, &rig.session).unwrap();
    let after = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    assert_eq!(after.transport, TimecodeTransportState::Playing);
    assert_eq!(after.frame, before.frame);
}

#[tokio::test]
async fn preload_transport_commit_captured_time_survives_preparation_delay_and_restores_live_clock()
{
    let rig = TransportRig::new().await;
    let at = rig.state.programming.clock().now();
    let micros = rig.state.timecodes.capture_transport_micros();
    let mut transport = rig.state.timecodes.prepare_transport_batch(micros);
    rig.clock.advance_millis(1_000); // Deliberate preparation delay, after the common GO anchor.
    let snapshot = rig.state.output.snapshot();
    let timing = rig.state.output.engine().cue_timing_masters();
    let command = light_engine::PlaybackBatchCommand {
        address: rig.address,
        action: light_engine::PlaybackBatchAction::Go,
        control_source: light_playback::PlaybackControlSource {
            surface_id: "main".into(),
            control_id: 796,
        },
        activation_origin: None,
    };
    let prepared = rig
        .state
        .output
        .engine()
        .prepare_playback_batch_with_steps(&[command.clone()], at, 0, None, |playback, _, _| {
            transport
                .handle(rig.timecode, light_playback::TimecodeTransportAction::Go)
                .map_err(|error| error.message)?;
            transport.reconcile(playback, &snapshot.cue_lists, timing)?;
            Ok(light_engine::PlaybackBatchOutcome {
                address: rig.address,
                permanent_started: true,
                released_playbacks: Vec::new(),
                addressed_effect: light_playback::PlaybackRuntimeEffect::Durable,
                effect: light_playback::PlaybackRuntimeEffect::Durable,
            })
        })
        .unwrap();
    rig.state
        .output
        .engine()
        .install_prepared_playback_batch_with(
            prepared,
            |_| {
                transport.accept_audio()?;
                Ok(transport.install())
            },
            |changes| rig.state.timecodes.publish_prepared_changes(changes),
        )
        .unwrap();
    let list = snapshot.cue_lists[0].id;
    let active = rig
        .state
        .output
        .playback_runtime_status_for_cue_list(list)
        .unwrap();
    assert_eq!(active.playback.activated_at, at);
    rig.state.timecodes.tick();
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap().frame.0,
        44
    );
    rig.clock.advance_millis(500);
    rig.state.timecodes.tick();
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap().frame.0,
        66
    );
    // Direct live Cue GO must use the advancing source, not the temporary captured clock.
    rig.state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::CueList {
            id: list,
            action: light_engine::CueListPlaybackAction::Go,
        })
        .unwrap();
    assert_eq!(
        rig.state
            .output
            .playback_runtime_status_for_cue_list(list)
            .unwrap()
            .playback
            .activated_at,
        at + chrono::Duration::milliseconds(1500)
    );
}

#[tokio::test]
async fn preload_transport_commit_generation_change_refuses_before_audio_or_transport_install() {
    let rig = TransportRig::new().await;
    let mut transport = rig
        .state
        .timecodes
        .prepare_transport_batch(rig.state.timecodes.capture_transport_micros());
    let snapshot = rig.state.output.snapshot();
    let timing = rig.state.output.engine().cue_timing_masters();
    let command = light_engine::PlaybackBatchCommand {
        address: rig.address,
        action: light_engine::PlaybackBatchAction::Go,
        control_source: light_playback::PlaybackControlSource {
            surface_id: "main".into(),
            control_id: 796,
        },
        activation_origin: None,
    };
    let prepared = rig
        .state
        .output
        .engine()
        .prepare_playback_batch_with_steps(
            &[command],
            rig.clock.now(),
            0,
            None,
            |playback, _, _| {
                transport
                    .handle(rig.timecode, light_playback::TimecodeTransportAction::Go)
                    .map_err(|error| error.message)?;
                transport.reconcile(playback, &snapshot.cue_lists, timing)?;
                Ok(light_engine::PlaybackBatchOutcome {
                    address: rig.address,
                    permanent_started: true,
                    released_playbacks: Vec::new(),
                    addressed_effect: light_playback::PlaybackRuntimeEffect::Durable,
                    effect: light_playback::PlaybackRuntimeEffect::Durable,
                })
            },
        )
        .unwrap();
    rig.state
        .output
        .replace_snapshot(snapshot.as_ref().clone())
        .unwrap();
    let error = rig
        .state
        .output
        .engine()
        .install_prepared_playback_batch_with(
            prepared,
            |_| {
                transport.accept_audio()?;
                Ok(transport.install())
            },
            |_| (),
        )
        .unwrap_err();
    assert!(error.contains("compiled show changed"), "{error}");
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.timecode)
            .unwrap()
            .transport,
        TimecodeTransportState::Stopped
    );
}

fn add_solo_dynamic(rig: &TransportRig) -> PlaybackAddress {
    let peer = PlaybackAddress::new(3, 4).unwrap();
    let mut dynamic = command_test_dynamic(Uuid::new_v4(), 1);
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![rig.fixture],
    };
    let mut playback = preload_test_playback([light_playback::PlaybackButtonAction::None; 3]);
    playback.target = light_playback::PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic.id),
                last_known_pool_number: 1,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(dynamic.clone()),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: true,
        },
    };
    let mut snapshot = rig.state.output.snapshot().as_ref().clone();
    snapshot.dynamics = vec![dynamic].into();
    Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .insert(4, playback);
    snapshot.playback_solo_regions = vec![light_playback::PlaybackSoloRegion {
        id: Uuid::new_v4(),
        name: "TC Dynamic order".into(),
        members: vec![rig.address, peer],
    }]
    .into();
    rig.state.output.replace_snapshot(snapshot).unwrap();
    peer
}

#[tokio::test]
async fn preload_transport_commit_dynamic_and_timecode_candidates_share_the_final_solo_winner() {
    use light_programmer::PreloadPlaybackQueueAction as Action;
    for final_transport in [false, true] {
        let rig = TransportRig::new().await;
        let peer = add_solo_dynamic(&rig);
        rig.capture(false).await;
        queue_transport(&rig, peer, Action::On);
        if final_transport {
            queue_transport(&rig, rig.address, Action::Go);
        }
        commit_preload(&rig.state, &rig.session).unwrap();
        assert_eq!(
            rig.state
                .timecodes
                .snapshot(rig.timecode)
                .unwrap()
                .transport,
            if final_transport {
                TimecodeTransportState::Playing
            } else {
                TimecodeTransportState::Stopped
            }
        );
        assert_eq!(
            rig.state
                .output
                .active_dynamic_playback(peer)
                .is_some_and(|runtime| runtime.enabled),
            !final_transport
        );
        assert_eq!(
            rig.state
                .output
                .dynamic_runtime_snapshot()
                .instances
                .is_empty(),
            final_transport
        );
    }
}

#[path = "preload_timecode_tests/audio.rs"]
mod audio;
