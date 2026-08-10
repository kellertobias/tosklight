use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage: fixture-projection generate <input.toskfixture> --output <output.toskfixture>"
    );
    std::process::exit(2);
}

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 4 || arguments[0] != "generate" || arguments[2] != "--output" {
        usage();
    }
    let input = PathBuf::from(&arguments[1]);
    let output = PathBuf::from(&arguments[3]);
    if input == output {
        eprintln!("refusing to overwrite the source package; choose a separate output path");
        std::process::exit(2);
    }
    let result = (|| -> Result<(), String> {
        let bytes =
            std::fs::read(&input).map_err(|error| format!("read {}: {error}", input.display()))?;
        let mut profile = light_fixture::read_fixture_package(&bytes)
            .map_err(|error| format!("read package: {error}"))?;
        profile.projection_assets = Some(
            viz_project::generate_profile_projections(&profile)
                .map_err(|error| format!("generate projections: {error}"))?,
        );
        let generated = light_fixture::write_fixture_package(&profile)
            .map_err(|error| format!("write package: {error}"))?;
        std::fs::write(&output, generated)
            .map_err(|error| format!("write {}: {error}", output.display()))?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
