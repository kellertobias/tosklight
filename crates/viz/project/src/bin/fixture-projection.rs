use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "usage:\n  fixture-projection generate <input.toskfixture> --output <output.toskfixture>\n  fixture-projection generate-defaults --output <directory>"
    );
    std::process::exit(2);
}

fn main() {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 3 && arguments[0] == "generate-defaults" && arguments[1] == "--output" {
        if let Err(error) = generate_defaults(PathBuf::from(&arguments[2])) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
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

fn generate_defaults(output: PathBuf) -> Result<(), String> {
    for shipped in viz_project::all_default_models() {
        let model = viz_scene::read_glb(shipped.bytes)
            .map_err(|error| format!("read default model {}: {error}", shipped.name))?;
        let model_directory = output.join(shipped.name);
        std::fs::create_dir_all(&model_directory)
            .map_err(|error| format!("create {}: {error}", model_directory.display()))?;
        for view in light_fixture::ProfileProjectionView::ALL {
            let projection = viz_project::generate_default_model_projection(&model, view)
                .map_err(|error| format!("generate {} {}: {error}", shipped.name, view.wire()))?;
            let encoded = projection
                .artwork_asset
                .strip_prefix("data:image/svg+xml;base64,")
                .ok_or_else(|| {
                    format!("{} {} did not produce an SVG", shipped.name, view.wire())
                })?;
            let svg = STANDARD
                .decode(encoded)
                .map_err(|error| format!("decode {} {}: {error}", shipped.name, view.wire()))?;
            let path = model_directory.join(format!("{}.svg", view.wire()));
            std::fs::write(&path, svg)
                .map_err(|error| format!("write {}: {error}", path.display()))?;
        }
    }
    Ok(())
}
