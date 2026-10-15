use super::*;

struct WavSource(Option<Vec<u8>>);
impl light_application::AssetChunkSource for WavSource {
    fn read_chunk(&mut self, _: usize) -> Result<Option<Vec<u8>>, light_application::AssetError> {
        Ok(self.0.take())
    }
}
#[derive(Default)]
struct AtomicTransportOutput(
    Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>>,
    std::sync::atomic::AtomicUsize,
);
impl light_application::TimecodeAudioOutput for AtomicTransportOutput {
    fn prepare_batch(
        &self,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<
        Option<Box<dyn light_application::timeline::PreparedTimecodeAudioOutputBatch>>,
        String,
    > {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Some(Box::new(RecordedAudioToken {
            batches: self.0.clone(),
            resources: audio_resources(commands),
            expected_play_micros: None,
        })))
    }

    fn output_latency_micros(&self) -> u64 {
        0
    }
    fn apply(&self, _: light_application::TimecodeAudioCommand) -> Result<(), String> {
        Err("a cold Preload must not write single commands to Live audio".into())
    }
    fn apply_batch(
        &self,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<(), String> {
        self.0.lock().unwrap().push(commands.to_vec());
        Ok(())
    }
}

#[tokio::test]
async fn preload_transport_commit_cold_linked_audio_prepares_and_plays_in_the_same_atomic_device_batch()
 {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    assert!(output.0.lock().unwrap().is_empty());
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    let result = rig.go().await;
    let status = result.status();
    let body = json(result).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let batches = output.0.lock().unwrap();
    assert_eq!(batches.len(), 1);
    assert!(
        matches!(batches[0].first(),Some(light_application::TimecodeAudioCommand::Prepare{timecode_id,..})if *timecode_id==rig.timecode)
    );
    assert!(batches[0].iter().any(|command|matches!(command,light_application::TimecodeAudioCommand::SetLoop{timecode_id,..}if *timecode_id==rig.timecode)));
    assert!(batches[0].iter().any(|command|matches!(command,light_application::TimecodeAudioCommand::Play{timecode_id,..}if *timecode_id==rig.timecode)));
    assert!(
        rig.state
            .timecodes
            .snapshot(rig.timecode)
            .unwrap()
            .audio_linked
    );
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.timecode)
            .unwrap()
            .transport,
        TimecodeTransportState::Playing
    );
}

fn audio_resources(
    commands: &[light_application::TimecodeAudioCommand],
) -> Vec<light_application::TimecodeAudioCommand> {
    commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                light_application::TimecodeAudioCommand::Prepare { .. }
            )
        })
        .copied()
        .collect()
}
struct RecordedAudioToken {
    batches: Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>>,
    resources: Vec<light_application::TimecodeAudioCommand>,
    expected_play_micros: Option<u64>,
}
impl light_application::timeline::PreparedTimecodeAudioOutputBatch for RecordedAudioToken {
    fn apply(
        self: Box<Self>,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<(), String> {
        if audio_resources(commands) != self.resources {
            return Err("test token resource mismatch".into());
        }
        if let Some(expected) = self.expected_play_micros {
            assert!(commands.iter().any(|command|matches!(command,light_application::TimecodeAudioCommand::Play{audible_at_micros,..}if *audible_at_micros==expected)),"audio Play must use the joint post-decode anchor: {commands:?}");
        }
        self.batches.lock().unwrap().push(commands.to_vec());
        Ok(())
    }
}

fn install_cold_audio(
    rig: &mut TransportRig,
    output: Arc<dyn light_application::TimecodeAudioOutput>,
) {
    rig.state.timecodes = light_application::timeline::TimecodeRuntimeService::new(
        crate::runtime::timecode_clock::runtime_clock(Some(&rig.clock)),
        Arc::new(TransportPublisher(rig.state.events.clone())),
        light_playback::TimecodeFrameRate::whole_frames(44).unwrap(),
    )
    .with_audio(Arc::new(light_application::TimecodeAudioService::new(
        output.clone(),
    )));
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&916_u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&44_u32.to_le_bytes());
    wav.extend_from_slice(&88_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&880_u32.to_le_bytes());
    wav.resize(924, 0);
    let show = rig.state.active_show.current().clone().unwrap();
    let imported = light_application::TimecodeWavImporter::new(rig.state.managed_assets.clone())
        .import(
            None,
            light_application::AssetNamespace(format!("show:{}", show.id.0)),
            "Cold.wav".into(),
            &mut WavSource(Some(wav)),
        )
        .unwrap();
    let store = ShowStore::open(&show.path).unwrap();
    let (_, Some(mut object)) = store
        .object_with_portable_revision("timecode", &rig.timecode.0.to_string())
        .unwrap()
    else {
        panic!("seed")
    };
    object.body["audio"] = serde_json::json!({"asset_id":imported.descriptor.asset.id.0,"asset_revision":imported.descriptor.asset.revision.0,"file_name":"Cold.wav"});
    store
        .put_object("timecode", &object.id, &object.body, object.revision)
        .unwrap();
}

