use super::*;

#[test]
fn physical_and_virtual_playback_ranges_are_explicit_and_disjoint() {
    assert_eq!(PhysicalPlaybackNumber::new(1).unwrap().get(), 1);
    assert_eq!(PhysicalPlaybackNumber::new(1_000).unwrap().get(), 1_000);
    assert!(PhysicalPlaybackNumber::new(0).is_err());
    assert!(PhysicalPlaybackNumber::new(1_001).is_err());

    assert_eq!(VirtualPlaybackNumber::new(1_001).unwrap().get(), 1_001);
    assert_eq!(VirtualPlaybackNumber::new(39_100).unwrap().get(), 39_100);
    assert!(VirtualPlaybackNumber::new(1_000).is_err());
    assert!(VirtualPlaybackNumber::new(39_101).is_err());

    assert!(VirtualPlaybackAddress::new(0, 1_001).is_err());
    assert!(VirtualPlaybackAddress::new(128, 1_001).is_err());
    assert!(VirtualPlaybackAddress::new(2, 1_001).is_err());
    assert_eq!(
        VirtualPlaybackAddress::from_number(1_301).unwrap().page(),
        2
    );
}

#[test]
fn banked_virtual_assignments_for_one_cuelist_share_runtime() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let page_one = VirtualPlaybackAddress::new(1, 1_001).unwrap();
    let page_two = VirtualPlaybackAddress::new(2, 1_301).unwrap();
    let page_one_identity = PlaybackIdentity::virtual_playback(1, 1_001).unwrap();
    let page_two_identity = PlaybackIdentity::virtual_playback(2, 1_301).unwrap();
    let mut first_definition = definition(1_001, cue_list_id);
    first_definition.auto_off = false;
    let second_definition = definition(1_301, cue_list_id);

    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_virtual_definition(page_one, first_definition)
        .unwrap();
    engine
        .register_virtual_definition(page_two, second_definition)
        .unwrap();

    engine.on_at(page_one_identity).unwrap();
    engine.on_at(page_two_identity).unwrap();
    assert_eq!(engine.runtime().len(), 1);
    assert!(
        engine
            .playback_runtime_at(page_one_identity)
            .is_some_and(|runtime| runtime.enabled)
    );
    assert!(
        engine
            .playback_runtime_at(page_two_identity)
            .is_some_and(|runtime| runtime.enabled)
    );

    assert!(engine.off_at(page_one_identity).unwrap());
    assert!(
        engine
            .playback_runtime_at(page_one_identity)
            .is_some_and(|runtime| !runtime.enabled)
    );
    assert!(
        engine
            .playback_runtime_at(page_two_identity)
            .is_some_and(|runtime| !runtime.enabled)
    );
}

#[test]
fn page_qualified_virtual_runtime_survives_persistence_restore() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let page_one = VirtualPlaybackAddress::new(1, 1_001).unwrap();
    let page_two = VirtualPlaybackAddress::new(2, 1_301).unwrap();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list.clone()).unwrap();
    engine
        .register_virtual_definition(page_one, definition(1_001, cue_list_id))
        .unwrap();
    engine
        .register_virtual_definition(page_two, definition(1_301, cue_list_id))
        .unwrap();
    engine.on_at(PlaybackIdentity::Virtual(page_one)).unwrap();
    engine.on_at(PlaybackIdentity::Virtual(page_two)).unwrap();
    let persisted = engine.runtime();

    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored
        .register_virtual_definition(page_one, definition(1_001, cue_list_id))
        .unwrap();
    restored
        .register_virtual_definition(page_two, definition(1_301, cue_list_id))
        .unwrap();
    restored.restore_active(persisted);

    assert!(
        restored
            .playback_runtime_at(PlaybackIdentity::Virtual(page_one))
            .is_some_and(|runtime| runtime.enabled)
    );
    assert!(
        restored
            .playback_runtime_at(PlaybackIdentity::Virtual(page_two))
            .is_some_and(|runtime| runtime.enabled)
    );
}

#[test]
fn legacy_duplicate_runtime_restore_uses_the_newest_activation_ordinal() {
    let cue_list = list(vec![
        Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap()),
        Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap()),
    ]);
    let cue_list_id = cue_list.id;
    let mut source = PlaybackEngine::default();
    source.register(cue_list.clone()).unwrap();
    source
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    source
        .register_definition(definition(2, cue_list_id))
        .unwrap();
    source.on(1).unwrap();

    let mut older = source.runtime()[0].clone();
    older.playback_number = Some(1);
    older.playback_identity = Some(PlaybackIdentity::physical(1).unwrap());
    older.activation = Some(PlaybackActivationProvenance {
        ordinal: 4,
        at: older.activated_at,
        desk_id: None,
        surface: PlaybackActivationSurface::Physical,
        exclusion_scope: PlaybackExclusionScope::Show,
    });
    let mut newer = older.clone();
    newer.playback_number = Some(2);
    newer.playback_identity = Some(PlaybackIdentity::physical(2).unwrap());
    newer.activation.as_mut().unwrap().ordinal = 5;
    newer.cue_index = 1;
    newer.current_cue_id = Some(cue_list.cues[1].id);
    newer.current_cue_number = Some(cue_number(2.0));

    let mut restored = PlaybackEngine::default();
    restored.register(cue_list).unwrap();
    restored
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    restored
        .register_definition(definition(2, cue_list_id))
        .unwrap();
    restored.restore_active([newer, older]);

    assert_eq!(restored.runtime().len(), 1);
    assert_eq!(
        restored.runtime()[0].current_cue_number,
        Some(cue_number(2.0))
    );
    assert_eq!(restored.playback_runtime(1), restored.playback_runtime(2));
}

