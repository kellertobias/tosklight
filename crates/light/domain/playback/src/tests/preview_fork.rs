use super::*;

#[test]
fn preview_fork_does_not_sample_the_original_clock() {
    #[derive(Debug)]
    struct UnavailableClock;

    impl light_core::ApplicationClock for UnavailableClock {
        fn now(&self) -> DateTime<Utc> {
            panic!("forking must use the supplied frame instant");
        }
    }

    let sampled_at = DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let live = PlaybackEngine::with_clock(Arc::new(UnavailableClock));
    let preview = live.fork_for_preview(sampled_at);
    assert_eq!(preview.clock().now(), sampled_at);
    assert!(!Arc::ptr_eq(&live.clock, &preview.clock));
    assert!(live.active.is_empty());
    assert!(preview.active.is_empty());
}

#[test]
fn preview_go_off_and_temp_use_pinned_time_and_leave_live_runtime_unchanged() {
    let started = DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock = Arc::new(light_core::ManualClock::new(started));
    let fixture = FixtureId::new();
    let mut first = Cue::new(cue_number(1.0));
    first.changes.push(value(fixture, "pan", 0.2));
    let mut second = Cue::new(cue_number(2.0));
    second.fade_millis = 1_000;
    second.changes.push(value(fixture, "pan", 0.8));
    let cue_list = list(vec![first, second]);
    let cue_list_id = cue_list.id;
    let mut live = PlaybackEngine::with_clock(clock.clone());
    live.register(cue_list).unwrap();
    live.register_definition(definition(1, cue_list_id))
        .unwrap();
    live.on(1).unwrap();
    let live_active = live.active.clone();
    let live_controls = live.control_states.clone();
    let live_ordinals = (live.next_activation_ordinal, live.next_transition_ordinal);
    let sampled_at = started + ChronoDuration::milliseconds(250);

    // The caller's captured instant can precede the current Live clock; forking must neither
    // sample it nor retime the cloned runtime. Later clock changes must remain isolated too.
    clock.advance_millis(1_000);
    let mut preview = live.fork_for_preview(sampled_at);
    assert_eq!(preview.active, live_active);
    assert!(Arc::ptr_eq(
        &live.compiled_cue_lists[&cue_list_id],
        &preview.compiled_cue_lists[&cue_list_id],
    ));
    clock.advance_millis(4_000);

    let active = preview.go_playback(1).unwrap();
    assert_eq!(active.cue_index, 1);
    assert_eq!(active.activated_at, sampled_at);
    assert!(preview.off(1).unwrap());
    assert!(!preview.active[&PlaybackKey::CueList(cue_list_id)].enabled);
    assert!(preview.toggle_temp(1).unwrap());
    let identity = PlaybackIdentity::physical(1).unwrap();
    let temporary = &preview.temporary[&(identity, TemporaryPlaybackKind::TempButton)];
    assert_eq!(
        temporary.activated_at,
        sampled_at + ChronoDuration::microseconds(1)
    );
    assert_eq!(preview.clock().now(), sampled_at);

    assert_eq!(live.active, live_active);
    assert_eq!(live.control_states, live_controls);
    assert_eq!(
        (live.next_activation_ordinal, live.next_transition_ordinal),
        live_ordinals
    );
    assert!(live.temporary.is_empty());
    assert_eq!(
        live.clock().now(),
        started + ChronoDuration::milliseconds(5_000)
    );
    assert!(Arc::ptr_eq(
        &live.compiled_cue_lists[&cue_list_id],
        &preview.compiled_cue_lists[&cue_list_id],
    ));
}
