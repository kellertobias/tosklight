fn test_state() -> (AppState, PathBuf) {
    test_state_with_programmers(ProgrammerRegistry::default(), None)
}

fn live_action_frame(
    session: &Session,
    request_id: impl Into<String>,
    action: light_wire::v2::live_action::LiveAction,
) -> light_wire::v2::live_action::LiveActionFrame {
    light_wire::v2::live_action::LiveActionFrame {
        message_type: light_wire::v2::live_action::LiveActionMessageType::Action,
        protocol_version: 2,
        request_id: request_id.into(),
        session_id: session.id.0,
        action,
    }
}

fn test_state_with_clock(clock: Arc<ManualClock>) -> (AppState, PathBuf) {
    test_state_with_programmers(ProgrammerRegistry::with_clock(clock.clone()), Some(clock))
}

fn test_state_with_programmers(
    programmers: ProgrammerRegistry,
    manual_clock: Option<Arc<ManualClock>>,
) -> (AppState, PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("light-headless-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let fixture_library =
        light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite")).unwrap();
    let engine = Arc::new(Engine::new(programmers.clone()));
    let application_events = EventBus::default();
    let active_show_service = ActiveShowService::new(application_events.clone());
    let highlight = Arc::new(HighlightRegistry::default());
    let highlight_service = light_application::HighlightService::new(Arc::clone(&highlight));
    let programming = ProgrammingService::new(
        programmers.clone(),
        application_events.clone(),
        Arc::clone(&highlight),
    );
    let output_rate = Arc::new(AtomicU16::new(44));
    (
        AppState {
            desk: Arc::new(Mutex::new(desk)),
            fixture_library: Arc::new(Mutex::new(fixture_library)),
            data_dir: data_dir.clone(),
            sessions: Arc::default(),
            session_clients: Arc::default(),
            programmers: programmers.clone(),
            programming,
            playback_service: PlaybackService::new(application_events.clone()),
            output_runtime_service: OutputRuntimeService::new(application_events.clone()),
            speed_group_service: SpeedGroupService::new(application_events.clone()),
            engine,
            highlight,
            highlight_service,
            patch_preview_highlights: Arc::default(),
            output_health: Arc::new(std::sync::Mutex::new(OutputHealth::default())),
            output_rate: Arc::clone(&output_rate),
            playback_telemetry: Arc::new(super::playback_telemetry::PlaybackTelemetrySampler::new(
                output_rate,
            )),
            configuration: Arc::new(RwLock::new(DeskConfiguration::default())),
            matter_bridge: Arc::new(matter::MatterBridgeAdapter::default()),
            matter_transport: None,
            output_control: Arc::new(Mutex::new(OutputControl::default())),
            output_runtime_persistence_attempts: Arc::new(AtomicU64::new(0)),
            output_runtime_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            speed_group_persistence_attempts: Arc::new(AtomicU64::new(0)),
            speed_group_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            activation_lock: Arc::new(tokio::sync::Mutex::new(())),
            timecode_router: Arc::new(Mutex::new(TimecodeRouter::default())),
            active_show: Arc::default(),
            active_show_document: Arc::default(),
            active_show_backup_checkpoint: Arc::default(),
            active_show_error: Arc::default(),
            application_events: application_events.clone(),
            active_show_service: active_show_service.clone(),
            playback_topology: PlaybackTopologyService::new(active_show_service.clone()),
            show_patch: ShowPatchService::new(active_show_service.clone()),
            show_library_replay: Arc::default(),
            fixture_library_replay: Arc::default(),
            show_object_replay: Arc::default(),
            show_object_intent_replay: Arc::default(),
            preset_generation_replay: Arc::default(),
            screen_configuration_replay: Arc::default(),
            control_desk_configuration_replay: Arc::default(),
            desk_management_replay: Arc::default(),
            stage_layout_replay: Arc::default(),
            virtual_playback_zones_replay: Arc::default(),
            selective_show_import: SelectiveShowImportService::new(active_show_service),
            patch_profile_resolution: Arc::default(),
            active_show_http_lifecycle: Arc::default(),
            preload_store_release_lifecycle: Arc::default(),
            patch_lifecycle: Arc::default(),
            audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
            command_history: Arc::new(Mutex::new(HashMap::new())),
            event_revision: Arc::new(AtomicU64::new(0)),
            desk_token: None,
            shutdown: CancellationToken::new(),
            media_cache: Arc::new(Mutex::new(MediaCache::default())),
            media_status: Arc::new(RwLock::new(HashMap::new())),
            file_input_contexts: Arc::new(Mutex::new(HashMap::new())),
            osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
            osc_cue_record_suppression: Arc::default(),
            osc_feedback: None,
            osc_feedback_capture: Arc::new(Mutex::new(Vec::new())),
            mvr_imports: Arc::new(Mutex::new(HashMap::new())),
            network_output: None,
            output_sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            manual_clock,
            test_clock_lock: Arc::new(tokio::sync::Mutex::new(())),
            speed_groups: Arc::new(Mutex::new(std::array::from_fn(|index| {
                SpeedGroupController::new(
                    default_speed_groups()[index],
                    SoundToLightConfig::default(),
                )
                .unwrap()
            }))),
            sound_capture_owners: Arc::new(Mutex::new([None; 5])),
        },
        data_dir,
    )
}

fn assert_programming_selection_event(
    state: &AppState,
    session: &Session,
    after_sequence: u64,
    source: light_application::ActionSource,
    expected_selection: &[light_core::FixtureId],
) {
    let filter = light_application::EventFilter::for_desk(session.desk.id).with_object(
        light_application::EventObject::programming_selection(session.desk.id),
    );
    let light_application::EventReplay::Events(events) =
        state.application_events.replay(after_sequence, &filter)
    else {
        panic!("expected a replayable Programming selection event");
    };
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.desk_id, Some(session.desk.id));
    assert_eq!(event.source, light_application::EventSource::Action(source));
    assert!(event.correlation_id.is_some());
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::InteractionChanged(change),
    ) = &event.payload
    else {
        panic!("expected a Programming interaction change");
    };
    assert!(change.command_line().is_none());
    assert_eq!(
        change.selection().unwrap().selected,
        expected_selection,
        "the event must carry the authoritative post-interaction selection"
    );
}
