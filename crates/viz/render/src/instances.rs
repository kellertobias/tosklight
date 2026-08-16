//! Per-frame translation from the semantic scene into GPU instance and light arrays.
//!
//! Structural instances (bodies, scenery) are rebuilt only when the scene revision changes.
//! Live values rebuild the moving parts, emitter apertures, and lights, which is the only work a
//! DMX frame is allowed to cause.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use viz_scene::{
    BodyKind, EmitterInstance, EmitterKind, EmitterValues, FixtureInstance, Scene, SceneValues,
    SourceForm, euler_degrees,
};

mod effects;

/// Which procedural mesh an instance draws.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MeshKind {
    Cube,
    Cylinder,
    Sphere,
    /// The face light leaves through: a lamp's lens, drawn thin and domed rather than round.
    Lens,
    Plane,
    /// One part of a fixture model read from the library: `(model index, part index)`.
    ///
    /// A part is its own mesh because pan and tilt move the yoke and the head but not the base,
    /// so they cannot share one instance transform.
    ModelPart(u32, u32),
    /// One package-owned SVG projection parsed into physical local-space triangles.
    PlanArtwork(u32),
}

impl MeshKind {
    pub const PROCEDURAL: [Self; 5] = [
        Self::Cube,
        Self::Cylinder,
        Self::Sphere,
        Self::Lens,
        Self::Plane,
    ];
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshInstance {
    pub model: [[f32; 4]; 4],
    pub normal0: [f32; 4],
    pub normal1: [f32; 4],
    pub normal2: [f32; 4],
    /// `rgb` base colour, `w` roughness.
    pub base_colour: [f32; 4],
    /// `rgb` emissive radiance, `w` metallic.
    pub emissive: [f32; 4],
}

impl MeshInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
            7 => Float32x4, 8 => Float32x4, 9 => Float32x4,
            10 => Float32x4, 11 => Float32x4
        ],
    };

    fn new(model: Mat4, base_colour: Vec3, roughness: f32, emissive: Vec3, metallic: f32) -> Self {
        let normal = Mat3::from_mat4(model).inverse().transpose();
        Self {
            model: model.to_cols_array_2d(),
            normal0: normal.x_axis.extend(0.0).to_array(),
            normal1: normal.y_axis.extend(0.0).to_array(),
            normal2: normal.z_axis.extend(0.0).to_array(),
            base_colour: base_colour.extend(roughness).to_array(),
            emissive: emissive.extend(metallic).to_array(),
        }
    }
}

/// One spot or point light consumed by the surface and beam passes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuLight {
    /// `xyz` world position, `w` range in metres.
    pub position_range: [f32; 4],
    /// `xyz` normalised aim, `w` cosine of the outer (field) half-angle.
    pub direction_cos_outer: [f32; 4],
    /// `xyz` linear radiance, `w` scalar intensity.
    pub colour_intensity: [f32; 4],
    /// `x` cosine of the inner (beam) half-angle, `y` feather, `z` uniformity, `w` how far behind
    /// the lens the cone's virtual apex sits.
    pub params: [f32; 4],
    /// `xyz` the beam's own right axis, which every pattern is oriented against; `w` frost.
    pub tangent_frost: [f32; 4],
    /// `x` gobo slot, `y` gobo rotation in radians, `z` prism facets, `w` prism rotation.
    pub optics: [f32; 4],
    /// Framing-shutter blade insertions, `0` open, in the beam's own frame after its rotation.
    pub shapers: [f32; 4],
    /// Framing-shutter blade angles in radians, one per explicitly supported canonical role.
    pub shaper_angles: [f32; 4],
    /// `x` the artwork layer this slot projects, or `-1` for a slot with none — which is every
    /// slot of a profile that declares no wheel, and is what selects the drawn patterns instead.
    /// `yzw` spare.
    pub gate: [f32; 4],
    /// `x` shadow-map index or `-1`, `yz` its tile origin in the atlas, `w` the tile size.
    pub shadow: [f32; 4],
}

/// One beam volume drawn as an instanced cone.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BeamInstance {
    pub model: [[f32; 4]; 4],
    /// `xyz` colour, `w` intensity.
    pub colour: [f32; 4],
    /// `x` light index, `y` cone length from the lens, `z` how far behind the lens the virtual
    /// apex sits, `w` spare.
    pub params: [f32; 4],
}

impl BeamInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
            7 => Float32x4, 8 => Float32x4
        ],
    };
}

/// One straight run of a laser's scan path, drawn as a camera-facing glowing ribbon.
///
/// A laser is not a cone and cannot be drawn as one. What is actually in the air is a beam a few
/// millimetres across that has been somewhere else a thirty-thousandth of a second ago, and what
/// an audience sees is the whole path at once because the eye cannot separate the parts. So the
/// path is what gets drawn: one of these per straight run between two control points, added
/// together in the haze.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LaserInstance {
    /// `xyz` where this run starts in world space, `w` the beam radius there in metres.
    pub start_radius: [f32; 4],
    /// `xyz` where it ends, `w` the beam radius there. Divergence widens the beam down its throw,
    /// so the two differ over a long shot.
    pub end_radius: [f32; 4],
    /// `xyz` linear radiance already weighted by dwell, `w` whether this run is the figure lying
    /// on the surface the beam landed on rather than the beam in the air on its way there. Haze is
    /// what makes the second visible and has nothing to do with the first.
    pub colour_landing: [f32; 4],
}

