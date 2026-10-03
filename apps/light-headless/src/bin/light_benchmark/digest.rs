//! TL-639: `--digest-ticks N` renders exactly N logical ticks back to back, without warmup or
//! pacing, and prints per-tick digests of everything a frame publishes: the encoded DMX, and per
//! fixture the resolved semantic and head values, the physical forward results with the final
//! native values, and (semantic runs) the accepted Color results. Two builds of the same
//! workload are compared fixture by fixture. Dynamic instance identities, which seed Random lanes
//! and order equal-priority Dynamics, are derived from the start order in this mode so both
//! builds see the same ones. The timed run is unchanged.
use crate::light_benchmark::{
    arguments::Arguments,
    runner::{CID, SOURCE_NAME, checksum, fnv1a, prepare_scenario, profile_configs},
    scenario::BenchmarkScenario,
};
use chrono::Duration as ChronoDuration;
use light_core::FixtureId;
use light_engine::RenderResult;
use light_output::encode_routes;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};

pub fn run(arguments: &Arguments, ticks: u64) -> Result<Value, String> {
    crate::light_benchmark::semantic_programming::DERIVED_INSTANCE_IDS
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let mut profiles = Vec::new();
    for config in profile_configs(arguments) {
        let (_loopback, scenario) = prepare_scenario(arguments, config)?;
        let mut sequences = HashMap::new();
        let mut frames = Vec::with_capacity(ticks as usize);
        for tick in 0..ticks {
            frames.push(digest_tick(
                &scenario,
                &mut sequences,
                tick,
                config.rate_hz,
            )?);
        }
        profiles.push(
            json!({ "profile": config.profile, "rate_hz": config.rate_hz, "frames": frames }),
        );
    }
    Ok(json!({ "digest_ticks": ticks, "profiles": profiles }))
}

fn digest_tick(
    scenario: &BenchmarkScenario,
    sequences: &mut HashMap<(light_output::Protocol, u16), u8>,
    tick: u64,
    rate_hz: u16,
) -> Result<Value, String> {
    let logical_nanos = tick.saturating_mul(1_000_000_000) / u64::from(rate_hz);
    let logical_time = scenario.logical_start
        + ChronoDuration::nanoseconds(i64::try_from(logical_nanos).unwrap_or(i64::MAX));
    scenario.clock.set(logical_time);
    if let Some(feed) = scenario
        .live
        .as_ref()
        .and_then(|live| live.tracking.as_ref())
    {
        feed.inject(&scenario.engine, logical_nanos / 1_000);
    }
    let mut rows = FixtureRows::default();
    let (rendered, work) = match scenario.live.as_ref() {
        Some(live) => {
            let frame = live
                .bench
                .render(Default::default(), &[])
                .map_err(|error| format!("render semantic digest frame: {error}"))?;
            for (target, row) in live.bench.readout_digest(&frame.rendered) {
                rows.push(target, row);
            }
            (frame.rendered, Some(format!("{:?}", frame.work)))
        }
        None => {
            let dynamic = scenario.dynamic_batch(logical_time);
            let rendered = match dynamic.as_ref() {
                Some(dynamic) => scenario.engine.render_with_contribution_batches(
                    Default::default(),
                    std::slice::from_ref(dynamic),
                ),
                None => scenario.engine.render(Default::default()),
            }
            .map_err(|error| format!("render digest frame: {error}"))?;
            (rendered, None)
        }
    };
    let packets = encode_routes(
        &rendered.routes,
        &rendered.universes,
        &rendered.patched_slots,
        sequences,
        CID,
        SOURCE_NAME,
        100,
    )
    .map_err(|error| format!("encode digest routes: {error}"))?;
    collect_rows(&rendered, &mut rows);
    if let Some(directory) = std::env::var_os("LIGHT_BENCHMARK_DIGEST_DUMP") {
        rows.dump(&std::path::Path::new(&directory).join(format!("tick-{tick}.txt")))?;
    }
    Ok(json!({
        "tick": tick,
        "dmx": format!("{:016x}", checksum(&rendered.universes, &packets)),
        "points": hash(&format!("{:?}{:?}", rendered.points.as_slice(), rendered.mounts.mounts())),
        "work": work,
        "fixtures": rows.digests(),
    }))
}

/// Text rows per fixture, hashed in a stable order.
#[derive(Default)]
struct FixtureRows(BTreeMap<u128, Vec<String>>);

impl FixtureRows {
    fn push(&mut self, fixture: FixtureId, row: String) {
        self.0.entry(fixture.0.as_u128()).or_default().push(row);
    }

    /// Every row in digest order, for locating a mismatch (`LIGHT_BENCHMARK_DIGEST_DUMP=DIR`).
    fn dump(&self, path: &std::path::Path) -> Result<(), String> {
        let mut text = String::new();
        for (fixture, rows) in &self.0 {
            let mut rows = rows.clone();
            rows.sort_unstable();
            for row in rows {
                text.push_str(&format!("{fixture:032x} {row}\n"));
            }
        }
        std::fs::write(path, text).map_err(|error| format!("write digest dump: {error}"))
    }

    fn digests(mut self) -> Value {
        let mut map = Map::new();
        for (fixture, rows) in &mut self.0 {
            rows.sort_unstable();
            map.insert(
                format!("{fixture:032x}"),
                Value::String(hash(&rows.join("\n"))),
            );
        }
        Value::Object(map)
    }
}

fn collect_rows(rendered: &RenderResult, rows: &mut FixtureRows) {
    for ((fixture, attribute), value) in rendered.resolved_values.values().iter() {
        rows.push(*fixture, format!("value {attribute:?}={value:?}"));
    }
    for ((fixture, attribute), value) in rendered.profile_visualization_values.iter() {
        rows.push(*fixture, format!("head {attribute:?}={value:?}"));
    }
    for instance in &rendered.physical.instances {
        rows.push(
            instance.fixture_id,
            format!(
                "physical {:?} {:?}{:?}{:?} c{} raw{:?} col{:?} ax{:?} lens{:?} opt{:?} {:?}{:?}{:?}",
                instance.instance_id,
                instance.color_support,
                instance.position_support,
                instance.optics_support,
                instance.complete,
                instance.native_raw,
                instance.colors(),
                instance.axes(),
                instance.lenses(),
                instance.optics(),
                instance.color_diagnostic,
                instance.position_diagnostic,
                instance.optics_diagnostic,
            ),
        );
    }
}

fn hash(text: &str) -> String {
    format!("{:016x}", fnv1a(0xcbf2_9ce4_8422_2325, text.as_bytes()))
}
