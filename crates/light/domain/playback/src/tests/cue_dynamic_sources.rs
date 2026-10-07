use super::*;

fn recorded_cue(fixture: FixtureId, number: f64) -> Cue {
    let mut cue = Cue::new(cue_number(number));
    cue.changes.push(value(fixture, "intensity", 0.4));
    cue.dynamic_changes.push(CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value: 0.8,
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        automatic_restore: false,
    });
    cue
}

fn instant() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-29T12:00:00.123456789Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn cue_dynamic_values_keep_the_actual_physical_and_virtual_temporary_sources() {
    let fixture = FixtureId::new();
    let list = list(vec![recorded_cue(fixture, 1.0)]);
    let list_id = list.id;
    let first = VirtualPlaybackAddress::new(1, 1001).unwrap();
    let second = VirtualPlaybackAddress::new(2, 1301).unwrap();
    let now = instant();
    let mut engine = PlaybackEngine::with_clock(Arc::new(light_core::ManualClock::new(now)));
    engine.register(list).unwrap();
    engine.register_definition(definition(1, list_id)).unwrap();
    engine
        .register_virtual_definition(first, definition(1001, list_id))
        .unwrap();
    engine
        .register_virtual_definition(second, definition(1301, list_id))
        .unwrap();
    engine.on(1).unwrap();
    engine
        .set_temp_button_at_mutation(PlaybackIdentity::Virtual(first), true)
        .unwrap();
    engine
        .set_temp_button_at_mutation(PlaybackIdentity::Virtual(second), true)
        .unwrap();

    let values = engine.active_cue_dynamic_values();
    assert_eq!(
        values.len(),
        3,
        "normal and both temporary sources survive projection"
    );
    let ordinary = engine.contributions_with_context(now, None);
    assert_eq!(ordinary.len(), 3);
    for playback in engine.active.values().chain(engine.temporary.values()) {
        let source = playback.sequence_master_source();
        let projected = values.iter().find(|value| value.source == source).unwrap();
        assert_eq!(projected.playback_number, playback.playback_number);
        assert_eq!(projected.source_key.source(), projected.source);
        assert!(projected.output_enabled);
        assert_eq!(projected.cue_list_id, playback.cue_list_id);
        assert_eq!(projected.authored_cue_id, projected.current_cue_id);
        assert_eq!(projected.changed_at, playback.activated_at);
        assert_eq!(projected.transition_ordinal, playback.transition_ordinal);
        assert_eq!(
            (projected.sequence_master, projected.snap_sequence_master),
            playback.sequence_masters()
        );
        assert_eq!(
            projected.changed_at_millis,
            u64::try_from(playback.activated_at.timestamp_millis()).unwrap()
        );
        assert!(ordinary.iter().any(|value| value.source == projected.source
            && value.transition_ordinal == projected.transition_ordinal));
    }
    assert!(
        values
            .iter()
            .any(|value| value.source.playback_number == Some(1)
                && value.source.playback_identity.is_none()
                && !value.source.temporary)
    );
    for address in [first, second] {
        assert!(values.iter().any(|value| value.source.playback_identity
            == Some(PlaybackIdentity::Virtual(address))
            && value.source.temporary));
    }
    assert!(
        values
            .iter()
            .any(|value| value.changed_at.timestamp_subsec_nanos() % 1_000_000 != 0),
        "the source instant is not rounded to milliseconds"
    );
}

#[test]
fn tracked_dynamic_source_is_stable_but_equal_timestamp_transitions_keep_their_order() {
    let fixture = FixtureId::new();
    let list = list(vec![recorded_cue(fixture, 1.0), Cue::new(cue_number(2.0))]);
    let list_id = list.id;
    let authored_cue = list.cues[0].id;
    let following_cue = list.cues[1].id;
    let mut engine = PlaybackEngine::with_clock(Arc::new(light_core::ManualClock::new(instant())));
    engine.register(list).unwrap();
    engine.register_definition(definition(1, list_id)).unwrap();
    engine.on(1).unwrap();
    let first = engine.active_cue_dynamic_values().remove(0);
    engine.go_playback(1).unwrap();
    let second = engine.active_cue_dynamic_values().remove(0);
    assert_eq!(
        first.source, second.source,
        "a tracked Dynamic source must not restart because the Cue advanced"
    );
    assert_eq!(first.source_key, second.source_key);
    assert_eq!(first.value, second.value);
    assert_eq!(first.current_cue_id, authored_cue);
    assert_eq!(second.current_cue_id, following_cue);
    assert_eq!(first.authored_cue_id, authored_cue);
    assert_eq!(second.authored_cue_id, authored_cue);
    assert_eq!(first.changed_at, second.changed_at);
    assert_eq!(first.changed_at_millis, second.changed_at_millis);
    assert!(second.transition_ordinal > first.transition_ordinal);

    let later = instant() + ChronoDuration::milliseconds(250);
    engine.back_at(list_id, later).unwrap();
    let previous = engine.active_cue_dynamic_values().remove(0);
    assert_eq!(previous.authored_cue_id, authored_cue);
    assert_eq!(previous.current_cue_id, authored_cue);
    assert_eq!(previous.changed_at, later);
    assert!(previous.transition_ordinal > second.transition_ordinal);
}

