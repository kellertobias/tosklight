fn operational_fixture(fixture_id: light_core::FixtureId) -> light_fixture::PatchedFixture {
    light_fixture::PatchedFixture {
        name: "Media Server".into(),
        layer_id: "default".into(),
        fixture_id,
        fixture_number: None,
        virtual_fixture_number: None,
        definition: light_fixture::FixtureDefinition {
            schema_version: 1,
            id: light_core::FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
            device_type: "dimmer".into(),
            name: "Dimmer".into(),
            model: "Dimmer".into(),
            mode: "1ch".into(),
            footprint: 1,
            heads: vec![light_fixture::LogicalHead {
                index: 0,
                name: "Main".into(),
                shared: true,
                parameters: vec![light_fixture::Parameter {
                    attribute: light_core::AttributeKey::intensity(),
                    components: vec![light_fixture::ChannelComponent {
                        offset: 0,
                        byte_order: light_fixture::ByteOrder::MsbFirst,
                    }],
                    default: 0.0,
                    virtual_dimmer: false,
                    metadata: light_fixture::ParameterMetadata::default(),
                    capabilities: vec![],
                }],
            }],
            color_calibration: None,
            physical: Default::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
            safe_values: std::collections::BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        },
        universe: Some(1),
        address: Some(1),
        split_patches: Vec::new(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        move_in_black_enabled: true,
        highlight_overrides: Default::default(),
        move_in_black_delay_millis: 0,
        multipatch: vec![],
    }
}

fn operational_cue_list(
    cue_list_id: light_core::CueListId,
    fixture_id: light_core::FixtureId,
) -> light_playback::CueList {
    let mut cue = light_playback::Cue::new(1.0);
    cue.changes.push(light_playback::CueChange::set(
        fixture_id,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(1.0),
    ));
    light_playback::CueList {
        id: cue_list_id,
        name: "Main".into(),
        priority: 10,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_step_millis: 1_000,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_group: None,
        speed_multiplier: 1.0,
        cues: vec![cue],
    }
}

fn operational_route() -> light_output::OutputRoute {
    light_output::OutputRoute {
        protocol: light_output::Protocol::Sacn,
        logical_universe: 1,
        destination_universe: 1,
        delivery_mode: Some(light_output::DeliveryMode::Multicast),
        destination: None,
        enabled: true,
        minimum_slots: 512,
    }
}

struct OperationalScenario {
    state: AppState,
    app: Router,
    token: String,
    session_id: String,
    first_id: String,
    fixture_id: light_core::FixtureId,
    cue_list_id: light_core::CueListId,
    data_dir: PathBuf,
}

impl OperationalScenario {
    async fn new() -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let first = create_show(&app, &token, "Programmed").await;
        Self {
            state,
            app,
            token,
            session_id,
            first_id: first["id"].as_str().unwrap().to_owned(),
            fixture_id: light_core::FixtureId::new(),
            cue_list_id: light_core::CueListId::new(),
            data_dir,
        }
    }
}
