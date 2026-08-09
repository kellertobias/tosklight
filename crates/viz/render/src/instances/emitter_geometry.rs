use super::*;

pub(super) fn push_laser_emitter(
    frame: &mut FrameInstances,
    emitter: &EmitterInstance,
    value: &EmitterValues,
    scan: Option<&viz_scene::LaserScan>,
    pose: EmitterPose,
    intensity: f32,
    installed_colour: Vec3,
) {
    if let Some(scanner) = &emitter.laser
        && let Some(scan) = scan
    {
        laser::push_laser(
            &mut frame.lasers,
            pose.origin,
            pose.orientation,
            scanner,
            scan,
            intensity,
            installed_colour,
        );
    }
    // The window itself, lit. It is a few millimetres across and adds nothing to the room, but a
    // firing projector has a bright spot where its beam leaves and an operator finds the fixture
    // by it — the more so because a laser lights nothing else around itself. None of the cone or
    // shadow machinery applies beyond that.
    push_aperture(
        frame,
        pose.origin,
        pose,
        aperture_size(emitter),
        emitter.optics.source.form,
        intensity,
        Vec3::from(value.colour) * installed_colour,
    );
    frame.poses.push(pose);
}

/// One of this frame's lights in the terms an optical instrument is described in, rather than the
/// cosines and packed vectors [`GpuLight`] carries for the shaders.
///
/// Same lights, same order, same frame: [`semantic_lights`] walks the scene exactly as
/// [`push_emitters`] does and resolves each head through the same [`resolve_optics`], so a
/// consumer outside this renderer sees what the picture is made of.
#[derive(Clone, Copy, Debug)]
pub struct SemanticLight {
    /// Index into [`Scene::emitters`].
    pub emitter_index: u32,
    /// Which cell of that emitter, `0` for a single-cell head.
    pub cell_index: u32,
    /// World position of the lit surface, metres.
    pub origin: Vec3,
    /// Normalised aim.
    pub direction: Vec3,
    /// Half-angle of the field edge, radians, after zoom, iris and frost.
    pub outer_half_angle: f32,
    /// Half-angle where the field is still full, radians. The two are equal for a hard edge.
    pub inner_half_angle: f32,
    /// Rim softness, `0` a cut edge and `1` no edge to speak of.
    pub feather: f32,
    /// How evenly the field is filled, `1` flat to the rim.
    pub uniformity: f32,
    /// Linear colour, each channel `0..=1`.
    pub colour: Vec3,
    /// Visible level after dimmer, shutter and strobe, `0..=1`.
    pub intensity: f32,
    /// Relative engine output; `1.0` is an ordinary fixture of its class, before any dimmer.
    pub output: f32,
    /// Radius of the lit surface, metres. A lamp is not a point.
    pub source_radius: f32,
    /// How far the light reaches before it meets the floor, metres.
    pub reach: f32,
}

/// Every light this frame makes, in semantic form.
///
/// Only heads that are actually emitting appear, which is the same rule the frame's own light
/// array follows: a fixture at zero is in the picture as a body, not as a light.
pub fn semantic_lights(scene: &Scene, values: &SceneValues) -> Vec<SemanticLight> {
    let head_angles = head_angles(scene, values);
    let fallback = EmitterValues::default();
    let mut lights = Vec::new();
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
        let cells = cell_states(emitter, value, installed_colour);
        let aperture = aperture_size(emitter);
        for (cell_index, offset) in emitter.cells.offsets.iter().enumerate() {
            let (intensity, colour) = cells
                .get(cell_index)
                .copied()
                .unwrap_or((value.visible_intensity(), Vec3::from(value.colour)));
            if emitter.kind != EmitterKind::Beam || intensity <= 0.002 {
                continue;
            }
            let origin = pose.origin + pose.orientation * *offset;
            lights.push(SemanticLight {
                emitter_index: index as u32,
                cell_index: cell_index as u32,
                origin,
                direction: pose.direction,
                outer_half_angle: pose.half_angle,
                inner_half_angle: pose.half_angle * (1.0 - optics.feather).clamp(0.05, 1.0),
                feather: optics.feather,
                uniformity: optics.uniformity,
                colour,
                intensity,
                output: emitter.optics.output,
                source_radius: aperture.element_sum() * 0.5,
                reach: beam_reach(origin, pose.direction, pose.half_angle).max(2.0),
            });
        }
    }
    lights
}

/// The lit surface one cell of this emitter presents, in metres.
///
/// A lamp is not a point: a beam that springs from nothing out of a fixture half a metre across
/// reads as wrong from anywhere near it. The size and shape come from the emitter's own light
/// source, which belongs to the fixture rather than to one patched instance, bounded by how far
/// apart its cells sit so a bar's cells stay separate instead of merging into one smear.
pub(super) fn aperture_size(emitter: &EmitterInstance) -> Vec2 {
    let source = emitter.optics.source;
    let mut half = Vec2::new(source.width.max(0.01), source.height.max(0.01)) * 0.5;
    if let Some(spacing) = neighbouring_cell_spacing(emitter) {
        half = half.min(Vec2::splat(spacing * 0.45));
    }
    half.clamp(Vec2::splat(0.008), Vec2::splat(0.4))
}

