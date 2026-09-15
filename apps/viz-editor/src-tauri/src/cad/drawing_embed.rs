//! Build-time half of the CAD model drawings: collects the 2D drawings of every shipped default
//! model from `assets/models/2d` and writes a table the crate `include!`s.
//!
//! Compiled only by `build.rs`. A drawing is embedded when its model id (without `-no-clamp`) is
//! named in `crates/viz/project/src/default_model.rs`, so a newly shipped model and a regenerated
//! drawing are both picked up by the next build with no list to edit here. Each file is compacted
//! (indentation, title and origin cross dropped) before it is embedded.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub fn generate() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let drawings = manifest.join("../../../assets/models/2d");
    let catalogue = manifest.join("../../../crates/viz/project/src/default_model.rs");
    println!("cargo:rerun-if-changed={}", drawings.display());
    watch_drawings(&drawings);
    println!("cargo:rerun-if-changed={}", catalogue.display());
    let names = std::fs::read_to_string(&catalogue).unwrap_or_default();
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let mut table = String::from("pub static MODEL_DRAWINGS: &[(&str, &str, &str)] = &[\n");
    for (id, view, source) in drawing_files(&drawings) {
        let base = id.strip_suffix("-no-clamp").unwrap_or(&id);
        if !names.contains(&format!("\"{base}\"")) {
            continue;
        }
        let Ok(svg) = std::fs::read_to_string(&source) else {
            continue;
        };
        let target = out.join("model_drawings").join(&id);
        std::fs::create_dir_all(&target).expect("drawing output directory");
        let target = target.join(format!("{view}.svg"));
        std::fs::write(&target, compact(&svg)).expect("compacted drawing");
        let _ = writeln!(
            table,
            "    ({id:?}, {view:?}, include_str!({:?})),",
            target.display().to_string()
        );
    }
    table.push_str("];\n");
    std::fs::write(out.join("model_drawings.rs"), table).expect("drawing table");
}

/// Every `<group>/<model>/<view>.svg`, sorted so the table is stable.
fn drawing_files(root: &Path) -> Vec<(String, String, PathBuf)> {
    let mut files = Vec::new();
    let Ok(groups) = std::fs::read_dir(root) else {
        return files;
    };
    for group in groups.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(models) = std::fs::read_dir(group.path()) else {
            continue;
        };
        for model in models.flatten().filter(|entry| entry.path().is_dir()) {
            for view in ["top", "front", "side"] {
                let path = model.path().join(format!("{view}.svg"));
                if path.is_file() {
                    let id = model.file_name().to_string_lossy().into_owned();
                    files.push((id, view.to_owned(), path));
                }
            }
        }
    }
    files.sort();
    files
}

fn compact(svg: &str) -> String {
    let mut text = svg
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("");
    text = remove_element(&text, "<title>", "</title>");
    if let Some(start) = text.find("<g id=\"origin\"") {
        let end = group_end(&text, start);
        text.replace_range(start..end, "");
    }
    text
}

fn remove_element(text: &str, open: &str, close: &str) -> String {
    match (text.find(open), text.find(close)) {
        (Some(start), Some(end)) if end > start => {
            format!("{}{}", &text[..start], &text[end + close.len()..])
        }
        _ => text.to_owned(),
    }
}

/// The byte just past the `</g>` closing the group that opens at `start`.
fn group_end(text: &str, start: usize) -> usize {
    let open_end = text[start..]
        .find('>')
        .map_or(text.len(), |at| start + at + 1);
    if text[..open_end].ends_with("/>") {
        return open_end;
    }
    let mut depth = 1;
    let mut cursor = open_end;
    while depth > 0 {
        let rest = &text[cursor..];
        let next_open = rest.find("<g");
        let Some(next_close) = rest.find("</g>") else {
            return text.len();
        };
        match next_open {
            Some(open) if open < next_close => {
                let tag_end = rest[open..].find('>').map_or(rest.len(), |at| open + at);
                if !rest[..tag_end].ends_with('/') {
                    depth += 1;
                }
                cursor += tag_end + 1;
            }
            _ => {
                depth -= 1;
                cursor += next_close + 4;
            }
        }
    }
    cursor
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
