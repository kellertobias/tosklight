//! Deterministic fake planning-software adapter.
//!
//! It produces the same semantic snapshot, entity delta, value batch, and view change sequence
//! every run without a socket, which is what lets the provider boundary be tested: the same
//! events through the desk adapter and this adapter must build the same renderer scene.

use glam::Vec3;
use viz_scene::{
    Aabb, BodyKind, ConnectionState, CrowdArea, CrowdDensity, CrowdPosture, EmitterInstance,
    EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody, FixtureInstance, LightSource,
    MotionAxis, ProviderCapabilities, ProviderEvent, ProviderKind, Scene, SceneProvider,
    SceneValues, SceneryKind, SceneryObject, SourceForm, uuid::Uuid,
};

pub struct DemoProvider {
    scene: Scene,
    values: SceneValues,
    frame: u64,
    emitted_snapshot: bool,
}

impl Default for DemoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoProvider {
    pub fn new() -> Self {
        let mut scene = build_scene();
        scene.recompute_bounds();
        let mut values = SceneValues::default();
        values.resize(scene.emitters.len());
        Self {
            scene,
            values,
            frame: 0,
            emitted_snapshot: false,
        }
    }

    fn advance(&mut self) {
        self.frame += 1;
        let time = self.frame as f32 / 60.0;
        for (index, emitter) in self.scene.emitters.iter().enumerate() {
            let value = &mut self.values.emitters[index];
            let phase = index as f32 * 0.7;
            match emitter.kind {
                EmitterKind::Atmosphere => {
                    value.intensity = 0.55;
                }
                // The scan path itself comes from the fixture's own engine, which runs in the
                // application. All the built-in look decides is that the projector is on.
                EmitterKind::Laser => {
                    value.intensity = 1.0;
                    value.colour = [1.0, 1.0, 1.0];
                }
                EmitterKind::Effect => {
                    value.intensity = 1.0;
                }
                EmitterKind::Beam => {
                    value.intensity = 0.35 + 0.65 * (time * 0.7 + phase).sin().abs();
                    value.pan = 0.5 + 0.28 * (time * 0.45 + phase).sin();
                    value.tilt = 0.42 + 0.16 * (time * 0.31 + phase * 1.3).cos();
                    value.zoom = 0.25 + 0.2 * (time * 0.2 + phase).sin().abs();
                    value.colour = hue(time * 0.12 + index as f32 * 0.11);
                    // Every optic the renderer supports is on some head in this scene, so the
                    // built-in look is enough to see gobos, prisms, shutters and frost working
                    // without a desk, a show file, or a fixture that happens to carry them.
                    // Every head sits on a different slot of the demo wheel, open included, so
                    // one picture shows the whole wheel at once.
                    let slots = DEMO_GOBOS.len() as f32 + 1.0;
                    value.gobo = match index % (DEMO_GOBOS.len() + 1) {
                        0 => 0.0,
                        other => (other as f32 + 0.5) / slots,
                    };
                    value.gobo_rotation = (time * 0.08 + index as f32 * 0.2).fract();
                    value.prism = if index % 7 == 3 { 0.4 } else { 0.0 };
                    value.prism_rotation = (time * 0.05).fract();
                    value.frost = if index % 11 == 5 { 0.6 } else { 0.0 };
                    value.focus = 0.5 + 0.28 * (time * 0.17 + phase).sin();
                    value.iris = if index % 9 == 4 {
                        0.35 + 0.3 * (time * 0.3).sin().abs()
                    } else {
                        0.0
                    };
                    if index % 13 == 6 {
                        let insertion = 0.25 + 0.2 * (time * 0.25 + phase).sin().abs();
                        value.shaper_blades = [insertion, 0.0, insertion * 0.7, 0.0];
                        value.shaper_rotation = (time * 0.03).fract();
                    }
                }
                EmitterKind::Emissive => {
                    value.intensity = 0.6 + 0.4 * (time * 1.4 + phase).sin();
                    value.colour = hue(time * 0.2 + index as f32 * 0.23);
                    if !emitter.cells.offsets.is_empty() {
                        value.cells = (0..emitter.cells.len())
                            .map(|cell| viz_scene::CellValue {
                                intensity: (0.5
                                    + 0.5 * (time * 3.0 + cell as f32 * 0.6 + phase).sin())
                                .clamp(0.0, 1.0),
                                colour: hue(time * 0.25 + cell as f32 * 0.08),
                                held_intensity: 0.0,
                            })
                            .collect();
                    }
                }
            }
        }
        self.values.frame = self.frame;
        // A planning provider has no packet timestamp; the session stamps arrival instead.
        self.values.newest_input_micros = 0;
    }
}

