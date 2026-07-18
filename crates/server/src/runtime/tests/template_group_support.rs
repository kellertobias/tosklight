fn template_parameter(
    attribute: &str,
    offsets: &[u16],
    default: f32,
    virtual_dimmer: bool,
) -> light_fixture::Parameter {
    light_fixture::Parameter {
        attribute: light_core::AttributeKey(attribute.into()),
        components: offsets
            .iter()
            .map(|offset| light_fixture::ChannelComponent {
                offset: *offset,
                byte_order: light_fixture::ByteOrder::MsbFirst,
            })
            .collect(),
        default,
        virtual_dimmer,
        metadata: light_fixture::ParameterMetadata::default(),
        capabilities: vec![],
    }
}

fn template_fixture(
    name: String,
    address: u16,
    parameters: Vec<light_fixture::Parameter>,
    footprint: u16,
) -> light_fixture::PatchedFixture {
    light_fixture::PatchedFixture {
        name: name.clone(),
        layer_id: "default".into(),
        fixture_id: light_core::FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        definition: light_fixture::FixtureDefinition {
            schema_version: 1,
            id: light_core::FixtureId::new(),
            revision: 1,
            manufacturer: "Scenario Test".into(),
            device_type: "other".into(),
            name: name.clone(),
            model: name,
            mode: format!("{footprint} channel"),
            footprint,
            heads: vec![light_fixture::LogicalHead {
                index: 0,
                name: "Main".into(),
                shared: true,
                parameters,
            }],
            color_calibration: None,
            physical: Default::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: vec![],
            signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
            safe_values: BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        },
        universe: Some(1),
        address: Some(address),
        split_patches: Vec::new(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
        multipatch: vec![],
    }
}

fn template_dimmer(name: &str, address: u16) -> light_fixture::PatchedFixture {
    template_fixture(
        name.into(),
        address,
        vec![template_parameter("intensity", &[0], 0.0, false)],
        1,
    )
}

fn template_profile(number: usize, address: u16) -> light_fixture::PatchedFixture {
    template_fixture(
        format!("Profile {number}"),
        address,
        vec![
            template_parameter("pan", &[0, 1], 0.5, false),
            template_parameter("tilt", &[2, 3], 0.5, false),
            template_parameter("intensity", &[4], 0.0, false),
            template_parameter("shutter", &[5], 1.0, false),
            template_parameter("color.emitter.red", &[6], 0.0, false),
            template_parameter("color.emitter.green", &[7], 0.0, false),
            template_parameter("color.emitter.blue", &[8], 0.0, false),
            template_parameter("gobo", &[9], 0.0, false),
            template_parameter("zoom", &[10], 0.0, false),
            template_parameter("focus", &[11], 0.0, false),
        ],
        12,
    )
}

fn template_led(number: usize, address: u16) -> light_fixture::PatchedFixture {
    template_fixture(
        format!("RGBW LED PAR {number}"),
        address,
        vec![
            template_parameter("color.emitter.red", &[0], 0.0, true),
            template_parameter("color.emitter.green", &[1], 0.0, true),
            template_parameter("color.emitter.blue", &[2], 0.0, true),
            template_parameter("color.emitter.white", &[3], 0.0, true),
        ],
        4,
    )
}

fn template_word(frame: &light_output::DmxFrame, address: u16) -> u16 {
    let offset = usize::from(address - 1);
    u16::from_be_bytes([frame[offset], frame[offset + 1]])
}

fn template_group_preset() -> light_programmer::Preset {
    let white = [
        ("intensity", light_core::AttributeValue::Normalized(1.0)),
        (
            "color.emitter.red",
            light_core::AttributeValue::Normalized(1.0),
        ),
        (
            "color.emitter.green",
            light_core::AttributeValue::Normalized(1.0),
        ),
        (
            "color.emitter.blue",
            light_core::AttributeValue::Normalized(1.0),
        ),
    ]
    .into_iter()
    .collect::<HashMap<_, _>>();
    let mut led_white = white.clone();
    led_white.insert(
        "color.emitter.white",
        light_core::AttributeValue::Normalized(1.0),
    );
    let values = |source: &HashMap<&str, light_core::AttributeValue>| {
        source
            .iter()
            .map(|(key, value)| (light_core::AttributeKey((*key).into()), value.clone()))
            .collect()
    };
    light_programmer::Preset {
        name: "All white at full".into(),
        family: light_programmer::PresetFamily::Mixed,
        number: 1,
        values: HashMap::new(),
        group_values: HashMap::from([
            (
                "front".into(),
                HashMap::from([(
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(1.0),
                )]),
            ),
            ("profile".into(), values(&white)),
            ("leds".into(), values(&led_white)),
        ]),
    }
}

fn template_cue_list(
    id: light_core::CueListId,
    preset: &light_programmer::Preset,
) -> light_playback::CueList {
    let mut cue = light_playback::Cue::new(1.0);
    cue.name = "All groups white".into();
    cue.trigger = light_playback::CueTrigger::Manual;
    cue.group_changes = preset
        .group_values
        .iter()
        .flat_map(|(group_id, values)| {
            values
                .iter()
                .map(move |(attribute, value)| light_playback::GroupCueChange {
                    group_id: group_id.clone(),
                    attribute: attribute.clone(),
                    value: Some(value.clone()),
                    automatic_restore: false,
                    fade_millis: None,
                    delay_millis: None,
                })
        })
        .collect();
    light_playback::CueList {
        id,
        name: "Main".into(),
        priority: 0,
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

struct TemplateGroupScenario {
    state: AppState,
    app: Router,
    token: String,
    session_id: SessionId,
    entry: ShowEntry,
    store: ShowStore,
    data_dir: PathBuf,
    dimmers: Vec<light_fixture::PatchedFixture>,
    profiles: Vec<light_fixture::PatchedFixture>,
    leds: Vec<light_fixture::PatchedFixture>,
    empty_groups: [light_programmer::GroupDefinition; 3],
    populated_groups: [light_programmer::GroupDefinition; 3],
    cue_list_id: light_core::CueListId,
}

impl TemplateGroupScenario {
    async fn new() -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let created = create_show(&app, &token, "Template group scenario").await;
        let show_id = light_core::ShowId(
            Uuid::parse_str(created["id"].as_str().unwrap()).unwrap(),
        );
        let entry = state.desk.lock().show(show_id).unwrap().unwrap();
        let store = ShowStore::open(&entry.path).unwrap();
        let dimmers = ["Front Left", "Front Mid Left", "Front Mid Right", "Front Right"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| template_dimmer(name, index as u16 + 1))
            .collect::<Vec<_>>();
        let profiles = (0..6)
            .map(|index| template_profile(index + 1, 5 + index as u16 * 12))
            .collect::<Vec<_>>();
        let leds = (0..16)
            .map(|index| template_led(index + 1, 77 + index as u16 * 4))
            .collect::<Vec<_>>();
        let empty_groups = [
            ("front", "Front Light", 1),
            ("leds", "LEDs", 2),
            ("profile", "Profile", 3),
        ]
        .map(|(id, name, fader)| light_programmer::GroupDefinition {
            id: id.into(),
            name: name.into(),
            fixtures: vec![],
            master: 0.0,
            playback_fader: Some(fader),
            ..Default::default()
        });
        let populated_groups = [
            group_with_fixtures(&empty_groups[0], &dimmers),
            group_with_fixtures(&empty_groups[1], &leds),
            group_with_fixtures(&empty_groups[2], &profiles),
        ];
        Self {
            state,
            app,
            token,
            session_id: SessionId(Uuid::parse_str(&session_id).unwrap()),
            entry,
            store,
            data_dir,
            dimmers,
            profiles,
            leds,
            empty_groups,
            populated_groups,
            cue_list_id: light_core::CueListId::new(),
        }
    }

    fn cue_object_id(&self) -> String {
        self.cue_list_id.0.to_string()
    }
}

fn group_with_fixtures(
    group: &light_programmer::GroupDefinition,
    fixtures: &[light_fixture::PatchedFixture],
) -> light_programmer::GroupDefinition {
    light_programmer::GroupDefinition {
        fixtures: fixtures.iter().map(|fixture| fixture.fixture_id).collect(),
        master: 1.0,
        ..group.clone()
    }
}
