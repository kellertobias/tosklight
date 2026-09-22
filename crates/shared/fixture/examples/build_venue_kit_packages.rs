//! Build the shipped visual-only Venue packages for the backline, PA, flight cases and figures.
//!
//! The Visualizer has shipped the geometry for these since the model set was built (see
//! `tools/stage_models/backline.py`, `cases.py` and `people.py`), but nothing pointed a fixture
//! profile at it, so an operator could not place a speaker, a rack or a singer. Each entry below
//! turns one shipped `.glb` into one visual-only Venue profile carrying that model and the
//! Model-Catalogue render as its photograph, at the size the model manifest measures.
//!
//! Run it from the repository root after changing an entry or rebuilding a model:
//!
//! ```sh
//! cargo run -p light-fixture --example build_venue_kit_packages
//! ```
//!
//! Identity is stable: every profile, mode and head ID is a UUID v5 under `NAMESPACE` derived from
//! the model's own name, so a rerun writes the same IDs and a patched show keeps its fixtures.
//! Sizes are read from `assets/models/manifest.json` rather than retyped, rounded to the whole
//! millimetres the profile editor works in.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_core::FixtureId;
use light_fixture::{FixtureProfile, read_fixture_package, write_fixture_package};
use serde_json::Value;
use std::{fs, path::PathBuf};
use uuid::Uuid;

/// The namespace the IDs of these packages are derived under, once and for good.
const NAMESPACE: Uuid = Uuid::from_u128(0x7b2c_41d6_9f18_5a44_bd07_3e6f_2c91_84ae);

/// One package: the shipped model it carries, and how the desk names and describes it.
struct Entry {
    group: &'static str,
    model: &'static str,
    file: &'static str,
    name: &'static str,
    short_name: &'static str,
    notes: &'static str,
}

