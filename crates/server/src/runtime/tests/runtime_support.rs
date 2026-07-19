fn test_state() -> (AppState, PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("light-server-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let desk = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let fixture_library =
        light_fixture::FixtureLibrary::open(data_dir.join("fixtures.sqlite")).unwrap();
    let programmers = ProgrammerRegistry::default();
    let engine = Arc::new(Engine::new(programmers.clone()));
    let (events, _) = broadcast::channel(32);
    let application_events = EventBus::default();
    let active_show_service = ActiveShowService::new(application_events.clone());
    (
        AppState {
            desk: Arc::new(Mutex::new(desk)),
            fixture_library: Arc::new(Mutex::new(fixture_library)),
            data_dir: data_dir.clone(),
            sessions: Arc::default(),
            session_clients: Arc::default(),
            ws_connections: Arc::new(Mutex::new(HashMap::new())),
            programmers: programmers.clone(),
            programming: ProgrammingService::new(programmers),
            playback_service: PlaybackService::new(application_events.clone()),
            engine,
            highlight: Arc::new(HighlightRegistry::default()),
            patch_preview_highlights: Arc::default(),
            output_health: Arc::new(std::sync::Mutex::new(OutputHealth::default())),
            output_rate: Arc::new(AtomicU16::new(44)),
            configuration: Arc::new(RwLock::new(DeskConfiguration::default())),
            matter_bridge: Arc::new(matter::MatterBridgeAdapter::default()),
            matter_transport: None,
            output_control: Arc::new(Mutex::new(OutputControl::default())),
            activation_lock: Arc::new(tokio::sync::Mutex::new(())),
            playback_action_lock: Arc::new(Mutex::new(())),
            timecode_router: Arc::new(Mutex::new(TimecodeRouter::default())),
            active_show: Arc::default(),
            active_show_error: Arc::default(),
            events,
            application_events: application_events.clone(),
            active_show_service: active_show_service.clone(),
            show_patch: ShowPatchService::new(active_show_service),
            patch_profile_resolution: Arc::default(),
            active_show_http_lifecycle: Arc::default(),
            patch_lifecycle: Arc::default(),
            audit_events: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
            command_history: Arc::new(Mutex::new(HashMap::new())),
            event_revision: Arc::new(AtomicU64::new(0)),
            desk_token: None,
            shutdown: CancellationToken::new(),
            media_cache: Arc::new(Mutex::new(MediaCache::default())),
            media_status: Arc::new(RwLock::new(HashMap::new())),
            input_locks: Arc::new(Mutex::new(HashMap::new())),
            file_input_contexts: Arc::new(Mutex::new(HashMap::new())),
            osc_subscribers: Arc::new(Mutex::new(HashMap::new())),
            osc_feedback: None,
            osc_feedback_capture: Arc::new(Mutex::new(Vec::new())),
            mvr_imports: Arc::new(Mutex::new(HashMap::new())),
            network_output: None,
            output_sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            manual_clock: None,
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
