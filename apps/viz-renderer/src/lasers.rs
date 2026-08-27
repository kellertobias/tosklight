//! Running the rig's laser scan engines, once per displayed frame.
//!
//! The engine lives here rather than in the decoder for two reasons. It has to run at frame rate
//! rather than at DMX rate — a scan path is geometry for one picture, and decoding a universe
//! three times between two frames should not compile it three times. And a JavaScript runtime is
//! bound to the thread that made it, which has to be the thread that draws.
//!
//! # Live reload
//!
//! A laser's engine ships inside its fixture package, which is what makes a show portable. That is
//! the wrong loop to author in: changing a pattern would mean rebuilding an archive, re-importing
//! it, and making a new profile revision for every experiment.
//!
//! So a directory of loose scripts overrides the packaged ones. Drop `<profile id>.js` in it and
//! that file is used instead of whatever the package carries, re-read whenever it changes on disk.
//! Nothing else about the show moves: the fixture, its patch and its profile are untouched, and
//! deleting the file puts the packaged engine straight back. When the pattern is right, it goes
//! into the package and the override comes out.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use viz_laser::{ScanEngine, ScanRequest};
use viz_scene::{EmitterKind, Scene, SceneValues};

/// Points the override directory somewhere else, which is how a test keeps out of an operator's
/// own scripts.
pub const SCRIPT_DIRECTORY_ENV: &str = "TOSKLIGHT_VIZ_LASER_SCRIPTS";

/// How often the override directory is re-checked, in seconds.
///
/// Fast enough that saving a file and looking up feels immediate, slow enough that it is a few
/// stat calls a second rather than per frame. Only files whose modification time actually moved
/// are re-read.
const RELOAD_INTERVAL: f32 = 0.25;

/// One overriding script read from disk.
struct Override {
    source: Arc<str>,
    key: u64,
    modified: Option<SystemTime>,
}

/// Runs every laser in the rig and keeps the override directory in step.
pub struct Lasers {
    engine: Option<ScanEngine>,
    /// Why there is no engine at all, when there is not. A visualizer that could not start a
    /// script runtime has to say so rather than showing an unexplained rig of dark lasers.
    unavailable: Option<String>,
    directory: Option<PathBuf>,
    overrides: HashMap<String, Override>,
    next_reload: f32,
    /// When each emitter last ran, so a script can integrate its own motion.
    last_run: HashMap<usize, f32>,
}

impl Lasers {
    pub fn new(directory: Option<PathBuf>) -> Self {
        let (engine, unavailable) = match ScanEngine::new() {
            Ok(engine) => (Some(engine), None),
            Err(reason) => (None, Some(reason)),
        };
        Self {
            engine,
            unavailable,
            directory,
            overrides: HashMap::new(),
            next_reload: f32::MIN,
            last_run: HashMap::new(),
        }
    }

