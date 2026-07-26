#![forbid(unsafe_code)]

mod light_benchmark;

use light_benchmark::{Arguments, ParseOutcome};

fn main() {
    let arguments = match Arguments::parse(std::env::args().skip(1)) {
        Ok(ParseOutcome::Run(arguments)) => arguments,
        Ok(ParseOutcome::Help) => {
            print!("{}", Arguments::help());
            return;
        }
        Err(error) => exit_with_error(&error, 2),
    };
    if cfg!(debug_assertions) {
        exit_with_error(
            "light-benchmark measures release builds; rerun cargo with --release",
            2,
        );
    }

    let report = light_benchmark::run(&arguments).unwrap_or_else(|error| {
        exit_with_error(&error, 1);
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("benchmark report is serializable")
    );
    if report.required_floor_met == Some(false)
        || report
            .show_mutation
            .as_ref()
            .is_some_and(|result| !result.gate_met)
        || report
            .patch_mutation
            .as_ref()
            .is_some_and(|result| !result.gate_met)
    {
        std::process::exit(1);
    }
}

fn exit_with_error(message: &str, code: i32) -> ! {
    eprintln!("light-benchmark: {message}");
    std::process::exit(code);
}
