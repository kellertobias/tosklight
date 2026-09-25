//! A headless gallery of the generated Venue objects, for judging their materials and rigging.
//!
//! The demo show supplies the lamps and the light; every truss section, a chain on each kind, a
//! drape and a stage element are added in front of them and rendered from a few fixed cameras.
//! Run it whenever the scenery materials or the chain fixings change:
//!
//! ```sh
//! cargo run --release -p viz-capture --example scenery_gallery -- .artifacts/tmp/gallery
//! ```

use std::path::{Path, PathBuf};
use viz_scene::glam::Vec3;
use viz_scene::{
    ChainRig, RiserFeet, Scene, SceneValues, SceneryDetail, SceneryKind, SceneryObject,
    ViewConfiguration,
};

fn main() {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| ".artifacts/tmp/gallery".to_owned()),
    );
    if let Err(message) = run(&output) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run(output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let show = Path::new("assets/demo.show");
    let staged = output.join("demo-show.show");
    std::fs::copy(show, &staged).map_err(|error| error.to_string())?;
    let mut scene = scene_from(&staged)?;

    // The demo rig: profile lamps at 4 m along z = -4 and z = 0 pointing down, with a truss
    // segment over each row. The gallery stands beneath them so it is lit.
    let mut objects = Vec::new();
    let truss = |x: f32, y: f32, z: f32, length: f32, chords: u8, roll: f32| SceneryObject {
        name: format!("{chords}-point truss"),
        position: Vec3::new(x, y, z),
        rotation_degrees: Vec3::new(roll, 0.0, 0.0),
        size: Vec3::new(length, 0.29, 0.29),
        colour: [0.33, 0.335, 0.345],
        roughness: 0.6,
        kind: SceneryKind::Truss,
        chords,
        ..SceneryObject::default()
    };
    let chain = |x: f32, top: f32, bottom: f32, z: f32, rig: ChainRig| SceneryObject {
        name: "chain".to_owned(),
        position: Vec3::new(x, (top + bottom) * 0.5, z),
        size: Vec3::new(0.05, top - bottom, 0.05),
        colour: [0.32, 0.32, 0.33],
        roughness: 0.45,
        kind: SceneryKind::Chain,
        detail: SceneryDetail {
            chain: rig,
            ..SceneryDetail::default()
        },
        ..SceneryObject::default()
    };
    // One of every section in a line under the mid lamps, each with a chain on it: pipe,
    // ladder, triangle apex up, triangle apex down, box; and a climbing hoist under the box.
    let (y, row_z) = (2.6, -2.0);
    let sections: [(u8, f32); 5] = [(1, 0.0), (2, 0.0), (3, 0.0), (3, 180.0), (4, 0.0)];
    for (index, (chords, roll)) in sections.into_iter().enumerate() {
        let x = -4.0 + index as f32 * 2.0;
        objects.push(truss(x, y, row_z, 1.5, chords, roll));
        // Hung at working height: the shackle a chord spacing above the top chords.
        objects.push(chain(
            x,
            y + 1.9,
            y + 0.145 + 0.2,
            row_z,
            ChainRig::MotorTop,
        ));
    }
    objects.push(chain(
        4.0,
        y - 0.145 - 0.2,
        y - 1.6,
        row_z,
        ChainRig::MotorBottom,
    ));
    // Stage elements under the front lamps, and drapes hanging under the back row.
    let stage_z = 3.6;
    objects.push(SceneryObject {
        name: "deck".to_owned(),
        position: Vec3::new(-2.2, 0.3, stage_z),
        size: Vec3::new(2.0, 0.6, 1.0),
        colour: [0.036, 0.019, 0.009],
        roughness: 0.8,
        kind: SceneryKind::Riser,
        detail: SceneryDetail {
            feet: RiserFeet::Fixed,
            ..SceneryDetail::default()
        },
        ..SceneryObject::default()
    });
    objects.push(SceneryObject {
        name: "lift".to_owned(),
        position: Vec3::new(0.0, 0.4, stage_z),
        size: Vec3::new(2.0, 0.8, 1.0),
        colour: [0.036, 0.019, 0.009],
        roughness: 0.8,
        kind: SceneryKind::Riser,
        detail: SceneryDetail {
            feet: RiserFeet::Scissor,
            ..SceneryDetail::default()
        },
        ..SceneryObject::default()
    });
    objects.push(SceneryObject {
        name: "stairs".to_owned(),
        position: Vec3::new(1.8, 0.3, stage_z),
        size: Vec3::new(1.0, 0.6, 1.0),
        colour: [0.036, 0.019, 0.009],
        roughness: 0.8,
        kind: SceneryKind::Stairs,
        ..SceneryObject::default()
    });
    let drape_z = -4.45;
    // The black drape hangs clear of the beams, where only the room lights it.
    objects.push(SceneryObject {
        name: "drape".to_owned(),
        position: Vec3::new(-1.5, 2.0, drape_z - 2.0),
        size: Vec3::new(5.0, 3.2, 0.12),
        colour: [0.008, 0.008, 0.01],
        roughness: 0.96,
        kind: SceneryKind::Curtain,
        ..SceneryObject::default()
    });
    objects.push(SceneryObject {
        name: "red drape".to_owned(),
        position: Vec3::new(2.5, 2.0, drape_z),
        size: Vec3::new(3.0, 3.2, 0.12),
        colour: [0.3, 0.02, 0.02],
        roughness: 0.96,
        kind: SceneryKind::Curtain,
        ..SceneryObject::default()
    });
    scene.scenery.extend(objects);
    scene.recompute_bounds();

    let mut values = SceneValues::default();
    values.resize(scene.emitters.len());
    values.atmosphere.density = 0.02;
    for emitter in &mut values.emitters {
        emitter.intensity = 1.0;
        emitter.colour = [1.0, 0.86, 0.62];
        emitter.pan = 0.5;
        emitter.tilt = 0.5;
        emitter.zoom = 0.6;
    }

    let mut renderer = viz_render::Renderer::headless(1600, 1000)?;
    let overlay = viz_render::Overlay::default();
    let close = |x: f32| {
        (
            Vec3::new(x + 1.1, y + 0.95, row_z + 1.3),
            Vec3::new(x, y + 0.35, row_z),
            30.0,
        )
    };
    let (pipe_at, pipe_to, pipe_fov) = close(-4.0);
    let (ladder_at, ladder_to, ladder_fov) = close(-2.0);
    let (apex_up_at, apex_up_to, apex_up_fov) = close(0.0);
    let (apex_down_at, apex_down_to, apex_down_fov) = close(2.0);
    let (box_at, box_to, box_fov) = close(4.0);
    let cameras = [
        (
            "sections",
            Vec3::new(0.0, 3.4, 4.5),
            Vec3::new(0.0, 2.5, row_z),
            40.0,
        ),
        ("chain-pipe", pipe_at, pipe_to, pipe_fov),
        ("chain-ladder", ladder_at, ladder_to, ladder_fov),
        ("chain-triangle-up", apex_up_at, apex_up_to, apex_up_fov),
        (
            "chain-triangle-down",
            apex_down_at,
            apex_down_to,
            apex_down_fov,
        ),
        ("chain-box", box_at, box_to, box_fov),
        (
            "chain-climbing",
            Vec3::new(5.1, y - 0.6, row_z + 1.3),
            Vec3::new(4.0, y - 0.3, row_z),
            30.0,
        ),
        (
            "lamps-through-truss",
            Vec3::new(0.0, 3.3, 6.0),
            Vec3::new(0.0, 4.0, -4.0),
            30.0,
        ),
        (
            "occlusion",
            Vec3::new(0.0, 1.0, 0.2),
            Vec3::new(0.0, 4.0, -4.0),
            16.0,
        ),
        (
            "lamps-close",
            Vec3::new(1.5, 3.2, 2.5),
            Vec3::new(0.0, 4.0, 0.0),
            30.0,
        ),
        (
            "stage",
            Vec3::new(0.0, 2.0, stage_z + 4.5),
            Vec3::new(0.0, 0.4, stage_z),
            35.0,
        ),
        (
            "deck-close",
            Vec3::new(-1.2, 1.3, stage_z + 1.6),
            Vec3::new(-2.0, 0.55, stage_z),
            30.0,
        ),
        (
            "drapes",
            Vec3::new(4.5, 2.4, drape_z + 2.2),
            Vec3::new(1.0, 1.6, drape_z),
            40.0,
        ),
        (
            "black-drape",
            Vec3::new(1.5, 2.2, drape_z - 0.3),
            Vec3::new(-1.5, 1.6, drape_z - 2.0),
            40.0,
        ),
    ];
    for (name, position, target, fov) in cameras {
        let mut view = ViewConfiguration {
            quality: viz_scene::RenderQuality::High,
            // A lit room rather than a blackout, so the materials are judged rather than the dark.
            ambient: 0.2,
            ..ViewConfiguration::default()
        };
        view.camera.position = position;
        view.camera.target = target;
        view.camera.up = Vec3::Y;
        view.camera.fov_degrees = fov;
        for frame in 0..3 {
            renderer
                .capture(&scene, &values, &view, &overlay, frame as f32 / 30.0)
                .map_err(|error| error.to_string())?;
        }
        let image = renderer
            .capture(&scene, &values, &view, &overlay, 0.1)
            .map_err(|error| error.to_string())?;
        let path = output.join(format!("{name}.png"));
        write_png(&path, image.width, image.height, &image.rgba)?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

fn scene_from(path: &Path) -> Result<Scene, String> {
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
        show_name: "Gallery".to_owned(),
        server_identity: path.display().to_string(),
    };
    Ok(viz_desk::build(&models).scene)
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
