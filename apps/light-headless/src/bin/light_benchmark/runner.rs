use crate::light_benchmark::{
    arguments::{Arguments, Expectation, ProfileConfig, Transport},
    loopback::LoopbackDelivery,
    metadata,
    report::{
        BenchmarkReport, ContributionSources, DeadlineReport, FrameRateReport, OutputReport,
        PhaseReport, RunConfiguration, ScenarioReport, coverage,
    },
    scenario::{BenchmarkScenario, SLOTS_PER_UNIVERSE},
    statistics::distribution,
};
use chrono::Duration as ChronoDuration;
use light_output::{DmxFrame, EncodedPacket, Protocol, encode_routes};
use std::{
    collections::HashMap,
    hint::black_box,
    thread,
    time::{Duration, Instant},
};

const CID: [u8; 16] = [0x42; 16];
const SOURCE_NAME: &str = "ToskLight output benchmark";
const SAMPLED_DIAGNOSTIC_SECONDS: u64 = 1;
const REPORTING_TARGET_HZ: u16 = 44;

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    let profiles = if arguments.headless_stress_fixtures.is_some() {
        vec![crate::light_benchmark::arguments::BenchmarkProfile::HeadlessStress]
    } else {
        arguments.profiles.clone()
    };
    let mut scenarios = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let mut config = profile.config();
        let required_minimum_hz = REPORTING_TARGET_HZ;
        if let Some(rate_hz) = arguments.rate_hz {
            config.rate_hz = rate_hz;
        }
        if let Some(universes) = arguments.universes {
            config.universes = universes;
        }
        if let Some(fixtures_per_universe) = arguments.fixtures_per_universe {
            config.fixtures_per_universe = fixtures_per_universe;
        }
        if let Some(fixture_count) = arguments.headless_stress_fixtures {
            eprintln!(
                "benchmarking headless stress: {fixture_count} mixed shipped-mode fixtures at {} Hz",
                config.rate_hz
            );
        } else if arguments.sustained_show {
            eprintln!(
                "benchmarking {:?}: mixed-fixture sustained show across {} fully packed universes at {} Hz",
                profile, config.universes, config.rate_hz
            );
        } else {
            eprintln!(
                "benchmarking {:?}: {} packed universes, {} fixtures per universe at {} Hz",
                profile, config.universes, config.fixtures_per_universe, config.rate_hz
            );
        }
        scenarios.push(run_scenario(arguments, config, required_minimum_hz)?);
    }
    let required_floor_met = required_floor_result(&scenarios);
    let show_mutation = arguments
        .mutation_gate
        .then(crate::light_benchmark::mutation::run)
        .transpose()?;
    let patch_mutation = arguments
        .patch_gate
        .then(crate::light_benchmark::patch_mutation::run)
        .transpose()?;
    Ok(BenchmarkReport {
        schema_version: 8,
        benchmark: "tosklight_render_to_protocol_encoding_pipeline",
        reference: metadata::capture(arguments.hardware_label.as_deref()),
        configuration: RunConfiguration {
            measured_seconds: arguments.seconds,
            warmup_seconds: arguments.warmup_seconds,
            protocol: arguments.protocol,
            transport: arguments.transport,
            pacing_clock: "std::time::Instant monotonic deadlines",
            application_clock: "deterministic ManualClock advanced by logical tick index",
        },
        scenarios,
        measurement_coverage: coverage(arguments.transport),
        process_resources: crate::light_benchmark::process_resources::capture(),
        required_floor_met,
        show_mutation,
        patch_mutation,
    })
}

fn required_floor_result(scenarios: &[ScenarioReport]) -> Option<bool> {
    required_floor_result_for(
        scenarios
            .iter()
            .map(|scenario| (scenario.expectation, scenario.met_configured_rate)),
    )
}

fn required_floor_result_for(
    results: impl IntoIterator<Item = (Expectation, bool)>,
) -> Option<bool> {
    let mut required = results
        .into_iter()
        .filter(|(expectation, _)| *expectation == Expectation::RequiredFloor)
        .peekable();
    required.peek()?;
    Some(required.all(|(_, met)| met))
}

