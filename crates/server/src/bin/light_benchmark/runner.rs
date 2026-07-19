use crate::light_benchmark::{
    arguments::{Arguments, Expectation, ProfileConfig, Transport},
    loopback::LoopbackDelivery,
    metadata,
    report::{
        BenchmarkReport, ContributionSources, DeadlineReport, OutputReport, PhaseReport,
        RunConfiguration, ScenarioReport, coverage,
    },
    scenario::{
        ANIMATED_SLOT, BenchmarkScenario, PROGRAMMER_ASSIGNMENT_DIVISOR, SLOTS_PER_UNIVERSE,
    },
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

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    let mut scenarios = Vec::with_capacity(arguments.profiles.len());
    for profile in &arguments.profiles {
        let config = profile.config();
        eprintln!(
            "benchmarking {:?}: {} fully packed universes at {} Hz",
            profile, config.universes, config.rate_hz
        );
        scenarios.push(run_scenario(arguments, config)?);
    }
    let required_floor_met = required_floor_result(&scenarios);
    Ok(BenchmarkReport {
        schema_version: 1,
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
        required_floor_met,
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

fn run_scenario(arguments: &Arguments, config: ProfileConfig) -> Result<ScenarioReport, String> {
    let loopback = match arguments.transport {
        Transport::EncodeOnly => None,
        Transport::Loopback => Some(
            LoopbackDelivery::start()
                .map_err(|error| format!("bind safe UDP loopback benchmark transport: {error}"))?,
        ),
    };
    let destination = loopback.as_ref().map(LoopbackDelivery::destination);
    let scenario = BenchmarkScenario::build(config, arguments.protocol, destination)?;
    let mut state = TickState::default();
    let warmup_started = Instant::now();
    let warmup_duration = Duration::from_secs(arguments.warmup_seconds);
    let mut warmup_ticks = 0_u64;
    while warmup_started.elapsed() < warmup_duration {
        run_tick(
            &scenario,
            loopback.as_ref(),
            &mut state.sequences,
            warmup_ticks,
            config.rate_hz,
        )?;
        warmup_ticks += 1;
    }
    let warmup_elapsed = warmup_started.elapsed();

    let expected_ticks = u64::from(config.rate_hz) * arguments.seconds;
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
            continue;
        }
        if previous_pipeline_completion > scheduled {
            state.deferred_ticks += 1;
        }
        let sample = run_tick(
            &scenario,
            loopback.as_ref(),
            &mut state.sequences,
            warmup_ticks + tick,
            config.rate_hz,
        )?;
        previous_pipeline_completion = sample.pipeline_completed_at;
        if sample.pipeline_completed_at > deadline {
            state.deadline_misses += 1;
        }
        state.record(sample);
        tick += 1;
    }
    let elapsed = measured_at.elapsed();
    let loopback_summary = loopback.map(LoopbackDelivery::finish);
    let achieved = state.completed_ticks as f64 / elapsed.as_secs_f64();
    let met_configured_rate = state.dropped_ticks == 0 && state.deadline_misses == 0;
    Ok(ScenarioReport {
        profile: config.profile,
        expectation: config.expectation,
        universes: config.universes,
        slots_per_universe: SLOTS_PER_UNIVERSE,
        configured_rate_hz: config.rate_hz,
        warmup_ticks,
        warmup_elapsed_seconds: warmup_elapsed.as_secs_f64(),
        expected_ticks,
        completed_ticks: state.completed_ticks,
        achieved_ticks_per_second: achieved,
        elapsed_seconds: elapsed.as_secs_f64(),
        met_configured_rate,
        deadline: DeadlineReport {
            period_microseconds: 1_000_000.0 / f64::from(config.rate_hz),
            dropped_ticks: state.dropped_ticks,
            deferred_ticks: state.deferred_ticks,
            deadline_misses: state.deadline_misses,
            definition: "dropped: scheduled interval elapsed before work began; deferred: prior pipeline work crossed this scheduled start; deadline miss: pipeline completed after its interval",
        },
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
            playback_attribute_phaser: true,
            exclusive_phaser_dmx_slot: ANIMATED_SLOT + 1,
            phaser_slot_has_static_or_programmer_value: false,
            programmer_slot_fraction: match PROGRAMMER_ASSIGNMENT_DIVISOR {
                4 => "1/4",
                _ => "configured divisor",
            },
        },
        loopback: loopback_summary,
    })
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
}

impl TickState {
    fn record(&mut self, sample: TickSample) {
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
    scenario.clock.set(
        scenario.logical_start
            + ChronoDuration::nanoseconds(i64::try_from(logical_nanos).unwrap_or(i64::MAX)),
    );
    let total_started = Instant::now();
    let render_started = Instant::now();
    let rendered = scenario
        .engine
        .render(Default::default())
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
        if !frames.contains_key(&universe) || patched_slots.get(&universe) != Some(&512) {
            return Err(format!(
                "logical universe {universe} is not fully patched to slot 512"
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

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
