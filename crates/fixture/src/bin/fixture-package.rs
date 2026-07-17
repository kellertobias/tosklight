use light_fixture::read_fixture_package;
use std::{env, fs, process::ExitCode};

fn usage() {
    eprintln!("usage: fixture-package validate <package.toskfixture>...");
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    if arguments.next().as_deref() != Some("validate") {
        usage();
        return ExitCode::from(2);
    }
    let paths = arguments.collect::<Vec<_>>();
    if paths.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    let mut failed = false;
    for path in paths {
        match fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| read_fixture_package(&bytes).map_err(|error| error.to_string()))
        {
            Ok(profile) => println!(
                "OK\t{path}\t{}\t{}\t{} mode{}",
                profile.manufacturer,
                profile.name,
                profile.modes.len(),
                if profile.modes.len() == 1 { "" } else { "s" }
            ),
            Err(error) => {
                eprintln!("ERROR\t{path}\t{error}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
