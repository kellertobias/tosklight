use crate::light_benchmark::{
    arguments::{Expectation, ProtocolSelection, Transport},
    loopback::LoopbackSummary,
    metadata::ReferenceMetadata,
    statistics::Distribution,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub schema_version: u16,
    pub benchmark: &'static str,
    pub reference: ReferenceMetadata,
    pub configuration: RunConfiguration,
    pub scenarios: Vec<ScenarioReport>,
    pub measurement_coverage: MeasurementCoverage,
    /// `None` means the hard-floor profile was not selected for this run.
    pub required_floor_met: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RunConfiguration {
    pub measured_seconds: u64,
    pub warmup_seconds: u64,
    pub protocol: ProtocolSelection,
    pub transport: Transport,
    pub pacing_clock: &'static str,
    pub application_clock: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ScenarioReport {
    pub profile: crate::light_benchmark::arguments::BenchmarkProfile,
    pub expectation: Expectation,
    pub universes: u16,
    pub slots_per_universe: u16,
    pub configured_rate_hz: u16,
    pub warmup_ticks: u64,
    pub warmup_elapsed_seconds: f64,
    pub expected_ticks: u64,
    pub completed_ticks: u64,
    pub achieved_ticks_per_second: f64,
    pub elapsed_seconds: f64,
    pub met_configured_rate: bool,
    pub deadline: DeadlineReport,
    pub phases: PhaseReport,
    pub output: OutputReport,
    pub contribution_sources: ContributionSources,
    pub loopback: Option<LoopbackSummary>,
}

#[derive(Debug, Serialize)]
pub struct DeadlineReport {
    pub period_microseconds: f64,
    pub dropped_ticks: u64,
    pub deferred_ticks: u64,
    pub deadline_misses: u64,
    pub definition: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PhaseReport {
    pub total_pipeline: Option<Distribution>,
    pub engine_render_combined: Option<Distribution>,
    pub protocol_encoding: Option<Distribution>,
    pub loopback_datagram_delivery: Option<Distribution>,
    pub benchmark_validation_overhead: Option<Distribution>,
}

#[derive(Debug, Serialize)]
pub struct OutputReport {
    pub packets_encoded: u64,
    pub dmx_slot_payload_bytes: u64,
    pub wire_bytes_encoded: u64,
    pub rolling_checksum_fnv1a64: String,
    pub full_universe_assertions: u64,
}

#[derive(Debug, Serialize)]
pub struct ContributionSources {
    pub playback_group_cue_changes: bool,
    pub programmer_fixture_values: bool,
    pub static_group_programming: bool,
    pub playback_attribute_phaser: bool,
    pub exclusive_phaser_dmx_slot: u16,
    pub phaser_slot_has_static_or_programmer_value: bool,
    pub programmer_slot_fraction: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MeasurementCoverage {
    pub contribution_sampling: CoverageItem,
    pub arbitration: CoverageItem,
    pub fixture_projection: CoverageItem,
    pub protocol_encoding: CoverageItem,
    pub socket_delivery: CoverageItem,
    pub cpu_usage: CoverageItem,
    pub allocation_rate: CoverageItem,
    pub sound_to_light_analysis: CoverageItem,
    pub timing_path_exclusions: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct CoverageItem {
    pub status: &'static str,
    pub note: &'static str,
}

pub fn coverage(transport: Transport) -> MeasurementCoverage {
    let socket_delivery = match transport {
        Transport::EncodeOnly => CoverageItem {
            status: "not_measured",
            note: "encode-only mode performs no socket I/O",
        },
        Transport::Loopback => CoverageItem {
            status: "measured_separately",
            note: "measures benchmark-owned loopback UDP send_to calls, not production NetworkOutput internals",
        },
    };
    MeasurementCoverage {
        contribution_sampling: CoverageItem {
            status: "included_not_separately_instrumented",
            note: "included in engine_render_combined",
        },
        arbitration: CoverageItem {
            status: "included_not_separately_instrumented",
            note: "included in engine_render_combined",
        },
        fixture_projection: CoverageItem {
            status: "included_not_separately_instrumented",
            note: "included in engine_render_combined",
        },
        protocol_encoding: CoverageItem {
            status: "measured",
            note: "production light_output::encode_routes implementation",
        },
        socket_delivery,
        cpu_usage: CoverageItem {
            status: "not_measured",
            note: "no portable process-CPU sampler is installed; latency and deadline results must not be read as CPU utilization",
        },
        allocation_rate: CoverageItem {
            status: "not_measured",
            note: "the workspace has no benchmark-safe allocation counter",
        },
        sound_to_light_analysis: CoverageItem {
            status: "omitted_accounted",
            note: "sound capture and analysis are asynchronous adapters outside Engine::render; no deterministic synthetic analyzer seam is available yet",
        },
        timing_path_exclusions: vec![
            "persistence",
            "fixture library reads",
            "frontend projections",
            "JSON serialization of this report",
            "external-device adapters",
        ],
    }
}