impl LaserInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
    };
}

/// One aim line drawn in the line and orthographic modes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub _pad: f32,
    pub colour: [f32; 4],
}

impl LineVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
    };
}

/// Where one emitter ended up this frame, in world space.
#[derive(Clone, Copy, Debug)]
pub struct EmitterPose {
    pub origin: Vec3,
    pub direction: Vec3,
    pub half_angle: f32,
    pub orientation: Quat,
}

/// Everything one frame uploads.
#[derive(Default)]
pub struct FrameInstances {
    pub meshes: Vec<(MeshKind, Vec<MeshInstance>)>,
    pub lights: Vec<GpuLight>,
    pub beams: Vec<BeamInstance>,
    pub lasers: Vec<LaserInstance>,
    pub lines: Vec<LineVertex>,
    pub poses: Vec<EmitterPose>,
    /// Authored versus actually rendered audience, exposed so a budget reduction is visible.
    pub crowd_authored: u32,
    pub crowd_requested: u32,
    pub crowd_drawn: u32,
    pub particles_requested: u32,
    pub particles_drawn: u32,
    /// Textured fronts are a dedicated pass so one source texture can feed any number of panels.
    pub media_panels: Vec<MediaPanel>,
}

#[derive(Clone, Debug)]
pub struct MediaPanel {
    pub source_id: Option<viz_scene::uuid::Uuid>,
    pub fallback_source_id: Option<viz_scene::uuid::Uuid>,
    pub model: Mat4,
    pub crop: [f32; 4],
    /// x kind: 0 reflective screen, 1 TV, 2 LED; y gain/spill, z roughness, w feather.
    pub material: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMediaPanel {
    pub model: [[f32; 4]; 4],
    pub crop: [f32; 4],
    pub material: [f32; 4],
}

impl GpuMediaPanel {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
            7 => Float32x4, 8 => Float32x4
        ],
    };
}

impl FrameInstances {
    fn mesh(&mut self, kind: MeshKind) -> &mut Vec<MeshInstance> {
        if let Some(index) = self
            .meshes
            .iter()
            .position(|(existing, _)| *existing == kind)
        {
            return &mut self.meshes[index].1;
        }
        self.meshes.push((kind, Vec::new()));
        &mut self.meshes.last_mut().expect("just pushed").1
    }

    /// One straight line between two points, each end with its own colour and opacity.
    fn line(&mut self, from: Vec3, to: Vec3, near: glam::Vec4, far: glam::Vec4) {
        self.lines.push(LineVertex {
            position: from.to_array(),
            _pad: 0.0,
            colour: near.to_array(),
        });
        self.lines.push(LineVertex {
            position: to.to_array(),
            _pad: 0.0,
            colour: far.to_array(),
        });
    }
}

/// Maximum beam throw used for the volumetric cone and the aim line.
const BEAM_THROW_METRES: f32 = 20.0;

/// How far behind a lens the cone's virtual apex is allowed to sit. A tight beam's apex is metres
/// back by geometry alone, and a pencil beam's would be out of the building.
const MAX_APEX_OFFSET: f32 = 12.0;

/// How thick a lit face is drawn, as a fraction of its own smaller dimension. A lens has real
/// glass in it and a panel has a real front, so neither is a flat sticker; neither is anywhere
/// near as deep as it is wide.
const SOURCE_THICKNESS: f32 = 0.16;

/// Radiance a lit face reaches at full, before exposure and the filmic curve.
///
/// Chosen so a lamp at full sits a little past the knee of that curve — bright, and still with
/// somewhere left to go — rather than far up its shoulder, where every level from about a tenth
/// of the dimmer upwards flattens into the same white. The whole fader has to be worth moving.
const APERTURE_RADIANCE: f32 = 2.2;

/// The cone every fixture's intensity is measured against: a 40-degree field, which is an ordinary
/// stage lantern. Narrower than this concentrates the same light and is brighter; wider spreads it
/// and is dimmer.
const REFERENCE_HALF_ANGLE: f32 = 0.349;

/// How much of the true spread relation to apply. `1.0` is literal inverse solid angle, which puts
/// several hundred to one between a beam light and a flood — true, and unwatchable on one screen.
const SPREAD_COMPRESSION: f32 = 0.55;