impl SceneProvider for DemoProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::PlanningSoftware,
            available: true,
            unavailable_reason: None,
            default_host: "127.0.0.1".into(),
            default_port: crate::settings::DEFAULT_PLANNER_PORT,
            uses_network_input: false,
        }
    }

    fn poll(&mut self) -> Vec<ProviderEvent> {
        let mut events = Vec::new();
        if !self.emitted_snapshot {
            self.emitted_snapshot = true;
            events.push(ProviderEvent::Connection(ConnectionState::Connected {
                endpoint: "built-in deterministic scene".into(),
                revision: self.scene.revision,
            }));
            events.push(ProviderEvent::Snapshot {
                scene: Box::new(self.scene.clone()),
                view: None,
            });
        }
        self.advance();
        events.push(ProviderEvent::Values(Box::new(self.values.clone())));
        events
    }

    fn request_resync(&mut self) {
        self.emitted_snapshot = false;
    }

    fn shutdown(&mut self) {}
}

/// The room the built-in rig hangs in: the truss it flies from, the deck under it and the drapes
/// behind, so a beam has something to land on and the picture reads as a stage.
fn push_demo_scenery(scene: &mut Scene) {
    scene.scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Stage floor".into(),
        position: Vec3::new(0.0, -0.05, 0.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(18.0, 0.1, 14.0),
        colour: [0.13, 0.13, 0.14],
        roughness: 0.85,
        kind: SceneryKind::Floor,
        chords: 0,
    });
    scene.scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Upstage wall".into(),
        position: Vec3::new(0.0, 4.0, -7.0),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(18.0, 8.0, 0.2),
        colour: [0.11, 0.11, 0.125],
        roughness: 0.9,
        kind: SceneryKind::Wall,
        chords: 0,
    });
    scene.scenery.push(SceneryObject {
        id: Uuid::nil(),
        name: "Riser".into(),
        position: Vec3::new(0.0, 0.3, -3.5),
        rotation_degrees: Vec3::ZERO,
        size: Vec3::new(6.0, 0.6, 3.0),
        colour: [0.16, 0.14, 0.13],
        roughness: 0.75,
        kind: SceneryKind::Riser,
        chords: 0,
    });
}

