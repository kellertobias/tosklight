//! Deterministic headless capture of a show, for the product demo video.
//!
//! This is the native render core with no window: the same scene projection, materials, lighting,
//! fixture models and quality configuration the interactive visualizer uses, drawn into an
//! offscreen texture and read back as PNG frames. Nothing here opens a WebView, and there is no
//! fallback that quietly renders something else — a machine that cannot provide a GPU or software
//! adapter fails loudly, because a capture that silently produced nothing looks exactly like one
//! that rendered a dark stage.
//!
//! Determinism is the point. Resolution, camera, time step and the value script are all pinned by
//! the arguments rather than by the machine, so two runs of the same commit produce the same
//! frames and CI can composite them into a video.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use viz_scene::{RenderQuality, Scene, SceneValues, ViewConfiguration, ViewMode};

const USAGE: &str = "viz-capture --show FILE --output DIR [options]

  --show FILE       Show file to render. The generated demo show is the usual one.
  --output DIR      Where the PNG frames are written. Created if absent.
  --width  N        Frame width in pixels (default 1920).
  --height N        Frame height in pixels (default 1080).
  --frames N        How many frames to write (default 1).
  --step SECONDS    Scene time between frames (default 1/30).
  --settle N        Frames rendered and discarded first, so time-based motion has run (default 30).
  --haze PERCENT    Haze the beams are drawn through (default 50).
  --quality NAME    draft | standard | high | ultra (default high).
  --fixture NAME    Frame close on fixtures whose name starts with NAME.
  --view NAME       full3d | simple3d | lines3d | top-down | left-to-right |
                    right-to-left | front-to-back | back-to-front (default full3d).";