/// How one frame should be drawn.
#[derive(Clone, Debug)]
pub struct FrameStyle {
    pub quality: viz_scene::RenderQuality,
    pub draw_beams: bool,
    pub draw_aim_lines: bool,
    /// Draw the scene as an outline plan instead of a shaded picture.
    pub plot: bool,
    /// Screen-plane axes used to billboard plot symbols so they read from any plan direction.
    pub plot_right: Vec3,
    pub plot_up: Vec3,
    pub projection_view: viz_scene::ProjectionView,
    /// World size one plot symbol should occupy, chosen so a symbol keeps a constant on-screen
    /// size however far the plan is zoomed out.
    pub symbol_metres: f32,
    /// Ink colour for a fixture that makes light.
    pub ink: Vec3,
    /// Ink colour for scenery and for a fixture that makes no light.
    pub faint_ink: Vec3,
    /// The one colour every beam is drawn in on a plan.
    pub beam_ink: Vec3,
    /// Ink for a fixture symbol or outline. Quieter than [`Self::ink`], which is for the things a
    /// plan is read *for*: a rig has far more lanterns on it than anything else, and drawn at full
    /// strength they are what the eye lands on instead of the light.
    pub symbol_ink: Vec3,
    /// Ink for a fixture the operator has selected — the one thing allowed to stand out.
    pub selected_ink: Vec3,
    /// Draw each fixture's own model, rather than a box standing where it is.
    pub fixture_models: bool,
    /// Draw the emitting faces that belong to a simulated-light picture.
    pub emitter_apertures: bool,
    /// Draw retained scenery as shaded surfaces instead of quiet outlines.
    pub scenery_surfaces: bool,
    /// Draw an aim guideline for every directional emitter, lit or not.
    pub aim_guides: bool,
    /// Lay the reference grid on the ground plane.
    pub floor_grid: bool,
    /// Which scenery this view draws at all.
    pub scenery: fn(viz_scene::SceneryKind) -> bool,
    /// Renderer-local fraction of every authored crowd to draw.
    pub crowd_amount: f32,
    /// Per-frame crowd budget selected from quality and the renderer's adaptive hardware ladder.
    pub crowd_person_budget: usize,
    /// Per-frame Effect-particle budget selected from quality and the same adaptive hardware
    /// ladder as the expensive Ultra rendering features.
    pub effect_particle_budget: usize,
    /// Live/fallback media is a standalone capability. Helpers and embedded Stage panes draw the
    /// same authored geometry with neutral faces and open no media transport.
    pub media_content: bool,
    /// Current decoded appearance by Media Surface identity. This is volatile renderer state,
    /// never authored show intent.
    pub media_appearance: std::collections::BTreeMap<viz_scene::uuid::Uuid, MediaAppearance>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MediaAppearance {
    pub average: Vec3,
    pub flicker: f32,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            quality: viz_scene::RenderQuality::High,
            draw_beams: true,
            draw_aim_lines: false,
            plot: false,
            plot_right: Vec3::X,
            plot_up: Vec3::Y,
            projection_view: viz_scene::ProjectionView::Top,
            symbol_metres: 0.3,
            beam_ink: Vec3::new(1.0, 0.82, 0.25),
            ink: Vec3::splat(0.85),
            faint_ink: Vec3::splat(0.35),
            symbol_ink: Vec3::splat(0.42),
            selected_ink: Vec3::new(0.25, 0.6, 1.0),
            fixture_models: true,
            emitter_apertures: true,
            scenery_surfaces: true,
            aim_guides: false,
            floor_grid: true,
            scenery: |_| true,
            crowd_amount: 1.0,
            crowd_person_budget: 384,
            effect_particle_budget: 2_048,
            media_content: true,
            media_appearance: std::collections::BTreeMap::new(),
        }
    }
}

/// Build every instance array for one frame.
pub fn build(scene: &Scene, values: &SceneValues, style: &FrameStyle) -> FrameInstances {
    let mut frame = FrameInstances::default();
    let head_angles = head_angles(scene, values);
    if style.plot {
        plot::push_plot(&mut frame, scene, values, &head_angles, style);
        return frame;
    }
    if style.floor_grid {
        push_floor_grid(&mut frame, scene, style);
    }
    scenery::push_scenery(&mut frame, scene, values, style);
    crowd::push_crowds(&mut frame, scene, style);
    media::push_media(&mut frame, scene, style);
    push_bodies(
        &mut frame,
        scene,
        &head_angles,
        style,
        &values.selected_fixtures,
    );
    push_emitters(&mut frame, scene, values, &head_angles, style);
    effects::push_effects(&mut frame, scene, values, style);
    frame
}

/// The reference grid on the ground plane.
///
/// Lines on the floor, not a floor. A filled plane is a surface: it takes light, it hides whatever
/// is under it, and it turns the bottom of the picture into a large flat area competing with the
/// rig for attention. What an operator actually wants from it is a sense of scale and of where the
/// centre line is, which is what a grid of dark lines gives without being lit at all.
fn push_floor_grid(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    /// Metres between lines. A metre is the unit a rig is measured and marked out in.
    const SPACING: f32 = 1.0;
    /// How far past the rig the grid runs, so it never stops at the edge of the fixtures.
    const MARGIN: f32 = 4.0;
    /// The largest grid worth drawing, so an accidentally enormous scene cannot fill the buffer.
    const MAX_LINES: i32 = 200;

    let bounds = scene.bounds;
    let (min_x, max_x, min_z, max_z) = if bounds.is_empty() {
        (-8.0, 8.0, -8.0, 8.0)
    } else {
        (
            bounds.min.x - MARGIN,
            bounds.max.x + MARGIN,
            bounds.min.z - MARGIN,
            bounds.max.z + MARGIN,
        )
    };
    let first = |value: f32| (value / SPACING).floor() as i32;
    let last = |value: f32| (value / SPACING).ceil() as i32;
    let (x0, x1) = (first(min_x), last(max_x));
    let (z0, z1) = (first(min_z), last(max_z));
    if x1 - x0 > MAX_LINES || z1 - z0 > MAX_LINES {
        return;
    }

    // Dark, and only just visible. The grid is a reference the eye can find when it looks for it
    // and ignore when it does not; a bright one draws attention away from the only thing on the
    // stage that is supposed to be bright.
    // These were chosen while every line was multiplied by the glow the lit views use. The glow now
    // belongs to the views that simulate light, so the grid carries its own weight.
    let line = (style.faint_ink * 0.09).extend(1.0);
    // The centre lines are the ones an operator counts from, so they are drawn a little stronger.
    let centre = (style.faint_ink * 0.30).extend(1.0);
    let y = FLOOR_HEIGHT + 0.002;

    for step in x0..=x1 {
        let x = step as f32 * SPACING;
        let colour = if step == 0 { centre } else { line };
        frame.line(
            Vec3::new(x, y, min_z),
            Vec3::new(x, y, max_z),
            colour,
            colour,
        );
    }
    for step in z0..=z1 {
        let z = step as f32 * SPACING;
        let colour = if step == 0 { centre } else { line };
        frame.line(
            Vec3::new(min_x, y, z),
            Vec3::new(max_x, y, z),
            colour,
            colour,
        );
    }
}