const ENTRIES: &[Entry] = &[
    Entry {
        group: "backline",
        model: "dj-mixer",
        file: "venue--dj-mixer.toskfixture",
        name: "DJ Mixer",
        short_name: "DJ Mixer",
        notes: "A club DJ mixer on the booth, 320 × 400 mm and 110 mm high.",
    },
    Entry {
        group: "backline",
        model: "dj-player",
        file: "venue--dj-media-player.toskfixture",
        name: "DJ Media Player",
        short_name: "DJ Player",
        notes: "A club media player with a jog wheel, 320 × 420 mm and 110 mm high.",
    },
    Entry {
        group: "backline",
        model: "drum-kit",
        file: "venue--drum-kit.toskfixture",
        name: "Drum Kit",
        short_name: "Drum Kit",
        notes: "A five-piece kit: 22\" kick, snare, two toms, floor tom and three cymbals.",
    },
    Entry {
        group: "backline",
        model: "guitar-amp",
        file: "venue--guitar-stack.toskfixture",
        name: "Guitar Stack",
        short_name: "Guitar Stack",
        notes: "A 4 × 12 guitar cabinet with its head on top.",
    },
    Entry {
        group: "backline",
        model: "guitar-in-stand",
        file: "venue--electric-guitar-on-a-stand.toskfixture",
        name: "Electric Guitar on a Stand",
        short_name: "Guitar",
        notes: "An electric guitar on an A-frame stand.",
    },
    Entry {
        group: "backline",
        model: "line-array-hang",
        file: "venue--line-array-hang.toskfixture",
        name: "Line Array Hang",
        short_name: "Line Array",
        notes: "Six 900 × 420 mm line-array elements flown from a fly frame, with a truss coupler at the top.",
    },
    Entry {
        group: "backline",
        model: "microphone-stand",
        file: "venue--microphone-stand.toskfixture",
        name: "Microphone Stand",
        short_name: "Mic Stand",
        notes: "A boom microphone stand set to 1500 mm.",
    },
    Entry {
        group: "backline",
        model: "saxophone-in-stand",
        file: "venue--saxophone-on-a-stand.toskfixture",
        name: "Saxophone on a Stand",
        short_name: "Saxophone",
        notes: "A tenor saxophone on its stand.",
    },
    Entry {
        group: "backline",
        model: "speaker-on-pole",
        file: "venue--pa-top-on-a-pole-stand.toskfixture",
        name: "PA Top on a Pole Stand",
        short_name: "PA on Pole",
        notes: "A PA top on a tripod pole stand.",
    },
    Entry {
        group: "backline",
        model: "speaker-top",
        file: "venue--pa-top.toskfixture",
        name: "PA Top",
        short_name: "PA Top",
        notes: "A two-way 380 × 380 × 620 mm PA top with a pole socket.",
    },
    Entry {
        group: "backline",
        model: "stage-monitor",
        file: "venue--stage-monitor-wedge.toskfixture",
        name: "Stage Monitor Wedge",
        short_name: "Monitor",
        notes: "A 560 × 440 × 360 mm stage monitor wedge.",
    },
    Entry {
        group: "backline",
        model: "stage-piano",
        file: "venue--stage-piano.toskfixture",
        name: "Stage Piano",
        short_name: "Piano",
        notes: "A 1320 mm stage piano on an X stand.",
    },
    Entry {
        group: "backline",
        model: "subwoofer",
        file: "venue--subwoofer.toskfixture",
        name: "Subwoofer",
        short_name: "Subwoofer",
        notes: "A 600 × 700 × 600 mm subwoofer with an 18\" driver.",
    },
    Entry {
        group: "cases",
        model: "rack-02u",
        file: "venue--flight-case-rack-2u.toskfixture",
        name: "Flight Case Rack 2U",
        short_name: "Rack 2U",
        notes: "A 2U flight-case rack in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "cases",
        model: "rack-04u",
        file: "venue--flight-case-rack-4u.toskfixture",
        name: "Flight Case Rack 4U",
        short_name: "Rack 4U",
        notes: "A 4U flight-case rack in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "cases",
        model: "rack-06u",
        file: "venue--flight-case-rack-6u.toskfixture",
        name: "Flight Case Rack 6U",
        short_name: "Rack 6U",
        notes: "A 6U flight-case rack in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "cases",
        model: "rack-08u-wheels",
        file: "venue--flight-case-rack-8u-on-castors.toskfixture",
        name: "Flight Case Rack 8U on Castors",
        short_name: "Rack 8U",
        notes: "An 8U flight-case rack on castors, in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "cases",
        model: "rack-14u-wheels",
        file: "venue--flight-case-rack-14u-on-castors.toskfixture",
        name: "Flight Case Rack 14U on Castors",
        short_name: "Rack 14U",
        notes: "A 14U flight-case rack on castors, in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "cases",
        model: "rack-18u-wheels",
        file: "venue--flight-case-rack-18u-on-castors.toskfixture",
        name: "Flight Case Rack 18U on Castors",
        short_name: "Rack 18U",
        notes: "An 18U flight-case rack on castors, in a 600 × 640 mm shell, open front and back.",
    },
    Entry {
        group: "people",
        model: "figure-deejay",
        file: "venue--figure-deejay.toskfixture",
        name: "Figure, Deejay",
        short_name: "Deejay",
        notes: "A 1750 mm figure at the booth, cueing one ear, facing the audience.",
    },
    Entry {
        group: "people",
        model: "figure-guitarist",
        file: "venue--figure-guitarist.toskfixture",
        name: "Figure, Guitarist",
        short_name: "Guitarist",
        notes: "A 1750 mm figure with a guitar in hand, facing the audience.",
    },
    Entry {
        group: "people",
        model: "figure-pianist",
        file: "venue--figure-pianist.toskfixture",
        name: "Figure, Pianist",
        short_name: "Pianist",
        notes: "A figure seated at a keyboard, 1300 mm seated, facing the audience.",
    },
    Entry {
        group: "people",
        model: "figure-singer",
        file: "venue--figure-singer.toskfixture",
        name: "Figure, Singer",
        short_name: "Singer",
        notes: "A 1750 mm figure with a microphone in hand, facing the audience.",
    },
];