fn run_scenario(
    arguments: &Arguments,
    config: ProfileConfig,
    required_minimum_hz: u16,
) -> Result<ScenarioReport, String> {
    let (loopback, scenario) = prepare_scenario(arguments, config)?;
    // Zeroed here and read straight after, so one scenario's phase costs are not the running total
    // of every scenario and diagnostic before it.
    let timed = execute_timed_run(arguments, config, &scenario, loopback.as_ref())?;
    let render_phase_microseconds = light_engine::render_phases_enabled().then(|| {
        light_engine::accumulated_microseconds()
            .into_iter()
            .collect()
    });
    let state = timed.state;
    let warmup_ticks = timed.warmup_ticks;
    let warmup_elapsed = timed.warmup_elapsed;
    let expected_ticks = timed.expected_ticks;
    let elapsed = timed.elapsed;
    let measurement_resources = timed.measurement_resources;
    let sampled_contributions = crate::light_benchmark::sampled::measure(
        &scenario,
        warmup_ticks + expected_ticks,
        config.rate_hz,
        u64::from(config.rate_hz) * SAMPLED_DIAGNOSTIC_SECONDS,
    )?;
    let loopback_summary = loopback.map(LoopbackDelivery::finish);
    let achieved = state.completed_ticks as f64 / elapsed.as_secs_f64();
    let frame_rate = frame_rate_report(
        &state.completed_ticks_by_second,
        arguments.seconds,
        required_minimum_hz,
        state.completed_ticks,
        elapsed,
    );
    let met_configured_rate = frame_rate.gate_met;
    let diagnostic_batches = scenario.sampled_batches(scenario.logical_start);
    let diagnostic_samples = diagnostic_batches
        .iter()
        .map(light_engine::ContributionBatch::len)
        .sum();
    Ok(ScenarioReport {
        profile: config.profile,
        expectation: config.expectation,
        workload_tier: scenario.workload_tier,
        release_blocking: scenario.release_blocking,
        active_ui_surfaces: scenario.active_ui_surfaces,
        visualization_enabled: scenario.visualization_enabled,
        universes: scenario.universes,
        slots_per_universe: SLOTS_PER_UNIVERSE,
        fixture_count: scenario.fixture_count,
        physical_instance_count: scenario.physical_instance_count,
        fixtures_per_universe: (!arguments.sustained_show
            && arguments.headless_stress_fixtures.is_none())
        .then_some(config.fixtures_per_universe),
        fixture_footprint: scenario.fixture_footprint,
        fixture_inventory: scenario.fixture_inventory.clone(),
        dynamic_definition_count: scenario.dynamic_definition_count,
        animated_attribute_count: scenario.animated_attribute_count,
        master_lane_count: scenario.dynamic_definition_count,
        dynamic_lane_attributes: scenario.dynamic_lane_attributes,
        dynamic_excluded_fixture_count: scenario.dynamic_excluded_fixture_count,
        configured_rate_hz: config.rate_hz,
        warmup_ticks,
        warmup_elapsed_seconds: warmup_elapsed.as_secs_f64(),
        expected_ticks,
        completed_ticks: state.completed_ticks,
        achieved_ticks_per_second: achieved,
        elapsed_seconds: elapsed.as_secs_f64(),
        measurement_resources,
        met_configured_rate,
        frame_rate,
        deadline: DeadlineReport {
            period_microseconds: 1_000_000.0 / f64::from(config.rate_hz),
            dropped_ticks: state.dropped_ticks,
            deferred_ticks: state.deferred_ticks,
            deadline_misses: state.deadline_misses,
            definition: "dropped: scheduled interval elapsed before work began; deferred: prior pipeline work crossed this scheduled start; deadline miss: pipeline completed after its interval",
        },
        render_phase_microseconds,
        phases: PhaseReport {
            total_pipeline: distribution(&state.total),
            engine_render_combined: distribution(&state.render),
            protocol_encoding: distribution(&state.encode),
            loopback_datagram_delivery: (arguments.transport == Transport::Loopback)
                .then(|| distribution(&state.delivery))
                .flatten(),
            benchmark_validation_overhead: distribution(&state.validation),
        },
        output: OutputReport {
            packets_encoded: state.packets,
            dmx_slot_payload_bytes: state.packets.saturating_mul(u64::from(SLOTS_PER_UNIVERSE)),
            wire_bytes_encoded: state.wire_bytes,
            rolling_checksum_fnv1a64: format!("{:016x}", state.checksum),
            full_universe_assertions: state.full_universe_assertions,
        },
        contribution_sources: ContributionSources {
            playback_group_cue_changes: true,
            programmer_fixture_values: true,
            static_group_programming: true,
            playback_attribute_dynamic: true,
            dynamic_attribute: scenario.dynamic_attribute.0.to_string(),
            dynamic_attribute_has_static_or_programmer_value: scenario
                .dynamic_overlaps_static_or_programmer,
            programmer_assignment_fraction: scenario.programmer_assignment_fraction,
            sampled_replacement_diagnostic_batches: diagnostic_batches.len(),
            sampled_replacement_diagnostic_samples: diagnostic_samples,
            sampled_batch_construction_in_timed_pipeline: false,
        },
        sampled_contributions,
        loopback: loopback_summary,
    })
}