/// Pan and tilt in degrees for every emitter, resolved once and reused by bodies and beams so the
/// yoke and the beam can never disagree.
fn head_angles(scene: &Scene, values: &SceneValues) -> Vec<(f32, f32)> {
    scene
        .emitters
        .iter()
        .enumerate()
        .map(|(index, emitter)| {
            let value = values.emitters.get(index);
            let pan = value.map_or(0.5, |value| value.pan);
            let tilt = value.map_or(0.5, |value| value.tilt);
            (
                value
                    .filter(|value| value.pan_motion.target.is_some())
                    .map_or_else(
                        || emitter.pan.map_or(0.0, |axis| axis.degrees_at(pan)),
                        |value| value.pan_motion.position_degrees,
                    ),
                value
                    .filter(|value| value.tilt_motion.target.is_some())
                    .map_or_else(
                        || emitter.tilt.map_or(0.0, |axis| axis.degrees_at(tilt)),
                        |value| value.tilt_motion.position_degrees,
                    ),
            )
        })
        .collect()
}

/// Draw one fixture from its library model, with pan and tilt applied to the parts that move.
#[allow(clippy::too_many_arguments)]
fn push_model(
    frame: &mut FrameInstances,
    scene: &Scene,
    fixture: &FixtureInstance,
    fixture_index: usize,
    model_index: u32,
    model: &viz_scene::FixtureModel,
    head_angles: &[(f32, f32)],
) {
    let (pan, tilt) = scene
        .emitters
        .iter()
        .position(|emitter| emitter.fixture_index == fixture_index as u32)
        .and_then(|index| head_angles.get(index).copied())
        .unwrap_or((0.0, 0.0));
    let pan_rotation = Quat::from_rotation_y(pan.to_radians());
    let tilt_rotation = Quat::from_rotation_x(tilt.to_radians());
    // A model authored at another size is scaled to the profile's physical dimensions, so a rig
    // never mixes lamps drawn at different scales. The scene plan puts the emitter through the
    // same call, so the beam keeps leaving the lens whatever the profile says the lamp measures.
    let scale = model.scale_to(fixture.body.size);
    let base = Mat4::from_rotation_translation(fixture.orientation(), fixture.position)
        * Mat4::from_scale(Vec3::splat(scale));

    // The yoke turns about the axis the fixture hangs on; the head turns about its own
    // trunnions. Tilting about the hanging point instead would swing the head through the air.
    let pivot = model.head_pivot;
    let tilt_about_trunnions = Mat4::from_translation(pivot)
        * Mat4::from_quat(tilt_rotation)
        * Mat4::from_translation(-pivot);
    for (part_index, part) in model.parts.iter().enumerate() {
        let transform = match part.kind {
            viz_scene::ModelPartKind::Base => base,
            viz_scene::ModelPartKind::Yoke => base * Mat4::from_quat(pan_rotation),
            viz_scene::ModelPartKind::Head => {
                base * Mat4::from_quat(pan_rotation) * tilt_about_trunnions
            }
        };
        frame
            .mesh(MeshKind::ModelPart(model_index, part_index as u32))
            .push(MeshInstance::new(
                transform,
                Vec3::from(part.colour),
                part.roughness,
                Vec3::ZERO,
                part.metallic,
            ));
    }
}

const BODY_COLOUR: Vec3 = Vec3::new(0.055, 0.06, 0.068);
const YOKE_COLOUR: Vec3 = Vec3::new(0.08, 0.085, 0.095);

/// Add a small cage around a selected shaded fixture without changing its authored material.
///
/// The gap keeps the line out of the model's depth surface, and the line pass gives selected ink
/// its full-output glow. This remains legible beside a bright beam while the model underneath is
/// exactly the one an unselected fixture would draw.
fn push_selection_cage(
    frame: &mut FrameInstances,
    base: Mat4,
    fixture_size: Vec3,
    selected_ink: Vec3,
) {
    const RELATIVE_GAP: f32 = 1.08;
    const MINIMUM_GAP_METRES: f32 = 0.04;
    let cage_size = fixture_size * RELATIVE_GAP + Vec3::splat(MINIMUM_GAP_METRES);
    push_box_outline(frame, base * Mat4::from_scale(cage_size), selected_ink, 1.0);
}

