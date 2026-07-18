#[derive(Clone, Copy)]
struct CueTransferCase {
    operation: &'static str,
    mode: &'static str,
    moves: bool,
    status: bool,
}

struct CueTransferBaseline {
    source_body: serde_json::Value,
    source_revision: u64,
    destination_body: serde_json::Value,
    destination_revision: u64,
}

struct CueTransferScenario {
    state: AppState,
    data_dir: PathBuf,
    session: Session,
    show_path: PathBuf,
    fixtures: [light_core::FixtureId; 3],
    source_cue_id: Uuid,
}

impl CueTransferScenario {
    fn new() -> Self {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let desk = state
            .desk
            .lock()
            .add_desk("Cue transfer", "cue-transfer")
            .unwrap();
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "cue-transfer".into(),
            connected: true,
            desk,
        };
        state.programmers.start(session.id, user.id);
        let fixtures = [
            light_core::FixtureId::new(),
            light_core::FixtureId::new(),
            light_core::FixtureId::new(),
        ];
        let (source, destination, source_cue_id) = transfer_cue_lists(fixtures);
        let show_path = data_dir.join("shows/cue-transfer.show");
        let show_id = initialise_show(&show_path, "Cue transfer").unwrap();
        let store = ShowStore::open(&show_path).unwrap();
        for list in [&source, &destination] {
            store
                .put_object(
                    "cue_list",
                    &list.id.0.to_string(),
                    &serde_json::to_value(list).unwrap(),
                    0,
                )
                .unwrap();
        }
        for definition in [
            transfer_playback(1, source.id),
            transfer_playback(2, destination.id),
        ] {
            store
                .put_object(
                    "playback",
                    &definition.number.to_string(),
                    &serde_json::to_value(&definition).unwrap(),
                    0,
                )
                .unwrap();
        }
        let entry = ShowEntry {
            id: show_id,
            name: "Cue transfer".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();
        Self {
            state,
            data_dir,
            session,
            show_path,
            fixtures,
            source_cue_id,
        }
    }

    fn baseline(&self) -> CueTransferBaseline {
        let store = ShowStore::open(&self.show_path).unwrap();
        let (_, source, _) = cue_list_for_playback(&store, &self.state.engine.snapshot(), 1).unwrap();
        let (_, destination, _) =
            cue_list_for_playback(&store, &self.state.engine.snapshot(), 2).unwrap();
        CueTransferBaseline {
            source_body: source.body,
            source_revision: source.revision,
            destination_body: destination.body,
            destination_revision: destination.revision,
        }
    }
}

fn transfer_cue_lists(
    fixtures: [light_core::FixtureId; 3],
) -> (light_playback::CueList, light_playback::CueList, Uuid) {
    let intensity = light_core::AttributeKey::intensity();
    let mut source_one = transfer_cue(1.0, fixtures[0], 1.0);
    source_one
        .group_changes
        .push(transfer_group_change("1", 1.0));
    let mut source_two = transfer_cue(2.0, fixtures[1], 1.0);
    source_two
        .group_changes
        .push(transfer_group_change("2", 1.0));
    let source_cue_id = source_two.id;
    let source_three = transfer_cue(3.0, fixtures[0], 0.0);
    let mut destination_one = transfer_cue(1.0, fixtures[0], 0.0);
    destination_one.changes.push(light_playback::CueChange::set(
        fixtures[2],
        intensity,
        light_core::AttributeValue::Normalized(1.0),
    ));
    destination_one.group_changes.extend([
        transfer_group_change("1", 0.0),
        transfer_group_change("3", 1.0),
    ]);
    let source_id = light_core::CueListId::new();
    let destination_id = light_core::CueListId::new();
    (
        transfer_cue_list(
            source_id,
            "Source",
            vec![source_one, source_two, source_three],
        ),
        transfer_cue_list(destination_id, "Destination", vec![destination_one]),
        source_cue_id,
    )
}

fn transfer_cue(
    number: f64,
    fixture: light_core::FixtureId,
    value: f32,
) -> light_playback::Cue {
    let mut cue = light_playback::Cue::new(number);
    cue.changes.push(light_playback::CueChange::set(
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(value),
    ));
    cue
}

fn transfer_group_change(group_id: &str, value: f32) -> light_playback::GroupCueChange {
    light_playback::GroupCueChange {
        group_id: group_id.into(),
        attribute: light_core::AttributeKey::intensity(),
        value: Some(light_core::AttributeValue::Normalized(value)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn transfer_cue_list(
    id: light_core::CueListId,
    name: &str,
    cues: Vec<light_playback::Cue>,
) -> light_playback::CueList {
    light_playback::CueList {
        id,
        name: name.into(),
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
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
    }
}

fn transfer_playback(
    number: u16,
    cue_list_id: light_core::CueListId,
) -> light_playback::PlaybackDefinition {
    light_playback::PlaybackDefinition {
        number,
        name: format!("Cuelist {number}"),
        target: light_playback::PlaybackTarget::CueList { cue_list_id },
        buttons: [light_playback::PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
