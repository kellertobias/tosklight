use super::*;

#[test]
fn programmer_overlap_is_exactly_one_quarter_of_slots() {
    let fixtures = (1..=4)
        .map(|number| FixtureId(fixed_uuid(1, number)))
        .collect::<Vec<_>>();
    let assignments = programmer_assignments(&fixtures, SLOTS_PER_UNIVERSE).collect::<Vec<_>>();
    assert_eq!(
        assignments.len(),
        4 * usize::from(SLOTS_PER_UNIVERSE - 1) / 4
    );
    assert!(assignments.iter().all(|(_, attribute, _)| {
        attribute != &slot_attribute(animated_slot(SLOTS_PER_UNIVERSE))
    }));
}

#[test]
fn animated_slot_is_reserved_exclusively_for_the_phaser() {
    let fixture = FixtureId(fixed_uuid(1, 1));
    let group = static_group(&[fixture], SLOTS_PER_UNIVERSE);
    let (cue_list, _) = playback(SLOTS_PER_UNIVERSE);
    let cue = &cue_list.cues[0];
    assert!(
        !group
            .programming
            .contains_key(&slot_attribute(animated_slot(SLOTS_PER_UNIVERSE)))
    );
    assert!(
        cue.group_changes.iter().all(|change| {
            change.attribute != slot_attribute(animated_slot(SLOTS_PER_UNIVERSE))
        })
    );
    assert_eq!(
        cue.phasers[0].attribute,
        slot_attribute(animated_slot(SLOTS_PER_UNIVERSE))
    );
    assert!(
        programmer_assignments(&[fixture], SLOTS_PER_UNIVERSE).all(|(_, attribute, _)| {
            attribute != slot_attribute(animated_slot(SLOTS_PER_UNIVERSE))
        })
    );
}

#[test]
fn sampled_workload_uses_multiple_replacement_batches_without_changing_output() {
    let config = ProfileConfig {
        profile: crate::light_benchmark::arguments::BenchmarkProfile::LowPower4,
        expectation: crate::light_benchmark::arguments::Expectation::LowPowerGoal,
        universes: 1,
        rate_hz: 40,
        fixtures_per_universe: 1,
    };
    let scenario = BenchmarkScenario::build(config, ProtocolSelection::ArtNet, None).unwrap();
    let sampled_batches = scenario.sampled_batches(scenario.logical_start);
    assert_eq!(sampled_batches.len(), SAMPLED_BATCH_COUNT);
    assert!(sampled_batches.iter().all(|batch| !batch.is_empty()));
    assert!(
        sampled_batches
            .iter()
            .flat_map(ContributionBatch::samples)
            .all(|sample| sample.replacement_source().is_some())
    );

    let ordinary = scenario.engine.render(Default::default()).unwrap();
    let sampled = scenario
        .engine
        .render_with_contribution_batches(Default::default(), &sampled_batches)
        .unwrap();
    assert_eq!(sampled.universes, ordinary.universes);
    assert_eq!(sampled.patched_slots, ordinary.patched_slots);
}

#[test]
fn consecutive_logical_ticks_move_the_exclusive_phaser_slot() {
    let config = ProfileConfig {
        profile: crate::light_benchmark::arguments::BenchmarkProfile::LowPower4,
        expectation: crate::light_benchmark::arguments::Expectation::LowPowerGoal,
        universes: 1,
        rate_hz: 120,
        fixtures_per_universe: 1,
    };
    let scenario = BenchmarkScenario::build(config, ProtocolSelection::ArtNet, None).unwrap();
    let first = scenario.engine.render(Default::default()).unwrap();
    scenario.clock.set(
        scenario.logical_start
            + chrono::Duration::nanoseconds(1_000_000_000_i64 / i64::from(config.rate_hz)),
    );
    let second = scenario.engine.render(Default::default()).unwrap();
    assert_ne!(
        first.universes[&1][usize::from(animated_slot(SLOTS_PER_UNIVERSE))],
        second.universes[&1][usize::from(animated_slot(SLOTS_PER_UNIVERSE))]
    );
    assert_eq!(first.patched_slots[&1], SLOTS_PER_UNIVERSE);
    assert_eq!(second.patched_slots[&1], SLOTS_PER_UNIVERSE);
}

#[test]
fn dense_fixture_layout_fills_every_slot_and_reports_fixture_count() {
    let config = ProfileConfig {
        profile: crate::light_benchmark::arguments::BenchmarkProfile::HardFloor,
        expectation: crate::light_benchmark::arguments::Expectation::RequiredFloor,
        universes: 2,
        rate_hz: 100,
        fixtures_per_universe: 64,
    };
    let scenario = BenchmarkScenario::build(config, ProtocolSelection::ArtNet, None).unwrap();
    let rendered = scenario.engine.render(Default::default()).unwrap();
    assert_eq!(scenario.fixture_count, 128);
    assert_eq!(scenario.fixture_footprint, 8);
    assert_eq!(rendered.universes.len(), 2);
    assert!(rendered.patched_slots.values().all(|slots| *slots == 512));
}

#[test]
fn route_matrix_preserves_protocol_selection_and_full_payloads() {
    let routes = routes(8, ProtocolSelection::Both, None);
    assert_eq!(routes.len(), 16);
    assert!(routes.iter().all(|route| route.minimum_slots == 512));
    assert_eq!(routes[0].protocol, light_output::Protocol::ArtNet);
    assert_eq!(routes[1].protocol, light_output::Protocol::Sacn);
}
