//! Draw every shipped 3D model as editable SVG line drawings.
//!
//! Reads `assets/models/manifest.json` and writes `<output>/<group>/<model>/<view>.svg` for the
//! top, front and side views. A drawing someone has edited since it was generated is kept: the
//! generator records what it wrote in `<output>/generated.json` and only replaces a file whose
//! content is still exactly that. Pass `--force` to regenerate edited drawings too, and `--only
//! <model>` (repeatable) to draw some models rather than all.
//!
//! Some models are drawn in the pose a plan reads best rather than as they hang (see
//! `viz_project::drawing_pose`):
//! - PARs and Fresnels are turned to point forward in the top view, so they show their length
//!   rather than a round face everyone else also has.
//! - Blinders, flat LED PARs, strobes and the flood face the audience in the top and front views,
//!   and lean 20° from vertical from the side; that side drawing records the bracket angle its pose
//!   equals (`data-bracket`), so the CAD can turn it to any fixture's own bracket angle.
//! - LED wash moving heads look at the viewer in the front view.
//! - People are drawn as the audience figure the crowd uses, so every person reads alike.

use glam::{Vec2, Vec3};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use viz_project::{
    DRAWING_VIEWS, ModelDrawing, drawing_pose, drawing_svg, model_drawing_svg_posed,
};
use viz_scene::ModelPartKind;

fn main() -> ExitCode {
    let mut manifest = PathBuf::from("assets/models/manifest.json");
    let mut output = PathBuf::from("assets/models/2d");
    let mut force = false;
    let mut only = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest = args.next().map(PathBuf::from).unwrap_or(manifest),
            "--output" => output = args.next().map(PathBuf::from).unwrap_or(output),
            "--force" => force = true,
            "--only" => only.extend(args.next()),
            other => {
                eprintln!(
                    "unknown argument {other}\nusage: model-drawings [--manifest PATH] [--output DIR] [--force] [--only MODEL]..."
                );
                return ExitCode::from(2);
            }
        }
    }
    match run(&manifest, &output, force, &only) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// What the generator did, for its closing report.
#[derive(Default)]
struct Tally {
    written: usize,
    unchanged: usize,
    kept: usize,
    failed: usize,
}

