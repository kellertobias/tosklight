//! Generates the canonical demo show from the shipped fixture packages.
//!
//! CI runs this and packages what it writes, so the demo that ships is always the one this
//! repository's rig and fixture packages currently describe.

use light_fixture::FixtureLibrary;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "viz-demo-show --packages DIR --output FILE [--library FILE]

  --packages DIR   Directory of .toskfixture packages to patch the rig from.
  --output FILE    Where to write the generated show. An existing file is replaced.
  --library FILE   Where to build the intermediate fixture library. Defaults to a file beside the
                   output, which is a generated artefact like the show itself.";

struct Options {
    packages: PathBuf,
    output: PathBuf,
    library: Option<PathBuf>,
}

fn main() -> ExitCode {
    let options = match parse(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: &Options) -> Result<(), String> {
    let library_path = options
        .library
        .clone()
        .unwrap_or_else(|| options.output.with_extension("fixtures.sqlite"));
    // The intermediate library is rebuilt every time: a stale one would silently patch the demo
    // from packages this checkout no longer ships.
    if library_path.exists() {
        std::fs::remove_file(&library_path).map_err(|error| error.to_string())?;
    }
    if let Some(parent) = library_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let library = FixtureLibrary::open(&library_path).map_err(|error| error.to_string())?;
    let report = library
        .load_fixture_package_directory(&options.packages)
        .map_err(|error| {
            format!(
                "loading packages from {}: {error}",
                options.packages.display()
            )
        })?;
    println!(
        "Loaded {} fixture packages from {}",
        report.installed + report.updated + report.unchanged,
        options.packages.display()
    );

    let generated =
        viz_demo::generate(library, &options.output).map_err(|error| error.to_string())?;
    println!(
        "Wrote {} with {} fixtures to {}",
        generated.name,
        generated.fixtures,
        generated.path.display()
    );
    for (profile, revision) in &generated.profile_revisions {
        println!("  {profile} revision {revision}");
    }
    // The library was scaffolding for the generation; the show carries its own embedded profile
    // revisions and does not need it afterwards.
    let _ = std::fs::remove_file(&library_path);
    Ok(())
}

fn parse(arguments: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut packages = None;
    let mut output = None;
    let mut library = None;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--packages" => packages = Some(required(&mut arguments, "--packages")?),
            "--output" => output = Some(required(&mut arguments, "--output")?),
            "--library" => library = Some(required(&mut arguments, "--library")?),
            other => return Err(format!("{other} is not an option this tool takes")),
        }
    }
    Ok(Some(Options {
        packages: packages.ok_or("--packages is required")?,
        output: output.ok_or("--output is required")?,
        library,
    }))
}

fn required(arguments: &mut impl Iterator<Item = String>, option: &str) -> Result<PathBuf, String> {
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{option} requires a path"))
}
