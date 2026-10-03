mod arguments;
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
mod statistics;
mod sustained_show;

pub use arguments::{Arguments, ParseOutcome};
pub use report::BenchmarkReport;

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    runner::run(arguments)
}
