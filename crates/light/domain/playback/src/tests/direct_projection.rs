use super::*;

#[test]
fn exact_direct_status_never_reports_assignment_owned_or_retained_off_runtime() {
    let cue_list = list(vec![Cue::new(cue_number(1.0))]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.register_definition(definition(1, id)).unwrap();
    let virtual_address = VirtualPlaybackAddress::new(1, 1001).unwrap();
    engine
        .register_virtual_definition(virtual_address, definition(1001, id))
        .unwrap();

    engine.go(id).unwrap();
    assert!(
        engine
            .runtime_status_for_direct_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
    assert!(engine.runtime_status_for_cue_list(id).is_some());

    engine.on(1).unwrap();
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());
    assert!(
        engine
            .runtime_status_for_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
    engine.off(1).unwrap();
    assert!(
        !engine
            .runtime_status_for_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());

    engine
        .on_at(PlaybackIdentity::Virtual(virtual_address))
        .unwrap();
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());
    assert!(
        engine
            .runtime_status_at(PlaybackIdentity::Virtual(virtual_address))
            .unwrap()
            .playback
            .enabled
    );
    engine.release(id);
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());
    engine.go(id).unwrap();
    assert!(
        engine
            .runtime_status_for_direct_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
}