#[test]
fn page_qualified_virtual_go_advances_the_shared_target_runtime() {
    let cue_list = list(vec![
        Cue::new(crate::CueNumber::try_from_legacy_f64(1.0).unwrap()),
        Cue::new(crate::CueNumber::try_from_legacy_f64(2.0).unwrap()),
    ]);
    let cue_list_id = cue_list.id;
    let page_one = VirtualPlaybackAddress::new(1, 1_001).unwrap();
    let page_two = VirtualPlaybackAddress::new(2, 1_301).unwrap();
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_virtual_definition(page_one, definition(1_001, cue_list_id))
        .unwrap();
    engine
        .register_virtual_definition(page_two, definition(1_301, cue_list_id))
        .unwrap();
    engine.on_at(PlaybackIdentity::Virtual(page_one)).unwrap();
    engine.on_at(PlaybackIdentity::Virtual(page_two)).unwrap();

    engine
        .go_playback_at(PlaybackIdentity::Virtual(page_two))
        .unwrap();

    assert_eq!(
        engine
            .playback_runtime_at(PlaybackIdentity::Virtual(page_one))
            .and_then(|runtime| runtime.current_cue_number.clone()),
        Some(cue_number(2.0))
    );
    assert_eq!(
        engine
            .playback_runtime_at(PlaybackIdentity::Virtual(page_two))
            .and_then(|runtime| runtime.current_cue_number.clone()),
        Some(cue_number(2.0))
    );
}

#[test]
fn virtual_temp_flash_and_swap_hold_full_page_qualified_identity() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let page_one = VirtualPlaybackAddress::new(1, 1_001).unwrap();
    let page_two = VirtualPlaybackAddress::new(2, 1_301).unwrap();
    let page_one_identity = PlaybackIdentity::Virtual(page_one);
    let page_two_identity = PlaybackIdentity::Virtual(page_two);
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_virtual_definition(page_one, definition(1_001, cue_list_id))
        .unwrap();
    engine
        .register_virtual_definition(page_two, definition(1_301, cue_list_id))
        .unwrap();

    engine
        .set_temp_button_at_mutation(page_one_identity, true)
        .unwrap();
    engine
        .set_temp_button_at_mutation(page_two_identity, true)
        .unwrap();
    engine
        .set_temp_button_at_mutation(page_one_identity, false)
        .unwrap();
    assert!(
        !engine
            .temporary
            .keys()
            .any(|(identity, _)| *identity == page_one_identity)
    );
    assert!(
        engine
            .temporary
            .keys()
            .any(|(identity, _)| *identity == page_two_identity)
    );

    engine
        .set_flash_at_mutation(page_one_identity, true)
        .unwrap();
    engine
        .set_flash_at_mutation(page_two_identity, true)
        .unwrap();
    engine
        .set_flash_at_mutation(page_one_identity, false)
        .unwrap();
    assert!(
        engine
            .temporary
            .keys()
            .any(|(identity, kind)| *identity == page_two_identity
                && *kind == TemporaryPlaybackKind::Flash)
    );
    assert!(
        !engine
            .temporary
            .keys()
            .any(|(identity, kind)| *identity == page_one_identity
                && *kind == TemporaryPlaybackKind::Flash)
    );

    engine
        .set_swap_at_mutation(page_one_identity, true)
        .unwrap();
    engine
        .set_swap_at_mutation(page_two_identity, true)
        .unwrap();
    engine
        .set_swap_at_mutation(page_one_identity, false)
        .unwrap();
    assert!(!engine.swap_held.contains(&page_one_identity));
    assert!(engine.swap_held.contains(&page_two_identity));
}

#[test]
fn physical_registration_still_rejects_virtual_numbers() {
    let cue_list = list(vec![Cue::new(
        crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
    )]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    assert_eq!(
        engine.register_definition(definition(1_001, cue_list_id)),
        Err("physical playback number must be within 1-1000".into())
    );
}

#[test]
fn legacy_page_slot_virtual_schema_is_rejected() {
    let legacy = serde_json::json!({
        "number": 1,
        "name": "Legacy Virtual Playbacks",
        "slots": {"1": 1, "2": 2}
    });
    assert!(serde_json::from_value::<PlaybackPage>(legacy).is_err());
}

#[test]
fn current_page_schema_serializes_the_sparse_virtual_assignment_boundary() {
    let page = PlaybackPage {
        number: 1,
        name: "Current".into(),
        slots: HashMap::new(),
        virtual_playbacks: HashMap::new(),
    };
    let encoded = serde_json::to_value(page).unwrap();
    assert_eq!(
        encoded.get("virtual_playbacks"),
        Some(&serde_json::json!({}))
    );
}
