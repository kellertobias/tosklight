mod arguments;
mod demo_show;
mod loopback;
mod metadata;
mod mutation;
mod patch_mutation;
mod report;
mod runner;
mod sampled;
mod scenario;
mod statistics;

pub use arguments::{Arguments, ParseOutcome};
pub use report::BenchmarkReport;

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    runner::run(arguments)
}