struct DelayedResourceOutput {
    clock: Arc<ManualClock>,
    after_preflight: Box<dyn Fn() + Send + Sync>,
    batches: Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>>,
    expected_play_micros: u64,
}
impl light_application::TimecodeAudioOutput for DelayedResourceOutput {
    fn output_latency_micros(&self) -> u64 {
        0
    }
    fn apply(&self, _: light_application::TimecodeAudioCommand) -> Result<(), String> {
        Err("unexpected single Live audio write".into())
    }
    fn prepare_batch(
        &self,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<
        Option<Box<dyn light_application::timeline::PreparedTimecodeAudioOutputBatch>>,
        String,
    > {
        self.clock.advance_millis(2_000);
        (self.after_preflight)();
        Ok(Some(Box::new(RecordedAudioToken {
            batches: self.batches.clone(),
            resources: audio_resources(commands),
            expected_play_micros: Some(self.expected_play_micros),
        })))
    }
    fn apply_batch(&self, _: &[light_application::TimecodeAudioCommand]) -> Result<(), String> {
        Err("decoded token must be consumed".into())
    }
}
#[tokio::test]
async fn preload_transport_commit_cold_decode_delay_uses_one_fresh_cue_transport_audio_anchor_and_initial_fade()
 {
    let mut rig = TransportRig::new().await;
    let initial = rig.clock.now();
    rig.state
        .installation
        .update_configuration(|config| config.programmer_fade_millis = 1234);
    let installation = rig.state.installation.clone();
    let batches = Arc::new(std::sync::Mutex::new(Vec::new()));
    let expected_play_micros = rig.state.timecodes.capture_transport_micros() + 2_000_000;
    let output = Arc::new(DelayedResourceOutput {
        clock: rig.clock.clone(),
        batches: batches.clone(),
        expected_play_micros,
        after_preflight: Box::new(move || {
            installation.update_configuration(|config| config.programmer_fade_millis = 9876);
        }),
    });
    install_cold_audio(&mut rig, output);
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let response = rig.go().await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["commit"]["programmer_fade_millis"], 1234,
        "fade is captured once before resource work: {body}"
    );
    let runtime = rig.state.timecodes.snapshot(rig.timecode).unwrap();
    assert_eq!(runtime.frame.0, 0);
    let list = rig.state.output.snapshot().cue_lists[0].id;
    assert_eq!(
        rig.state
            .output
            .playback_runtime_status_for_cue_list(list)
            .unwrap()
            .playback
            .activated_at,
        initial + chrono::Duration::seconds(2)
    );
    assert_eq!(batches.lock().unwrap().len(), 1);
    let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap().frame.0,
        0
    );
    rig.clock.advance_millis(500);
    let _ = output_scheduler::render_test_tick(rig.state.clone()).await;
    assert_eq!(
        rig.state.timecodes.snapshot(rig.timecode).unwrap().frame.0,
        22
    );
}