struct TimedScenarioRun {
    state: TickState,
    warmup_ticks: u64,
    warmup_elapsed: Duration,
    expected_ticks: u64,
    elapsed: Duration,
    measurement_resources: crate::light_benchmark::report::MeasurementResourceReport,
}

fn execute_timed_run(
    arguments: &Arguments,
    config: ProfileConfig,
    scenario: &BenchmarkScenario,
    loopback: Option<&LoopbackDelivery>,
) -> Result<TimedScenarioRun, String> {
    let mut state = TickState::new(arguments.seconds);
    let warmup_started = Instant::now();
    let warmup_duration = Duration::from_secs(arguments.warmup_seconds);
    let mut warmup_ticks = 0_u64;
    while warmup_started.elapsed() < warmup_duration {
        run_tick(
            scenario,
            loopback,
            &mut state.sequences,
            warmup_ticks,
            config.rate_hz,
        )?;
        warmup_ticks += 1;
    }
    let warmup_elapsed = warmup_started.elapsed();
    // Zeroed after warmup, not before it: a warmup render is cheaper than a measured one, and
    // counting both makes every phase look like a fraction of a frame that nothing accounts for.
    light_engine::reset_render_phases();
    let expected_ticks = u64::from(config.rate_hz) * arguments.seconds;
    let mut resource_sampler =
        crate::light_benchmark::process_resources::MeasurementSampler::start();
    let measured_at = Instant::now();
    let mut tick = 0_u64;
    let mut previous_pipeline_completion = measured_at;
    while tick < expected_ticks {
        let scheduled = measured_at + scheduled_offset(tick, config.rate_hz);
        let deadline = measured_at + scheduled_offset(tick + 1, config.rate_hz);
        let now = Instant::now();
        if now < scheduled {
            thread::sleep(scheduled - now);
        }
        let now = Instant::now();
        if now >= deadline {
            let current_tick = tick_at(now.duration_since(measured_at), config.rate_hz);
            let skipped = current_tick.saturating_sub(tick).min(expected_ticks - tick);
            state.dropped_ticks += skipped;
            tick += skipped;
            resource_sampler.sample_if_due();
            continue;
        }
        if previous_pipeline_completion > scheduled {
            state.deferred_ticks += 1;
        }
        let sample = run_tick(
            scenario,
            loopback,
            &mut state.sequences,
            warmup_ticks + tick,
            config.rate_hz,
        )?;
        previous_pipeline_completion = sample.pipeline_completed_at;
        if sample.pipeline_completed_at > deadline {
            state.deadline_misses += 1;
        }
        state.record(sample, tick, config.rate_hz);
        resource_sampler.sample_if_due();
        tick += 1;
    }
    Ok(TimedScenarioRun {
        state,
        warmup_ticks,
        warmup_elapsed,
        expected_ticks,
        elapsed: measured_at.elapsed(),
        measurement_resources: resource_sampler.finish(),
    })
}

fn prepare_scenario(
    arguments: &Arguments,
    config: ProfileConfig,
) -> Result<(Option<LoopbackDelivery>, BenchmarkScenario), String> {
    let loopback = match arguments.transport {
        Transport::EncodeOnly => None,
        Transport::Loopback => Some(
            LoopbackDelivery::start()
                .map_err(|error| format!("bind safe UDP loopback benchmark transport: {error}"))?,
        ),
    };
    let destination = loopback.as_ref().map(LoopbackDelivery::destination);
    let scenario = if let Some(fixture_count) = arguments.headless_stress_fixtures {
        let package_dir = arguments.fixture_package_dir.as_deref().ok_or_else(|| {
            "--headless-stress-fixtures requires --fixture-package-dir".to_owned()
        })?;
        crate::light_benchmark::headless_stress_show::build(
            fixture_count,
            config,
            arguments.protocol,
            destination,
            std::path::Path::new(package_dir),
        )?
    } else if arguments.sustained_show {
        let package_dir = arguments
            .fixture_package_dir
            .as_deref()
            .ok_or_else(|| "--sustained-show requires --fixture-package-dir".to_owned())?;
        crate::light_benchmark::sustained_show::build(
            config,
            arguments.protocol,
            destination,
            std::path::Path::new(package_dir),
        )?
    } else {
        BenchmarkScenario::build(config, arguments.protocol, destination)?
    };
    Ok((loopback, scenario))
}

