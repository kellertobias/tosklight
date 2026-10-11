use super::*;

#[test]
fn exact_direct_status_never_reports_assignment_owned_or_retained_off_runtime() {
    let cue_list = list(vec![Cue::new(cue_number(1.0))]);
    let id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(address(1), definition(1, id))
        .unwrap();
    let virtual_address = PlaybackAddress::new(2, 1).unwrap();
    engine
        .register_definition(virtual_address, definition(1001, id))
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

    engine.on(address(1)).unwrap();
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());
    assert!(
        engine
            .runtime_status_for_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
    engine.off(address(1)).unwrap();
    assert!(
        !engine
            .runtime_status_for_cue_list(id)
            .unwrap()
            .playback
            .enabled
    );
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());

    engine.on(virtual_address).unwrap();
    assert!(engine.runtime_status_for_direct_cue_list(id).is_none());
    assert!(
        engine
            .runtime_status_for_playback(virtual_address)
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
