#![forbid(unsafe_code)]
//! Freezing the visualizer's picture and handing it to a modelling package.
//!
//! A snapshot is one moment of a rig: every fixture where it hangs, every head where it was
//! pointing, every colour and level the desk had it at, and the room around it. Taking one is
//! instant and needs nothing installed — it is triangles and numbers written to a folder.
//!
//! Turning one into a Blender file is a separate step, because only Blender can write a Blender
//! file. That separation is the point: an operator captures the moment while it is happening, on
//! whatever machine is beside the stage, and produces the file later on whatever machine has
//! Blender on it.

mod blender;
mod document;
mod gltf;

pub use blender::{BLENDER_ENV, ExportError, ExportedBlend, export_blend, find_blender};
pub use document::{
    FORMAT_VERSION, REFERENCE_INTENSITY, SnapshotBounds, SnapshotCamera, SnapshotCounts,
    SnapshotDocument, SnapshotLight, SnapshotLook, to_z_up, watts,
};

use std::path::{Path, PathBuf};
use viz_scene::{Scene, SceneValues};

/// The geometry file inside a snapshot folder.
pub const GEOMETRY_FILE: &str = "rig.glb";
/// The document beside it.
pub const DOCUMENT_FILE: &str = "snapshot.json";
/// The Blender file an export writes into the same folder.
pub const BLEND_FILE: &str = "rig.blend";
/// Environment variable that moves the snapshot folder, for a test or an operator who wants their
/// captures somewhere they already back up.
pub const SNAPSHOT_DIR_ENV: &str = "TOSKLIGHT_VIZ_SNAPSHOT_DIR";
/// How many captures are kept before the oldest is dropped.
pub const DEFAULT_KEPT: usize = 12;

/// What the application knows about the moment that the scene itself does not.
#[derive(Clone, Debug, Default)]
pub struct CaptureContext {
    pub show: String,
    /// The desk, file, or planner this scene came from, as the status surface names it.
    pub source: String,
    pub scene_revision: u64,
    pub look: SnapshotLook,
    /// The camera in the visualizer's own Y-up space; converted on the way into the document.
    pub camera: SnapshotCamera,
}

/// A captured moment, in memory, ready to be written.
pub struct Capture {
    pub document: SnapshotDocument,
    pub geometry: Vec<u8>,
}

/// Freeze `scene` and `values` into a capture.
///
/// Everything expensive happens here and nothing here touches the disk, so a caller can clone the
/// scene on the thread that is drawing — which is instant — and do this on one that is not.
pub fn capture(scene: &Scene, values: &SceneValues, context: &CaptureContext) -> Capture {
    let geometry = viz_render::scene_geometry(scene, values);
    let lights = describe_lights(scene, values);
    let generator = format!("ToskLight Visualizer snapshot {FORMAT_VERSION}");
    let proxies = scene
        .fixtures
        .iter()
        .filter(|fixture| fixture.model.is_none())
        .count();

    let mut notes = vec![
        "Gobos, prisms and framing shutters are this visualizer's own optics and are not carried \
         over; a beam arrives as its cone, colour and level."
            .to_owned(),
        "Haze is the renderer's own amount, never a hazer's DMX output.".to_owned(),
    ];
    if proxies > 0 {
        notes.push(format!(
            "{proxies} of {} fixtures were drawn from procedural proxies because their profiles \
             carry no model.",
            scene.fixtures.len()
        ));
    }

    let document = SnapshotDocument {
        format: FORMAT_VERSION,
        application: "ToskLight Visualizer".to_owned(),
        captured_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        show: context.show.clone(),
        source: context.source.clone(),
        scene_revision: context.scene_revision,
        geometry_file: GEOMETRY_FILE.to_owned(),
        units: "metres".to_owned(),
        up_axis: "z".to_owned(),
        counts: SnapshotCounts {
            fixtures: scene.fixtures.len(),
            heads: scene.emitters.len(),
            live_beams: lights.len(),
            triangles: geometry.triangle_count(),
        },
        look: context.look,
        camera: SnapshotCamera {
            position: to_z_up(context.camera.position),
            target: to_z_up(context.camera.target),
            ..context.camera
        },
        bounds: SnapshotBounds {
            // Converting a corner at a time can swap an axis end for end, so the box is rebuilt
            // from both corners rather than assumed to still be the right way round.
            min: corner(scene.bounds, f32::min),
            max: corner(scene.bounds, f32::max),
        },
        lights,
        notes,
    };
    Capture {
        document,
        geometry: gltf::write_glb(&geometry, &generator),
    }
}

/// Air left between a lamp and the light that leaves it, in metres.
const LENS_CLEARANCE: f32 = 0.02;