struct MismatchedResourceOutput(
    Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>>,
);
impl light_application::TimecodeAudioOutput for MismatchedResourceOutput {
    fn output_latency_micros(&self) -> u64 {
        0
    }
    fn apply(&self, _: light_application::TimecodeAudioCommand) -> Result<(), String> {
        Err("unexpected single audio write".into())
    }
    fn prepare_batch(
        &self,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<
        Option<Box<dyn light_application::timeline::PreparedTimecodeAudioOutputBatch>>,
        String,
    > {
        let mut resources = audio_resources(commands);
        if let light_application::TimecodeAudioCommand::Prepare { asset, .. } = &mut resources[0] {
            asset.revision = light_application::AssetRevision(asset.revision.0 + 1);
        }
        Ok(Some(Box::new(RecordedAudioToken {
            batches: self.0.clone(),
            resources,
            expected_play_micros: None,
        })))
    }
}
#[tokio::test]
async fn preload_transport_commit_resource_token_mismatch_cannot_install_cold_transport_or_pending_programmer()
 {
    let mut rig = TransportRig::new().await;
    let batches = Arc::new(std::sync::Mutex::new(Vec::new()));
    install_cold_audio(
        &mut rig,
        Arc::new(MismatchedResourceOutput(batches.clone())),
    );
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let before = serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap();
    let cursor = rig.state.events.latest_sequence();
    let response = rig.go().await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("token resource mismatch"),
        "{body}"
    );
    assert!(batches.lock().unwrap().is_empty());
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    assert_eq!(
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap(),
        before
    );
    assert!(rig.state.output.playback_runtime().is_empty());
    assert!(
        rig.state
            .output
            .dynamic_runtime_snapshot()
            .instances
            .is_empty()
    );
    assert_eq!(rig.state.events.latest_sequence(), cursor);
}

#[tokio::test]
async fn preload_transport_commit_generation_changed_during_decode_refuses_both_passes_without_late_audio()
 {
    let mut rig = TransportRig::new().await;
    let output_resource = rig.state.output.clone();
    let batches = Arc::new(std::sync::Mutex::new(Vec::new()));
    let expected_play_micros = rig.state.timecodes.capture_transport_micros() + 2_000_000;
    let output = Arc::new(DelayedResourceOutput {
        clock: rig.clock.clone(),
        batches: batches.clone(),
        expected_play_micros,
        after_preflight: Box::new(move || {
            let snapshot = output_resource.snapshot();
            output_resource
                .replace_snapshot(snapshot.as_ref().clone())
                .unwrap();
        }),
    });
    install_cold_audio(&mut rig, output);
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let before = serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap();
    let cursor = rig.state.events.latest_sequence();
    let response = rig.go().await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("compiled show changed"),
        "{body}"
    );
    assert!(batches.lock().unwrap().is_empty());
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    assert_eq!(
        serde_json::to_value(rig.state.programming.get(rig.session.id).unwrap()).unwrap(),
        before
    );
    assert!(rig.state.output.playback_runtime().is_empty());
    assert_eq!(rig.state.events.latest_sequence(), cursor);
}

