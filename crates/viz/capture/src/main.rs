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
  --settle N        Frames rendered and discarded first, so time-based motion has run (default 30).
  --haze PERCENT    Haze the beams are drawn through (default 50).
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
    apply_prism_demo(&mut state, &snapshot)?;
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

/// Fixture 512 is the canonical demo's prism evidence. Simple Viz preview deliberately exposes
/// only Gobo, so the product capture sets the prism through the same fixture-relative raw slot as
/// Full DMX mode. The profile remains authoritative for that slot and its resolution.
fn apply_prism_demo(
    state: &mut viz_planning::preview::PreviewState,
    snapshot: &light_application::PatchSnapshot,
) -> Result<(), String> {
    let Some(fixture) = snapshot.fixtures.iter().find(|fixture| {
        fixture.patch.fixture_number == Some(512) && fixture.patch.name == "Prism Demo"
    }) else {
        return Ok(());
    };
    let revision = snapshot
        .profile_revisions
        .iter()
        .find(|profile| {
            profile.profile_id == fixture.profile.profile_id
                && profile.profile_revision == fixture.profile.profile_revision
        })
        .ok_or_else(|| "Prism Demo has no embedded profile revision".to_owned())?;
    let profile: light_fixture::FixtureProfile =
        serde_json::from_value(revision.profile_snapshot.clone())
            .map_err(|error| format!("Prism Demo profile: {error}"))?;
    let mode = profile
        .mode(fixture.profile.mode_id)
        .ok_or_else(|| "Prism Demo profile has no selected mode".to_owned())?;
    let primary = mode.primary_slots().map_err(|error| error.to_string())?;
    for (attribute, value) in [("prism.1", 255), ("prism.1.rotation", 166)] {
        let channel = mode
            .channels
            .iter()
            .find(|channel| channel.attribute.0 == attribute)
            .ok_or_else(|| format!("Prism Demo mode has no {attribute}"))?;
        let offset = *primary
            .get(&channel.id)
            .ok_or_else(|| format!("Prism Demo {attribute} has no primary slot"))?;
        state.apply(viz_planning::PreviewSet::Slot {
            fixture_id: fixture.patch.fixture_id.0,
            split: channel.split,
            offset,
            value,
        });
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
mod tests {
    use super::*;
    use viz_scene::{
        BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
        FixtureInstance, RenderQuality, SceneryKind, SceneryObject,
    };

    fn two_light_surface_scene() -> (Scene, SceneValues, ViewConfiguration) {
        use viz_scene::{glam::Vec3, uuid::Uuid};

        let fixture = |name: &str, position: Vec3| FixtureInstance {
            instance_id: Uuid::new_v4(),
            fixture_id: Uuid::new_v4(),
            name: name.to_owned(),
            number: None,
            position,
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody::default(),
            patched: true,
            address: None,
            model: None,
            fallback: None,
        };
        let emitter = |fixture_index| EmitterInstance {
            fixture_index,
            head_index: 0,
            label: "Main".to_owned(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 12.0,
            field_angle_degrees: 28.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        };

        // Light zero is deliberately far outside the shot. Light one illuminates the receiving
        // deck in front of the camera. A storage-array stride error makes culling light one from
        // fields in the middle of light zero, which removes this pool while leaving a plausible
        // fixture aperture and beam volume behind.
        let mut scene = Scene {
            fixtures: vec![
                fixture("Offscreen", Vec3::new(-20.0, 4.0, 0.0)),
                fixture("Surface", Vec3::new(6.0, 4.0, 0.0)),
            ],
            emitters: vec![emitter(0), emitter(1)],
            scenery: vec![SceneryObject {
                id: Uuid::new_v4(),
                name: "Receiving deck".to_owned(),
                position: Vec3::new(6.0, -0.05, 0.0),
                rotation_degrees: Vec3::ZERO,
                size: Vec3::new(4.0, 0.1, 4.0),
                colour: [0.35, 0.35, 0.35],
                roughness: 0.8,
                kind: SceneryKind::Riser,
                chords: 0,
            }],
            ..Scene::default()
        };
        scene.recompute_bounds();

        let mut values = SceneValues::default();
        values.resize(2);
        values.atmosphere.density = 0.0;
        for value in &mut values.emitters {
            value.intensity = 1.0;
            value.held_intensity = 1.0;
        }

        let mut view = ViewConfiguration::default();
        view.camera.position = Vec3::new(6.0, 4.5, 7.0);
        view.camera.target = Vec3::new(6.0, 0.0, 0.0);
        view.ambient = 0.0;
        view.show_labels = false;
        view.floor_grid = false;
        (scene, values, view)
    }

    fn picture_brightness(image: &viz_render::CapturedImage) -> f64 {
        let total: u64 = image
            .rgba
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
            .sum();
        total as f64 / (image.rgba.len() / 4) as f64
    }

    /// The generated demo show, built exactly as a capture builds it.
    fn demo_scene_and_values(name: &str) -> (Scene, SceneValues) {
        let directory = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
            PathBuf::from,
        );
        // One workspace per test: these run in parallel and each builds its own library.
        let workspace = directory.join("viz-capture-golden").join(name);
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("workspace");
        let library = light_fixture::FixtureLibrary::open(workspace.join("fixtures.sqlite"))
            .expect("library");
        library
            .load_fixture_package_directory(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library"),
            )
            .expect("the shipped packages load");
        let show = workspace.join("demo-show.show");
        viz_demo::generate(library, &show).expect("the demo generates");
        let (scene, bindings, preview) = scene_from(&show).expect("the scene builds");
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        let mut decoder = viz_project::Decoder::new(bindings);
        decoder.apply(&scene, &preview, &mut values, 0.0);
        (scene, values)
    }

    fn demo_scene(name: &str) -> Scene {
        demo_scene_and_values(name).0
    }

    fn body_of(scene: &Scene, name: &str) -> BodyKind {
        scene
            .fixtures
            .iter()
            .find(|fixture| fixture.name.starts_with(name))
            .unwrap_or_else(|| panic!("the demo rig has no fixture named {name}"))
            .body
            .kind
    }

    /// The surface pool from a light after index zero must survive quality changes, small camera
    /// moves and repeated redraws. This is the actual operator path: the culling compute shader
    /// chooses the lights evaluated by the surface shader, while the beam pass is independent and
    /// can otherwise make the broken frame look superficially alive.
    #[test]
    fn a_second_light_keeps_its_surface_pool_across_views_and_quality_tiers() {
        let (scene, values, mut view) = two_light_surface_scene();
        let overlay = viz_render::Overlay::default();
        let mut renderer = viz_render::Renderer::headless(320, 180)
            .expect("a headless renderer; this machine has no GPU or software adapter");

        let mut reference = values.clone();
        reference.emitters[1].intensity = 0.0;
        reference.emitters[1].held_intensity = 0.0;
        let dark_image = renderer
            .capture(&scene, &reference, &view, &overlay, 0.0)
            .expect("the receiving surface without its light");
        let dark = picture_brightness(&dark_image);
        let evidence = std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from);
        if let Some(directory) = evidence.as_deref() {
            std::fs::create_dir_all(directory).expect("capture evidence directory");
            write_png(
                &directory.join("tl-202-surface-dark.png"),
                dark_image.width,
                dark_image.height,
                &dark_image.rgba,
            )
            .expect("dark surface evidence");
        }

        let base_position = view.camera.position;
        for quality in RenderQuality::ALL {
            view.quality = quality;
            for camera_offset in [-0.3_f32, 0.3] {
                view.camera.position.x = base_position.x + camera_offset;
                let first = renderer
                    .capture(&scene, &values, &view, &overlay, 1.0)
                    .expect("the active pool renders");
                let repeated = renderer
                    .capture(&scene, &values, &view, &overlay, 1.0)
                    .expect("the same active pool renders again");
                let first_brightness = picture_brightness(&first);
                let repeated_brightness = picture_brightness(&repeated);
                if quality == RenderQuality::Ultra
                    && camera_offset > 0.0
                    && let Some(directory) = evidence.as_deref()
                {
                    write_png(
                        &directory.join("tl-202-surface-lit.png"),
                        repeated.width,
                        repeated.height,
                        &repeated.rgba,
                    )
                    .expect("lit surface evidence");
                }
                assert!(
                    first_brightness > dark + 4.0,
                    "{} at camera offset {camera_offset} lost the surface pool: dark {dark:.2}, lit {first_brightness:.2}",
                    quality.label()
                );
                assert!(
                    (first_brightness - repeated_brightness).abs() < 0.25,
                    "{} changed on an identical redraw: {first_brightness:.2} to {repeated_brightness:.2}",
                    quality.label()
                );
            }
        }
    }

    #[test]
    fn a_named_fixture_capture_frames_the_fixture_body() {
        let scene = demo_scene("fixture-framing");
        let bounds = fixture_bounds(&scene, "Sunstrip").expect("the demo carries Sunstrips");
        assert!(
            bounds.extent().x > 0.5,
            "the one-metre extrusion is in frame"
        );
        assert!(
            fixture_bounds(&scene, "Not in this show").is_err(),
            "a typo must fail rather than silently capture the whole rig"
        );
    }

    #[test]
    fn the_capture_look_decodes_visible_gobo_and_prism_beams() {
        let (scene, values) = demo_scene_and_values("gobo-prism-look");
        let emitter_values = |name: &str| {
            let fixture_index = scene
                .fixtures
                .iter()
                .position(|fixture| fixture.name == name)
                .unwrap_or_else(|| panic!("the demo has no {name}"));
            scene
                .emitters
                .iter()
                .enumerate()
                .filter(|(_, emitter)| emitter.fixture_index == fixture_index as u32)
                .map(|(index, _)| &values.emitters[index])
                .collect::<Vec<_>>()
        };
        let gobo = emitter_values("Gobo Demo");
        assert!(
            gobo.iter()
                .any(|value| { value.visible_intensity() > 0.99 && value.gobo_slot(8) > 0 })
        );
        let prism = emitter_values("Prism Demo");
        assert!(prism.iter().any(|value| {
            value.visible_intensity() > 0.99 && value.gobo_slot(8) > 0 && value.prism_facets() >= 3
        }));
    }

    #[test]
    fn the_shipped_sunstrip_has_ten_cells_inside_its_extrusion() {
        let scene = demo_scene("sunstrip-face-bounds");
        let (fixture_index, fixture) = scene
            .fixtures
            .iter()
            .enumerate()
            .find(|(_, fixture)| fixture.name == "Sunstrip 1")
            .expect("the representative strip");
        let emitters: Vec<_> = scene
            .emitters
            .iter()
            .filter(|emitter| emitter.fixture_index == fixture_index as u32)
            .collect();
        assert_eq!(
            emitters.len(),
            10,
            "the shared master is not an eleventh cell"
        );

        let left = emitters
            .iter()
            .map(|emitter| emitter.local_origin.x - emitter.optics.source.width * 0.5)
            .fold(f32::INFINITY, f32::min);
        let right = emitters
            .iter()
            .map(|emitter| emitter.local_origin.x + emitter.optics.source.width * 0.5)
            .fold(f32::NEG_INFINITY, f32::max);
        let body_half = fixture.body.size.x * 0.5;
        assert!(left >= -body_half - 1e-6, "left cell overhangs: {left}");
        assert!(right <= body_half + 1e-6, "right cell overhangs: {right}");
    }

    /// The golden scene the plan asks for: every class the renderer has to draw, resolving to the
    /// body it is meant to resolve to rather than merely to something visible.
    ///
    /// A fixture package that loses its type, or a fallback that starts answering `Generic`, turns
    /// a Sunstrip into a box or a scanner into a lantern — visible in a capture, and invisible in
    /// a test that only counted fixtures.
    #[test]
    fn every_demo_fixture_class_resolves_to_its_intended_body() {
        let scene = demo_scene("bodies");
        for (fixture, expected) in [
            // Moving fixtures are bodies with a yoke, whatever else they are.
            ("Wash", BodyKind::MovingHead),
            ("Profile", BodyKind::MovingHead),
            ("Beam", BodyKind::MovingHead),
            // Bars of cells.
            ("Sunstrip", BodyKind::Bar),
            ("Strobe", BodyKind::Bar),
            ("Blinder", BodyKind::Bar),
            ("JDC1", BodyKind::Bar),
            // Static lanterns on a clamp.
            ("FOH", BodyKind::Lantern),
            ("PAR", BodyKind::Lantern),
            ("ACL", BodyKind::Lantern),
            ("Fresnel", BodyKind::Lantern),
            // Machines that make no light of their own, and the laser, which is its own thing.
            ("Hazer", BodyKind::Machine),
            ("Laser", BodyKind::Machine),
        ] {
            assert_eq!(
                body_of(&scene, fixture),
                expected,
                "{fixture} resolved to the wrong body"
            );
        }
    }

    /// The determinism the demo video depends on.
    ///
    /// CI composites a video from separate capture runs, so the same commit and the same arguments
    /// have to produce the same picture. Everything that could vary is stated rather than measured
    /// — resolution, camera, time step, haze — and auto-exposure adapts over time, which is why a
    /// capture settles first.
    ///
    /// It is not bit-exact, and measuring showed why: two runs differ on a few dozen bytes of a
    /// 230,400-byte frame, each by a single value. Settling longer shrinks it (38 bytes at 8
    /// frames, 13 at 60) without reaching zero, so it is not exposure converging — it is the GPU
    /// resolving multisampled coverage in whatever order it likes. So the guarantee is stated as
    /// what the video actually needs: nothing moves perceptibly between runs.
    #[test]
    fn two_capture_runs_of_the_same_frame_are_identical() {
        let scene = demo_scene("determinism");
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::Full3d;
        view.camera = viz_scene::Camera::framed(ViewMode::Full3d, scene.bounds);
        let overlay = viz_render::Overlay::default();
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        values.atmosphere.density = viz_scene::DEFAULT_DENSITY;

        let capture_once = || {
            let mut renderer = viz_render::Renderer::headless(320, 180)
                .expect("a headless renderer; this machine has no GPU or software adapter");
            for frame in 0..60 {
                renderer
                    .capture(&scene, &values, &view, &overlay, frame as f32 / 30.0)
                    .expect("settling frame");
            }
            renderer
                .capture(&scene, &values, &view, &overlay, 60.0 / 30.0)
                .expect("recorded frame")
                .rgba
        };

        let first = capture_once();
        let second = capture_once();
        assert_eq!(first.len(), second.len(), "the two runs differ in size");
        let deltas: Vec<u8> = first
            .iter()
            .zip(&second)
            .map(|(left, right)| left.abs_diff(*right))
            .filter(|delta| *delta > 0)
            .collect();
        let worst = deltas.iter().copied().max().unwrap_or(0);
        // Not bit-exact, and it cannot be: a GPU is free to vary the order it resolves
        // multisampled coverage in, so a handful of pixels land a single value apart between runs.
        // What matters for a composited video is that nothing moves perceptibly, so the bound is
        // stated in those terms instead of pretending to an exactness the hardware does not offer.
        assert!(
            worst <= 1,
            "{} bytes differ between two runs of the same frame, the worst by {worst}; that is \
             more than rounding and a composited demo video would flicker",
            deltas.len()
        );
        assert!(
            deltas.len() * 1_000 < first.len(),
            "{} of {} bytes differ between two runs — under a tenth of a percent is rounding, \
             this is something moving",
            deltas.len(),
            first.len()
        );
    }

    /// The whole dimmer has to be worth moving.
    ///
    /// The Stage used to adapt its exposure to how much light the rig was producing, the way an
    /// eye does. On a rig that is the wrong instinct: taking the fixtures down opened the exposure
    /// by nearly as much as they had closed, so the first tenth of a fade was the entire visible
    /// change and the top nine tenths drew the same picture. What an operator needs from a Stage
    /// is the opposite — dim has to look dim, and the difference between half and full has to be
    /// as plain as the difference between out and a tenth.
    #[test]
    fn the_picture_keeps_getting_brighter_all_the_way_up_the_dimmer() {
        let scene = demo_scene("exposure");
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::Full3d;
        view.camera = viz_scene::Camera::framed(ViewMode::Full3d, scene.bounds);
        let overlay = viz_render::Overlay::default();
        let mut renderer = viz_render::Renderer::headless(320, 180)
            .expect("a headless renderer; this machine has no GPU or software adapter");

        let mut brightness_at = |level: f32| {
            let mut values = SceneValues::default();
            values.resize(scene.emitters.len());
            values.atmosphere.density = viz_scene::DEFAULT_DENSITY;
            for emitter in &mut values.emitters {
                emitter.intensity = level;
                emitter.held_intensity = level;
            }
            let frame = renderer
                .capture(&scene, &values, &view, &overlay, 1.0)
                .expect("a captured frame");
            let total: u64 = frame
                .rgba
                .chunks_exact(4)
                .map(|pixel| u64::from(pixel[0]) + u64::from(pixel[1]) + u64::from(pixel[2]))
                .sum();
            total as f64 / (frame.rgba.len() / 4) as f64
        };

        let levels = [0.0_f32, 0.1, 0.25, 0.5, 1.0];
        let measured: Vec<f64> = levels.iter().copied().map(&mut brightness_at).collect();
        for pair in measured.windows(2) {
            assert!(
                pair[1] > pair[0],
                "every step up the dimmer is brighter than the one below it: {measured:?}"
            );
        }

        // The half-to-full step is the one that used to vanish. It has to be a real share of the
        // range rather than the rounding it became.
        let range = measured[4] - measured[0];
        let top_step = measured[4] - measured[3];
        assert!(
            top_step > range * 0.12,
            "half to full is {top_step:.2} of a {range:.2} range, which is a fader with nothing \
             in its top half: {measured:?}"
        );

        // And the bottom of the fader must not already be most of the picture.
        let bottom_step = measured[1] - measured[0];
        assert!(
            bottom_step < range * 0.5,
            "out to a tenth is {bottom_step:.2} of a {range:.2} range, so the fade happens \
             entirely in the bottom of the fader: {measured:?}"
        );
    }

    /// Exact fixture models live in transferable packages and the audited built-in set covers a
    /// profile that carries none. Whichever route supplied it, every fixture must resolve to some
    /// geometry: a fixture with no model draws as nothing, which in a still capture is
    /// indistinguishable from a fixture that is simply unlit.
    #[test]
    fn every_demo_fixture_resolves_to_geometry() {
        let scene = demo_scene("models");
        let shapeless: Vec<&str> = scene
            .fixtures
            .iter()
            .filter(|fixture| fixture.model.is_none())
            .map(|fixture| fixture.name.as_str())
            .collect();
        assert!(
            shapeless.is_empty(),
            "these fixtures resolved to no model at all and would draw as nothing: {shapeless:?}"
        );
    }

    /// A scanner classifies as a lantern, and that is correct rather than a gap.
    ///
    /// `BodyKind` is assigned by whether pan and tilt are on the body, and a scanner is a
    /// mirror-mover whose body is bolted down — so it is not `MovingHead`. That looked like a
    /// defect until the drawing path settled it: `BodyKind` only selects the *procedural* geometry
    /// used when a fixture has no model. The scanner has one — `lamps/scanner-mirror-spot.glb`,
    /// matched by `default_model.rs` on "scanner" or "mirror" — and `push_model` articulates model
    /// parts from the fixture's head angles by each part's own declared kind, whatever the body
    /// says. So the mesh is right and the mirror moves.
    ///
    /// Pinned so that a scanner losing its model, and falling back to a procedural lantern that
    /// cannot articulate, is caught here rather than seen in a demo video.
    #[test]
    fn a_scanner_has_no_body_of_its_own_yet() {
        assert_eq!(
            body_of(&demo_scene("scanner"), "Scanner"),
            BodyKind::Lantern
        );
    }

    /// Nothing in the demo rig may fall through to the shapeless default: `Generic` means the
    /// projection could not tell what the fixture was, which is the failure this guards.
    #[test]
    fn no_demo_fixture_falls_through_to_a_shapeless_body() {
        let scene = demo_scene("shapeless");
        let shapeless: Vec<&str> = scene
            .fixtures
            .iter()
            .filter(|fixture| fixture.body.kind == BodyKind::Generic)
            .map(|fixture| fixture.name.as_str())
            .collect();
        assert!(
            shapeless.is_empty(),
            "these fixtures resolved to no recognisable body: {shapeless:?}"
        );
    }

    /// A capture of the demo has to have something to light. An empty or unpatched rig renders a
    /// dark stage, which is indistinguishable from a broken renderer in a still frame.
    #[test]
    fn the_demo_scene_carries_emitters_for_every_fixture() {
        let scene = demo_scene("emitters");
        assert!(!scene.fixtures.is_empty(), "the demo rig is empty");
        assert!(
            scene.emitters.len() >= scene.fixtures.len(),
            "{} fixtures produced only {} emitters",
            scene.fixtures.len(),
            scene.emitters.len()
        );
    }
}

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

    /// The demo show is regenerated from source on every build, so its embedded profile revisions
    /// are always this commit's. That is what the plan asks for in place of a migration: a
    /// repository-owned show cannot carry a stale model if it is rebuilt rather than stored.
    #[test]
    fn the_demo_show_is_generated_rather_than_stored() {
        let stored = repository().join("assets/demo-show.show");
        assert!(
            !stored.exists(),
            "a committed demo show would embed profile revisions that age; it is generated by \
             `npm run demo-show` instead"
        );
        assert!(
            Path::new(&repository().join("crates/viz/demo/src/rig.rs")).is_file(),
            "the demo rig is declared in source, which is what makes regeneration possible"
        );
    }
}
