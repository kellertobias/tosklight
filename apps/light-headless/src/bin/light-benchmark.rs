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

    // The output worker count (TL-639 round 5): a count or `max`; unset uses the engine default.
    if let Ok(value) = std::env::var("LIGHT_OUTPUT_WORKERS") {
        match light_engine::parallel::parse_output_workers(&value) {
            Some(workers) => light_engine::parallel::set_default_output_workers(workers),
            None => exit_with_error("LIGHT_OUTPUT_WORKERS must be a positive count or max", 2),
        }
    }
    // TL-639 round 5: `LIGHT_VERIFY_PLANS=1` builds every planned Position forest through the
    // general walk as well and asserts they are equal.
    if std::env::var("LIGHT_VERIFY_PLANS").is_ok_and(|value| value == "1") {
        light_dynamics::set_plan_verification(true);
    }
    if let Some(ticks) = arguments.semantic.digest_ticks {
        let digest = light_benchmark::digest(&arguments, ticks).unwrap_or_else(|error| {
            exit_with_error(&error, 1);
        });
        println!("{digest}");
        return;
    }
    if arguments.semantic.start_latency {
        let report = light_benchmark::start_latency(&arguments).unwrap_or_else(|error| {
            exit_with_error(&error, 1);
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("start-latency report is serializable")
        );
        return;
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