    /// Where loose override scripts are read from, or `None` when the operator has not named one.
    pub fn directory(options_directory: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(path) = options_directory {
            return Some(path);
        }
        std::env::var_os(SCRIPT_DIRECTORY_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// Run every laser in the scene for this frame, writing the paths into `values`.
    pub fn run(&mut self, scene: &Scene, values: &mut SceneValues, time: f32) {
        if !scene
            .emitters
            .iter()
            .any(|emitter| emitter.kind == EmitterKind::Laser)
        {
            return;
        }
        self.refresh_overrides(time);
        let Some(engine) = self.engine.as_mut() else {
            let reason = self
                .unavailable
                .clone()
                .unwrap_or_else(|| "no scan runtime".into());
            for (index, emitter) in scene.emitters.iter().enumerate() {
                if emitter.kind == EmitterKind::Laser
                    && let Some(scan) = values.laser_scans.get_mut(index)
                {
                    scan.points.clear();
                    scan.fault = Some(reason.clone());
                }
            }
            return;
        };

        for (index, emitter) in scene.emitters.iter().enumerate() {
            if emitter.kind != EmitterKind::Laser {
                continue;
            }
            let Some(optics) = emitter.laser.as_ref() else {
                continue;
            };
            let Some(scan) = values.laser_scans.get_mut(index) else {
                continue;
            };
            // An override, if one exists for this fixture's profile; otherwise what the package
            // shipped. A laser with neither stays dark and says why.
            let profile = scene
                .fixtures
                .get(emitter.fixture_index as usize)
                .map(|fixture| fixture.fixture_id.to_string());
            let overriding = profile
                .as_deref()
                .and_then(|profile| self.overrides.get(profile));
            let (source, key) = match overriding {
                Some(script) => (script.source.clone(), script.key),
                None => match optics.script.clone() {
                    Some(source) => (source, optics.script_key),
                    None => {
                        scan.points.clear();
                        scan.fault = Some("this laser's profile ships no scan script".into());
                        continue;
                    }
                },
            };

            let previous = self.last_run.insert(index, time);
            let elapsed = previous.map(|last| (time - last).max(0.0)).unwrap_or(0.0);
            let value = values.emitters.get(index);
            let intensity = value.map(|value| value.intensity).unwrap_or(0.0);
            let produced = engine.scan(
                index,
                &ScanRequest {
                    source: &source,
                    source_key: key,
                    slots: &scan.slots,
                    time_seconds: f64::from(time),
                    elapsed_seconds: f64::from(elapsed),
                    intensity,
                },
            );
            // The slots belong to the decoder and are overwritten every time a packet lands; only
            // the parts the engine produced are replaced here.
            scan.points = produced.points;
            scan.fault = produced.fault;
            scan.points_per_second = if produced.points_per_second > 0.0 {
                produced.points_per_second
            } else {
                optics.points_per_second
            };
        }

        // A rig edited during a show must not leave a JavaScript context behind per removed laser.
        let live = scene.emitters.len();
        engine.retain(|emitter| emitter < live);
        self.last_run.retain(|emitter, _| *emitter < live);
    }

    /// Whatever the operator most needs to know about the laser scripts, or nothing when they are
    /// all working.
    pub fn fault(&self, values: &SceneValues) -> Option<String> {
        if let Some(reason) = &self.unavailable {
            return Some(format!("laser scripts unavailable: {reason}"));
        }
        let faults = values
            .laser_scans
            .iter()
            .filter_map(|scan| scan.fault.as_deref())
            .collect::<Vec<_>>();
        let first = faults.first()?;
        Some(match faults.len() {
            1 => format!("laser: {first}"),
            count => format!("laser: {first} (and {} more)", count - 1),
        })
    }

    /// Re-read any override whose file changed, and notice files added or removed.
    fn refresh_overrides(&mut self, time: f32) {
        let Some(directory) = self.directory.clone() else {
            return;
        };
        if time < self.next_reload {
            return;
        }
        self.next_reload = time + RELOAD_INTERVAL;
        let Ok(entries) = std::fs::read_dir(&directory) else {
            // A directory that does not exist yet is the normal state, not a fault: an operator
            // creates it the first time they want to author a pattern.
            self.overrides.clear();
            return;
        };
        let mut seen = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(profile) = script_profile(&path) else {
                continue;
            };
            seen.push(profile.clone());
            let modified = entry.metadata().ok().and_then(|data| data.modified().ok());
            if self
                .overrides
                .get(&profile)
                .is_some_and(|existing| existing.modified == modified && modified.is_some())
            {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let key = script_key(&source);
            self.overrides.insert(
                profile,
                Override {
                    source: source.into(),
                    key,
                    modified,
                },
            );
        }
        // A deleted override puts the packaged engine straight back.
        self.overrides.retain(|profile, _| seen.contains(profile));
    }
}

/// The profile a loose script overrides, taken from its filename.
fn script_profile(path: &Path) -> Option<String> {
    if path.extension()?.to_str()? != "js" {
        return None;
    }
    Some(path.file_stem()?.to_str()?.to_ascii_lowercase())
}

/// The same hash the packaged path uses, so a changed override recompiles exactly as a changed
/// package does.
fn script_key(source: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash | 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::{
        BodyKind, EmitterInstance, EmitterLayoutCells, EmitterOptics, FixtureBody, FixtureInstance,
        LaserOptics,
    };

    const PACKAGED: &str =
        "export function scan() { return { points: [{ x: -1, y: 0, r: 1, amount: 100 }] }; }";
    const OVERRIDING: &str =
        "export function scan() { return { points: [{ x: 1, y: 0, g: 1, amount: 100 }] }; }";

    fn scene(script: Option<&str>) -> Scene {
        let mut scene = Scene::default();
        let fixture_id = viz_scene::uuid::Uuid::from_u128(7);
        scene.fixtures.push(FixtureInstance {
            instance_id: fixture_id,
            fixture_id,
            name: "Laser".into(),
            number: Some(1),
            position: glam::Vec3::new(0.0, 5.0, 0.0),
            rotation_degrees: glam::Vec3::ZERO,
            position_master: None,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: glam::Vec3::splat(0.3),
                kind: BodyKind::Lantern,
            },
            patched: true,
            address: Some((1, 1)),
            model: None,
            fallback: None,
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Scanner".into(),
            local_origin: glam::Vec3::ZERO,
            tilt_pivot: glam::Vec3::ZERO,
            local_orientation_degrees: glam::Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 0.1,
            field_angle_degrees: 0.1,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Laser,
            cells: EmitterLayoutCells::single(),
            laser: Some(LaserOptics {
                script: script.map(Arc::from),
                script_key: script.map(script_key).unwrap_or(0),
                ..LaserOptics::default()
            }),
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
        scene
    }

    fn values(scene: &Scene) -> SceneValues {
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values.emitters[0].intensity = 1.0;
        values
    }

    /// The packaged engine is what a laser projects with when nobody has overridden it.
    #[test]
    fn a_packaged_script_projects() {
        let scene = scene(Some(PACKAGED));
        let mut values = values(&scene);
        let mut lasers = Lasers::new(None);
        lasers.run(&scene, &mut values, 0.0);
        assert_eq!(values.laser_scans[0].fault, None);
        assert_eq!(values.laser_scans[0].points.len(), 1);
        assert_eq!(values.laser_scans[0].points[0].x, -1.0);
        // The fixture's rated scanner speed fills in for a script that named none.
        assert_eq!(values.laser_scans[0].points_per_second, 30_000.0);
    }

    /// A laser whose profile ships nothing to run must be dark and diagnosed, never quietly
    /// projecting an invented pattern.
    #[test]
    fn a_laser_with_no_script_is_dark_and_says_so() {
        let scene = scene(None);
        let mut values = values(&scene);
        let mut lasers = Lasers::new(None);
        lasers.run(&scene, &mut values, 0.0);
        assert!(values.laser_scans[0].points.is_empty());
        assert!(
            values.laser_scans[0]
                .fault
                .as_deref()
                .is_some_and(|fault| fault.contains("no scan script"))
        );
        assert!(lasers.fault(&values).is_some());
    }

    /// The authoring loop: a loose file takes over from the package, a saved edit is picked up,
    /// and deleting it hands the fixture straight back to what it shipped with.
    #[test]
    fn a_loose_script_overrides_the_package_and_reloads_when_it_changes() {
        let directory = std::env::temp_dir().join(format!(
            "tosklight-laser-{}",
            std::process::id() as u64 * 31 + 5
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{}.js", viz_scene::uuid::Uuid::from_u128(7)));

        let scene = scene(Some(PACKAGED));
        let mut values = values(&scene);
        let mut lasers = Lasers::new(Some(directory.clone()));

        // Nothing on disk yet: the package is what runs.
        lasers.run(&scene, &mut values, 0.0);
        assert_eq!(values.laser_scans[0].points[0].x, -1.0);

        // A loose script takes over.
        std::fs::write(&path, OVERRIDING).unwrap();
        lasers.next_reload = f32::MIN;
        lasers.run(&scene, &mut values, 1.0);
        assert_eq!(
            values.laser_scans[0].points[0].x, 1.0,
            "the override did not take effect"
        );

        // An edit to it is picked up without restarting anything.
        std::fs::write(
            &path,
            "export function scan() { return { points: [{ x: 0.5, y: 0, b: 1, amount: 100 }] }; }",
        )
        .unwrap();
        lasers.next_reload = f32::MIN;
        lasers.overrides.clear();
        lasers.run(&scene, &mut values, 2.0);
        assert_eq!(
            values.laser_scans[0].points[0].x, 0.5,
            "the edit was missed"
        );

        // Deleting it restores the packaged engine.
        std::fs::remove_file(&path).unwrap();
        lasers.next_reload = f32::MIN;
        lasers.run(&scene, &mut values, 3.0);
        assert_eq!(
            values.laser_scans[0].points[0].x, -1.0,
            "the packaged engine did not come back"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A broken override must report itself rather than leaving an operator wondering why the
    /// laser stopped.
    #[test]
    fn a_broken_script_surfaces_as_a_fault() {
        let scene = scene(Some(
            "export function scan() { throw new Error('bad pattern'); }",
        ));
        let mut values = values(&scene);
        let mut lasers = Lasers::new(None);
        lasers.run(&scene, &mut values, 0.0);
        assert!(values.laser_scans[0].points.is_empty());
        assert!(
            lasers
                .fault(&values)
                .is_some_and(|fault| fault.contains("bad pattern"))
        );
    }

    /// A rig with no lasers must not pay for any of this.
    #[test]
    fn a_rig_without_lasers_does_no_work() {
        let mut scene = scene(Some(PACKAGED));
        scene.emitters[0].kind = EmitterKind::Beam;
        let mut values = values(&scene);
        let mut lasers = Lasers::new(None);
        lasers.run(&scene, &mut values, 0.0);
        assert!(values.laser_scans[0].points.is_empty());
        assert_eq!(values.laser_scans[0].fault, None);
    }
}

/// The built-in look has to exercise the whole laser path, because it is the one scene that can be
/// rendered with no desk, no show file and no fixture library — which makes it the only place a
/// laser regression is caught without a rig.
#[cfg(test)]
mod demo_tests {
    use super::*;

    #[test]
    fn the_built_in_look_projects_its_laser() {
        let scene = crate::demo::build_scene();
        let index = scene
            .emitters
            .iter()
            .position(|emitter| emitter.kind == EmitterKind::Laser)
            .expect("the built-in look must carry a laser");
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values.emitters[index].intensity = 1.0;
        values.emitters[index].held_intensity = 1.0;

        let mut lasers = Lasers::new(None);
        lasers.run(&scene, &mut values, 0.0);
        let scan = &values.laser_scans[index];
        assert_eq!(scan.fault, None, "the demo scan script failed");
        assert!(
            scan.points.len() > 50,
            "the demo laser produced {} points",
            scan.points.len()
        );

        let instances =
            viz_render::build_instances(&scene, &values, &viz_render::FrameStyle::default());
        assert!(
            !instances.lasers.is_empty(),
            "the demo laser produced no drawable runs"
        );
    }
}