#[test]
fn equal_dynamic_writes_and_release_reintroduction_keep_their_actual_authored_cue() {
    let fixture = FixtureId::new();
    let first = recorded_cue(fixture, 1.0);
    let replacement = recorded_cue(fixture, 2.0);
    assert_eq!(
        first.dynamic_changes[0].value,
        replacement.dynamic_changes[0].value
    );
    let mut release = Cue::new(cue_number(3.0));
    release.dynamic_changes.push(CueDynamicChange {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::Release,
        automatic_restore: false,
    });
    let reintroduced = recorded_cue(fixture, 4.0);
    let follow = Cue::new(cue_number(5.0));
    let ids = [
        first.id,
        replacement.id,
        release.id,
        reintroduced.id,
        follow.id,
    ];
    let list = list(vec![first, replacement, release, reintroduced, follow]);
    let list_id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    for (index, authored) in [Some(ids[0]), Some(ids[1]), None, Some(ids[3]), Some(ids[3])]
        .into_iter()
        .enumerate()
    {
        let activated_at = instant() + ChronoDuration::milliseconds(index as i64);
        engine.go_at(list_id, activated_at).unwrap();
        let values = engine.active_cue_dynamic_values();
        if let Some(authored) = authored {
            assert_eq!(values.len(), 1);
            assert_eq!(values[0].authored_cue_id, authored);
            assert_eq!(values[0].current_cue_id, ids[index]);
            assert_eq!(values[0].changed_at, activated_at);
        } else {
            assert!(
                values.is_empty(),
                "Release removes both the value and its provenance"
            );
        }
    }
}

#[test]
fn cue_dynamic_swap_suppression_matches_ordinary_playback_contributions() {
    let fixture = FixtureId::new();
    let first = list(vec![recorded_cue(fixture, 1.0)]);
    let second = list(vec![recorded_cue(fixture, 1.0)]);
    let first_id = first.id;
    let second_id = second.id;
    let now = instant();
    let mut engine = PlaybackEngine::with_clock(Arc::new(light_core::ManualClock::new(now)));
    engine.register(first).unwrap();
    engine.register(second).unwrap();
    engine.register_definition(definition(1, first_id)).unwrap();
    engine
        .register_definition(definition(2, second_id))
        .unwrap();
    engine.on(1).unwrap();
    let swap = PlaybackIdentity::physical(2).unwrap();
    engine.set_swap_at_mutation(swap, true).unwrap();
    let values = engine.active_cue_dynamic_values();
    assert!(!values.is_empty());
    assert!(
        values
            .iter()
            .any(|value| value.source.cue_list_id == first_id && !value.output_enabled),
        "suppressed controllers remain discoverable and keep running"
    );
    assert!(
        values
            .iter()
            .filter(|value| value.output_enabled)
            .all(|value| value.source.cue_list_id == second_id)
    );
    let ordinary = engine.contributions_with_context(now, None);
    assert_eq!(
        values.iter().filter(|value| value.output_enabled).count(),
        ordinary.len()
    );
    assert!(
        values
            .iter()
            .filter(|value| value.output_enabled)
            .all(|value| ordinary
                .iter()
                .any(|ordinary| ordinary.source == value.source))
    );
    engine.set_swap_at_mutation(swap, false).unwrap();
    assert!(
        engine
            .active_cue_dynamic_values()
            .iter()
            .any(|value| value.source.cue_list_id == first_id && value.output_enabled)
    );
}

#[test]
fn directly_started_cuelist_preserves_its_absent_assignment_identity() {
    let fixture = FixtureId::new();
    let list = list(vec![recorded_cue(fixture, 1.0)]);
    let list_id = list.id;
    let mut engine = PlaybackEngine::default();
    engine.register(list).unwrap();
    engine.go_at(list_id, instant()).unwrap();
    let value = engine.active_cue_dynamic_values().remove(0);
    assert_eq!(
        value.source,
        SequenceMasterSource {
            playback_number: None,
            playback_identity: None,
            cue_list_id: list_id,
            temporary: false
        }
    );
    assert_eq!(value.changed_at, instant());
}