struct Options {
    show: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    frames: u32,
    step: f32,
    settle: u32,
    view: ViewMode,
    quality: RenderQuality,
    fixture: Option<String>,
    /// Haze, `0..=1`. Renderer-local by design — a hazer's DMX says how hard the machine is
    /// working, not what the room ends up like — so the capture states it rather than reading it
    /// from the show.
    haze: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show: PathBuf::new(),
            output: PathBuf::new(),
            width: 1920,
            height: 1080,
            frames: 1,
            // Thirty frames a second, stated rather than measured: a capture must not depend on
            // how fast the machine drew the previous one.
            step: 1.0 / 30.0,
            settle: 30,
            view: ViewMode::Full3d,
            quality: RenderQuality::High,
            fixture: None,
            haze: viz_scene::DEFAULT_DENSITY,
        }
    }
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1)) {
        Ok(None) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Some(options)) => match run(&options) {
            Ok(written) => {
                println!("Wrote {written} frames to {}", options.output.display());
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(options: &Options) -> Result<u32, String> {
    let (scene, bindings, preview) = scene_from(&options.show)?;
    if scene.fixtures.is_empty() {
        return Err(format!(
            "{} has no patched fixtures; there would be nothing to capture",
            options.show.display()
        ));
    }
    std::fs::create_dir_all(&options.output)
        .map_err(|error| format!("could not create {}: {error}", options.output.display()))?;

    let mut renderer = viz_render::Renderer::headless(options.width, options.height)?;
    let mut view = ViewConfiguration::default();
    view.mode = options.view;
    view.quality = options.quality;
    // The camera is framed from the rig rather than left wherever a previous session put it, so
    // the same show always yields the same shot.
    // The rig for the house view and the whole room for a plan, exactly as the desk's pane frames
    // them, so a capture is the picture an operator gets rather than a differently framed one.
    let framing = if let Some(fixture) = options.fixture.as_deref() {
        fixture_bounds(&scene, fixture)?
    } else if options.view.is_orthographic() {
        scene.bounds
    } else {
        scene.rig_bounds()
    };
    view.camera = viz_scene::Camera::framed(options.view, framing);
    if options.fixture.is_some() && !options.view.is_orthographic() {
        // A named fixture capture is inspection evidence, not another whole-stage shot. Stand
        // close, slightly below the fixture, and look at its centre so a down-facing lens and the
        // body carrying it are both visible. This camera is deterministic and exists only for an
        // explicit inspection capture; the ordinary product-demo camera remains the house view.
        let centre = framing.centre();
        let radius = framing.radius().max(0.25);
        view.camera.position =
            centre + viz_scene::glam::Vec3::new(0.0, -radius * 0.5, radius * 3.0);
        view.camera.target = centre;
        view.camera.up = viz_scene::glam::Vec3::Y;
        view.camera.fov_degrees = 35.0;
    }
    let overlay = viz_render::Overlay::default();
    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    // Without haze the beams are invisible and the frame is a few lit lamps in the dark, which is
    // not what the demo is showing.
    values.atmosphere.density = options.haze.clamp(0.0, 1.0);

    // The look is decoded from real DMX frames, not written straight into the value array: the
    // preview plane projects it onto universes through the fixture library, and the renderer's own
    // decoder reads them. So a capture exercises the same path a console would drive, and a
    // fixture whose channels are wrong in its package is wrong here too rather than quietly right.
    let mut decoder = viz_project::Decoder::new(bindings);
    decoder.apply(&scene, &preview, &mut values, 0.0);
    if values
        .emitters
        .iter()
        .all(|emitter| emitter.intensity <= 0.0)
    {
        return Err(
            "the scripted look decoded to a dark stage; the demo rig's channels did not resolve"
                .to_owned(),
        );
    }

    // Anything the renderer runs on a clock — gobo rotation, prisms, persistence of vision — is
    // given the same run-up every time, so the recorded frame does not depend on being the first.
    // Exposure is not among them: it is fixed, so that a rig at half never records as a rig at
    // full drawn dimmer.
    for frame in 0..options.settle {
        values.apply_physical_motion(options.step);
        values.apply_calibrated_motion(&scene, options.step);
        let seconds = frame as f32 * options.step;
        renderer
            .capture(&scene, &values, &view, &overlay, seconds)
            .map_err(|error| format!("settling frame {frame}: {error}"))?;
    }

    let mut written = 0;
    for frame in 0..options.frames {
        values.apply_physical_motion(options.step);
        values.apply_calibrated_motion(&scene, options.step);
        let seconds = (options.settle + frame) as f32 * options.step;
        let image = renderer
            .capture(&scene, &values, &view, &overlay, seconds)
            .map_err(|error| format!("frame {frame}: {error}"))?;
        let path = options.output.join(format!("frame-{frame:05}.png"));
        write_png(&path, image.width, image.height, &image.rgba)?;
        written += 1;
    }
    Ok(written)
}

/// Bounds for one named fixture class in the demo rig.
///
/// Fixture names carry numbers after a stable class label (`Wash 1`, `Wash 2`, ...), so a prefix
/// frames the complete named class. The body extent is included rather than framing only on rig
/// points; otherwise a single fixture has effectively zero size and remains too distant to judge.
fn fixture_bounds(scene: &Scene, prefix: &str) -> Result<viz_scene::Aabb, String> {
    let folded = prefix.trim().to_lowercase();
    let mut bounds = viz_scene::Aabb::empty();
    for fixture in &scene.fixtures {
        if !fixture.name.to_lowercase().starts_with(&folded) {
            continue;
        }
        let half = fixture.body.size * 0.6;
        bounds.expand(fixture.position - half);
        bounds.expand(fixture.position + half);
    }
    if bounds.is_empty() {
        Err(format!(
            "the show has no fixture whose name starts with {prefix:?}"
        ))
    } else {
        Ok(bounds)
    }
}

/// Build the scene from a show file, through the same projection the renderer uses against a desk.
///
/// The patch crosses the planning wire contract and is read back as the renderer's own type, so a
/// capture cannot drift from what the visualizer would draw for the same show.
type Capture = (
    Scene,
    Vec<viz_project::EmitterBinding>,
    Vec<viz_dmx::UniverseFrame>,
);

fn scene_from(path: &Path) -> Result<Capture, String> {
    let document = viz_document::PlanningDocument::open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let dto = viz_planning::wire::patch_snapshot(snapshot);
    let json = serde_json::to_value(&dto).map_err(|error| error.to_string())?;
    let patch: viz_desk::wire::PatchSnapshot =
        serde_json::from_value(json).map_err(|error| error.to_string())?;
    let models = viz_desk::DeskReadModels {
        patch,
        stage_layout: viz_desk::wire::StageLayoutBody::default(),
        venue_objects: Vec::new(),
        fixture_visibility: Vec::new(),
        patch_layers: Vec::new(),
        media_servers: Vec::new(),
        media_fallback_assets: Vec::new(),
        media_sources: Vec::new(),
        led_module_types: Vec::new(),
        media_surfaces: Vec::new(),
        media_projectors: Vec::new(),
        venue_groups: Vec::new(),
        show_name: document.name().unwrap_or_else(|_| "Show".to_owned()),
        server_identity: path.display().to_string(),
    };
    let mut plan = viz_desk::build(&models);
    plan.scene.recompute_bounds();
    let preview = scripted_look(&document)?;
    Ok((plan.scene, plan.bindings, preview))
}

/// The look the demo is shot under, as DMX frames.
///
/// Every fixture up, in a warm wash, with the movers aimed into the stage. It is deliberately one
/// static state rather than a chase: the demo video is composited from captures, and a look that
/// depended on when a frame was taken could not be reproduced from the same commit.
fn scripted_look(
    document: &viz_document::PlanningDocument,
) -> Result<Vec<viz_dmx::UniverseFrame>, String> {
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| error.to_string())?;
    let mut state = viz_planning::preview::PreviewState::default();
    for fixture in &snapshot.fixtures {
        let fixture_id = fixture.patch.fixture_id.0;
        for (parameter, value) in [
            (viz_planning::PreviewParameter::Intensity, 1.0),
            // Half travel on both axes points a moving head down the middle of the stage rather
            // than at whichever end of its range zero happens to be.
            (viz_planning::PreviewParameter::Pan, 0.5),
            (viz_planning::PreviewParameter::Tilt, 0.5),
        ] {
            state.apply(viz_planning::PreviewSet::Semantic {
                fixture_id,
                parameter,
                value,
                colour: [0.0; 3],
            });
        }
        state.apply(viz_planning::PreviewSet::Semantic {
            fixture_id,
            parameter: viz_planning::PreviewParameter::Colour,
            value: 0.0,
            // A warm white: full red and green with the blue pulled back, which reads as stage
            // tungsten rather than as a colour effect.
            colour: [1.0, 0.86, 0.62],
        });
        if matches!(fixture.patch.name.as_str(), "Gobo Demo" | "Prism Demo") {
            state.apply(viz_planning::PreviewSet::Semantic {
                fixture_id,
                parameter: viz_planning::PreviewParameter::Gobo,
                value: if fixture.patch.name == "Gobo Demo" {
                    0.42
                } else {
                    0.68
                },
                colour: [0.0; 3],
            });
        }
    }
    apply_gobo_prism_demo(&mut state, &snapshot)?;
    let projected = viz_planning::preview::project(&state, &snapshot, 1);
    Ok(projected
        .universes
        .into_iter()
        .map(|universe| {
            let mut slots = [0_u8; viz_dmx::DMX_SLOTS];
            let length = universe.slots.len().min(viz_dmx::DMX_SLOTS);
            slots[..length].copy_from_slice(&universe.slots[..length]);
            viz_dmx::UniverseFrame {
                logical_universe: universe.universe,
                slots,
                received_micros: 0,
                stale: false,
            }
        })
        .collect())
}

/// Simple Viz deliberately keeps shutter and prism out of its five planning controls. The product
/// demo therefore opens both featured fixtures and inserts the prism through their profile-owned
/// raw slots, exactly as Full DMX mode does.
fn apply_gobo_prism_demo(
    state: &mut viz_planning::preview::PreviewState,
    snapshot: &light_application::PatchSnapshot,
) -> Result<(), String> {
    for name in ["Gobo Demo", "Prism Demo"] {
        let Some(fixture) = snapshot
            .fixtures
            .iter()
            .find(|fixture| fixture.patch.name == name)
        else {
            continue;
        };
        let revision = snapshot
            .profile_revisions
            .iter()
            .find(|profile| {
                profile.profile_id == fixture.profile.profile_id
                    && profile.profile_revision == fixture.profile.profile_revision
            })
            .ok_or_else(|| format!("{name} has no embedded profile revision"))?;
        let profile: light_fixture::FixtureProfile =
            serde_json::from_value(revision.profile_snapshot.clone())
                .map_err(|error| format!("{name} profile: {error}"))?;
        let mode = profile
            .mode(fixture.profile.mode_id)
            .ok_or_else(|| format!("{name} profile has no selected mode"))?;
        let primary = mode.primary_slots().map_err(|error| error.to_string())?;
        let shutter = mode
            .channels
            .iter()
            .find(|channel| *channel.attribute.0 == *"shutter")
            .ok_or_else(|| format!("{name} mode has no shutter"))?;
        let shutter_offset = *primary
            .get(&shutter.id)
            .ok_or_else(|| "shutter channel has no primary slot".to_owned())?;
        state.apply(viz_planning::PreviewSet::Slot {
            fixture_id: fixture.patch.fixture_id.0,
            split: shutter.split,
            offset: shutter_offset,
            value: shutter.highlight_raw as u8,
        });
        if name == "Prism Demo" {
            for (attribute, value) in [("prism.1", 255), ("prism.1.rotation", 166)] {
                let channel = mode
                    .channels
                    .iter()
                    .find(|channel| &*channel.attribute.0 == attribute)
                    .ok_or_else(|| format!("{name} mode has no {attribute}"))?;
                let offset = *primary
                    .get(&channel.id)
                    .ok_or_else(|| format!("{attribute} channel has no primary slot"))?;
                state.apply(viz_planning::PreviewSet::Slot {
                    fixture_id: fixture.patch.fixture_id.0,
                    split: channel.split,
                    offset,
                    value,
                });
            }
        }
    }
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let file =
        std::fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn parse(arguments: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut arguments = arguments;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--show" => options.show = required(&mut arguments, "--show")?.into(),
            "--output" => options.output = required(&mut arguments, "--output")?.into(),
            "--width" => options.width = number(&mut arguments, "--width")?,
            "--height" => options.height = number(&mut arguments, "--height")?,
            "--frames" => options.frames = number(&mut arguments, "--frames")?,
            "--fixture" => options.fixture = Some(required(&mut arguments, "--fixture")?),
            "--settle" => options.settle = number(&mut arguments, "--settle")?,
            "--haze" => {
                options.haze = required(&mut arguments, "--haze")?
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map(|percent| percent / 100.0)
                    .map_err(|_| "--haze needs a percentage".to_owned())?;
            }
            "--step" => {
                options.step = required(&mut arguments, "--step")?
                    .parse()
                    .map_err(|_| "--step needs a number of seconds".to_owned())?;
            }
            "--view" => {
                let name = required(&mut arguments, "--view")?;
                options.view = match name.as_str() {
                    "full3d" => ViewMode::Full3d,
                    "simple3d" => ViewMode::Simple3d,
                    "lines3d" => ViewMode::Lines3d,
                    "top-down" => ViewMode::TopDown,
                    "left-to-right" => ViewMode::LeftToRight,
                    "right-to-left" => ViewMode::RightToLeft,
                    "front-to-back" => ViewMode::FrontToBack,
                    "back-to-front" => ViewMode::BackToFront,
                    other => return Err(format!("{other} is not a view")),
                };
            }
            "--quality" => {
                let name = required(&mut arguments, "--quality")?;
                options.quality = RenderQuality::from_wire(&name)
                    .ok_or_else(|| format!("{name} is not a render quality"))?;
            }
            other => return Err(format!("{other} is not an option this tool takes")),
        }
    }
    if options.show.as_os_str().is_empty() {
        return Err("--show is required".to_owned());
    }
    if options.output.as_os_str().is_empty() {
        return Err("--output is required".to_owned());
    }
    Ok(Some(options))
}