/// The built-in look's show laser.
///
/// It exercises the scan path the same way a patched fixture does: through a real script, run by
/// the real engine, with no shortcut for the demo. It is drawn as the shipped projector body, and
/// its beam leaves the window in that body's front face — which is why it hangs nose-down rather
/// than upside down like a lantern.
fn push_demo_laser(scene: &mut Scene) {
    // A show laser, so the built-in look exercises the scan path the same way a patched fixture
    // does: through a real script, run by the real engine, with no shortcut for the demo. It is
    // drawn as the shipped projector body, and its beam leaves the window in that body's front
    // face — which is why it hangs nose-down rather than upside down like a lantern.
    let laser_body = Vec3::new(0.39, 0.43, 0.33);
    let laser_model = viz_scene::read_glb(
        viz_project::choose_default_model("laser", viz_project::FixtureTraits::default()).bytes,
    )
    .ok()
    .map(|model| {
        let scale = model.scale_to(laser_body);
        let window = model.emitter_anchor.unwrap_or(Vec3::ZERO) * scale;
        let aim = model.emitter_axis.unwrap_or(Vec3::NEG_Y);
        scene.models.push(model);
        (scene.models.len() as u32 - 1, window, aim)
    });
    let laser = scene.fixtures.len();
    scene.fixtures.push(FixtureInstance {
        instance_id: Uuid::from_u128(0x1a5e_0000_0000_0000_0000_0000_0000_0001),
        fixture_id: Uuid::from_u128(0x1a5e_0000_0000_0000_0000_0000_0000_0001),
        name: "Laser".into(),
        number: Some((laser + 1) as u32),
        position: Vec3::new(0.0, 5.4, -6.2),
        // Nose-down far enough that the top of a 50-degree scan field still meets the deck. A
        // laser aimed flatter throws the top of every figure out of the room, which is truthful
        // and useless to look at.
        rotation_degrees: Vec3::new(48.0, 0.0, 0.0),
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody {
            size: laser_body,
            kind: BodyKind::Machine,
        },
        patched: true,
        address: None,
        model: laser_model.map(|(index, _, _)| index),
        fallback: None,
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: laser as u32,
        head_index: 0,
        label: "Scanner".into(),
        local_origin: laser_model.map_or(Vec3::new(0.0, 0.0, 0.12), |(_, window, _)| window),
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: laser_model.map_or(Vec3::ZERO, |(_, _, aim)| {
            let (x, y, z) =
                glam::Quat::from_rotation_arc(Vec3::NEG_Y, aim).to_euler(glam::EulerRot::XYZ);
            Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
        }),
        pan: None,
        tilt: None,
        beam_angle_degrees: 0.1,
        field_angle_degrees: 0.1,
        optics: EmitterOptics {
            output: 1.0,
            sharpness: 1.0,
            uniformity: 1.0,
            source: LightSource::round(0.004),
            gobo_wheel: Vec::new(),
        },
        kind: EmitterKind::Laser,
        cells: EmitterLayoutCells::single(),
        laser: Some(viz_scene::LaserOptics {
            script: Some(DEMO_SCAN_SCRIPT.into()),
            script_key: 1,
            scan_half_angle_x: 25.0_f32.to_radians(),
            scan_half_angle_y: 25.0_f32.to_radians(),
            ..viz_scene::LaserOptics::default()
        }),
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
}

fn push_demo_effect(scene: &mut Scene) {
    let fixture_index = scene.fixtures.len();
    let identity = Uuid::from_u128(0xefec_7000_0000_0000_0000_0000_0000_0001);
    scene.fixtures.push(FixtureInstance {
        instance_id: identity,
        fixture_id: identity,
        name: "Cold Spark".into(),
        number: Some((fixture_index + 1) as u32),
        position: Vec3::new(2.8, 0.18, -2.2),
        rotation_degrees: Vec3::ZERO,
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody {
            size: Vec3::new(0.32, 0.36, 0.32),
            kind: BodyKind::Machine,
        },
        patched: true,
        address: None,
        model: None,
        fallback: None,
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: fixture_index as u32,
        head_index: 0,
        label: "Fountain".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 1.0,
        field_angle_degrees: 1.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Effect,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: Some(viz_scene::EffectProgram {
            script: Some(DEMO_EFFECT_SCRIPT.into()),
            script_key: 1,
            result_version: 1,
        }),
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
}

/// The hazer, without which none of the beams above it would be visible in the air.
fn push_demo_hazer(scene: &mut Scene) {
    let hazer = scene.fixtures.len();
    scene.fixtures.push(FixtureInstance {
        instance_id: Uuid::nil(),
        fixture_id: Uuid::nil(),
        name: "Hazer".into(),
        number: Some((hazer + 1) as u32),
        position: Vec3::new(-7.5, 0.3, -5.5),
        rotation_degrees: Vec3::ZERO,
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody {
            size: Vec3::new(0.5, 0.4, 0.35),
            kind: BodyKind::Machine,
        },
        patched: true,
        address: None,
        model: None,
        fallback: None,
    });
    scene.emitters.push(EmitterInstance {
        fixture_index: hazer as u32,
        head_index: 0,
        label: "Fog".into(),
        local_origin: Vec3::new(0.0, 0.1, 0.2),
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 30.0,
        field_angle_degrees: 60.0,
        optics: EmitterOptics {
            output: 1.0,
            sharpness: 0.0,
            uniformity: 0.6,
            source: LightSource::round(0.05),
            gobo_wheel: Vec::new(),
        },
        kind: EmitterKind::Atmosphere,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
}

fn hue(position: f32) -> [f32; 3] {
    let wrapped = position.rem_euclid(1.0) * 6.0;
    let sector = wrapped.floor() as i32;
    let fraction = wrapped - sector as f32;
    match sector.rem_euclid(6) {
        0 => [1.0, fraction, 0.0],
        1 => [1.0 - fraction, 1.0, 0.0],
        2 => [0.0, 1.0, fraction],
        3 => [0.0, 1.0 - fraction, 1.0],
        4 => [fraction, 0.0, 1.0],
        _ => [1.0, 0.0, 1.0 - fraction],
    }
}

/// The artwork the demo rig's profile heads carry, so the built-in scene exercises a wheel of
/// real glass and not only the drawn patterns. These are the shipped masks, embedded because the
/// demo scene has no library to read and no desk to ask.
const DEMO_GOBOS: [(&str, &[u8]); 4] = [
    (
        "Breakup",
        include_bytes!("../../../assets/gobos/breakup.png"),
    ),
    ("Rings", include_bytes!("../../../assets/gobos/rings.png")),
    ("Spokes", include_bytes!("../../../assets/gobos/spokes.png")),
    ("Stars", include_bytes!("../../../assets/gobos/stars.png")),
];

/// Decode the demo wheel into the scene, returning the wheel the profile heads turn.
fn demo_gobo_wheel(scene: &mut Scene) -> Vec<viz_scene::GoboSlot> {
    let mut wheel = vec![viz_scene::GoboSlot::default()];
    for (name, bytes) in DEMO_GOBOS {
        let artwork = viz_project::decode_gobo_artwork(bytes)
            .map(|image| {
                scene.gobo_artwork.push(image);
                scene.gobo_artwork.len() as u32 - 1
            })
            .ok();
        wheel.push(viz_scene::GoboSlot {
            artwork,
            name: name.to_owned(),
        });
    }
    wheel
}

pub(crate) fn build_scene() -> Scene {
    let mut scene = Scene {
        revision: 1,
        show_id: None,
        show_name: "Built-in demonstration rig".into(),
        source_identity: "viz-renderer built-in".into(),
        bounds: Aabb::empty(),
        ..Scene::default()
    };
    let wheel = demo_gobo_wheel(&mut scene);
    push_demo_scenery(&mut scene);
    // One stable audience block makes the built-in benchmark exercise the crowd budget on every
    // machine without requiring a private show file.
    scene.crowds.push(CrowdArea {
        id: Uuid::nil(),
        name: "Audience".into(),
        position: Vec3::new(0.0, 0.0, 4.0),
        rotation_degrees: Vec3::ZERO,
        width_metres: 40.0,
        depth_metres: 20.0,
        posture: CrowdPosture::StandingStill,
        density: CrowdDensity::Dense,
        seed: 108,
    });

    for (truss, (height, depth)) in [(7.2_f32, -4.5_f32), (7.6, 0.0), (7.0, 4.0)]
        .into_iter()
        .enumerate()
    {
        for slot in 0..8 {
            let x = -6.3 + slot as f32 * 1.8;
            let index = scene.fixtures.len();
            scene.fixtures.push(FixtureInstance {
                instance_id: Uuid::nil(),
                fixture_id: Uuid::nil(),
                name: format!("Moving head {}.{}", truss + 1, slot + 1),
                number: Some((index + 1) as u32),
                position: Vec3::new(x, height, depth),
                rotation_degrees: Vec3::ZERO,
                bracket_degrees: 0.0,
                shaper_degrees: None,
                installed_colour: [1.0; 3],
                installed_shaper_angles_degrees: [0.0; 4],
                body: FixtureBody {
                    size: Vec3::new(0.34, 0.5, 0.34),
                    kind: BodyKind::MovingHead,
                },
                patched: true,
                address: None,
                model: None,
                fallback: None,
            });
            scene.emitters.push(EmitterInstance {
                fixture_index: index as u32,
                head_index: 0,
                label: "Main".into(),
                local_origin: Vec3::new(0.0, -0.2, 0.0),
                tilt_pivot: Vec3::ZERO,
                local_orientation_degrees: Vec3::ZERO,
                pan: Some(MotionAxis {
                    axis: Vec3::Y,
                    min_degrees: -270.0,
                    max_degrees: 270.0,
                }),
                tilt: Some(MotionAxis {
                    axis: Vec3::X,
                    min_degrees: -135.0,
                    max_degrees: 135.0,
                }),
                beam_angle_degrees: 6.0,
                field_angle_degrees: 34.0,
                // A hard-edged profile head: a crisp rim and an even field, turning a wheel of
                // real glass.
                optics: EmitterOptics {
                    output: 1.0,
                    sharpness: 0.82,
                    uniformity: 0.85,
                    source: LightSource::round(0.14),
                    gobo_wheel: wheel.clone(),
                },
                kind: EmitterKind::Beam,
                cells: EmitterLayoutCells::single(),
                laser: None,
                effect: None,
                live_shaper_angle_roles: [false; 4],
                shaper_roles: [false; 4],
                live_shaper_rotation_role: false,
            });
        }
    }

    for slot in 0..6 {
        let index = scene.fixtures.len();
        let x = -5.0 + slot as f32 * 2.0;
        scene.fixtures.push(FixtureInstance {
            instance_id: Uuid::nil(),
            fixture_id: Uuid::nil(),
            name: format!("Pixel bar {}", slot + 1),
            number: Some((index + 1) as u32),
            position: Vec3::new(x, 0.35, -6.2),
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::new(1.1, 0.12, 0.12),
                kind: BodyKind::Bar,
            },
            patched: true,
            address: None,
            model: None,
            fallback: None,
        });
        scene.emitters.push(EmitterInstance {
            fixture_index: index as u32,
            head_index: 0,
            label: "Cells".into(),
            local_origin: Vec3::new(0.0, 0.08, 0.0),
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 60.0,
            field_angle_degrees: 90.0,
            // A pixel bar: a row of small rectangular sources with no rim at all.
            optics: EmitterOptics {
                output: 0.7,
                sharpness: 0.1,
                uniformity: 0.9,
                source: LightSource {
                    form: SourceForm::Rectangular,
                    width: 0.06,
                    height: 0.05,
                },
                gobo_wheel: Vec::new(),
            },
            kind: EmitterKind::Emissive,
            laser: None,
            effect: None,
            cells: EmitterLayoutCells {
                offsets: (0..10)
                    .map(|cell| Vec3::new(-0.45 + cell as f32 * 0.1, 0.0, 0.0))
                    .collect(),
            },
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        });
    }

    push_demo_laser(&mut scene);
    push_demo_effect(&mut scene);
    push_demo_hazer(&mut scene);

    scene
}

/// The scan engine the built-in look's laser runs.
///
/// Written the way a fixture package's own script would be — an ES module exporting `scan`, state
/// held between frames for the animation, and dwell weighting on the corners — so what the demo
/// shows is the real path, not a special case.
///
/// Three figures in turn, because one pattern held for ever shows neither what a scan engine can
/// do nor what the renderer does with it: a closed curve with corners, a line sweeping the field,
/// and a figure that changes colour around itself while it grows.
const DEMO_SCAN_SCRIPT: &str = r#"
// How long each figure holds before the next one takes over.
const HOLD_SECONDS = 7;

// Integrated rather than read off the clock, so the figures keep their own rates whatever the
// frame rate is and pick up where they left off.
let clock = 0;
let spin = 0;

export function scan(input) {
  clock += input.elapsed;
  spin += input.elapsed * 0.6;
  switch (Math.floor(clock / HOLD_SECONDS) % 3) {
    case 1: return wave();
    case 2: return pulse();
    default: return rose();
  }
}

function clamp(value) {
  return Math.max(-1, Math.min(1, value));
}

/// A rose curve: a closed figure with real corners, which is what makes the dwell weighting
/// visible as bright points rather than an evenly lit outline.
function rose() {
  const arms = 5;
  const steps = 96;
  const points = [];
  for (let step = 0; step <= steps; step++) {
    const t = (step / steps) * Math.PI * 2;
    const radius = Math.cos(arms * t + spin);
    const hue = (t / (Math.PI * 2) + spin * 0.2) % 1;
    points.push({
      x: radius * Math.cos(t),
      y: radius * Math.sin(t) * 0.7,
      r: Math.max(0, Math.cos(hue * Math.PI * 2)),
      g: Math.max(0, Math.cos((hue - 0.33) * Math.PI * 2)),
      b: Math.max(0, Math.cos((hue - 0.66) * Math.PI * 2)),
      amount: 100 / (steps + 1),
    });
  }
  return { points, pointsPerSecond: 30000 };
}

/// A green sine wave lying across the field, travelling down it.
///
/// The whole wave moves and its shape does not, which is what makes the sheet in the air read as
/// one moving surface rather than as a pattern rippling in place.
function wave() {
  const steps = 128;
  const cycles = 2;
  const points = [];
  // A positive deflection pitches the beam further down, so the travel runs from -1 at the top of
  // the scan field to +1 at the bottom and starts again.
  const travel = (clock / 3) % 1;
  const centre = -1 + travel * 2;
  for (let step = 0; step <= steps; step++) {
    const across = step / steps;
    points.push({
      x: across * 2 - 1,
      y: clamp(centre + Math.sin(across * Math.PI * 2 * cycles) * 0.28),
      r: 0,
      g: 1,
      b: 0,
      amount: 100 / (steps + 1),
    });
  }
  return { points, pointsPerSecond: 30000 };
}

/// A striped ring growing out of the middle of the field and fading as it goes.
///
/// The stripes are what the fan in the air makes of a figure whose colour changes around it: the
/// sheet comes out banded, which a single-colour circle cannot show.
function pulse() {
  const steps = 144;
  const stripes = 14;
  const points = [];
  // One pulse: out from the middle, dimming over the last of its travel, then away and again from
  // the middle. Never fully at the rim, so the ring is gone before it reaches the edge.
  const grow = (clock / 3.5) % 1;
  const radius = 0.06 + grow * 0.94;
  const level = grow < 0.55 ? 1 : Math.max(0, 1 - (grow - 0.55) / 0.45);
  for (let step = 0; step <= steps; step++) {
    const around = step / steps;
    const angle = around * Math.PI * 2;
    // A point carries the colour of the run arriving at it, so the stripe a point belongs to is
    // the stripe drawn on the way to it and the change lands on the leading edge of each band.
    const white = Math.floor(around * stripes) % 2 === 0;
    points.push({
      x: clamp(Math.cos(angle) * radius),
      y: clamp(Math.sin(angle) * radius),
      r: white ? level : 0,
      g: white ? level : level * 0.35,
      b: level,
      amount: 100 / (steps + 1),
    });
  }
  return { points, pointsPerSecond: 30000 };
}
"#;

const DEMO_EFFECT_SCRIPT: &str = r#"
export function effect(input) {
  return {version:1, emitters:[{family:'spark',origin:[0,0.18,0],direction:[0,1,0],width:0.28,height:3.4,
    intensity:input.intensity,density:1,lifetime:2.2,color:[1,0.58,0.08],state:'hold'}]};
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_demo_scene_contains_beam_pixel_and_atmosphere_emitters() {
        let provider = DemoProvider::new();
        let kinds: Vec<_> = provider
            .scene
            .emitters
            .iter()
            .map(|emitter| emitter.kind)
            .collect();
        assert!(kinds.contains(&EmitterKind::Beam));
        assert!(kinds.contains(&EmitterKind::Emissive));
        assert!(kinds.contains(&EmitterKind::Atmosphere));
        assert_eq!(provider.scene.crowds.len(), 1);
        assert!(!provider.scene.bounds.is_empty());
    }

    /// The built-in laser's script is run by the same engine a packaged one is, so a slip in it
    /// shows up as a dark laser at runtime and nowhere else. This runs it.
    ///
    /// Each figure is checked for the thing that makes it that figure — a curve that closes on
    /// itself, a green line lying across the field, a ring of white and blue — and the three are
    /// checked to arrive in turn.
    #[test]
    fn the_built_in_laser_cycles_through_its_three_figures() {
        // This checks the built-in figure sequence, not the production execution deadline. The
        // laser crate tests that deadline independently; a busy shared CI runner must not turn a
        // correct figure into a wall-clock scheduling failure here.
        let mut engine = viz_laser::ScanEngine::new()
            .expect("a scan runtime")
            .with_budget(Duration::from_millis(100));
        // A frame every sixtieth of a second, as the renderer runs it, for long enough to see
        // every figure hold and the first come back around.
        let frames = (0..(60 * 7 * 4)).map(|frame| {
            let seconds = f64::from(frame) / 60.0;
            engine.scan(
                0,
                &viz_laser::ScanRequest {
                    source: DEMO_SCAN_SCRIPT,
                    source_key: 1,
                    slots: &[],
                    time_seconds: seconds,
                    elapsed_seconds: 1.0 / 60.0,
                    intensity: 1.0,
                },
            )
        });

        let spread = |values: &[f32]| {
            values.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                - values.iter().copied().fold(f32::INFINITY, f32::min)
        };

        let mut seen = Vec::new();
        let mut wave_heights = Vec::new();
        let mut pulse_radii = Vec::new();
        for scan in frames {
            assert!(scan.fault.is_none(), "{:?}", scan.fault);
            assert!(!scan.points.is_empty(), "the laser drew nothing");
            let lit: Vec<_> = scan
                .points
                .iter()
                .filter(|point| point.colour.iter().any(|channel| *channel > 0.01))
                .collect();
            if lit.is_empty() {
                // The pulse fades to nothing before it starts again, and a frame with no light in
                // it says nothing about which figure is holding.
                continue;
            }
            // Told apart by shape rather than by brightness, so a figure part-way through a fade
            // is still the figure it is.
            let heights: Vec<f32> = lit.iter().map(|point| point.y).collect();
            let radii: Vec<f32> = lit
                .iter()
                .map(|point| point.x.hypot(point.y))
                .collect::<Vec<_>>();
            let green = lit
                .iter()
                .all(|point| point.colour[1] > 0.5 && point.colour[0] < 0.01);
            let figure = if green {
                // The wave lies across the field: it is wide and it is not tall.
                assert!(
                    spread(&heights) < 0.7 && spread(&radii) > 0.5,
                    "the wave is not lying across the field"
                );
                wave_heights.push(heights.iter().sum::<f32>() / heights.len() as f32);
                "wave"
            } else if spread(&radii) < 0.02 {
                // The pulse is the round one: every point of it the same distance out.
                let white = lit
                    .iter()
                    .any(|point| point.colour[0] > 0.01 && point.colour[2] > 0.01);
                let blue = lit
                    .iter()
                    .any(|point| point.colour[2] > 0.01 && point.colour[0] < 0.001);
                assert!(white && blue, "the ring is not striped white and blue");
                pulse_radii.push(radii[0]);
                "pulse"
            } else {
                "rose"
            };
            if seen.last().map(String::as_str) != Some(figure) {
                seen.push(figure.to_owned());
            }
        }
        assert_eq!(
            seen,
            vec!["rose", "wave", "pulse", "rose"],
            "the three figures did not each hold in turn"
        );

        // The wave travels the whole field and travels it downwards, a positive deflection being
        // the downward one. It wraps back to the top between sweeps, so most of its steps are
        // down rather than all of them.
        assert!(
            wave_heights.iter().copied().fold(f32::INFINITY, f32::min) < -0.6
                && wave_heights
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max)
                    > 0.6,
            "the wave did not travel the whole field"
        );
        let downwards = wave_heights
            .windows(2)
            .filter(|step| step[1] > step[0])
            .count();
        assert!(
            downwards > wave_heights.len() * 9 / 10,
            "the wave travelled downwards on only {downwards} of {} steps",
            wave_heights.len()
        );
        // The ring grows out of the middle and is gone before the rim, then starts again.
        assert!(
            pulse_radii.iter().any(|radius| *radius < 0.2)
                && pulse_radii.iter().any(|radius| *radius > 0.8),
            "the ring did not grow out of the middle of the field"
        );
    }

    #[test]
    fn the_first_poll_delivers_a_snapshot_and_later_polls_only_values() {
        let mut provider = DemoProvider::new();
        let first = provider.poll();
        assert!(
            first
                .iter()
                .any(|event| matches!(event, ProviderEvent::Snapshot { .. }))
        );
        let second = provider.poll();
        assert!(
            second
                .iter()
                .all(|event| matches!(event, ProviderEvent::Values(_)))
        );
    }

    #[test]
    fn the_value_stream_is_deterministic() {
        let mut first = DemoProvider::new();
        let mut second = DemoProvider::new();
        for _ in 0..10 {
            first.poll();
            second.poll();
        }
        assert_eq!(
            first.values.emitters[0].intensity,
            second.values.emitters[0].intensity
        );
        assert_eq!(first.values.atmosphere, second.values.atmosphere);
    }
}
