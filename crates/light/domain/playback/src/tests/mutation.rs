use super::*;

fn engine_with_cues(cues: Vec<Cue>) -> (PlaybackEngine, CueListId) {
    let cue_list = list(cues);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();
    (engine, cue_list_id)
}

fn single_cue_engine() -> (PlaybackEngine, CueListId) {
    engine_with_cues(vec![Cue::new(1.0)])
}

#[test]
fn runtime_effect_helpers_preserve_the_strongest_consequence() {
    assert!(!PlaybackRuntimeEffect::None.changed());
    assert!(PlaybackRuntimeEffect::Transient.changed());
    assert!(!PlaybackRuntimeEffect::Transient.durable());
    assert!(PlaybackRuntimeEffect::Durable.durable());
    assert_eq!(
        PlaybackRuntimeEffect::None.combine(PlaybackRuntimeEffect::Transient),
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        PlaybackRuntimeEffect::Transient.combine(PlaybackRuntimeEffect::Durable),
        PlaybackRuntimeEffect::Durable
    );

    let related_only = PlaybackMutation::with_related_effect(
        7,
        PlaybackRuntimeEffect::None,
        PlaybackRuntimeEffect::Durable,
    );
    assert_eq!(related_only.addressed_effect, PlaybackRuntimeEffect::None);
    assert_eq!(related_only.effect, PlaybackRuntimeEffect::Durable);
    let mapped = related_only.map(|value| value.to_string());
    assert_eq!(mapped.value, "7");
    assert_eq!(mapped.addressed_effect, PlaybackRuntimeEffect::None);
    assert_eq!(mapped.effect, PlaybackRuntimeEffect::Durable);
}

#[test]
fn on_and_off_report_exact_durable_changes() {
    let (mut engine, _) = single_cue_engine();
    assert_eq!(
        engine.on_mutation(1).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.on_mutation(1).unwrap().effect,
        PlaybackRuntimeEffect::None
    );

    let off = engine.off_mutation(1).unwrap();
    assert!(off.value);
    assert_eq!(off.effect, PlaybackRuntimeEffect::Durable);
    let repeated = engine.off_mutation(1).unwrap();
    assert!(!repeated.value);
    assert_eq!(repeated.effect, PlaybackRuntimeEffect::None);
}