#[derive(Default)]
struct TickState {
    sequences: HashMap<(Protocol, u16), u8>,
    total: Vec<Duration>,
    render: Vec<Duration>,
    encode: Vec<Duration>,
    delivery: Vec<Duration>,
    validation: Vec<Duration>,
    completed_ticks: u64,
    dropped_ticks: u64,
    deferred_ticks: u64,
    deadline_misses: u64,
    packets: u64,
    wire_bytes: u64,
    checksum: u64,
    full_universe_assertions: u64,
    completed_ticks_by_second: Vec<u64>,
}

impl TickState {
    fn new(seconds: u64) -> Self {
        Self {
            completed_ticks_by_second: vec![0; seconds as usize],
            ..Default::default()
        }
    }

    fn record(&mut self, sample: TickSample, scheduled_tick: u64, rate_hz: u16) {
        self.total.push(sample.total);
        self.render.push(sample.render);
        self.encode.push(sample.encode);
        self.delivery.push(sample.delivery);
        self.validation.push(sample.validation);
        self.completed_ticks += 1;
        self.packets += sample.packets;
        self.wire_bytes += sample.wire_bytes;
        self.checksum = self.checksum.rotate_left(7) ^ sample.checksum;
        self.full_universe_assertions += 1;
        let second = scheduled_tick / u64::from(rate_hz);
        if let Some(completed) = self.completed_ticks_by_second.get_mut(second as usize) {
            *completed += 1;
        }
    }
}

struct TickSample {
    total: Duration,
    render: Duration,
    encode: Duration,
    delivery: Duration,
    validation: Duration,
    pipeline_completed_at: Instant,
    packets: u64,
    wire_bytes: u64,
    checksum: u64,
}

fn run_tick(
    scenario: &BenchmarkScenario,
    loopback: Option<&LoopbackDelivery>,
    sequences: &mut HashMap<(Protocol, u16), u8>,
    logical_tick: u64,
    rate_hz: u16,
) -> Result<TickSample, String> {
    let logical_nanos = logical_tick.saturating_mul(1_000_000_000) / u64::from(rate_hz);
    let logical_time = scenario.logical_start
        + ChronoDuration::nanoseconds(i64::try_from(logical_nanos).unwrap_or(i64::MAX));
    scenario.clock.set(logical_time);
    let total_started = Instant::now();
    let render_started = Instant::now();
    let dynamic = scenario.dynamic_batch(logical_time);
    let rendered = match dynamic.as_ref() {
        Some(dynamic) => scenario
            .engine
            .render_with_contribution_batches(Default::default(), std::slice::from_ref(dynamic)),
        None => scenario.engine.render(Default::default()),
    }
    .map_err(|error| format!("render benchmark frame: {error}"))?;
    let render = render_started.elapsed();
    let encode_started = Instant::now();
    let packets = encode_routes(
        &rendered.routes,
        &rendered.universes,
        &rendered.patched_slots,
        sequences,
        CID,
        SOURCE_NAME,
        100,
    )
    .map_err(|error| format!("encode benchmark routes: {error}"))?;
    let encode = encode_started.elapsed();
    let delivery_started = Instant::now();
    if let Some(loopback) = loopback {
        loopback
            .send(&packets)
            .map_err(|error| format!("send benchmark loopback datagrams: {error}"))?;
    }
    let delivery = delivery_started.elapsed();
    let pipeline_completed_at = Instant::now();
    let total = total_started.elapsed();
    let validation_started = Instant::now();
    validate_full_output(
        scenario,
        &rendered.universes,
        &rendered.patched_slots,
        &packets,
    )?;
    let checksum = checksum(&rendered.universes, &packets);
    black_box(checksum);
    let validation = validation_started.elapsed();
    Ok(TickSample {
        total,
        render,
        encode,
        delivery,
        validation,
        pipeline_completed_at,
        packets: packets.len() as u64,
        wire_bytes: packets.iter().map(|packet| packet.bytes.len() as u64).sum(),
        checksum,
    })
}

