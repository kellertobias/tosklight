//! Build the shipped generated stage parts: the flights of stairs, the parametric handrail, and the
//! equipment generated at the size it is placed — a flight rack, a PA speaker and a line array.
//!
//! Stairs used to be one profile declaring itself a riser, told apart from a deck only by having
//! "stair" in its name. They are their own generated kind now, in two variants — with a handrail
//! up each side, and without — so a plan says which one is on the drawing rather than leaving the
//! rigger to guess. The handrail is the part that was missing entirely: the Visualizer has built
//! railings since the kind existed, but no profile pointed at it, so a deck could not be edged.
//!
//! Run it from the repository root after changing an entry:
//!
//! ```sh
//! cargo run -p light-fixture --example build_stage_access_packages
//! ```
//!
//! Identity is stable. The plain flight keeps the id it has always had, so shows that already
//! hold one keep their stairs; the new parts take UUID v5 ids under `NAMESPACE`, derived from
//! their own slugs, so a rerun writes the same ids.
use light_core::FixtureId;
use light_fixture::{
    FixtureProfile, ProfileScenery, ProfileSceneryKind, SceneryAxes, Vector3, read_fixture_package,
    write_fixture_package,
};
use std::{fs, path::PathBuf};
use uuid::Uuid;

/// The namespace the ids of the new stage-access parts are derived under, once and for good.
const NAMESPACE: Uuid = Uuid::from_u128(0x2f83_4c0e_7d15_5b92_a4e6_18cb_37f0_9d2a);

struct Entry {
    file: &'static str,
    /// The slug the id is derived from; `None` keeps the id the package already carries.
    slug: Option<&'static str>,
    name: &'static str,
    notes: &'static str,
    /// Width, height and depth in metres, and which of them the operator sets.
    size: (f32, f32, f32),
    minimum: (f32, f32, f32),
    maximum: (f32, f32, f32),
    adjustable: SceneryAxes,
    kind: ProfileSceneryKind,
    handrails: bool,
    /// The shipped package whose photograph this part shows, when it is not the stairs'.
    photograph_from: Option<&'static str>,
}