struct BlockingResourceOutput {
    entered: std::sync::mpsc::Sender<()>,
    release: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    batches: Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>>,
}
impl light_application::TimecodeAudioOutput for BlockingResourceOutput {
    fn output_latency_micros(&self) -> u64 {
        0
    }
    fn apply(&self, _: light_application::TimecodeAudioCommand) -> Result<(), String> {
        Err("cold resources must be prepared without live audio commands".into())
    }
    fn prepare_batch(
        &self,
        commands: &[light_application::TimecodeAudioCommand],
    ) -> Result<
        Option<Box<dyn light_application::timeline::PreparedTimecodeAudioOutputBatch>>,
        String,
    > {
        self.entered.send(()).unwrap();
        let (lock, wake) = &*self.release;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = wake.wait(released).unwrap();
        }
        Ok(Some(Box::new(RecordedAudioToken {
            batches: self.batches.clone(),
            resources: audio_resources(commands),
            expected_play_micros: None,
        })))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preload_transport_cold_resource_preflight_does_not_block_live_ordered_render_or_transport()
{
    let mut rig = TransportRig::new().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    install_cold_audio(
        &mut rig,
        Arc::new(BlockingResourceOutput {
            entered: entered_tx,
            release: release.clone(),
            batches: Arc::default(),
        }),
    );
    let peer = TimecodeId(Uuid::new_v4());
    let definition = serde_json::from_value(serde_json::json!({"id":peer,"number":4,"name":"Existing live clock","duration":440,"transport_offset":0,"auto_start":false,"markers":[],"lanes":[]})).unwrap();
    rig.state.timecodes.install(definition, None).unwrap();
    rig.state.timecodes.handle_playback_go(peer).unwrap();
    let fading_fixture = light_core::FixtureId::new();
    let fading_list = light_core::CueListId::new();
    let mut fixture = operational_fixture(fading_fixture);
    fixture.address = Some(3);
    let mut list = operational_cue_list(fading_list, fading_fixture);
    list.cues[0].fade_millis = 2000;
    list.cues[0].changes[0].fade_millis = Some(2000);
    list.cues[0].changes[0].delay_millis = Some(0);
    let mut snapshot = (*rig.state.output.snapshot()).clone();
    let mut fixtures = snapshot.fixtures.to_vec();
    fixtures.push(fixture);
    snapshot.fixtures = fixtures.into();
    let mut lists = snapshot.cue_lists.to_vec();
    lists.push(list);
    snapshot.cue_lists = lists.into();
    rig.state.output.replace_snapshot(snapshot).unwrap();
    rig.state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::CueList {
            id: fading_list,
            action: light_engine::CueListPlaybackAction::Go,
        })
        .unwrap();
    let fade_anchor = rig
        .state
        .output
        .playback_runtime_status_for_cue_list(fading_list)
        .unwrap()
        .playback
        .activated_at;
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let show = rig.state.active_show.current().clone().unwrap();
    let revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let request = Request::post("/api/v2/programmer/preload/actions").header("authorization", format!("Bearer {}",rig.token)).header("content-type","application/json").body(Body::from(serde_json::json!({"request_id":"unblocked-cold-go","expected_capture_mode_revision":1,"expected_values_revision":1,"expected_queue_revision":1,"expected_selection_revision":0,"action":{"type":"go","show_id":show.id,"expected_show_revision":revision,"expected_playback_event_sequence":rig.state.events.latest_sequence()}}).to_string())).unwrap();
    let app = rig.app.clone();
    let go = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    rig.clock.advance_millis(500);
    let state = rig.state.clone();
    let (progress_tx, progress_rx) = std::sync::mpsc::channel();
    let render = std::thread::spawn(move || {
        let activation_available = state.active_show.try_acquire().is_ok();
        let sample =
            output_scheduler::ordered_output_operation(&state.playback.render_capability(), || {
                state.timecodes.tick();
                let frame = state.timecodes.snapshot(peer).unwrap().frame.0;
                let rendered = state.output.render(RenderOptions::default()).unwrap();
                let faded = rendered.universes.get(&1).unwrap()[2];
                state
                    .output
                    .execute_playback(light_engine::EnginePlaybackCommand::CueList {
                        id: fading_list,
                        action: light_engine::CueListPlaybackAction::SetMaster(0.5),
                    })
                    .unwrap();
                light_application::PlaybackOperation::new((frame, faded))
            });
        let _ = progress_tx.send((activation_available, sample));
    });
    let progress =
        tokio::task::spawn_blocking(move || progress_rx.recv_timeout(Duration::from_millis(500)))
            .await
            .unwrap();
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
    let response = go.await.unwrap();
    render.join().unwrap();
    assert!(
        progress.is_ok(),
        "cold resource work blocked the production ordered render gate: {progress:?}"
    );
    assert_eq!(
        progress.unwrap(),
        (true, (22, 64)),
        "the old live clock and Cue fade must progress before resource completion"
    );
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    assert_eq!(
        rig.state
            .output
            .playback_runtime_status_for_cue_list(fading_list)
            .unwrap()
            .playback
            .activated_at,
        fade_anchor
    );
    let rendered = rig.state.output.render(RenderOptions::default()).unwrap();
    assert_eq!(
        rendered.universes.get(&1).unwrap()[2],
        32,
        "the concurrent half-master edit and continued fade must survive commit"
    );
    assert_eq!(
        rig.state.timecodes.snapshot(peer).unwrap().frame.0,
        22,
        "commit must preserve the latest live peer, not install its old discovery copy"
    );
}