fn validate_full_output(
    scenario: &BenchmarkScenario,
    frames: &HashMap<u16, DmxFrame>,
    patched_slots: &HashMap<u16, u16>,
    packets: &[EncodedPacket],
) -> Result<(), String> {
    if frames.len() != usize::from(scenario.universes) || packets.len() != scenario.packet_count {
        return Err("pipeline did not produce every configured universe and route".into());
    }
    for universe in 1..=scenario.universes {
        let expected = scenario.expected_patched_slots.get(&universe);
        if !frames.contains_key(&universe) || patched_slots.get(&universe) != expected {
            return Err(format!(
                "logical universe {universe} ended at {:?} instead of expected {:?}",
                patched_slots.get(&universe),
                expected,
            ));
        }
    }
    for packet in packets {
        let expected = match packet.protocol {
            Protocol::ArtNet => 18 + usize::from(SLOTS_PER_UNIVERSE),
            Protocol::Sacn => 126 + usize::from(SLOTS_PER_UNIVERSE),
        };
        if packet.bytes.len() != expected {
            return Err(format!(
                "{:?} universe {} encoded {} bytes instead of {expected}",
                packet.protocol,
                packet.universe,
                packet.bytes.len()
            ));
        }
    }
    Ok(())
}

fn checksum(frames: &HashMap<u16, DmxFrame>, packets: &[EncodedPacket]) -> u64 {
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    let mut universes = frames.keys().copied().collect::<Vec<_>>();
    universes.sort_unstable();
    for universe in universes {
        checksum = fnv1a(checksum, &universe.to_le_bytes());
        checksum = fnv1a(checksum, &frames[&universe]);
    }
    for packet in packets {
        checksum = fnv1a(checksum, &packet.bytes);
    }
    checksum
}

fn fnv1a(mut checksum: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        checksum ^= u64::from(*byte);
        checksum = checksum.wrapping_mul(0x0000_0100_0000_01b3);
    }
    checksum
}

fn scheduled_offset(tick: u64, rate_hz: u16) -> Duration {
    Duration::from_nanos(tick.saturating_mul(1_000_000_000) / u64::from(rate_hz))
}

fn tick_at(elapsed: Duration, rate_hz: u16) -> u64 {
    let nanos = elapsed.as_nanos();
    let tick = nanos.saturating_mul(u128::from(rate_hz)) / 1_000_000_000;
    tick.min(u128::from(u64::MAX)) as u64
}

fn frame_rate_report(
    completed_ticks_by_second: &[u64],
    measured_seconds: u64,
    required_minimum_hz: u16,
    completed_ticks: u64,
    elapsed: Duration,
) -> FrameRateReport {
    let required = u64::from(required_minimum_hz);
    let minimum = completed_ticks_by_second.iter().copied().min().unwrap_or(0);
    let mut sorted_windows = completed_ticks_by_second.to_vec();
    sorted_windows.sort_unstable();
    let p95_index = sorted_windows
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    let p95 = sorted_windows.get(p95_index).copied().unwrap_or(0);
    let maximum = sorted_windows.last().copied().unwrap_or(0);
    let windows_below_minimum = completed_ticks_by_second
        .iter()
        .filter(|completed| **completed < required)
        .count() as u64;
    let windows_below_reporting_target = completed_ticks_by_second
        .iter()
        .filter(|completed| **completed < u64::from(REPORTING_TARGET_HZ))
        .count() as u64;
    let average = completed_ticks as f64 / measured_seconds as f64;
    FrameRateReport {
        required_minimum_hz,
        average_completed_hz: average,
        wall_clock_average_completed_hz: completed_ticks as f64 / elapsed.as_secs_f64(),
        minimum_one_second_completed_hz: minimum as f64,
        p95_one_second_completed_hz: p95 as f64,
        maximum_one_second_completed_hz: maximum as f64,
        one_second_windows: completed_ticks_by_second.len() as u64,
        windows_below_minimum,
        reporting_target_hz: REPORTING_TARGET_HZ,
        windows_below_reporting_target,
        gate_met: average >= f64::from(required_minimum_hz) && windows_below_minimum == 0,
        definition: "average uses completed scheduled frames over the configured measurement duration; minimum, p95, and maximum describe completed scheduled frames in non-overlapping one-second intervals",
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
