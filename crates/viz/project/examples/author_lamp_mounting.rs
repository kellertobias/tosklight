//! Author the mounting clip and volume on every shipped lamp package.
//!
//! A lantern is hung by its hardware — the hook clamp on a PAR's yoke, the omega bracket under a
//! moving head — and until this ran, no package said so: the plan guessed a clip from the top
//! quarter of whatever box the fixture happened to have. This reads the real hardware off the
//! shipped model drawings and writes it into each package as a declared clip.
//!
//! Where the hardware comes from. Every shipped 2D drawing of a lamp that carries mounting
//! hardware also ships a `-no-clamp` twin drawn from the same model with that hardware removed
//! (see `assets/models/2d/`). The difference between the two drawings' view boxes *is* the
//! hardware: the band of the model that only the clamped drawing reaches into. That band, as a
//! share of the model's own height, is the clip; at authoring time it is multiplied by the
//! fixture's declared physical size, so each lantern gets its own clamp at its own scale rather
//! than a fixed share of a bounding box. Across and deep the hardware reaches the whole drawing —
//! a yoke is as wide as the lantern it carries — which is what the full view box measures.
//!
//! Fixtures the drawings cannot answer for are not skipped silently. A profile whose body ships no
//! `-no-clamp` twin takes the clamp its family carries (see `FAMILY_CLIPS`), and a profile that
//! hangs from nothing at all — a rack dimmer, a machine that stands on the floor — is written as
//! `hardware: "none"` so the plan knows not to rig it. Every one of those decisions is recorded in
//! the audit.
//!
//! Run it from the repository root:
//!
//! ```sh
//! cargo run -p viz-project --example author_lamp_mounting -- --audit   # report only
//! cargo run -p viz-project --example author_lamp_mounting              # report and write
//! ```
//!
//! The audit is written to `docs/engineering/fixture-mounting-audit.md` either way.
use light_fixture::{
    FixtureProfile, MountingHardware, ProfileMounting, Vector3, read_fixture_package,
    write_fixture_package,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use viz_project::profile_default_model;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The band of a body its mounting hardware occupies, as a share of the body's own height.
///
/// Measured up from the middle of the body, so `0.5` is the top of the lantern and anything above
/// that is hardware standing proud of it.
#[derive(Clone, Copy, Debug)]
struct ClipShare {
    bottom: f32,
    top: f32,
}

/// The clamp a family of bodies carries, for the models whose drawings ship no `-no-clamp` twin.
///
/// These are read off the drawings by eye rather than measured by subtraction, so they are
/// deliberately few, and each one says which body it stands for.
const FAMILY_CLIPS: &[(&str, ClipShare)] = &[
    // A moving head hangs from an omega bracket bolted under its base.
    (
        "moving-head",
        ClipShare {
            bottom: 0.38,
            top: 0.5,
        },
    ),
    // A bar or strip hangs from a bracket at each end, standing a little proud of the run.
    (
        "strip",
        ClipShare {
            bottom: 0.5,
            top: 0.62,
        },
    ),
    (
        "sunstrip",
        ClipShare {
            bottom: 0.5,
            top: 0.62,
        },
    ),
];

/// Bodies that carry no mounting hardware at all, by the name the catalogue gives them.
const STANDS_ON_ITS_OWN: &[&str] = &[
    "dimmer-rack",
    "hazer",
    "smoke-machine",
    "follow-spot",
    "mirror-ball-motor",
    "mirror-ball-0200",
    "mirror-ball-0300",
    "mirror-ball-0400",
    "mirror-ball-0500",
];

/// A drawing's view box as `(min_x, min_y, width, height)` in millimetres.
fn view_box(path: &Path) -> Option<(f32, f32, f32, f32)> {
    let svg = fs::read_to_string(path).ok()?;
    let start = svg.find("viewBox=\"")? + "viewBox=\"".len();
    let end = start + svg[start..].find('"')?;
    let numbers: Vec<f32> = svg[start..end]
        .split_whitespace()
        .filter_map(|value| value.parse().ok())
        .collect();
    match numbers[..] {
        [min_x, min_y, width, height] => Some((min_x, min_y, width, height)),
        _ => None,
    }
}

/// Where a model's drawings live, whichever group they were filed under.
fn drawing_dir(model: &str) -> Option<PathBuf> {
    let base = root().join("assets/models/2d");
    for group in fs::read_dir(&base).ok()?.flatten() {
        let dir = group.path().join(model);
        if dir.join("front.svg").exists() {
            return Some(dir);
        }
    }
    None
}

/// The band measured from the difference between a model's drawing and its `-no-clamp` twin.
///
/// The twin is the same model without its mounting hardware, so the strip of the drawing only the
/// clamped one reaches is the hardware. `None` when the model ships no twin.
fn measured_clip(model: &str) -> Option<ClipShare> {
    let full = drawing_dir(model)?;
    let bare = drawing_dir(&format!("{model}-no-clamp"))?;
    let (_, full_top, _, full_height) = view_box(&full.join("front.svg"))?;
    let (_, bare_top, _, _) = view_box(&bare.join("front.svg"))?;
    // The drawings measure downwards from the model's own origin; a clip measures upwards from the
    // middle of the body, which is what the plan places a fixture by.
    let up = |svg_y: f32| (full_top + full_height / 2.0 - svg_y) / full_height;
    let (bottom, top) = (up(bare_top), up(full_top));
    (top > bottom).then_some(ClipShare { bottom, top })
}

/// The band a profile's body carries, and where that answer came from.
fn clip_for(model: &str) -> (Option<ClipShare>, &'static str) {
    if STANDS_ON_ITS_OWN.contains(&model) {
        return (None, "no hardware");
    }
    if let Some(share) = measured_clip(model) {
        return (Some(share), "measured");
    }
    for (family, share) in FAMILY_CLIPS {
        if model.contains(family) {
            return (Some(*share), "family");
        }
    }
    (None, "no hardware")
}

/// The clip in the fixture's own millimetres, from the band of the body it occupies.
fn mounting(share: Option<ClipShare>, (width, depth, height): (f32, f32, f32)) -> ProfileMounting {
    let Some(share) = share else {
        return ProfileMounting {
            hardware: MountingHardware::None,
            ..Default::default()
        };
    };
    let (bottom, top) = (share.bottom * height, share.top * height);
    ProfileMounting {
        hardware: MountingHardware::Clamp,
        centre_millimetres: Vector3 {
            x: 0.0,
            y: 0.0,
            z: (bottom + top) / 2.0,
        },
        // A yoke is as wide and as deep as the lantern it carries, which is what the drawing of
        // the whole body measures; only the band above it belongs to the clamp alone.
        half_extent_millimetres: Vector3 {
            x: width / 2.0,
            y: depth / 2.0,
            z: (top - bottom) / 2.0,
        },
        // The pipe sits at the top of the clamp: the line the clamp closes around, and the point
        // the plan drops onto a chord when a lamp is rigged.
        pipe_millimetres: Vector3 {
            x: 0.0,
            y: 0.0,
            z: top,
        },
        body_millimetres: Vector3 {
            x: width,
            y: depth,
            z: height,
        },
    }
}

/// Whether the profile measures its own body, as opposed to falling back to its family's.
fn declares_its_size(profile: &FixtureProfile) -> bool {
    profile.physical.width_millimetres.is_some()
        && profile.physical.depth_millimetres.is_some()
        && profile.physical.height_millimetres.is_some()
}

struct Row {
    file: String,
    manufacturer: String,
    name: String,
    model: String,
    source: &'static str,
    hardware: &'static str,
    note: &'static str,
}

fn main() {
    let write = !std::env::args().any(|argument| argument == "--audit");
    let library = root().join("assets/fixture-library");
    let mut packages: Vec<PathBuf> = fs::read_dir(&library)
        .expect("the fixture library ships")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|value| value == "toskfixture")
                && !path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.starts_with("venue--"))
        })
        .collect();
    packages.sort();

    let mut rows = Vec::new();
    let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
    let mut written = 0usize;
    for path in &packages {
        let bytes = fs::read(path).expect("a shipped package reads");
        let mut profile = read_fixture_package(&bytes).expect("a shipped package parses");
        let file = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned();
        let chosen = profile_default_model(&profile, None);
        let model = chosen.map_or_else(
            || "(its own model)".to_owned(),
            |chosen| chosen.model.name.to_owned(),
        );
        let (share, found) =
            chosen.map_or((None, "own model"), |chosen| clip_for(chosen.model.name));
        // The plan draws a fixture that declares no size at the size its family of bodies falls
        // back to, so that is the body the clip is measured against — not a box of zeroes.
        let body = chosen.map_or((0.0, 0.0, 0.0), |chosen| {
            let metres = chosen.body_size_metres;
            (metres.x * 1000.0, metres.z * 1000.0, metres.y * 1000.0)
        });
        let note = match (share, declares_its_size(&profile)) {
            (Some(_), false) => "measured against the body its family falls back to",
            (Some(_), true) => "",
            (None, _) => "hangs from nothing, so the plan does not rig it",
        };
        let source = found;
        let block = mounting(share, body);
        let hardware = match block.hardware {
            MountingHardware::Clamp => "clamp",
            MountingHardware::Yoke => "yoke",
            MountingHardware::None => "none",
        };
        *by_source.entry(source).or_default() += 1;
        rows.push(Row {
            file,
            manufacturer: profile.manufacturer.clone(),
            name: profile.name.clone(),
            model,
            source,
            hardware,
            note,
        });
        if write && profile.mounting != Some(block) {
            profile.mounting = Some(block);
            // A new revision, so an installation that already holds the package is offered the
            // clip rather than being changed under the operator's feet.
            profile.revision += 1;
            let package = write_fixture_package(&profile).expect("the profile packages");
            fs::write(path, package).expect("the package writes back");
            written += 1;
        }
    }

    let mut report = String::new();
    report.push_str("# Shipped lamp mounting audit\n\n");
    report.push_str(
        "<!-- Generated by `cargo run -p viz-project --example author_lamp_mounting`. -->\n\n",
    );
    report.push_str(
        "Every shipped lamp package, the body the plan draws it as, and where its mounting clip \
came from.\n\n\
* **measured** — the difference between the body's drawing and its `-no-clamp` twin, which is the \
mounting hardware itself.\n\
* **family** — the clamp the body's family carries, for a body that ships no twin.\n\
* **no hardware** — a fixture that hangs from nothing, written out as `none` rather than left \
undeclared.\n\
* **no size** — a profile that declares no physical dimensions, so there is no box for a clamp to \
sit on.\n\n",
    );
    report.push_str(&format!("{} packages.", rows.len()));
    for (source, count) in &by_source {
        report.push_str(&format!(" {source}: {count}."));
    }
    report.push_str("\n\n| Package | Manufacturer | Name | Body | Clip from | Hardware | Note |\n");
    report.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for row in &rows {
        report.push_str(&format!(
            "| `{}` | {} | {} | `{}` | {} | {} | {} |\n",
            row.file, row.manufacturer, row.name, row.model, row.source, row.hardware, row.note
        ));
    }
    let audit = root().join("docs/engineering/fixture-mounting-audit.md");
    fs::write(&audit, report).expect("the audit writes");
    println!(
        "{} packages, {written} rewritten; audit at {}",
        rows.len(),
        audit.display()
    );
    for (source, count) in &by_source {
        println!("  {source}: {count}");
    }
}