fn run(manifest: &Path, output: &Path, force: bool, only: &[String]) -> Result<usize, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
    let manifest_json: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let models = manifest_json["models"]
        .as_array()
        .ok_or_else(|| format!("{} has no models list", manifest.display()))?;
    let root = manifest.parent().unwrap_or(Path::new("."));
    let hinges: HashMap<&str, Vec3> = models
        .iter()
        .filter_map(|entry| {
            Some((
                entry["model"].as_str()?,
                viz_project::manifest_bracket_hinge(entry)?,
            ))
        })
        .collect();
    let audience = std::fs::read_to_string(root.join("../viz/crowd/audience-outline.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let record_path = output.join("generated.json");
    let mut record: BTreeMap<String, String> = std::fs::read_to_string(&record_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut tally = Tally::default();
    for entry in models {
        let (Some(id), Some(group), Some(file)) = (
            entry["model"].as_str(),
            entry["group"].as_str(),
            entry["file"].as_str(),
        ) else {
            continue;
        };
        if !only.is_empty() && !only.iter().any(|wanted| wanted == id) {
            continue;
        }
        if group == "people" {
            let Some(audience) = &audience else {
                eprintln!("{id}: assets/viz/crowd/audience-outline.json is missing");
                tally.failed += 1;
                continue;
            };
            for (name, _) in DRAWING_VIEWS {
                let relative = format!("{group}/{id}/{name}.svg");
                match person(audience, entry, name) {
                    Some(drawing) => {
                        let svg = drawing_svg(&drawing, id, name);
                        write(output, &relative, &svg, force, &mut record, &mut tally)?;
                    }
                    None => {
                        eprintln!("{relative}: the audience outline has no {name} view");
                        tally.failed += 1;
                    }
                }
            }
            continue;
        }
        let model = match std::fs::read(root.join(file))
            .map_err(|error| error.to_string())
            .and_then(|bytes| viz_scene::read_glb(&bytes).map_err(|error| error.0))
        {
            Ok(model) => model,
            Err(error) => {
                eprintln!("{id}: {error}");
                tally.failed += 1;
                continue;
            }
        };
        // A model without its clamp still has the body it hangs, turned about the same hinge.
        let base = id.trim_end_matches("-no-clamp");
        let hinge = hinges.get(id).or_else(|| hinges.get(base)).copied();
        for (name, view) in DRAWING_VIEWS {
            let relative = format!("{group}/{id}/{name}.svg");
            let pose = drawing_pose(base, name, hinge);
            match model_drawing_svg_posed(&model, id, name, view, &pose) {
                Ok(svg) => write(output, &relative, &svg, force, &mut record, &mut tally)?,
                Err(error) => {
                    eprintln!("{relative}: {error}");
                    tally.failed += 1;
                }
            }
        }
    }
    let mut json = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
    json.push('\n');
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    std::fs::write(&record_path, json)
        .map_err(|error| format!("cannot write {}: {error}", record_path.display()))?;
    println!(
        "model drawings: {} written, {} unchanged, {} edited and kept, {} failed",
        tally.written, tally.unchanged, tally.kept, tally.failed
    );
    Ok(tally.failed)
}

/// A person drawn as the audience figure, scaled to the model's own height or width.
fn person(
    audience: &serde_json::Value,
    entry: &serde_json::Value,
    view: &str,
) -> Option<ModelDrawing> {
    let ring = points(&audience[view])?;
    let strokes: Vec<Vec<Vec2>> = audience[format!("{view}_strokes")]
        .as_array()?
        .iter()
        .filter_map(points)
        .collect();
    let height = entry["height_millimetres"].as_f64()? as f32;
    let width = entry["width_millimetres"].as_f64()? as f32;
    let place: Box<dyn Fn(Vec2) -> Vec2> = if view == "top" {
        // The plan outline is normalised to its own size, so it is scaled to the figure's width
        // and centred on the spot the figure stands on.
        let xs = ring.iter().map(|point| point.x);
        let span = xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
        let ys = ring.iter().map(|point| point.y);
        let middle = (ys.clone().fold(f32::MIN, f32::max) + ys.fold(f32::MAX, f32::min)) / 2.0;
        let scale = width / span.max(1e-3);
        Box::new(move |point| Vec2::new(point.x, point.y - middle) * scale)
    } else {
        // Elevations are normalised to the figure's height with the feet at zero; the page runs
        // down, so up is negative.
        Box::new(move |point| Vec2::new(point.x, -point.y) * height)
    };
    let ring: Vec<Vec2> = ring.into_iter().map(&place).collect();
    let min = ring
        .iter()
        .fold(Vec2::splat(f32::MAX), |low, point| low.min(*point));
    let max = ring
        .iter()
        .fold(Vec2::splat(f32::MIN), |high, point| high.max(*point));
    let lines = strokes
        .into_iter()
        .map(|stroke| {
            let mut line: Vec<Vec2> = stroke.into_iter().map(&place).collect();
            if let Some(first) = line.first().copied() {
                line.push(first);
            }
            (ModelPartKind::Base, line)
        })
        .collect();
    Some(ModelDrawing {
        pose: "authored-home",
        min,
        max,
        silhouette: vec![ring],
        lines,
        ..ModelDrawing::default()
    })
}

fn points(value: &serde_json::Value) -> Option<Vec<Vec2>> {
    value
        .as_array()?
        .iter()
        .map(|point| {
            Some(Vec2::new(
                point.get(0)?.as_f64()? as f32,
                point.get(1)?.as_f64()? as f32,
            ))
        })
        .collect()
}

/// Write one drawing, unless someone has edited the file since the generator last wrote it.
fn write(
    output: &Path,
    relative: &str,
    svg: &str,
    force: bool,
    record: &mut BTreeMap<String, String>,
    tally: &mut Tally,
) -> Result<(), String> {
    let path = output.join(relative);
    let digest = sha256(svg.as_bytes());
    if let Ok(existing) = std::fs::read(&path) {
        let current = sha256(&existing);
        if current == digest {
            record.insert(relative.to_owned(), digest);
            tally.unchanged += 1;
            return Ok(());
        }
        // Someone has drawn over it since it was generated: leave their work alone.
        if !force && record.get(relative) != Some(&current) {
            eprintln!("{relative}: edited since it was generated, kept");
            tally.kept += 1;
            return Ok(());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(&path, svg)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    record.insert(relative.to_owned(), digest);
    tally.written += 1;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
