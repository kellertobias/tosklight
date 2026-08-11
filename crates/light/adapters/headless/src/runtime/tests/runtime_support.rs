fn test_state() -> (AppState, PathBuf) {
    test_state_with_programmers(ProgrammerRegistry::default(), None)
}

fn persist_test_virtual_playback_exclusions(
    state: &AppState,
    show_id: light_core::ShowId,
    store: &VirtualPlaybackExclusionStore,
) {
    let entry = state.active_show.current().unwrap();
    let show_store = match light_show::ShowStore::open(&entry.path) {
        Ok(show_store) => show_store,
        Err(_) => {
            light_show::ShowStore::create(&entry.path, &entry.name)
                .unwrap()
                .0
        }
    };
    show_store.set_identity(show_id, &entry.name, None).unwrap();
    state.active_show.clear_document_cache();
    update_virtual_playback_exclusions(
        state,
        show_id,
        0,
        &store.zones,
        "test-virtual-playback-exclusions",
    )
    .unwrap();
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
    let engine = Arc::new(Engine::new(programmers.clone()));
    let application_events = EventBus::default();
    let active_show_service = ActiveShowService::new(application_events.clone());
    let highlight = Arc::new(HighlightRegistry::default());
    let programming = ProgrammingService::new(
        programmers.clone(),
        application_events.clone(),
        Arc::clone(&highlight),
    );
    let output_rate = Arc::new(AtomicU16::new(44));
    let active_show_service_for_patch = active_show_service.clone();
    (
        AppState {
            action_timing: ActionTimingResource::default(),
            attributes: AttributeConfigurationResource::new(
                crate::runtime::attribute_configuration::InstalledAttributeConfiguration::recommended(
                    None, 0,
                ),
            ),
            installation: InstallationResource::open_test_installation(data_dir.clone()).unwrap(),
            sessions: SessionResource::new(),
            dynamics: light_application::DynamicsService::new(programmers.clone()),
            macros: light_application::CommandMacroExecutionService::default(),
            timecodes: crate::runtime::timecode_v2::new_service_with_clock(
                Arc::new(light_application::timeline::SystemTimecodeClock::default()),
                None,
                application_events.clone(),
            ),
            managed_assets: Arc::new(
                light_application::FilesystemManagedAssetStore::open(
                    data_dir.join("managed-assets"),
                )
                .unwrap(),
            ),
            programming: ProgrammingResource::new(programmers, programming),
            playback: PlaybackResource::new(
                PlaybackService::new(application_events.clone()),
                PlaybackTopologyService::new(active_show_service.clone()),
                Arc::new(
                    super::playback_telemetry::PlaybackTelemetrySampler::new(
                        Arc::clone(&output_rate),
                    ),
                ),
            ),
            highlight: HighlightResource::new(highlight),
            output: OutputResource::new(
                OutputRuntimeService::new(application_events.clone()),
                SpeedGroupService::new(application_events.clone()),
                engine,
                Arc::new(std::sync::Mutex::new(OutputHealth::default())),
                output_rate,
                OutputControlCapability::new(Arc::new(Mutex::new(OutputControl::default()))),
                Arc::new(Mutex::new(TimecodeRouter::default())),
                None,
                Arc::new(light_output::UsbOutputFanout::new(Arc::new(
                    light_output::UnavailableUsbDriverFactory,
                ))),
                Arc::default(),
                manual_clock,
                Arc::new(Mutex::new(std::array::from_fn(|index| {
                    SpeedGroupController::new(
                        default_speed_groups()[index],
                        SoundToLightConfig::default(),
                    )
                    .unwrap()
                }))),
                Arc::new(Mutex::new(light_dynamics::DynamicRuntime::default())),
                Arc::new(Mutex::new(Vec::new())),
                Arc::new(crate::runtime::visualization_frame::VisualizationFrameHub::default()),
            ),
            active_show: ActiveShowResource::new(
                ActiveShowCoordinator::new(),
                Arc::default(),
                None,
                active_show_service.clone(),
                ShowPatchService::new(active_show_service_for_patch),
                SelectiveShowImportService::new(active_show_service),
            ),
            events: EventResource::new(application_events),
            extensions: crate::runtime::extensions_runtime::ExtensionResource::start(
                data_dir.join("extensions"),
                data_dir.join("extensions.json"),
            ),
            integrations: IntegrationResource::new(
                Arc::new(matter::MatterBridgeAdapter::default()),
                None,
                None,
            ),
            media: MediaResource::default(),
            replay: ReplayResource::default(),
            lifecycle: LifecycleResource::new(CancellationToken::new()),
            // A test desk announces nothing and looks for nothing: the network is not part of
            // what is under test, and a responder per test would be.
            discovery: crate::runtime::discovery_http::DiscoveryResource::default(),
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
        state.events.replay(after_sequence, &filter)
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