/// Move a head's light clear of the lamp it belongs to.
///
/// The visualizer draws a shaft from the lens and never asks whether the fixture's own body is in
/// the way. A renderer that traces light does ask, and a light sitting inside the head that carries
/// it lights nothing at all: the lamp goes dark, the stage stays black, and the only thing left in
/// the picture is the glow of the lens itself.
///
/// So the light is put where it actually leaves the lamp — on the aim, just past the body and past
/// its own lit surface. On a seven-metre throw the difference is nothing; it is the difference
/// between a rig that lights the stage and one that does not.
fn clear_of_the_body(
    fixture: &viz_scene::FixtureInstance,
    origin: glam::Vec3,
    direction: glam::Vec3,
    source_radius: f32,
) -> glam::Vec3 {
    let local = fixture.orientation().inverse() * direction;
    let half = fixture.body.size.abs() * 0.5;
    // How far the body reaches from its own centre in the direction the light goes.
    let reach = local.x.abs() * half.x + local.y.abs() * half.y + local.z.abs() * half.z;
    let wanted = reach + source_radius + LENS_CLEARANCE;
    let already = (origin - fixture.position).dot(direction);
    origin + direction * (wanted - already).max(0.0)
}

/// One corner of the converted bounding box, taken axis by axis.
fn corner(bounds: viz_scene::Aabb, pick: fn(f32, f32) -> f32) -> [f32; 3] {
    let min = to_z_up(bounds.min.to_array());
    let max = to_z_up(bounds.max.to_array());
    [
        pick(min[0], max[0]),
        pick(min[1], max[1]),
        pick(min[2], max[2]),
    ]
}

/// Every emitting head as a spot light, named so an operator recognises it in the outliner.
fn describe_lights(scene: &Scene, values: &SceneValues) -> Vec<SnapshotLight> {
    let mut lights = Vec::new();
    let mut used: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    // A fixture with one head is that fixture; only a bar or a multi-head lamp needs the head
    // named as well, and adding "Main" to every lantern in a rig helps nobody.
    let mut heads_per_fixture = vec![0_usize; scene.fixtures.len()];
    for emitter in &scene.emitters {
        if let Some(count) = heads_per_fixture.get_mut(emitter.fixture_index as usize) {
            *count += 1;
        }
    }
    for light in viz_render::semantic_lights(scene, values) {
        let Some(emitter) = scene.emitters.get(light.emitter_index as usize) else {
            continue;
        };
        let Some(fixture) = scene.fixtures.get(emitter.fixture_index as usize) else {
            continue;
        };
        let mut name = match fixture.number {
            Some(number) => format!("{number} {}", fixture.name),
            None => fixture.name.clone(),
        };
        let multi_head = heads_per_fixture
            .get(emitter.fixture_index as usize)
            .is_some_and(|count| *count > 1);
        if multi_head && !emitter.label.is_empty() {
            name.push_str(&format!(" {}", emitter.label));
        }
        if emitter.cells.len() > 1 {
            name.push_str(&format!(" cell {}", light.cell_index + 1));
        }
        // Blender makes its own names unique with a numeric suffix. Doing it here instead keeps
        // the suffix meaningful and the name stable between two exports of the same rig.
        let seen = used.entry(name.clone()).or_insert(0);
        *seen += 1;
        if *seen > 1 {
            name.push_str(&format!(" ({seen})"));
        }

        lights.push(SnapshotLight {
            name,
            fixture: fixture.name.clone(),
            fixture_number: fixture.number,
            address: fixture
                .address
                .map(|(universe, address)| format!("{universe}.{address}")),
            position: to_z_up(
                clear_of_the_body(fixture, light.origin, light.direction, light.source_radius)
                    .to_array(),
            ),
            direction: to_z_up(light.direction.to_array()),
            colour: light.colour.to_array(),
            intensity: light.intensity,
            power_watts: watts(light.output, light.intensity, light.outer_half_angle),
            cone_degrees: light.outer_half_angle.to_degrees() * 2.0,
            blend: light.feather,
            radius: light.source_radius,
            reach: light.reach,
        });
    }
    lights
}

/// One capture on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotEntry {
    pub directory: PathBuf,
    /// Local time it was taken, as the operator reads it back.
    pub captured_at: String,
    pub show: String,
    pub counts: SnapshotCounts,
    /// The Blender file, once one has been exported.
    pub blend: Option<PathBuf>,
}

impl SnapshotEntry {
    /// How the capture is listed: the time it was taken, which is what an operator remembers.
    pub fn label(&self) -> String {
        self.captured_at
            .split_once(' ')
            .map(|(_, time)| time.to_owned())
            .unwrap_or_else(|| self.captured_at.clone())
    }

    /// What was in it, for the row beside the label.
    pub fn summary(&self) -> String {
        format!(
            "{} fixtures, {} live",
            self.counts.fixtures, self.counts.live_beams
        )
    }

