//! Embeds the 2D drawings of the shipped default models for the Visualizer's plan views.
//!
//! A drawing is embedded when its model id is quoted in `src/default_model.rs`, so a newly shipped
//! model and a regenerated or hand-edited drawing are both picked up by the next build with no
//! list to edit. Only the drawings with mounting hardware are taken — the Visualizer always draws
//! the clamp — and each is stored as the compact binary `src/plan_drawing/svg.rs` defines rather
//! than as SVG text.

#[path = "src/plan_drawing/svg.rs"]
mod svg;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const VIEWS: [&str; 3] = ["top", "front", "side"];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let drawings = manifest.join("../../../assets/models/2d");
    let catalogue = manifest.join("src/default_model.rs");
    println!("cargo:rerun-if-changed={}", drawings.display());
    watch_drawings(&drawings);
    println!("cargo:rerun-if-changed={}", catalogue.display());
    println!("cargo:rerun-if-changed=src/plan_drawing/svg.rs");
    let names = std::fs::read_to_string(&catalogue).unwrap_or_default();
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let blobs = out.join("plan_drawings");
    std::fs::create_dir_all(&blobs).expect("drawing output directory");
    let mut table = String::from("static PLAN_DRAWINGS: &[(&str, &str, &[u8])] = &[\n");
    for (id, directory) in model_directories(&drawings) {
        if id.ends_with("-no-clamp") || !names.contains(&format!("\"{id}\"")) {
            continue;
        }
        for view in VIEWS {
            let Ok(source) = std::fs::read_to_string(directory.join(format!("{view}.svg"))) else {
                continue;
            };
            let drawing = svg::parse(&source);
            if drawing.silhouette.is_empty() && drawing.lines.is_empty() {
                continue;
            }
            let target = blobs.join(format!("{id}.{view}.bin"));
            std::fs::write(&target, svg::encode(&drawing)).expect("encoded drawing");
            let _ = writeln!(
                table,
                "    ({id:?}, {view:?}, include_bytes!({:?})),",
                target.display().to_string()
            );
        }
    }
    table.push_str("];\n");
    std::fs::write(out.join("plan_drawings.rs"), table).expect("drawing table");
}

/// Every `<group>/<model>` directory, sorted so the table is stable.
fn model_directories(root: &Path) -> Vec<(String, PathBuf)> {
    let mut models = Vec::new();
    let Ok(groups) = std::fs::read_dir(root) else {
        return models;
    };
    for group in groups.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(entries) = std::fs::read_dir(group.path()) else {
            continue;
        };
        for model in entries.flatten().filter(|entry| entry.path().is_dir()) {
            models.push((
                model.file_name().to_string_lossy().into_owned(),
                model.path(),
            ));
        }
    }
    models.sort();
    models
}

/// Cargo only notices a directory's own entries changing, so every drawing is watched by name:
/// a regenerated or hand-edited SVG then rebuilds what embeds it.
fn watch_drawings(dir: &std::path::Path) {
    let Ok(groups) = std::fs::read_dir(dir) else {
        return;
    };
    for group in groups.flatten() {
        let Ok(models) = std::fs::read_dir(group.path()) else {
            continue;
        };
        for model in models.flatten() {
            let Ok(views) = std::fs::read_dir(model.path()) else {
                continue;
            };
            for view in views.flatten() {
                if view
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "svg")
                {
                    println!("cargo:rerun-if-changed={}", view.path().display());
                }
            }
        }
    }
}