#[test]
fn off_reports_cleanup_even_when_the_playback_was_disabled() {
    let (mut engine, _) = single_cue_engine();
    assert_eq!(
        engine.load_playback_mutation(1, 1.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    let cleanup = engine.off_mutation(1).unwrap();
    assert!(!cleanup.value);
    assert_eq!(cleanup.effect, PlaybackRuntimeEffect::Durable);
    assert_eq!(
        engine.off_mutation(1).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn load_and_pause_suppress_repeated_explicit_state() {
    let (mut engine, cue_list_id) = engine_with_cues(vec![Cue::new(1.0), Cue::new(2.0)]);
    assert_eq!(
        engine.load_playback_mutation(1, 1.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.load_playback_mutation(1, 1.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.load_playback_mutation(1, 2.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    engine.on(1).unwrap();
    assert_eq!(
        engine.pause_playback_mutation(1).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.pause_mutation(cue_list_id).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn goto_is_exact_for_the_same_runtime_instant_but_retriggering_is_durable() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let cue_list = list(vec![Cue::new(1.0)]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(cue_list).unwrap();
    engine
        .register_definition(definition(1, cue_list_id))
        .unwrap();

    let first = engine.goto_playback_mutation(1, 1.0).unwrap();
    assert_eq!(first.addressed_effect, PlaybackRuntimeEffect::Durable);
    let repeated = engine.goto_playback_mutation(1, 1.0).unwrap();
    assert_eq!(repeated.addressed_effect, PlaybackRuntimeEffect::None);

    clock.advance_millis(1);
    let retriggered = engine.goto_playback_mutation(1, 1.0).unwrap();
    assert_eq!(retriggered.addressed_effect, PlaybackRuntimeEffect::Durable);
}

#[test]
fn direct_cue_list_pause_reports_exact_repetition() {
    let cue_list = list(vec![Cue::new(1.0)]);
    let cue_list_id = cue_list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(cue_list).unwrap();
    engine.go(cue_list_id).unwrap();
    assert_eq!(
        engine.pause_mutation(cue_list_id).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.pause_mutation(cue_list_id).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn master_and_pickup_changes_are_detected_without_runtime_snapshots() {
    let (mut engine, _) = single_cue_engine();
    assert_eq!(
        engine.set_master_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_master_mutation(1, 0.4).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.set_virtual_master_mutation(1, 0.4).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    engine.off(1).unwrap();
    assert_eq!(
        engine.set_master_mutation(1, 0.9).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.set_master_mutation(1, 0.9).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_master_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.set_master_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    engine
        .active
        .get_mut(&PlaybackKey::Number(1))
        .unwrap()
        .fader_pickup_required = true;
    assert_eq!(
        engine.set_master_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
}

#[test]
fn temporary_controls_are_transient_and_duplicate_explicit_state_is_none() {
    let (mut engine, cue_list_id) = single_cue_engine();
    engine.definitions.get_mut(&1).unwrap().fader = PlaybackFaderMode::Temp;
    assert_eq!(
        engine.set_temp_button_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_temp_button_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_temp_button_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_temp_button_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_temp_fader_mutation(1, 0.3).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_master_mutation(1, 0.3).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_temp_fader_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_temp_fader_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(engine.cue_lists[&cue_list_id].id, cue_list_id);
}

#[test]
fn flash_and_swap_release_promotion_is_durable() {
    let first = list(vec![Cue::new(1.0)]);
    let first_id = first.id;
    let second = list(vec![Cue::new(1.0)]);
    let second_id = second.id;
    let mut engine = PlaybackEngine::default();
    engine.register(first).unwrap();
    engine.register(second).unwrap();
    let mut flash = definition(1, first_id);
    flash.flash_release = FlashReleaseMode::ReleaseIntensityOnly;
    engine.register_definition(flash).unwrap();
    let mut swap = definition(2, second_id);
    swap.flash_release = FlashReleaseMode::ReleaseIntensityOnly;
    engine.register_definition(swap).unwrap();

    assert_eq!(
        engine.set_flash_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_flash_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_flash_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.set_flash_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::None
    );

    assert_eq!(
        engine.set_swap_mutation(2, true).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_swap_mutation(2, true).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        engine.set_swap_mutation(2, false).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine.set_swap_mutation(2, false).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn release_all_flash_remains_transient() {
    let (mut engine, _) = single_cue_engine();
    assert_eq!(
        engine.set_flash_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_flash_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine.set_flash_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn manual_and_automatic_xfade_report_endpoint_no_ops() {
    let (mut manual, _) = engine_with_cues(vec![Cue::new(1.0), Cue::new(2.0)]);
    manual.definitions.get_mut(&1).unwrap().fader = PlaybackFaderMode::XFade;
    manual.on(1).unwrap();
    assert_eq!(
        manual.set_manual_xfade_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        manual.set_manual_xfade_mutation(1, 0.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        manual.set_manual_xfade_mutation(1, 1.0).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        manual.set_manual_xfade_mutation(1, 1.0).unwrap().effect,
        PlaybackRuntimeEffect::None
    );

    let (mut automatic, _) = single_cue_engine();
    assert_eq!(
        automatic.xfade_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        automatic.xfade_mutation(1, true).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
    assert_eq!(
        automatic.xfade_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        automatic.xfade_mutation(1, false).unwrap().effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn timed_xfade_retrigger_is_always_durable() {
    let (mut engine, _) = single_cue_engine();
    engine.definitions.get_mut(&1).unwrap().xfade_millis = 1_000;
    for on in [true, true, false, false] {
        assert_eq!(
            engine.xfade_mutation(1, on).unwrap().effect,
            PlaybackRuntimeEffect::Durable
        );
    }
}

#[test]
fn preload_timing_reports_durable_and_transient_metadata_exactly() {
    let (mut engine, _) = single_cue_engine();
    engine.on(1).unwrap();
    let started_at = Utc::now();
    assert_eq!(
        engine
            .apply_preload_timing_mutation(1, "go", started_at, 500, None)
            .unwrap()
            .effect,
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        engine
            .apply_preload_timing_mutation(1, "go", started_at, 500, None)
            .unwrap()
            .effect,
        PlaybackRuntimeEffect::None
    );

    engine.set_temp_button(1, true).unwrap();
    assert_eq!(
        engine
            .apply_preload_timing_mutation(1, "temp-on", started_at, 0, None)
            .unwrap()
            .effect,
        PlaybackRuntimeEffect::Transient
    );
    assert_eq!(
        engine
            .apply_preload_timing_mutation(1, "temp-on", started_at, 0, None)
            .unwrap()
            .effect,
        PlaybackRuntimeEffect::None
    );
}

#[test]
fn auto_off_peer_changes_upgrade_an_otherwise_repeated_on() {
    let started = Utc::now();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut low = Cue::new(1.0);
    low.changes.push(value(fixture, "pan", 0.2));
    let mut high = Cue::new(1.0);
    high.changes.push(value(fixture, "pan", 0.8));
    let low = list(vec![low]);
    let low_id = low.id;
    let high = list(vec![high]);
    let high_id = high.id;
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(low).unwrap();
    engine.register(high).unwrap();
    let mut low_definition = definition(1, low_id);
    low_definition.auto_off = false;
    let mut high_definition = definition(2, high_id);
    high_definition.auto_off = false;
    engine.register_definition(low_definition).unwrap();
    engine.register_definition(high_definition).unwrap();
    engine.on(1).unwrap();
    clock.advance_millis(1);
    engine.on(2).unwrap();
    engine.definitions.get_mut(&1).unwrap().auto_off = true;

    let peer_only = engine.on_mutation(2).unwrap();
    assert_eq!(peer_only.addressed_effect, PlaybackRuntimeEffect::None);
    assert_eq!(peer_only.effect, PlaybackRuntimeEffect::Durable);
    assert!(!engine.playback_runtime(1).unwrap().enabled);
    let repeated = engine.on_mutation(2).unwrap();
    assert_eq!(repeated.addressed_effect, PlaybackRuntimeEffect::None);
    assert_eq!(repeated.effect, PlaybackRuntimeEffect::None);
}
