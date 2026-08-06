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
use viz_scene::{Scene, SceneValues, ViewConfiguration, ViewMode};

const USAGE: &str = "viz-capture --show FILE --output DIR [options]

  --show FILE       Show file to render. The generated demo show is the usual one.
  --output DIR      Where the PNG frames are written. Created if absent.
  --width  N        Frame width in pixels (default 1920).
  --height N        Frame height in pixels (default 1080).
  --frames N        How many frames to write (default 1).
  --step SECONDS    Scene time between frames (default 1/30).
  --settle N        Frames rendered and discarded first, so exposure has adapted (default 30).
  --haze PERCENT    Haze the beams are drawn through (default 50).
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
    // The camera is framed from the rig rather than left wherever a previous session put it, so
    // the same show always yields the same shot.
    view.camera = viz_scene::Camera::framed(options.view, scene.bounds);
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

    // Exposure adapts over time by design, so a capture settles first and only then records.
    // Without this the first frame of every run is a different brightness from the rest.
    for frame in 0..options.settle {
        let seconds = frame as f32 * options.step;
        renderer
            .capture(&scene, &values, &view, &overlay, seconds)
            .map_err(|error| format!("settling frame {frame}: {error}"))?;
    }

    let mut written = 0;
    for frame in 0..options.frames {
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
    }
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
