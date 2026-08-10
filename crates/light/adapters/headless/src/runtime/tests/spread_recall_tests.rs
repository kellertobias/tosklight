// Plan-50 acceptance coverage item 7: a live Group holding a multi-point
// `AttributeValue::Spread` resolves through the deterministic anchor rule when recalled
// directly, from a Preset, and from a Cue — before and after its ordered membership is edited —
// while `DEGRP` freezes per-fixture values that must not follow later membership edits.

struct SpreadRecallRig {
    state: AppState,
    session: Session,
    data_dir: PathBuf,
    entry: ShowEntry,
    /// Front Fresnel dimmers 1..=6 from the default show, in fixture-number order.
    fixtures: Vec<light_core::FixtureId>,
}

impl SpreadRecallRig {
    /// Default show with a persisted Group 1 whose ordered members are fresnels 1..=5.
    fn new(name: &str) -> Self {
        let (state, data_dir) = test_state();
        let user = state.installation.users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "spread-recall".into(),
            connected: true,
            desk: test_control_desk(),
        };
        state.programming.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        // Resolve command values instantly; the spread rule under test is fade-independent.
        state.installation.update_configuration(|configuration| configuration.programmer_fade_millis = 0);
        let show_path = data_dir.join(format!("shows/{name}.show"));
        let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
        let entry = ShowEntry {
            id: show_id,
            name: name.into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        let snapshot = load_engine_snapshot(&entry).unwrap();
        let fixtures = (1..=6)
            .map(|number| {
                snapshot
                    .fixtures
                    .iter()
                    .find(|fixture| fixture.fixture_number == Some(number))
                    .unwrap()
                    .fixture_id
            })
            .collect::<Vec<_>>();
        state.active_show.replace_current(Some(entry.clone()));
        let rig = Self {
            state,
            session,
            data_dir,
            entry,
            fixtures,
        };
        rig.set_membership(rig.fixtures[..5].to_vec());
        rig
    }

    /// Persists the ordered membership of Group 1 and recompiles the engine snapshot, matching
    /// the production edit path so stored spread control points re-resolve against it.
    fn set_membership(&self, members: Vec<light_core::FixtureId>) {
        let store = ActiveShowRepository::open(&self.entry.path).unwrap();
        let revision = store
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "1")
            .map(|object| object.revision)
            .unwrap_or(0);
        store
            .put_object(
                "group",
                "1",
                &serde_json::to_value(light_programmer::GroupDefinition {
                    id: "1".into(),
                    name: "Wave".into(),
                    fixtures: members,
                    ..Default::default()
                })
                .unwrap(),
                revision,
            )
            .unwrap();
        self.state
            .output.replace_snapshot(load_engine_snapshot(&self.entry).unwrap())
            .unwrap();
    }

    fn command(&self, command_line: &str) -> usize {
        execute_programmer_command(&self.state, &self.session, command_line).unwrap()
    }

    /// Normalized resolved intensity per fixture; `None` when the fixture has no contribution.
    fn resolved(&self, fixtures: &[light_core::FixtureId]) -> Vec<Option<f32>> {
        let resolved = self.state.output.resolved_values();
        fixtures
            .iter()
            .map(|fixture| {
                resolved
                    .get(&(*fixture, light_core::AttributeKey::intensity()))
                    .and_then(light_core::AttributeValue::normalized)
            })
            .collect()
    }

    fn clear_programmer(&self) {
        let mut programmer = self.state.programming.get(self.session.id).unwrap();
        programmer.values.clear();
        programmer.group_values.clear();
        self.state.programming.restore(programmer);
    }

    fn stored_group_spread(&self) -> light_core::AttributeValue {
        self.state.programming.get(self.session.id).unwrap().group_values["1"]
            [&light_core::AttributeKey::intensity()]
            .value
            .clone()
    }

    fn finish(self) {
        let _ = std::fs::remove_dir_all(self.data_dir);
    }
}

fn levels(values: &[Option<f32>]) -> Vec<f32> {
    values
        .iter()
        .map(|value| value.expect("every ordered member must resolve a value"))
        .collect()
}

#[test]
fn live_group_spread_recall_re_resolves_after_ordered_membership_edits() {
    let rig = SpreadRecallRig::new("live-group-spread");
    assert_eq!(rig.command("GROUP 1 AT 100 THRU 0 THRU 100"), 5);
    // The live Group retains the control points, not resolved per-fixture values.
    assert_eq!(
        rig.stored_group_spread(),
        light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0])
    );
    // Direct recall before the edit: five ordered members resolve 100, 50, 0, 50, 100.
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures[..5])),
        [1.0, 0.5, 0.0, 0.5, 1.0]
    );
    assert_eq!(rig.resolved(&rig.fixtures[5..]), [None]);

    // Grow the ordered membership to six: the same stored spread re-resolves to the normative
    // 100, 50, 0, 0, 50, 100 without re-entering the command.
    rig.set_membership(rig.fixtures.clone());
    assert_eq!(
        rig.stored_group_spread(),
        light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0])
    );
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures)),
        [1.0, 0.5, 0.0, 0.0, 0.5, 1.0]
    );

    // Reorder the membership: values follow the stored order, not fixture identity.
    let reordered = vec![
        rig.fixtures[2],
        rig.fixtures[0],
        rig.fixtures[4],
        rig.fixtures[1],
        rig.fixtures[3],
    ];
    rig.set_membership(reordered.clone());
    assert_eq!(levels(&rig.resolved(&reordered)), [1.0, 0.5, 0.0, 0.5, 1.0]);

    // Shrink to four: the interior anchor expands across both middle items.
    rig.set_membership(rig.fixtures[..4].to_vec());
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures[..4])),
        [1.0, 0.0, 0.0, 1.0]
    );
    rig.finish();
}

