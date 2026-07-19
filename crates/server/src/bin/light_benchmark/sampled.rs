use crate::light_benchmark::{
    report::SampledContributionReport,
    scenario::{BenchmarkScenario, SAMPLED_ASSIGNMENT_DIVISOR, SLOTS_PER_UNIVERSE},
    statistics::distribution,
};
use chrono::Duration as ChronoDuration;
use std::{hint::black_box, time::Instant};

pub fn measure(
    scenario: &BenchmarkScenario,
    first_logical_tick: u64,
    rate_hz: u16,
    measured_renders: u64,
) -> Result<SampledContributionReport, String> {
    let sampled_at = logical_time(scenario, first_logical_tick, rate_hz);
    let sampled_batches = scenario.sampled_batches(sampled_at);
    let samples_per_render = sampled_batches
        .iter()
        .map(light_engine::ContributionBatch::len)
        .sum();
    let replacements_per_render = sampled_batches
        .iter()
        .flat_map(|batch| batch.samples())
        .filter(|sample| sample.replacement_source().is_some())
        .count();
    let mut render_durations = Vec::with_capacity(measured_renders as usize);
    let started = Instant::now();
    for offset in 0..measured_renders {
        set_logical_time(scenario, first_logical_tick.saturating_add(offset), rate_hz);
        let render_started = Instant::now();
        let rendered = scenario
            .engine
            .render_with_contribution_batches(Default::default(), &sampled_batches)
            .map_err(|error| format!("render sampled benchmark frame: {error}"))?;
        render_durations.push(render_started.elapsed());
        black_box((&rendered.universes, &rendered.patched_slots));
        let complete_patch = rendered.patched_slots.len() == usize::from(scenario.universes)
            && rendered
                .patched_slots
                .values()
                .all(|slots| *slots == SLOTS_PER_UNIVERSE);
        if rendered.universes.len() != usize::from(scenario.universes) || !complete_patch {
            return Err("sampled render did not produce every configured universe".into());
        }
    }
    let elapsed = started.elapsed();
    Ok(SampledContributionReport {
        batches_per_render: sampled_batches.len(),
        samples_per_render,
        replacements_per_render,
        source_selection: match SAMPLED_ASSIGNMENT_DIVISOR {
            8 => "every eighth Programmer and Playback assignment, round-robin across batches",
            _ => "configured sampled assignment divisor",
        },
        measured_renders,
        elapsed_seconds: elapsed.as_secs_f64(),
        achieved_renders_per_second: measured_renders as f64 / elapsed.as_secs_f64(),
        engine_render_combined: distribution(&render_durations),
        measurement_mode: "unpaced render-only diagnostic after the ordinary scheduled pipeline; ContributionBatch construction is outside the timed path",
        included_in_required_floor: false,
    })
}

fn set_logical_time(scenario: &BenchmarkScenario, tick: u64, rate_hz: u16) {
    scenario.clock.set(logical_time(scenario, tick, rate_hz));
}

fn logical_time(
    scenario: &BenchmarkScenario,
    tick: u64,
    rate_hz: u16,
) -> chrono::DateTime<chrono::Utc> {
    let nanos = tick.saturating_mul(1_000_000_000) / u64::from(rate_hz);
    scenario.logical_start + ChronoDuration::nanoseconds(i64::try_from(nanos).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light_benchmark::arguments::{
        BenchmarkProfile, Expectation, ProfileConfig, ProtocolSelection,
    };

    #[test]
    fn report_counts_the_prebuilt_replacement_workload() {
        let config = ProfileConfig {
            profile: BenchmarkProfile::LowPower4,
            expectation: Expectation::LowPowerGoal,
            universes: 1,
            rate_hz: 40,
        };
        let scenario = BenchmarkScenario::build(config, ProtocolSelection::ArtNet, None).unwrap();
        let report = measure(&scenario, 0, config.rate_hz, 2).unwrap();

        assert_eq!(report.batches_per_render, 4);
        assert_eq!(report.samples_per_render, report.replacements_per_render);
        assert!(report.samples_per_render > 4);
        assert_eq!(report.engine_render_combined.unwrap().samples, 2);
        assert!(!report.included_in_required_floor);
    }
}
