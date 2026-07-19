use super::*;

#[test]
fn programmer_overlap_is_exactly_one_quarter_of_slots() {
    let fixtures = (1..=4)
        .map(|number| FixtureId(fixed_uuid(1, number)))
        .collect::<Vec<_>>();
    let assignments = programmer_assignments(&fixtures).collect::<Vec<_>>();
    assert_eq!(
        assignments.len(),
        4 * usize::from(SLOTS_PER_UNIVERSE - 1) / 4
    );
    assert!(
        assignments
            .iter()
            .all(|(_, attribute, _)| attribute != &slot_attribute(ANIMATED_SLOT))
    );
}

#[test]
fn animated_slot_is_reserved_exclusively_for_the_phaser() {
    let fixture = FixtureId(fixed_uuid(1, 1));
    let group = static_group(&[fixture]);
    let (cue_list, _) = playback();
    let cue = &cue_list.cues[0];
    assert!(
        !group
            .programming
            .contains_key(&slot_attribute(ANIMATED_SLOT))
    );
    assert!(
        cue.group_changes
            .iter()
            .all(|change| change.attribute != slot_attribute(ANIMATED_SLOT))
    );
    assert_eq!(cue.phasers[0].attribute, slot_attribute(ANIMATED_SLOT));
    assert!(
        programmer_assignments(&[fixture])
            .all(|(_, attribute, _)| attribute != slot_attribute(ANIMATED_SLOT))
    );
}

#[test]
fn consecutive_logical_ticks_move_the_exclusive_phaser_slot() {
    let config = ProfileConfig {
        profile: crate::light_benchmark::arguments::BenchmarkProfile::LowPower4,
        expectation: crate::light_benchmark::arguments::Expectation::LowPowerGoal,
        universes: 1,
        rate_hz: 120,
    };
    let scenario = BenchmarkScenario::build(config, ProtocolSelection::ArtNet, None).unwrap();
    let first = scenario.engine.render(Default::default()).unwrap();
    scenario.clock.set(
        scenario.logical_start
            + chrono::Duration::nanoseconds(1_000_000_000_i64 / i64::from(config.rate_hz)),
    );
    let second = scenario.engine.render(Default::default()).unwrap();
    assert_ne!(
        first.universes[&1][usize::from(ANIMATED_SLOT)],
        second.universes[&1][usize::from(ANIMATED_SLOT)]
    );
    assert_eq!(first.patched_slots[&1], SLOTS_PER_UNIVERSE);
    assert_eq!(second.patched_slots[&1], SLOTS_PER_UNIVERSE);
}

#[test]
fn route_matrix_preserves_protocol_selection_and_full_payloads() {
    let routes = routes(8, ProtocolSelection::Both, None);
    assert_eq!(routes.len(), 16);
    assert!(routes.iter().all(|route| route.minimum_slots == 512));
    assert_eq!(routes[0].protocol, light_output::Protocol::ArtNet);
    assert_eq!(routes[1].protocol, light_output::Protocol::Sacn);
}
