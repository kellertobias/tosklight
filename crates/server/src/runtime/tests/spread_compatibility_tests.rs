// Compatibility coverage for docs/plans/Next/50-deterministic-multi-point-value-spreading.minor.md
// acceptance item 8: a show file written by an older build that already contains a multi-point
// `AttributeValue::Spread` must load without any schema migration and resolve through the
// deterministic anchor rule (`light_core::resolve_spread`).
//
// The group and cue_list bodies are deliberately authored as raw JSON documents — the exact
// persisted representation an older build wrote — instead of being serialized from the current
// structs, so the test breaks if the persisted spread representation stops decoding unchanged.

/// Five patched dimmers plus one legacy cue list whose group cue change stores the raw
/// multi-point spread; returns the show entry and the dimmer DMX addresses in membership order.
fn legacy_spread_show(
    data_dir: &FsPath,
    group_spread: serde_json::Value,
    membership_addresses: &[u16],
    cue_list_id: Uuid,
) -> ShowEntry {
    let path = data_dir.join("shows/legacy-spread.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (store, show_id) = ShowStore::create(&path, "Legacy spread show").unwrap();
    let authored_spread = group_spread.clone();
    let mut fixtures_by_address = HashMap::new();
    for address in membership_addresses {
        let fixture = template_dimmer(&format!("Legacy dimmer {address}"), *address);
        fixtures_by_address.insert(*address, fixture.fixture_id);
        store
            .put_object(
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                &serde_json::to_value(&fixture).unwrap(),
                0,
            )
            .unwrap();
    }
    let members = membership_addresses
        .iter()
        .map(|address| serde_json::json!(fixtures_by_address[address].0))
        .collect::<Vec<_>>();
    store
        .put_object(
            "group",
            "1",
            &serde_json::json!({
                "id": "1",
                "name": "Legacy spread group",
                "fixtures": members,
                "master": 1.0
            }),
            0,
        )
        .unwrap();
    store
        .put_object(
            "cue_list",
            &cue_list_id.to_string(),
            &serde_json::json!({
                "id": cue_list_id,
                "name": "Legacy spread list",
                "priority": 0,
                "mode": "sequence",
                "looped": false,
                "cues": [{
                    "id": Uuid::new_v4(),
                    "number": 1.0,
                    "name": "Stored spread",
                    "changes": [],
                    "fade_millis": 0,
                    "delay_millis": 0,
                    "trigger": {"type": "manual"},
                    "group_changes": [{
                        "group_id": "1",
                        "attribute": "intensity",
                        "value": group_spread
                    }]
                }]
            }),
            0,
        )
        .unwrap();
    drop(store);
    let entry = |revision| ShowEntry {
        id: show_id,
        name: "Legacy spread show".into(),
        path: path.display().to_string(),
        revision,
        updated_at: String::new(),
        revision_copy: None,
    };
    // Settle the seeded patch once through the committed startup path (the inline fixture
    // definitions canonicalize into lean profile records, and object normalization fills
    // defaulted cue-list fields). The spread itself needs no migration: its stored control
    // points must come out of that startup byte-identical.
    let settle_engine = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&settle_engine, &entry(0), data_dir, 5),
        None
    );
    let store = ShowStore::open(&path).unwrap();
    assert_eq!(
        store
            .portable_document()
            .unwrap()
            .object("cue_list", &cue_list_id.to_string())
            .unwrap()
            .body()["cues"][0]["group_changes"][0]["value"],
        authored_spread,
        "startup canonicalization must not rewrite the persisted spread control points"
    );
    let settled_revision = store.portable_revision().unwrap().value();
    entry(settled_revision)
}