/// What every entry says about itself beyond its own line: a ToskLight-authored visual-only object
/// standing in for whatever the venue actually has.
const PROVENANCE: &str = "ToskLight-authored visual-only Venue object drawn from the shipped \
Visualizer model, for planning and demonstration. It represents the class of equipment rather \
than a particular manufacturer's product, and it carries no DMX footprint.";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The shipped railing: the one existing package that is a plain fixed model with no generated
/// scenery, so it is the shape every entry here needs.
fn template() -> FixtureProfile {
    let package = root().join("assets/fixture-library/venue--stage-railing-2-m.toskfixture");
    read_fixture_package(&fs::read(package).expect("the railing package is shipped"))
        .expect("the railing package reads")
}

/// The size the model manifest measured, in millimetres, so no dimension is retyped by hand.
fn measured(group: &str, model: &str) -> (f64, f64, f64) {
    let manifest: Value = serde_json::from_slice(
        &fs::read(root().join("assets/models/manifest.json")).expect("the model manifest ships"),
    )
    .expect("the model manifest parses");
    let entry = manifest["models"]
        .as_array()
        .expect("the manifest lists models")
        .iter()
        .find(|entry| entry["group"] == group && entry["model"] == model)
        .unwrap_or_else(|| panic!("{group}/{model} is not in the model manifest"));
    let read = |key: &str| entry[key].as_f64().expect("a measured millimetre value");
    (
        read("width_millimetres"),
        read("height_millimetres"),
        read("depth_millimetres"),
    )
}

fn data_url(mime: &str, path: PathBuf) -> String {
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn build(entry: &Entry) {
    let mut profile = template();
    profile.id = FixtureId(Uuid::new_v5(&NAMESPACE, entry.model.as_bytes()));
    profile.revision = 1;
    profile.name = entry.name.into();
    profile.short_name = entry.short_name.into();
    profile.notes = format!("{} {PROVENANCE}", entry.notes);
    profile.photograph_asset = Some(data_url(
        "image/png",
        root()
            .join("docs/help/assets/models")
            .join(entry.group)
            .join(format!("{}.png", entry.model)),
    ));
    profile.model_asset = Some(data_url(
        "model/gltf-binary",
        root()
            .join("assets/models")
            .join(entry.group)
            .join(format!("{}.glb", entry.model)),
    ));
    // The library is authored to the editor's precision: whole millimetres, so an operator can save
    // a new revision of any of these without first correcting a figure the package shipped.
    let (width, height, depth) = measured(entry.group, entry.model);
    profile.physical.width_millimetres = Some(width.round() as f32);
    profile.physical.height_millimetres = Some(height.round() as f32);
    profile.physical.depth_millimetres = Some(depth.round() as f32);
    let mode = profile
        .modes
        .first_mut()
        .expect("the template has its one visual mode");
    mode.id = Uuid::new_v5(&NAMESPACE, format!("{}:mode", entry.model).as_bytes());
    mode.name = "Default".into();
    mode.notes = String::new();
    let head = mode
        .heads
        .first_mut()
        .expect("the template has its one visual head");
    head.id = Uuid::new_v5(&NAMESPACE, format!("{}:head", entry.model).as_bytes());
    head.name = "Visual".into();
    fs::write(
        root().join("assets/fixture-library").join(entry.file),
        write_fixture_package(&profile).expect("the authored package writes"),
    )
    .expect("the fixture library is writable");
    println!(
        "wrote {} ({:.0} × {:.0} × {:.0} mm)",
        entry.file,
        width.round(),
        height.round(),
        depth.round()
    );
}

fn main() {
    for entry in ENTRIES {
        build(entry);
    }
    println!("{} packages", ENTRIES.len());
}