fn required(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn number(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<u32, String> {
    required(arguments, flag)?
        .parse()
        .map_err(|_| format!("{flag} needs a whole number"))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod profile_moving_light {
    use std::path::{Path, PathBuf};
    fn repository() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }
    /// TL-68's deliberate exception, guarded.
    ///
    /// The plan preserves *both* profile-moving-light implementations — the native renderer's
    /// generic GLB and the desk web Stage's procedural body — because the desk version is
    /// preferred visually today but has not been selected as the final shared model. Neither may
    /// be deleted, overwritten, or made unrecoverable during consolidation; they stay side by side
    /// until somebody decides between them.
    ///
    /// Consolidation is exactly the kind of work that tidies away "the one we are not using", so
    /// the requirement is worth a test rather than a paragraph.
    #[test]
    fn both_profile_moving_light_implementations_survive_consolidation() {
        let native = repository().join("assets/models/lamps/moving-head-profile.glb");
        assert!(
            native.is_file(),
            "the native renderer's generic profile-moving-head GLB is gone: {}",
            native.display()
        );
        let procedural = repository().join("apps/light-desktop/src/windows/builtInStageModels.ts");
        let source = std::fs::read_to_string(&procedural).unwrap_or_else(|_| {
            panic!(
                "the desk's built-in stage models are gone: {}",
                procedural.display()
            )
        });
        assert!(
            source.contains("moving-yoke"),
            "the desk web Stage's procedural profile-moving light is gone; the plan keeps both \
             until the choice between them is made"
        );
    }
    /// Desk, PreViz, and capture use one committed portable show rather than parallel demo rigs.
    #[test]
    fn the_demo_show_has_one_canonical_asset() {
        let canonical = repository().join("assets/demo.show");
        assert!(
            canonical.is_file(),
            "the canonical demo show is missing: {}",
            canonical.display()
        );
        assert!(
            !Path::new(&repository().join("crates/viz/demo/src/rig.rs")).exists(),
            "a second Rust demo rig would let capture drift from Desk and PreViz"
        );
    }
}