fn go_and_render(entry: &ShowEntry, cue_list_id: Uuid) -> (EngineSnapshot, light_output::DmxFrame) {
    let snapshot = load_engine_snapshot(entry).unwrap();
    // A staged migration would have predicted revision source+1; equality proves the legacy
    // document compiled as-is, without any schema migration.
    assert_eq!(
        snapshot.revision, entry.revision,
        "the legacy spread show must load without a staged migration"
    );
    let engine = Engine::new(ProgrammerRegistry::default());
    engine.replace_snapshot(snapshot.clone()).unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: light_core::CueListId(cue_list_id),
            action: light_engine::CueListPlaybackAction::Go,
        })
        .unwrap();
    let frame = engine.render(RenderOptions::default()).unwrap().universes[&1];
    (snapshot, frame)
}

#[test]
fn persisted_multi_point_spread_loads_without_migration_and_resolves_by_membership_order() {
    let data_dir = std::env::temp_dir().join(format!("light-spread-compat-{}", Uuid::new_v4()));
    let cue_list_id = Uuid::new_v4();
    // Membership order deliberately differs from patch-address order: the anchor rule must
    // follow the stored ordered membership, not the DMX addressing.
    let membership = [3_u16, 1, 5, 2, 4];
    let entry = legacy_spread_show(
        &data_dir,
        serde_json::json!({"kind": "spread", "value": [1.0, 0.0, 1.0]}),
        &membership,
        cue_list_id,
    );

    let (snapshot, frame) = go_and_render(&entry, cue_list_id);

    // The persisted representation stays a Spread after load — no eager expansion on read.
    let change = &snapshot.cue_lists[0].cues[0].group_changes[0];
    assert_eq!(
        change.value,
        Some(light_core::AttributeValue::Spread(vec![1.0, 0.0, 1.0]))
    );
    assert!(
        snapshot.cue_lists[0].cues[0].changes.is_empty(),
        "the stored cue must keep its group-level spread instead of frozen per-fixture values"
    );

    // Anchor rule over five ordered members: 100 / 50 / 0 / 50 / 100 percent in membership
    // order, i.e. normalized 1.0, 0.5, 0.0, 0.5, 1.0 → DMX 255, 128, 0, 128, 255.
    let by_membership = membership
        .iter()
        .map(|address| frame[usize::from(address - 1)])
        .collect::<Vec<_>>();
    assert_eq!(by_membership, [255, 128, 0, 128, 255]);

    // The show file itself is untouched: same revision, same raw spread control points.
    let store = ShowStore::open(&entry.path).unwrap();
    assert_eq!(store.portable_revision().unwrap().value(), entry.revision);
    let stored = store
        .portable_document()
        .unwrap()
        .object("cue_list", &cue_list_id.to_string())
        .unwrap()
        .body()
        .clone();
    assert_eq!(
        stored["cues"][0]["group_changes"][0]["value"],
        serde_json::json!({"kind": "spread", "value": [1.0, 0.0, 1.0]})
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn persisted_spread_with_more_points_than_members_degrades_to_legacy_linear_sampling() {
    let data_dir = std::env::temp_dir().join(format!("light-spread-legacy-{}", Uuid::new_v4()));
    let cue_list_id = Uuid::new_v4();
    // Four stored control points over a three-member group cannot place every anchor. The
    // documented compatibility ruling (light_core::resolve_spread) degrades this to the old
    // linear sampling so existing shows keep rendering: 0 / 50 / 100 percent.
    let membership = [1_u16, 2, 3];
    let entry = legacy_spread_show(
        &data_dir,
        serde_json::json!({"kind": "spread", "value": [0.0, 1.0, 0.0, 1.0]}),
        &membership,
        cue_list_id,
    );

    let (snapshot, frame) = go_and_render(&entry, cue_list_id);

    assert_eq!(
        snapshot.cue_lists[0].cues[0].group_changes[0].value,
        Some(light_core::AttributeValue::Spread(vec![0.0, 1.0, 0.0, 1.0]))
    );
    assert_eq!(&frame[0..3], &[0, 128, 255]);
    let _ = std::fs::remove_dir_all(data_dir);
}