fn push_bodies(
    frame: &mut FrameInstances,
    scene: &Scene,
    head_angles: &[(f32, f32)],
    style: &FrameStyle,
    selection: &std::collections::HashSet<viz_scene::uuid::Uuid>,
) {
    for (fixture_index, fixture) in scene.fixtures.iter().enumerate() {
        let base = Mat4::from_rotation_translation(fixture.orientation(), fixture.position);
        let size = fixture.body.size;
        let selected = selection.contains(&fixture.fixture_id);
        // A view that draws no models draws the outline of a box the size of the fixture, standing
        // and turned where the fixture does. Outline rather than a solid: this view simulates no
        // light, so a solid box has nothing to reveal it and would be a black shape in a black
        // room. It is the fixture's own footprint rather than a token, because an operator judging
        // whether two heads will foul each other needs the box to be the size of the thing.
        if !style.fixture_models {
            let (ink, opacity) = if selected {
                (style.selected_ink, 1.0)
            } else {
                (style.symbol_ink, 0.75)
            };
            push_box_outline(frame, base * Mat4::from_scale(size), ink, opacity);
            continue;
        }
        // A fixture whose profile carries a model is drawn as that model. The proxy shapes below
        // are what a fixture gets when its library entry has no geometry to offer.
        if let Some(model_index) = fixture.model
            && let Some(model) = scene.models.get(model_index as usize)
        {
            push_model(
                frame,
                scene,
                fixture,
                fixture_index,
                model_index,
                model,
                head_angles,
            );
            if selected {
                push_selection_cage(frame, base, size, style.selected_ink);
            }
            continue;
        }
        match fixture.body.kind {
            BodyKind::MovingHead => {
                let (pan, tilt) = scene
                    .emitters
                    .iter()
                    .position(|emitter| emitter.fixture_index == fixture_index as u32)
                    .and_then(|index| head_angles.get(index).copied())
                    .unwrap_or((0.0, 0.0));
                let pan_rotation = Quat::from_rotation_y(pan.to_radians());
                let tilt_rotation = Quat::from_rotation_x(tilt.to_radians());
                // Base plate hangs from the rig point.
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    base * Mat4::from_scale_rotation_translation(
                        Vec3::new(size.x, size.y * 0.28, size.z),
                        Quat::IDENTITY,
                        Vec3::new(0.0, size.y * 0.36, 0.0),
                    ),
                    BODY_COLOUR,
                    0.55,
                    Vec3::ZERO,
                    0.35,
                ));
                let yoke = base * Mat4::from_quat(pan_rotation);
                for side in [-1.0_f32, 1.0] {
                    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                        yoke * Mat4::from_scale_rotation_translation(
                            Vec3::new(size.x * 0.14, size.y * 0.5, size.z * 0.3),
                            Quat::IDENTITY,
                            Vec3::new(side * size.x * 0.42, 0.0, 0.0),
                        ),
                        YOKE_COLOUR,
                        0.5,
                        Vec3::ZERO,
                        0.4,
                    ));
                }
                frame.mesh(MeshKind::Cylinder).push(MeshInstance::new(
                    yoke * Mat4::from_quat(tilt_rotation)
                        * Mat4::from_scale_rotation_translation(
                            Vec3::new(size.x * 0.66, size.y * 0.62, size.z * 0.66),
                            Quat::IDENTITY,
                            Vec3::ZERO,
                        ),
                    BODY_COLOUR,
                    0.45,
                    Vec3::ZERO,
                    0.5,
                ));
            }
            BodyKind::Bar | BodyKind::Matrix => {
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    base * Mat4::from_scale(size),
                    BODY_COLOUR,
                    0.6,
                    Vec3::ZERO,
                    0.2,
                ));
            }
            BodyKind::Lantern => {
                frame.mesh(MeshKind::Cylinder).push(MeshInstance::new(
                    base * Mat4::from_scale_rotation_translation(
                        Vec3::new(size.x, size.z, size.y),
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        Vec3::ZERO,
                    ),
                    BODY_COLOUR,
                    0.5,
                    Vec3::ZERO,
                    0.4,
                ));
            }
            BodyKind::Machine | BodyKind::Generic => {
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    base * Mat4::from_scale(size),
                    BODY_COLOUR,
                    0.65,
                    Vec3::ZERO,
                    0.15,
                ));
            }
        }
        if selected {
            push_selection_cage(frame, base, size, style.selected_ink);
        }
    }
}

/// Resolve one emitter's world pose from its fixture, pan, and tilt.
/// How many slots the gobo wheel is divided into when the profile carries no slot table.
pub const GOBO_SLOTS: u32 = 8;

/// Rotate `vector` about `axis` by `angle` radians.
fn rotate_about(vector: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    if angle.abs() < 1e-5 {
        return vector;
    }
    Quat::from_axis_angle(axis, angle) * vector
}

pub fn emitter_pose(
    fixture: &FixtureInstance,
    emitter: &EmitterInstance,
    pan_degrees: f32,
    tilt_degrees: f32,
    zoom: f32,
) -> EmitterPose {
    let mount = fixture.orientation();
    let pan = Quat::from_rotation_y(pan_degrees.to_radians());
    let tilt = Quat::from_rotation_x(tilt_degrees.to_radians());
    let local = euler_degrees(emitter.local_orientation_degrees);
    let orientation = mount * pan * tilt * local;
    // Tilt turns the emitter about the head's own trunnions, the same point the head geometry
    // turns about. A pivot of zero is a fixture that tilts about its hanging point, which is what
    // an emitter with nothing better to go on gets.
    let pivot = emitter.tilt_pivot;
    let origin = fixture.position + mount * (pan * (pivot + tilt * (emitter.local_origin - pivot)));
    // Emitters aim along local `-Y`, matching a lantern hung pointing down at rest.
    let direction = (orientation * Vec3::NEG_Y).normalize_or(Vec3::NEG_Y);
    EmitterPose {
        origin,
        direction,
        half_angle: emitter.cone_half_angle(zoom),
        orientation,
    }
}

