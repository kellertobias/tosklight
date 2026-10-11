use super::*;
fn second_page(position: u16) -> PlaybackAddress {
    PlaybackAddress::new(2, position).unwrap()
}
fn shared_engine() -> (PlaybackEngine, CueList, PlaybackAddress, PlaybackAddress) {
    let cue_list = list(vec![
        Cue::new(CueNumber::from(1_u8)),
        Cue::new(CueNumber::from(2_u8)),
    ]);
    let a = address(1);
    let b = second_page(1);
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    let mut first = definition(1, cue_list.id);
    first.auto_off = false;
    engine.register_definition(a, first).unwrap();
    engine
        .register_definition(b, definition(1, cue_list.id))
        .unwrap();
    (engine, cue_list, a, b)
}
#[test]
fn address_ranges_validate_both_axes() {
    assert_eq!(address(1000).position(), 1000);
    assert_eq!(PlaybackAddress::new(99, 1000).unwrap().page(), 99);
    for (page, position) in [(0, 1), (100, 1), (1, 0), (1, 1001)] {
        assert!(PlaybackAddress::new(page, position).is_err());
    }
}
#[test]
fn different_pages_for_one_cuelist_share_runtime() {
    let (mut engine, _, a, b) = shared_engine();
    engine.on(a).unwrap();
    engine.on(b).unwrap();
    assert_eq!(engine.runtime().len(), 1);
    assert!(engine.playback_runtime(a).unwrap().enabled);
    assert!(engine.off(a).unwrap());
    assert!(!engine.playback_runtime(b).unwrap().enabled);
}
#[test]
fn page_qualified_runtime_survives_persistence_restore() {
    let (mut engine, cue_list, a, b) = shared_engine();
    engine.on(b).unwrap();
    let persisted = engine.runtime();
    let mut restored = PlaybackEngine::default();
    restored.register(cue_list.clone()).unwrap();
    restored
        .register_definition(a, definition(1, cue_list.id))
        .unwrap();
    restored
        .register_definition(b, definition(1, cue_list.id))
        .unwrap();
    restored.restore_active(persisted);
    assert_eq!(restored.runtime()[0].playback_address, Some(b));
    assert!(restored.playback_runtime(a).unwrap().enabled);
    assert!(restored.playback_runtime(b).unwrap().enabled);
}
#[test]
fn duplicate_runtime_restore_uses_the_newest_activation_ordinal() {
    let (mut source, cue_list, a, b) = shared_engine();
    source.on(a).unwrap();
    let mut older = source.runtime()[0].clone();
    older.playback_address = Some(a);
    older.activation = Some(PlaybackActivationProvenance {
        ordinal: 4,
        at: older.activated_at,
        desk_id: None,
        surface: PlaybackActivationSurface::Physical,
        exclusion_scope: PlaybackExclusionScope::Show,
    });
    let mut newer = older.clone();
    newer.playback_address = Some(b);
    newer.activation.as_mut().unwrap().ordinal = 5;
    newer.cue_index = 1;
    newer.current_cue_id = Some(cue_list.cues[1].id);
    newer.current_cue_number = Some(cue_number(2.0));
    source.restore_active([newer, older]);
    assert_eq!(source.runtime().len(), 1);
    assert_eq!(
        source.runtime()[0].current_cue_number,
        Some(cue_number(2.0))
    );
    assert_eq!(source.playback_runtime(a), source.playback_runtime(b));
}
#[test]
fn page_qualified_go_advances_the_shared_target_runtime() {
    let (mut engine, _, a, b) = shared_engine();
    engine.on(a).unwrap();
    engine.go_playback(b).unwrap();
    assert_eq!(
        engine.playback_runtime(a).unwrap().current_cue_number,
        Some(cue_number(2.0))
    );
    assert_eq!(engine.playback_runtime(a), engine.playback_runtime(b));
}
#[test]
fn temp_flash_and_swap_hold_full_page_qualified_address() {
    let (mut engine, _, a, b) = shared_engine();
    let source = test_source(1);
    engine.set_temp_button_mutation(a, &source, true).unwrap();
    engine.set_temp_button_mutation(b, &source, true).unwrap();
    engine.set_temp_button_mutation(a, &source, false).unwrap();
    assert!(!engine.temporary.keys().any(|(held, _)| held.address == a));
    assert!(engine.temporary.keys().any(|(held, _)| held.address == b));
    engine.set_flash_mutation(a, &source, true).unwrap();
    engine.set_flash_mutation(b, &source, true).unwrap();
    engine.set_flash_mutation(a, &source, false).unwrap();
    assert!(
        engine
            .temporary
            .keys()
            .any(|(held, kind)| held.address == b && *kind == TemporaryPlaybackKind::Flash)
    );
    assert!(
        !engine
            .temporary
            .keys()
            .any(|(held, kind)| held.address == a && *kind == TemporaryPlaybackKind::Flash)
    );
    engine.set_swap_mutation(a, &source, true).unwrap();
    engine.set_swap_mutation(b, &source, true).unwrap();
    engine.set_swap_mutation(a, &source, false).unwrap();
    assert!(!engine.swap_held.iter().any(|held| held.address == a));
    assert!(engine.swap_held.iter().any(|held| held.address == b));
}
#[test]
fn registration_rejects_duplicate_canonical_address() {
    let (mut engine, cue_list, a, _) = shared_engine();
    assert!(
        engine
            .register_definition(a, definition(1, cue_list.id))
            .is_err()
    );
}
#[test]
fn retired_page_slot_schema_is_rejected() {
    assert!(
        serde_json::from_value::<PlaybackPage>(
            serde_json::json!({"number":1,"name":"Old","slots":{"1":12},"virtual_playbacks":{}})
        )
        .is_err()
    );
}
#[test]
fn page_schema_serializes_sparse_assignments_without_global_aliases() {
    let cue_list = list(vec![Cue::new(CueNumber::from(1_u8))]);
    let page = PlaybackPage {
        number: 99,
        name: "Sparse".into(),
        assignments: HashMap::from([(1000, definition(1000, cue_list.id))]),
    };
    page.validate().unwrap();
    let body = serde_json::to_value(&page).unwrap();
    assert!(body.get("slots").is_none());
    assert!(body.get("virtual_playbacks").is_none());
    assert!(body["assignments"]["1000"].get("number").is_none());
    assert_eq!(serde_json::from_value::<PlaybackPage>(body).unwrap(), page);
}
