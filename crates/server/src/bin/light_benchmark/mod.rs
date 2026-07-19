mod arguments;
mod loopback;
mod metadata;
mod report;
mod runner;
mod scenario;
mod statistics;

pub use arguments::{Arguments, ParseOutcome};
pub use report::BenchmarkReport;

pub fn run(arguments: &Arguments) -> Result<BenchmarkReport, String> {
    runner::run(arguments)
}