/// The optical state of one head, resolved from its values into what the shaders need.
///
/// Zoom, iris, focus and frost all change the same two things — the shape of the cone and how
/// much light is in it — so they are resolved once here rather than four times per march step.
#[derive(Clone, Copy)]
struct BeamOptics {
    /// Half-angle of the field edge after zoom, iris and frost.
    half_angle: f32,
    /// Edge softness `0..=1` after focus and frost.
    feather: f32,
    /// How evenly the field is filled after frost, `1.0` being flat to the rim.
    uniformity: f32,
    /// Radiance multiplier. Zooming in concentrates the same light; closing the iris does not.
    gain: f32,
}

fn resolve_optics(emitter: &EmitterInstance, value: &EmitterValues) -> BeamOptics {
    let zoomed = emitter.cone_half_angle(value.zoom);
    // Light is flux spread over a cone, so the angle decides the intensity: the same lamp through
    // a narrower gate is brighter, and a flood laying the same light across a wall is dimmer. One
    // reference angle serves every fixture, so this holds between a beam and a flood as well as
    // across one fixture's own zoom.
    //
    // Compressed rather than literal. The honest ratio between a 3-degree beam and a 90-degree
    // flood is several hundred to one, and no display shows both ends of that at once; the curve
    // below keeps the order and the feel without blowing one of them out.
    let solid_angle = |half: f32| (1.0 - half.cos()).max(1e-6);
    let spread = (solid_angle(REFERENCE_HALF_ANGLE) / solid_angle(zoomed))
        .powf(SPREAD_COMPRESSION)
        .clamp(0.15, 12.0);

    // The iris masks the beam: the pool gets smaller and the light that is left is as bright as
    // it was. That is the whole difference between an iris and a zoom.
    let iris = value.iris.clamp(0.0, 1.0);
    let after_iris = zoomed * (1.0 - iris * 0.92).max(0.02);

    // Frost throws light outside the field edge and destroys the edge itself.
    let frost = value.frost.clamp(0.0, 1.0);
    let half_angle = (after_iris * (1.0 + frost * 0.35)).clamp(0.002, 1.55);

    // Focus is sharp in the middle of its travel and soft at either end, which is how a lens
    // moved either side of the gate behaves. The fixture's own rim is where that starts from: a
    // profile out of focus is a soft profile, not a wash.
    let defocus = ((value.focus.clamp(0.0, 1.0) - 0.5).abs() * 2.0).clamp(0.0, 1.0);
    let softness = 1.0 - emitter.optics.sharpness.clamp(0.0, 1.0);
    let feather = (softness + defocus * 0.55 + frost * 0.6).clamp(0.02, 0.98);
    // A diffuser evens out what it softens: frost fills in a hot centre as it destroys the edge.
    let uniformity = (emitter.optics.uniformity.clamp(0.0, 1.0) + frost * 0.5).clamp(0.0, 1.0);
    BeamOptics {
        half_angle,
        feather,
        uniformity,
        // A brighter engine is a brighter fixture, whatever the desk asks of it.
        gain: spread * emitter.optics.output.clamp(0.05, 8.0),
    }
}