const ENTRIES: &[Entry] = &[
    Entry {
        file: "venue--stage-stairs.toskfixture",
        slug: None,
        name: "Stage Stairs",
        notes: "A flight of stage steps up onto a deck, built to the height it is placed at. \
Generated at the size it is placed rather than drawn from a model made for one height.",
        size: (1.0, 0.6, 2.8),
        minimum: (0.6, 0.1, 0.4),
        maximum: (2.4, 1.6, 4.0),
        adjustable: SceneryAxes {
            width: true,
            height: true,
            depth: true,
        },
        kind: ProfileSceneryKind::Stairs,
        handrails: false,
        photograph_from: None,
    },
    Entry {
        file: "venue--stage-stairs-with-handrails.toskfixture",
        slug: Some("stage-stairs-with-handrails"),
        name: "Stage Stairs with Handrails",
        notes: "A flight of stage steps with a handrail up each side, built to the height it is \
placed at. Generated at the size it is placed rather than drawn from a model made for one height.",
        size: (1.0, 0.6, 2.8),
        minimum: (0.6, 0.1, 0.4),
        maximum: (2.4, 1.6, 4.0),
        adjustable: SceneryAxes {
            width: true,
            height: true,
            depth: true,
        },
        kind: ProfileSceneryKind::Stairs,
        handrails: true,
        photograph_from: None,
    },
    Entry {
        file: "venue--stage-handrail.toskfixture",
        slug: Some("stage-handrail"),
        name: "Stage Handrail",
        notes: "A handrail for the edge of a stage: posts, a top rail and a knee rail, at any \
length. It stands 1 m high, as a stage edge guard is built, and is generated at the width it is \
placed rather than drawn from a model made for one length.",
        // A handrail is fixed at guard height and as thin as its posts; only its run is set.
        size: (2.0, 1.0, 0.04),
        minimum: (0.4, 1.0, 0.04),
        maximum: (24.0, 1.0, 0.04),
        adjustable: SceneryAxes {
            width: true,
            height: false,
            depth: false,
        },
        kind: ProfileSceneryKind::Railing,
        handrails: false,
        photograph_from: None,
    },
    // Equipment whose one count is read off its height, as it is bought: a rack by its 19-inch
    // units (120 mm of case plus 44.45 mm a unit), a line array by its elements (a 100 mm frame
    // plus 250 mm an element), a PA speaker by its pole (anything above its 600 mm cabinet).
    Entry {
        file: "venue--flight-rack.toskfixture",
        slug: Some("flight-rack"),
        name: "Flight Rack",
        notes: "A 19-inch flight-case rack on the floor, generated at the rack units and depth it is \
placed at: 1 to 24 units, 0.4 to 1 m deep.",
        size: (0.6, 0.12 + 0.04445 * 6.0, 0.6),
        minimum: (0.6, 0.12 + 0.04445, 0.4),
        maximum: (0.6, 0.12 + 0.04445 * 24.0, 1.0),
        adjustable: SceneryAxes {
            width: false,
            height: true,
            depth: true,
        },
        kind: ProfileSceneryKind::FlightRack,
        handrails: false,
        photograph_from: Some("venue--flight-case-rack-6u.toskfixture"),
    },
    Entry {
        file: "venue--pa-speaker.toskfixture",
        slug: Some("pa-speaker"),
        name: "PA Speaker",
        notes: "A PA top cabinet, standing on its own or on a pole stand: placed taller than its \
600 mm cabinet, the rest of its height is the pole, up to a 2 m pole.",
        size: (0.35, 0.6, 0.4),
        minimum: (0.35, 0.6, 0.4),
        maximum: (0.35, 2.6, 0.4),
        adjustable: SceneryAxes {
            width: false,
            height: true,
            depth: false,
        },
        kind: ProfileSceneryKind::PaTop,
        handrails: false,
        photograph_from: Some("venue--pa-top.toskfixture"),
    },
    Entry {
        file: "venue--line-array.toskfixture",
        slug: Some("line-array"),
        name: "Line Array",
        notes: "A hanging line-array PA: a flying frame over 1 to 24 elements, generated at the \
number of elements it is placed with.",
        size: (1.0, 0.1 + 0.25 * 8.0, 0.6),
        minimum: (1.0, 0.1 + 0.25, 0.6),
        maximum: (1.0, 0.1 + 0.25 * 24.0, 0.6),
        adjustable: SceneryAxes {
            width: false,
            height: true,
            depth: false,
        },
        kind: ProfileSceneryKind::LineArray,
        handrails: false,
        photograph_from: Some("venue--line-array-hang.toskfixture"),
    },
    // A disco ball hung on its chain: its width and depth are the ball's diameter, and its height
    // is the chain and the ball together, so the drop is whatever the height leaves above the ball.
    Entry {
        file: "venue--disco-ball.toskfixture",
        slug: Some("disco-ball"),
        name: "Disco Ball",
        notes: "A mirror ball hung on a chain, generated at the diameter and drop it is placed \
with: a 0.2 to 1.5 m ball on up to 3 m of chain.",
        size: (0.5, 0.75, 0.5),
        minimum: (0.2, 0.2, 0.2),
        maximum: (1.5, 4.5, 1.5),
        adjustable: SceneryAxes {
            width: true,
            height: true,
            depth: false,
        },
        kind: ProfileSceneryKind::MirrorBall,
        handrails: false,
        photograph_from: Some("venue--disco-ball-50-cm.toskfixture"),
    },
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The shipped flight of stairs: a generated Venue object with one visual mode, which is the
/// shape every entry here needs.
fn template() -> FixtureProfile {
    let package = root().join("assets/fixture-library/venue--stage-stairs.toskfixture");
    read_fixture_package(&fs::read(package).expect("the stairs package is shipped"))
        .expect("the stairs package reads")
}

fn vector((x, y, z): (f32, f32, f32)) -> Vector3 {
    Vector3 { x, y, z }
}

/// The package as it is on disk now, when the entry has one.
fn existing(file: &str) -> Option<FixtureProfile> {
    let path = root().join("assets/fixture-library").join(file);
    read_fixture_package(&fs::read(path).ok()?).ok()
}

fn build(entry: &Entry) {
    let held = existing(entry.file);
    let mut profile = template();
    if let Some(slug) = entry.slug {
        profile.id = FixtureId(Uuid::new_v5(&NAMESPACE, slug.as_bytes()));
    }
    // A part keeps the revision it already ships; it is raised only when this writes a different
    // package, so an installation that already holds it is offered the change once and no more.
    profile.revision = held.as_ref().map_or(1, |held| held.revision);
    if let Some(source) = entry.photograph_from {
        profile.photograph_asset = existing(source)
            .expect("the package the photograph comes from is shipped")
            .photograph_asset;
    }
    profile.name = entry.name.into();
    profile.short_name = entry.name.into();
    profile.notes = entry.notes.into();
    // The profile editor works in whole millimetres; a rack's units need not come out whole.
    let (width, height, depth) = entry.size;
    profile.physical.width_millimetres = Some((width * 1000.0).round());
    profile.physical.height_millimetres = Some((height * 1000.0).round());
    profile.physical.depth_millimetres = Some((depth * 1000.0).round());
    profile.scenery = Some(ProfileScenery {
        kind: entry.kind,
        chords: 0,
        pattern: Default::default(),
        feet: Default::default(),
        handrails: entry.handrails,
        default_size_metres: vector(entry.size),
        adjustable: entry.adjustable,
        minimum_size_metres: vector(entry.minimum),
        maximum_size_metres: vector(entry.maximum),
    });
    // A new part needs its own mode and head identity as well as its own profile id, or two parts
    // would claim the same mode.
    if let Some(slug) = entry.slug {
        let mode = profile
            .modes
            .first_mut()
            .expect("the template's visual mode");
        mode.id = Uuid::new_v5(&NAMESPACE, format!("{slug}:mode").as_bytes());
        if let Some(head) = mode.heads.first_mut() {
            head.id = Uuid::new_v5(&NAMESPACE, format!("{slug}:head").as_bytes());
        }
    }
    let unchanged = held.as_ref().is_some_and(|held| {
        let mut same = profile.clone();
        same.revision = held.revision;
        serde_json::to_value(&same).ok() == serde_json::to_value(held).ok()
    });
    if unchanged {
        println!("{}\tunchanged\t{}", entry.file, profile.id.0);
        return;
    }
    profile.revision = held.map_or(1, |held| held.revision + 1);
    let package = write_fixture_package(&profile).expect("the profile packages");
    let path = root().join("assets/fixture-library").join(entry.file);
    fs::write(&path, package).expect("the package writes");
    println!(
        "{}\t{}\trevision {}\t{}",
        entry.file, profile.name, profile.revision, profile.id.0
    );
}

fn main() {
    for entry in ENTRIES {
        build(entry);
    }
}
