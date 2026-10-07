mod arguments;
mod digest;
mod digest_lifecycle;
mod headless_stress_show;
mod loopback;
mod metadata;
mod mutation;
mod patch_mutation;
mod process_resources;
mod report;
mod runner;
mod sampled;
mod scenario;
mod semantic_arguments;
mod semantic_programming;
mod semantic_runner;
mod semantic_workload;
mod start_latency;
mod statistics;
mod sustained_show;

pub use arguments::{Arguments, ParseOutcome};
pub use report::BenchmarkReport;

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    runner::run(arguments)
}

/// TL-639: `--digest-ticks N` prints per-tick output digests instead of a timed report.
pub fn digest(arguments: &Arguments, ticks: u64) -> Result<serde_json::Value, String> {
    digest::run(arguments, ticks)
}

/// TL-641: `--start-latency` times one Start Now Dynamic gesture instead of a timed report.
pub fn start_latency(arguments: &Arguments) -> Result<serde_json::Value, String> {
    start_latency::run(arguments)
}