fn push_emitters(
    frame: &mut FrameInstances,
    scene: &Scene,
    values: &SceneValues,
    head_angles: &[(f32, f32)],
    style: &FrameStyle,
) {
    let fallback = EmitterValues::default();
    for (index, emitter) in scene.emitters.iter().enumerate() {
        let Some(fixture) = scene.fixtures.get(emitter.fixture_index as usize) else {
            continue;
        };
        let value = values.emitters.get(index).unwrap_or(&fallback);
        let installed_colour = Vec3::from(fixture.installed_colour);
        let (pan, tilt) = head_angles.get(index).copied().unwrap_or((0.0, 0.0));
        let optics = resolve_optics(emitter, value);
        let mut pose = emitter_pose(fixture, emitter, pan, tilt, value.zoom);
        pose.half_angle = optics.half_angle;
        // What an observer still has, not what the desk is sending this instant. For most heads
        // the two agree; for a strobe or a laser they are the whole point of the difference.
        let intensity = value.held_intensity.max(value.visible_intensity());
        if emitter.kind == EmitterKind::Laser {
            push_laser_emitter(
                frame,
                emitter,
                value,
                values.laser_scans.get(index),
                pose,
                intensity,
                installed_colour,
            );
            continue;
        }
        let cells = cell_states(emitter, value, installed_colour);
        let aperture = aperture_size(emitter);
        // Where the cone would converge behind the lens. A wide lamp's apex sits just behind its
        // face; a narrow one's is metres back, which is exactly why a beam holds together down its
        // throw and a wash does not. Bounded so a pencil beam cannot push it out of the room.
        // The shaft itself is round, so an oval or rectangular lens is taken at its mean radius.
        let apex_offset = (aperture.element_sum() * 0.5 / optics.half_angle.tan().max(0.002))
            .min(MAX_APEX_OFFSET);
        for (cell_index, offset) in emitter.cells.offsets.iter().enumerate() {
            let (cell_intensity, cell_colour) = cells
                .get(cell_index)
                .copied()
                .unwrap_or((intensity, Vec3::from(value.colour)));
            let origin = pose.origin + pose.orientation * *offset;
            /*
             * The lit face is a surface, so it belongs to the views that draw surfaces. An outline
             * view has no lighting to reveal one and no models to hang one on: what it drew instead
             * was an ambient-lit solid at every lamp, which is the brightest thing in a picture
             * whose whole point is that the lines are the picture.
             */
            if style.emitter_apertures {
                push_aperture(
                    frame,
                    origin,
                    pose,
                    aperture,
                    emitter.optics.source.form,
                    cell_intensity,
                    cell_colour,
                );
            }
            if emitter.kind != EmitterKind::Beam {
                continue;
            }
            /*
             * The guideline is what a dark lamp has instead of a beam. Where the lamp is doing
             * something, its own line says the same thing and says more, so the dashes go: two
             * lines down one aim read as two aims, and a rig of them is a picture nobody can
             * count. What an operator needs the dashes for is the lamp that is off.
             */
            let lit = cell_intensity > 0.002;
            if style.aim_guides && !lit {
                push_aim_guide(frame, origin, pose, style.faint_ink);
            }
            if !lit {
                continue;
            }
            let light_index = push_gpu_light(
                frame,
                fixture,
                emitter,
                value,
                pose,
                origin,
                cell_intensity,
                cell_colour,
                optics,
                apex_offset,
            );
            if style.draw_beams {
                push_beam(
                    frame,
                    origin,
                    pose,
                    light_index,
                    cell_intensity,
                    cell_colour,
                    apex_offset,
                );
            }
            if style.draw_aim_lines {
                push_aim_line(frame, origin, pose, cell_intensity, cell_colour);
            }
        }
        frame.poses.push(pose);
    }
}

#[allow(clippy::too_many_arguments)]
fn push_gpu_light(
    frame: &mut FrameInstances,
    fixture: &FixtureInstance,
    emitter: &EmitterInstance,
    value: &EmitterValues,
    pose: EmitterPose,
    origin: Vec3,
    cell_intensity: f32,
    cell_colour: Vec3,
    optics: BeamOptics,
    apex_offset: f32,
) -> u32 {
    let light_index = frame.lights.len() as u32;
    // Which slot is in the beam, and what is etched on it. A profile that declares its own
    // wheel is divided into the slots it actually has and projects its own glass; one that
    // declares none keeps the drawn patterns, evenly divided.
    let wheel = &emitter.optics.gobo_wheel;
    let slots = if wheel.is_empty() {
        GOBO_SLOTS
    } else {
        wheel.len() as u32
    };
    let slot = value.gobo_slot(slots);
    let artwork = wheel
        .get(slot as usize)
        .and_then(|entry| entry.artwork)
        .map_or(-1.0, |layer| layer as f32);
    // The beam's own right axis: every pattern the head projects turns with the head.
    let tangent = (pose.orientation * Vec3::X).normalize_or(Vec3::X);
    // Where the blades sit is either the installed module pose or the desk's live pose.
    // A live canonical role owns that physical component completely; the two values are
    // never added into a competing second authority.
    let shaper_turn = if emitter.live_shaper_rotation_role {
        value.shaper_rotation_degrees.to_radians()
    } else if emitter.shaper_roles.iter().any(|supported| *supported) {
        fixture.shaper_degrees.unwrap_or(0.0).to_radians()
    } else {
        0.0
    };
    let shaper_angles = std::array::from_fn(|index| {
        if emitter.live_shaper_angle_roles[index] {
            value.shaper_blade_angles_degrees[index].to_radians()
        } else if emitter.shaper_roles[index] {
            fixture.installed_shaper_angles_degrees[index].to_radians()
        } else {
            0.0
        }
    });
    frame.lights.push(GpuLight {
        position_range: origin
            .extend(beam_reach(origin, pose.direction, pose.half_angle).max(2.0))
            .to_array(),
        direction_cos_outer: pose.direction.extend(pose.half_angle.cos()).to_array(),
        colour_intensity: (cell_colour * cell_intensity * optics.gain)
            .extend(cell_intensity)
            .to_array(),
        params: [
            (pose.half_angle * (1.0 - optics.feather).clamp(0.05, 1.0)).cos(),
            optics.feather,
            optics.uniformity,
            apex_offset,
        ],
        tangent_frost: rotate_about(tangent, pose.direction, shaper_turn)
            .extend(value.frost.clamp(0.0, 1.0))
            .to_array(),
        optics: [
            slot as f32,
            value.gobo_rotation * std::f32::consts::TAU,
            value.prism_facets() as f32,
            value.prism_rotation * std::f32::consts::TAU,
        ],
        shapers: value.shaper_blades,
        shaper_angles,
        gate: [artwork, 0.0, 0.0, 0.0],
        // Filled in once the frame knows which lights are worth a map.
        shadow: [-1.0, 0.0, 0.0, 0.0],
    });
    light_index
}