/// How far apart neighbouring cells sit, or `None` for a single-cell emitter.
///
/// Layouts are generated in order, so consecutive entries are neighbours: comparing those is
/// enough to size a cell, and it stays linear on a matrix with a thousand of them.
fn neighbouring_cell_spacing(emitter: &EmitterInstance) -> Option<f32> {
    let mut closest = f32::INFINITY;
    for pair in emitter.cells.offsets.windows(2) {
        let spacing = (pair[1] - pair[0]).length();
        if spacing > 1e-4 {
            closest = closest.min(spacing);
        }
    }
    closest.is_finite().then_some(closest)
}

pub(super) fn cell_states(
    emitter: &EmitterInstance,
    value: &EmitterValues,
    installed_colour: Vec3,
) -> Vec<(f32, Vec3)> {
    let shutter = value.shutter.clamp(0.0, 1.0);
    if value.cells.is_empty() {
        let intensity = value.held_intensity.max(value.visible_intensity());
        return vec![
            (intensity, Vec3::from(value.colour) * installed_colour,);
            emitter.cells.len()
        ];
    }
    emitter
        .cells
        .offsets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let cell = value.cells.get(index).copied().unwrap_or_default();
            // A cell keeps its own tail: a chased pixel strip and a strobing blinder are both
            // per-cell effects, and holding only the head's level would smear the whole panel to
            // the brightness of its brightest pixel.
            let gated = (cell.intensity * shutter).clamp(0.0, 1.0);
            (
                cell.held_intensity.max(gated).clamp(0.0, 1.0),
                Vec3::from(cell.colour) * installed_colour,
            )
        })
        .collect()
}

pub(super) fn push_aperture(
    frame: &mut FrameInstances,
    origin: Vec3,
    pose: EmitterPose,
    aperture: Vec2,
    form: SourceForm,
    intensity: f32,
    colour: Vec3,
) {
    // The visible source stays present at zero intensity so a fixture never disappears.
    //
    // The gain lands a lamp at full just past the knee of the filmic curve rather than far up its
    // shoulder. Driven ten times harder — which is what this was — a lens is fully white by about
    // a tenth of the dimmer and every level above that draws the same, so an operator sees the
    // whole fade happen in the bottom of the fader and nothing at all in the rest of it.
    let radiance = colour * (0.02 + intensity * APERTURE_RADIANCE);
    // Light leaves a lamp through a face, and the face is what the operator recognises the lamp
    // by: a moving head's lens, a Fresnel's glass, the round lens of one blinder lamp, the front
    // of an LED strip. So the source is drawn as that face — round glass for a round or oval
    // source, the lit panel front for a rectangular one — and only as thick as such a face is.
    let face = aperture * 2.0;
    let thickness = (face.min_element() * SOURCE_THICKNESS).max(0.004);
    let mesh = match form {
        SourceForm::Round | SourceForm::Oval => MeshKind::Lens,
        SourceForm::Rectangular => MeshKind::Cube,
    };
    frame.mesh(mesh).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(face.extend(thickness), source_facing(pose), origin),
        colour * 0.05,
        0.35,
        radiance,
        0.0,
    ));
}

/// The frame a lit face is drawn in: `+Z` down the aim, `X` across the head's own width.
///
/// An emitter aims along its local `-Y`, which is the axis a lantern hangs pointing down. A face
/// has to stand across that aim rather than along it, and it has to keep the head's own width axis
/// so an oval PAR lens and a rectangular panel turn with the fixture instead of spinning in place.
fn source_facing(pose: EmitterPose) -> Quat {
    pose.orientation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
}

pub(super) fn push_beam(
    frame: &mut FrameInstances,
    origin: Vec3,
    pose: EmitterPose,
    light_index: u32,
    intensity: f32,
    colour: Vec3,
    apex_offset: f32,
) {
    // A beam stops where it lands. Without this the volumetric integral would keep scattering
    // below the deck, because clipping the view ray against scene depth cannot tell that the
    // light path itself is blocked.
    // The volume is drawn to the same reach, so the edges of the beam land on the deck at the
    // same time as its centre. Anything the cone drives through the floor is clipped by scene
    // depth in the volumetric pass.
    let length = beam_reach(origin, pose.direction, pose.half_angle);
    let tangent = pose.half_angle.tan().max(0.002);
    // The shaft leaves a lens, not a point, so the proxy is the cone this one is a frustum of:
    // it starts at the virtual apex behind the lens and the shader drops everything in front of
    // the lens itself.
    let total = apex_offset + length;
    // The unit cone points along `+Z` from its apex, so align `+Z` with the aim direction.
    let rotation = Quat::from_rotation_arc(Vec3::Z, pose.direction);
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(tangent * total, tangent * total, total),
        rotation,
        origin - pose.direction * apex_offset,
    );
    frame.beams.push(BeamInstance {
        model: model.to_cols_array_2d(),
        colour: colour.extend(intensity).to_array(),
        // The edge softness a beam is drawn with comes from its light, which the fragment shader
        // reads anyway, so the fourth slot is spare.
        params: [light_index as f32, length, apex_offset, 0.0],
    });
}