    /// Where a Blender export of this capture goes.
    pub fn blend_destination(&self) -> PathBuf {
        self.directory.join(BLEND_FILE)
    }
}

/// The folder captures are kept in, and the rule for how many.
#[derive(Clone, Debug)]
pub struct SnapshotStore {
    root: PathBuf,
    keep: usize,
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new(default_root(), DEFAULT_KEPT)
    }
}

impl SnapshotStore {
    pub fn new(root: impl Into<PathBuf>, keep: usize) -> Self {
        Self {
            root: root.into(),
            keep: keep.max(1),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Write a capture as its own folder and drop whatever falls off the end.
    pub fn write(&self, capture: &Capture) -> std::io::Result<SnapshotEntry> {
        let directory = self.root.join(folder_name(&capture.document));
        std::fs::create_dir_all(&directory)?;
        std::fs::write(directory.join(GEOMETRY_FILE), &capture.geometry)?;
        std::fs::write(
            directory.join(DOCUMENT_FILE),
            serde_json::to_vec_pretty(&capture.document)
                .map_err(|error| std::io::Error::other(error.to_string()))?,
        )?;
        let entry = SnapshotEntry {
            directory,
            captured_at: capture.document.captured_at.clone(),
            show: capture.document.show.clone(),
            counts: capture.document.counts,
            blend: None,
        };
        self.prune();
        Ok(entry)
    }

    /// Every capture that is still kept, newest first.
    pub fn list(&self) -> Vec<SnapshotEntry> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut folders: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join(DOCUMENT_FILE).is_file())
            .collect();
        // Folder names begin with the capture time, so their order is the order they were taken
        // in without reading a single document.
        folders.sort();
        folders
            .into_iter()
            .rev()
            .take(self.keep)
            .filter_map(|directory| self.read_entry(&directory))
            .collect()
    }

    fn read_entry(&self, directory: &Path) -> Option<SnapshotEntry> {
        let text = std::fs::read_to_string(directory.join(DOCUMENT_FILE)).ok()?;
        let document: SnapshotDocument = serde_json::from_str(&text).ok()?;
        let blend = directory.join(BLEND_FILE);
        Some(SnapshotEntry {
            directory: directory.to_path_buf(),
            captured_at: document.captured_at,
            show: document.show,
            counts: document.counts,
            blend: blend.is_file().then_some(blend),
        })
    }

    /// Remove everything past the kept count, oldest first.
    ///
    /// A folder that is not a capture is left alone: this owns the captures it wrote, not whatever
    /// else an operator has put beside them.
    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        let mut folders: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join(DOCUMENT_FILE).is_file())
            .collect();
        folders.sort();
        let excess = folders.len().saturating_sub(self.keep);
        for folder in folders.into_iter().take(excess) {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}

/// Where captures go when nobody says otherwise: beside the operator's own documents, in the
/// place their platform keeps application data.
pub fn default_root() -> PathBuf {
    if let Some(override_path) =
        std::env::var_os(SNAPSHOT_DIR_ENV).filter(|value| !value.is_empty())
    {
        return PathBuf::from(override_path);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if cfg!(target_os = "macos") {
        if let Some(home) = home {
            return home.join("Library/Application Support/ToskLight/Visualizer/Snapshots");
        }
    } else if cfg!(windows) {
        if let Some(data) = std::env::var_os("APPDATA") {
            return PathBuf::from(data).join("ToskLight/Visualizer/Snapshots");
        }
    } else if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(data).join("tosklight/visualizer/snapshots");
    } else if let Some(home) = home.clone() {
        return home.join(".local/share/tosklight/visualizer/snapshots");
    }
    // Nowhere to put it but here, which is better than losing the capture.
    PathBuf::from("tosklight-viz-snapshots")
}

/// `2026-07-31 14-22-08 Tour Rig` — sortable by time, and recognisable in a file browser.
fn folder_name(document: &SnapshotDocument) -> String {
    let stamp = document.captured_at.replace(':', "-");
    let show = sanitise(&document.show);
    if show.is_empty() {
        stamp
    } else {
        format!("{stamp} {show}")
    }
}

/// How much of a show's name a folder name carries. A show can be called anything, including an
/// identifier nobody reads, and a folder that runs off the end of a file browser helps no one.
const NAME_LIMIT: usize = 48;

/// A show name goes into a folder name, so everything a file system objects to comes out.
fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                ' '
            }
        })
        .collect();
    let joined = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    // Cut on a character, never inside one, and never leave a trailing space behind.
    match joined.char_indices().nth(NAME_LIMIT) {
        Some((cut, _)) => joined[..cut].trim_end().to_owned(),
        None => joined,
    }
}

#[cfg(test)]
mod tests;