#[test]
fn preset_recall_of_live_group_spread_re_resolves_after_membership_edit() {
    let rig = SpreadRecallRig::new("preset-group-spread");
    let preset = light_programmer::Preset {
        name: "Wave".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::new(),
        group_values: HashMap::from([(
            "1".to_string(),
            HashMap::from([(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0]),
            )]),
        )]),
    };
    ActiveShowRepository::open(&rig.entry.path)
        .unwrap()
        .put_object("preset", "1.1", &serde_json::to_value(&preset).unwrap(), 0)
        .unwrap();

    // Recall onto the live Group before the edit: the Preset lands the control points as a live
    // group value that resolves 100, 50, 0, 50, 100 across the five ordered members.
    assert_eq!(rig.command("GROUP 1 AT 1.1"), 5);
    assert_eq!(
        rig.stored_group_spread(),
        light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0])
    );
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures[..5])),
        [1.0, 0.5, 0.0, 0.5, 1.0]
    );

    // Membership grows while the recalled value is live: it re-resolves against six members.
    rig.set_membership(rig.fixtures.clone());
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures)),
        [1.0, 0.5, 0.0, 0.0, 0.5, 1.0]
    );

    // A fresh recall after the edit resolves against the new membership identically.
    rig.clear_programmer();
    assert_eq!(rig.resolved(&rig.fixtures), vec![None; 6]);
    assert_eq!(rig.command("GROUP 1 AT 1.1"), 6);
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures)),
        [1.0, 0.5, 0.0, 0.0, 0.5, 1.0]
    );
    rig.finish();
}

#[test]
fn cue_recall_of_live_group_spread_re_resolves_after_membership_edit() {
    let rig = SpreadRecallRig::new("cue-group-spread");
    assert_eq!(rig.command("GROUP 1 AT 100 THRU 0 THRU 100"), 5);
    rig.command("RECORD SET 25");
    // The recorded cue stores the control points, not frozen per-fixture values.
    let store = ActiveShowRepository::open(&rig.entry.path).unwrap();
    let (_, _, cue_list) =
        cue_list_for_playback(&store, &rig.state.output.snapshot(), 25).unwrap();
    assert_eq!(cue_list.cues.len(), 1);
    assert_eq!(cue_list.cues[0].group_changes.len(), 1);
    assert_eq!(cue_list.cues[0].group_changes[0].group_id, "1");
    assert_eq!(
        cue_list.cues[0].group_changes[0].value,
        Some(light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0]))
    );
    drop(store);

    rig.clear_programmer();
    rig.state
        .output.replace_snapshot(load_engine_snapshot(&rig.entry).unwrap())
        .unwrap();
    assert_eq!(rig.resolved(&rig.fixtures), vec![None; 6]);
    rig.state
        .output.execute_playback(EnginePlaybackCommand::Pool {
            number: 25,
            action: PoolPlaybackAction::Go,
        })
        .unwrap();
    // Cue recall before the edit.
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures[..5])),
        [1.0, 0.5, 0.0, 0.5, 1.0]
    );
    assert_eq!(rig.resolved(&rig.fixtures[5..]), [None]);

    // Ordered membership grows while the cue is active: the stored spread re-resolves against
    // the six-member membership on the next resolution.
    rig.set_membership(rig.fixtures.clone());
    assert_eq!(
        levels(&rig.resolved(&rig.fixtures)),
        [1.0, 0.5, 0.0, 0.0, 0.5, 1.0]
    );
    rig.finish();
}

#[test]
fn degrp_spread_freezes_per_fixture_values_that_ignore_membership_edits() {
    let rig = SpreadRecallRig::new("degrp-spread-freeze");
    assert_eq!(rig.command("DEGRP 1 AT 100 THRU 0 THRU 100"), 5);
    // DEGRP resolves once into per-fixture values; no live group reference remains.
    let programmer = rig.state.programming.get(rig.session.id).unwrap();
    assert!(programmer.group_values.is_empty());
    assert_eq!(programmer.values.len(), 5);
    let frozen = [1.0, 0.5, 0.0, 0.5, 1.0];
    assert_eq!(levels(&rig.resolved(&rig.fixtures[..5])), frozen);

    // Growing the membership must not touch the frozen values or light the new member.
    rig.set_membership(rig.fixtures.clone());
    assert_eq!(levels(&rig.resolved(&rig.fixtures[..5])), frozen);
    assert_eq!(rig.resolved(&rig.fixtures[5..]), [None]);

    // Reordering the membership must not redistribute the frozen values either.
    rig.set_membership(vec![
        rig.fixtures[4],
        rig.fixtures[3],
        rig.fixtures[2],
        rig.fixtures[1],
        rig.fixtures[0],
    ]);
    assert_eq!(levels(&rig.resolved(&rig.fixtures[..5])), frozen);
    rig.finish();
}