#[test]
fn overlapping_temporary_kinds_keep_distinct_keys_at_identical_source_stamps() {
    let fixture = FixtureId::new();
    let list = list(vec![recorded_cue(fixture, 1.0)]);
    let list_id = list.id;
    let address = VirtualPlaybackAddress::new(2, 1301).unwrap();
    let identity = PlaybackIdentity::Virtual(address);
    let mut engine = PlaybackEngine::with_clock(Arc::new(light_core::ManualClock::new(instant())));
    engine.register(list).unwrap();
    engine
        .register_virtual_definition(address, definition(1301, list_id))
        .unwrap();
    engine.set_temp_button_at_mutation(identity, true).unwrap();
    engine.set_temp_fader_at_mutation(identity, 0.6).unwrap();
    let values = engine.active_cue_dynamic_values();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].source, values[1].source);
    assert_eq!(values[0].changed_at, values[1].changed_at);
    assert_eq!(values[0].transition_ordinal, values[1].transition_ordinal);
    assert_ne!(values[0].source_key, values[1].source_key);
    assert_eq!(
        values
            .iter()
            .find(|value| matches!(
                value.source_key,
                CueDynamicSourceKey::Temporary {
                    kind: TemporaryPlaybackKind::TempFader,
                    ..
                }
            ))
            .unwrap()
            .sequence_master,
        0.6
    );
    let link = Uuid::from_u128(100);
    assert_ne!(
        values[0].source_key.controller_id(link),
        values[1].source_key.controller_id(link)
    );
    for kind in [
        TemporaryPlaybackKind::TempButton,
        TemporaryPlaybackKind::TempFader,
    ] {
        assert!(values.iter().any(|value| value.source_key
            == CueDynamicSourceKey::Temporary {
                source: value.source,
                kind
            }));
    }
    engine.set_temp_button_at_mutation(identity, false).unwrap();
    let remaining = engine.active_cue_dynamic_values();
    assert_eq!(remaining.len(), 1);
    assert!(matches!(
        remaining[0].source_key,
        CueDynamicSourceKey::Temporary {
            kind: TemporaryPlaybackKind::TempFader,
            ..
        }
    ));
}

#[test]
fn cue_dynamic_controller_identity_is_canonical_stable_and_scoped() {
    let source = SequenceMasterSource {
        playback_number: Some(7),
        playback_identity: None,
        cue_list_id: CueListId(Uuid::from_u128(500)),
        temporary: false,
    };
    let link = Uuid::from_u128(100);
    let key = CueDynamicSourceKey::Normal { source };
    let explicit = CueDynamicSourceKey::Normal {
        source: SequenceMasterSource {
            playback_identity: Some(PlaybackIdentity::physical(7).unwrap()),
            ..source
        },
    };
    assert_eq!(key.controller_id(link), explicit.controller_id(link));
    assert_ne!(
        key.controller_id(link),
        key.controller_id(Uuid::from_u128(101))
    );
    let mut identities = std::collections::HashSet::from([key.controller_id(link)]);
    for other in [
        SequenceMasterSource {
            playback_number: None,
            ..source
        },
        SequenceMasterSource {
            playback_number: Some(8),
            ..source
        },
        SequenceMasterSource {
            cue_list_id: CueListId(Uuid::from_u128(501)),
            ..source
        },
        SequenceMasterSource {
            playback_number: Some(1001),
            playback_identity: Some(PlaybackIdentity::virtual_playback(1, 1001).unwrap()),
            ..source
        },
        SequenceMasterSource {
            playback_number: Some(1301),
            playback_identity: Some(PlaybackIdentity::virtual_playback(2, 1301).unwrap()),
            ..source
        },
    ] {
        assert!(
            identities.insert(CueDynamicSourceKey::Normal { source: other }.controller_id(link))
        );
    }
    for kind in [
        TemporaryPlaybackKind::Flash,
        TemporaryPlaybackKind::TempButton,
        TemporaryPlaybackKind::TempFader,
        TemporaryPlaybackKind::Swap,
    ] {
        assert!(
            identities.insert(
                CueDynamicSourceKey::Temporary {
                    source: SequenceMasterSource {
                        temporary: true,
                        ..source
                    },
                    kind
                }
                .controller_id(link)
            )
        );
    }
}

#[test]
fn cue_dynamic_master_capture_keeps_current_and_snap_values_without_live_rereads() {
    let fixture = FixtureId::new();
    let list = list(vec![recorded_cue(fixture, 1.0)]);
    let list_id = list.id;
    let started = instant();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let mut engine = PlaybackEngine::with_clock(clock.clone());
    engine.register(list).unwrap();
    engine.register_definition(definition(1, list_id)).unwrap();
    engine.set_master(1, 0.2).unwrap();
    engine
        .set_master_transition_mutation(1, 0.8, 1_000)
        .unwrap();
    clock.advance_millis(500);
    engine.tick(started + ChronoDuration::milliseconds(500), None);

    let captured = engine.active_cue_dynamic_values().remove(0);
    assert!((captured.sequence_master - 0.5).abs() < 0.001);
    assert_eq!(captured.snap_sequence_master, 0.8);
    let ordinary = engine.contributions_with_context(started, None);
    assert!((ordinary[0].sequence_master - captured.sequence_master).abs() < 0.001);

    clock.advance_millis(500);
    engine.tick(started + ChronoDuration::milliseconds(1_000), None);
    let later = engine.active_cue_dynamic_values().remove(0);
    assert_eq!(
        (later.sequence_master, later.snap_sequence_master),
        (0.8, 0.8)
    );
    assert!((captured.sequence_master - 0.5).abs() < 0.001);
    assert_eq!(captured.snap_sequence_master, 0.8);
}
