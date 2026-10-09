pub(in crate::runtime) fn test_state() -> (AppState, PathBuf) {
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
    test_state_with_programming_contract(programmers, manual_clock, light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT)
}

/// TL-548 C3: contract 1 AND the explicit all-family Live opt-in. `test_state` stays legacy.
pub(in crate::runtime) fn test_state_with_family_adapters(
    programmers: ProgrammerRegistry,
    manual_clock: Option<Arc<ManualClock>>,
    supported_contract: u16,
) -> (AppState, PathBuf) {
    let (mut state, data_dir) =
        test_state_with_programming_contract(programmers, manual_clock, supported_contract);
    let adapters = Arc::new(output_scheduler::LiveFamilyAdapters::new(true));
    state.output = state.output.with_live_family_adapters(adapters);
    (state, data_dir)
}

fn test_state_with_programming_contract(
    programmers: ProgrammerRegistry,
    manual_clock: Option<Arc<ManualClock>>,
    supported_contract: u16,
) -> (AppState, PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("light-headless-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(data_dir.join("shows")).unwrap();
    let engine = Arc::new(Engine::with_programming_contract_support(programmers.clone(), supported_contract));
    let dynamic_snapshot = Arc::new(DynamicSnapshotPublication::new(engine.snapshot()));
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
    let state = (
        AppState {
            action_timing: ActionTimingResource::default(),
            attributes: AttributeConfigurationResource::new(
                crate::runtime::attribute_configuration::InstalledAttributeConfiguration::recommended(
                    None, 0,
                ),
            ),
            installation: InstallationResource::open_test_installation(data_dir.clone()).unwrap(),
            psn: crate::runtime::psn::service::PsnResource::new(),
            sessions: SessionResource::new(),
            dynamics: light_application::DynamicsService::new(programmers.clone()),
            macros: light_application::CommandMacroExecutionService::default(),
            timecodes: crate::runtime::timecode_v2::new_service_with_clock(
                crate::runtime::timecode_clock::runtime_clock(manual_clock.as_ref()),
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
            fixture_freeze_history: Default::default(),
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
                dynamic_snapshot,
                Arc::new(arc_swap::ArcSwap::from_pointee(
                    crate::runtime::dynamic_source_origins::DynamicSourceOrigins::default(),
                )),
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
            internal_audio: InternalAudioResource::new(Arc::new(Mutex::new(
                crate::runtime::internal_audio::InternalAudioRuntime::default(),
            ))),
            replay: ReplayResource::default(),
            lifecycle: LifecycleResource::new(CancellationToken::new()),
            // A test desk announces nothing and looks for nothing: the network is not part of
            // what is under test, and a responder per test would be.
            discovery: crate::runtime::discovery_http::DiscoveryResource::default(),
        },
        data_dir,
    );
    // TL-552: synthetic test states engage the show-activation legacy-programming gate at their
    // engine contract, exactly like `build_app_state` does for a real startup.
    state.0.active_show.engage_legacy_programming_gate(supported_contract);
    state
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

/// Read the independent operator pool, retaining the actual optional physical assignment.
fn stored_cuelist_pool(
    store: &ActiveShowRepository,
    snapshot: &light_engine::EngineSnapshot,
    number: u16,
) -> Result<(Option<light_playback::PlaybackDefinition>, light_show::VersionedObject, light_playback::CueList), String> {
    let id = show_command_update::cuelist_pool_id(snapshot, number)?;
    let object = store.objects("cue_list").map_err(|error| error.to_string())?.into_iter()
        .find(|object| object.body["id"] == id.0.to_string()).ok_or("Cuelist does not exist")?;
    let list = serde_json::from_value(object.body.clone()).map_err(|error| error.to_string())?;
    let assignment = snapshot.playbacks.iter().find(|playback| playback.target == light_playback::PlaybackTarget::CueList { cue_list_id: id }).cloned();
    Ok((assignment, object, list))
}