async fn assert_cold_command_preload_go(osc: bool) {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    rig.app = router(rig.state.clone());
    rig.capture(osc).await;
    if osc {
        handle_control_event(
            &rig.state,
            ControlEvent::Osc {
                address: "/light/desk/programmer/preload".into(),
                arguments: vec![OscArgument::Bool(true)],
                source: Some("127.0.0.1:19141".into()),
            },
        );
    } else {
        let response = rig
            .post(
                "/api/v2/command-line/keys",
                serde_json::json!({"request_id":"cold-key-go","key":"PRE","phase":"press"}),
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            json(response).await
        );
    }
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
    assert_eq!(output.0.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn preload_transport_cold_software_key_uses_authorized_outside_gate_preflight() {
    assert_cold_command_preload_go(false).await;
}
#[tokio::test]
async fn preload_transport_cold_supported_osc_key_uses_authorized_outside_gate_preflight() {
    assert_cold_command_preload_go(true).await;
}

#[tokio::test]
async fn preload_transport_cold_off_does_not_read_or_prepare_an_unused_missing_audio_asset() {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    let active = rig.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&active.path).unwrap();
    let mut object = store
        .object("timecode", &rig.timecode.0.to_string())
        .unwrap()
        .unwrap();
    object.body["audio"]["asset_id"] = serde_json::json!(Uuid::new_v4());
    store
        .put_object("timecode", &object.id, &object.body, object.revision)
        .unwrap();
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    rig.state
        .programming
        .take_preload_playback_actions(rig.session.id);
    queue_transport(
        &rig,
        rig.address,
        light_programmer::PreloadPlaybackQueueAction::Off,
    );
    let result = preload::commit_preload(&rig.state, &rig.session);
    assert!(result.is_ok(), "{result:?}");
    assert!(
        rig.state.timecodes.snapshot(rig.timecode).is_err(),
        "an uninstalled stopped peer remains uninstalled"
    );
    assert!(
        output.0.lock().unwrap().is_empty(),
        "no unused voice/resource batch is accepted"
    );
}

#[tokio::test]
async fn preload_transport_cold_production_async_websocket_go_uses_authorized_preflight() {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let show = rig.state.active_show.current().clone().unwrap();
    let revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let id = "cold-websocket-go";
    let action=serde_json::from_value(serde_json::json!({"type":"programmer_preload_lifecycle","request":{"request_id":id,"expected_capture_mode_revision":1,"expected_values_revision":1,"expected_queue_revision":1,"expected_selection_revision":0,"action":{"type":"go","show_id":show.id,"expected_show_revision":revision,"expected_playback_event_sequence":rig.state.events.latest_sequence()}}})).unwrap();
    let frame = live_action_frame(&rig.session, id, action);
    let mut wrong = frame.clone();
    wrong.session_id = Uuid::new_v4();
    let refused = dispatch_live_action_live(&rig.state, &rig.session, wrong).await;
    assert!(!refused.ok);
    assert!(output.0.lock().unwrap().is_empty());
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    assert_eq!(
        output.1.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "invalid frames cannot decode assets"
    );
    let mut guest = rig.session.clone();
    guest.capability = light_core::SurfaceCapability::PlaybackOnly;
    let refused = dispatch_live_action_live(&rig.state, &guest, frame.clone()).await;
    assert!(!refused.ok);
    assert_eq!(
        output.1.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "Not Editable surfaces cannot decode Preload assets"
    );
    let response = dispatch_live_action_live(&rig.state, &rig.session, frame).await;
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(rig.timecode)
            .unwrap()
            .transport,
        TimecodeTransportState::Playing
    );
    assert_eq!(output.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn preload_transport_cold_extension_bound_key_uses_authorized_outside_gate_preflight() {
    use light_extensions_contract::{
        CanonicalControlIntent, ControlInput, ControlInputEvent, ProgrammerKey,
    };
    use light_extensions_host::{BoundControlInput, HostControlContext};
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let host = HostControlContext {
        extension_id: "de.tosklight.surface".into(),
        extension_instance_id: "preload-surface".into(),
        desk_id: rig.session.desk.id.to_string(),
        source: "native_extension",
    };
    extensions_runtime::apply_bound_control(
        &rig.state,
        &host,
        &BoundControlInput {
            input: ControlInputEvent {
                input_id: 796,
                occurred_at_micros: 0,
                control: ControlInput::Button {
                    control_id: "preload".into(),
                    pressed: true,
                },
            },
            intent: CanonicalControlIntent::ProgrammerKey {
                key: ProgrammerKey::Preload,
            },
        },
    )
    .unwrap();
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
    assert_eq!(output.0.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preload_transport_async_websocket_rechecks_desk_lock_after_resource_await_without_mutation()
 {
    let mut rig = TransportRig::new().await;
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let batches: Arc<std::sync::Mutex<Vec<Vec<light_application::TimecodeAudioCommand>>>> =
        Arc::default();
    install_cold_audio(
        &mut rig,
        Arc::new(BlockingResourceOutput {
            entered: entered_tx,
            release: release.clone(),
            batches: batches.clone(),
        }),
    );
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let show = rig.state.active_show.current().clone().unwrap();
    let revision = ShowStore::open(&show.path)
        .unwrap()
        .portable_revision()
        .unwrap()
        .value();
    let id = "locked-after-decode";
    let action=serde_json::from_value(serde_json::json!({"type":"programmer_preload_lifecycle","request":{"request_id":id,"expected_capture_mode_revision":1,"expected_values_revision":1,"expected_queue_revision":1,"expected_selection_revision":0,"action":{"type":"go","show_id":show.id,"expected_show_revision":revision,"expected_playback_event_sequence":rig.state.events.latest_sequence()}}})).unwrap();
    let frame = live_action_frame(&rig.session, id, action);
    let (state, session) = (rig.state.clone(), rig.session.clone());
    let action =
        tokio::spawn(async move { dispatch_live_action_live(&state, &session, frame).await });
    tokio::task::spawn_blocking(move || entered_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    let pending = rig
        .state
        .programming
        .preload_playback_actions(rig.session.id)
        .unwrap();
    write_desk_lock(
        &rig.state,
        &DeskLockConfiguration {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
    let refused = action.await.unwrap();
    assert!(!refused.ok);
    assert!(refused.error.unwrap().contains("locked"));
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    assert_eq!(
        rig.state
            .programming
            .preload_playback_actions(rig.session.id)
            .unwrap(),
        pending
    );
    assert!(batches.lock().unwrap().is_empty());
}

#[tokio::test]
async fn preload_transport_cold_resource_refusal_keeps_normal_software_rejected_outcome_and_pending_work()
 {
    let mut rig = TransportRig::new().await;
    let output = Arc::new(AtomicTransportOutput::default());
    install_cold_audio(&mut rig, output.clone());
    let active = rig.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&active.path).unwrap();
    let mut object = store
        .object("timecode", &rig.timecode.0.to_string())
        .unwrap()
        .unwrap();
    object.body["audio"]["asset_id"] = serde_json::json!(Uuid::new_v4());
    store
        .put_object("timecode", &object.id, &object.body, object.revision)
        .unwrap();
    rig.app = router(rig.state.clone());
    rig.capture(false).await;
    let queue = rig
        .state
        .programming
        .preload_playback_actions(rig.session.id)
        .unwrap();
    let response = rig
        .post(
            "/api/v2/command-line/keys",
            serde_json::json!({"request_id":"refused-cold-key","key":"PRE","phase":"press"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["outcome"], "rejected", "{body}");
    assert_eq!(
        rig.state
            .programming
            .preload_playback_actions(rig.session.id)
            .unwrap(),
        queue
    );
    assert!(
        rig.state
            .programming
            .capture_mode(rig.session.id)
            .unwrap()
            .blind
    );
    assert!(rig.state.timecodes.snapshot(rig.timecode).is_err());
    assert!(output.0.lock().unwrap().is_empty());
}