mod emitter_geometry;
mod media;
pub use emitter_geometry::{SemanticLight, semantic_lights};
use emitter_geometry::{aperture_size, cell_states, push_aperture, push_beam, push_laser_emitter};
/// Stage floor height in metres. Beams and aim lines stop here instead of running through the
/// deck.
pub(crate) const FLOOR_HEIGHT: f32 = 0.0;

/// Throw length before the beam meets the stage floor, bounded by the maximum throw.
/// How far the light actually reaches, which is further than the point straight below it.
///
/// A cone's rim lands further away than its axis does: a lamp four metres above the deck with a
/// forty-degree field reaches its own centre at four metres and the edge of its pool at four and
/// a half. Handing the axial distance to the shaders as the light's range culls every part of the
/// pool except the exact centre, which is why a lamp pointing straight down showed a single dot
/// instead of a pool.
fn beam_reach(origin: Vec3, direction: Vec3, half_angle: f32) -> f32 {
    let axial = beam_length(origin, direction);
    (axial / half_angle.cos().clamp(0.2, 1.0)).min(BEAM_THROW_METRES * 1.6)
}

fn beam_length(origin: Vec3, direction: Vec3) -> f32 {
    if direction.y >= -1e-3 {
        return BEAM_THROW_METRES;
    }
    let to_floor = (FLOOR_HEIGHT - origin.y) / direction.y;
    if to_floor <= 0.0 {
        return BEAM_THROW_METRES;
    }
    to_floor.clamp(0.25, BEAM_THROW_METRES)
}

fn push_aim_line(
    frame: &mut FrameInstances,
    origin: Vec3,
    pose: EmitterPose,
    intensity: f32,
    colour: Vec3,
) {
    let end =
        origin + pose.direction * beam_length(origin, pose.direction).min(BEAM_THROW_METRES * 0.55);
    /*
     * How bright the line is drawn, from the level the fixture is at.
     *
     * Curved rather than proportional, and with almost no floor. Nothing in this view is
     * tonemapped, so a line drawn at its literal level reads far brighter than the level is: half
     * and full looked nearly the same, and a lamp at one percent looked like a lamp that was on.
     * The curve puts the visible difference where an operator is working — half against full is a
     * real step now — and lets one percent be the barely-there line it should be.
     */
    let level = intensity.clamp(0.0, 1.0).powf(1.9).max(0.02);
    let near = colour.extend(level);
    let far = colour.extend(0.0);
    frame.lines.push(LineVertex {
        position: origin.to_array(),
        _pad: 0.0,
        colour: near.to_array(),
    });
    frame.lines.push(LineVertex {
        position: end.to_array(),
        _pad: 0.0,
        colour: far.to_array(),
    });
}

/// The twelve edges of a unit cube carried through `transform`.
///
/// What an outline view is made of. A rig drawn as outlines stays readable however many fixtures
/// are in it, because an outline hides nothing behind it: two heads at the same depth are both
/// still visible, which is exactly what a solid box in an unlit room cannot manage.
pub(crate) fn push_box_outline(
    frame: &mut FrameInstances,
    transform: Mat4,
    ink: Vec3,
    opacity: f32,
) {
    const CORNERS: [Vec3; 8] = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(-0.5, -0.5, 0.5),
        Vec3::new(-0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
        Vec3::new(-0.5, 0.5, 0.5),
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let colour = ink.extend(opacity);
    let world: Vec<Vec3> = CORNERS
        .iter()
        .map(|corner| transform.transform_point3(*corner))
        .collect();
    for (from, to) in EDGES {
        frame.line(world[from], world[to], colour, colour);
    }
}

/// The dotted line showing where an emitter is aimed, lit or not.
///
/// Dotted rather than solid, and drawn in the faint ink, so it never reads as light: a solid line
/// down the aim of every dark lamp in a rig is a picture of a hundred beams that are not on. It is
/// dashed by emitting the dashes as separate segments, which is what a line list can express — a
/// stipple pattern would be a whole pipeline for one kind of line.
fn push_aim_guide(frame: &mut FrameInstances, origin: Vec3, pose: EmitterPose, ink: Vec3) {
    /// Length of one dash and of the gap after it, in metres.
    const DASH: f32 = 0.22;
    const GAP: f32 = 0.28;
    /// Most dashes worth drawing down one guide, so a long throw cannot flood the line buffer.
    const MAX_DASHES: usize = 48;

    let reach = beam_length(origin, pose.direction).min(BEAM_THROW_METRES * 0.55);
    if reach <= DASH {
        return;
    }
    let colour = (ink * 1.3).extend(0.85);
    // Fading out along the throw keeps the far end of a long guide from cluttering the picture,
    // and reads the way an aim does: certain at the lamp, less so where it lands.
    let mut travelled = 0.0;
    let mut drawn = 0;
    while travelled < reach && drawn < MAX_DASHES {
        let start = travelled;
        let end = (travelled + DASH).min(reach);
        let fade = |along: f32| colour * Vec3::ONE.extend(1.0 - (along / reach) * 0.75);
        frame.line(
            origin + pose.direction * start,
            origin + pose.direction * end,
            fade(start),
            fade(end),
        );
        travelled = end + GAP;
        drawn += 1;
    }
}

mod crowd;
mod laser;
mod plot;
mod scenery;

#[cfg(test)]
mod tests;
