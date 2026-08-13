//! Compile a patched show into a renderer scene plus the channel bindings that decode it.
//!
//! This runs once per patch revision. A DMX frame never re-enters this code.

use crate::binding::ChannelRef;
use crate::default_model::{self, FixtureTraits};
use crate::fallback::{self, OpticalClass};
use glam::{Quat, Vec3};
use light_fixture::{
    ChannelBehavior, FixtureMode, FixtureProfile, GeometryMotionKind, InstalledFixtureAppearance,
    LightSourceForm, PatchPolicy, ProfileEffect, ProfileLaser, ProfileOptics, ProfilePhysics,
    ProfilePhysicsSceneryKind, Vector3,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use viz_scene::{
    EffectProgram, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FallbackReason,
    FixtureBody, FixtureInstance, FixturePlanBinding, LaserOptics, LightSource, MotionAxis,
    PhysicsBody, PhysicsConstraints, PhysicsProgram, PhysicsSceneryObject, PlanFallback, Scene,
    SceneryKind, SceneryObject, SourceForm,
};

/// One physical placement of a logical fixture: the root fixture or one multi-patch instance.
#[derive(Clone, Debug)]
pub struct PhysicalInstance {
    pub instance_id: Uuid,
    pub name: String,
    /// Split number to `(logical universe, start address)`. An unpatched split is absent.
    pub split_patches: Vec<(u16, Option<(u16, u16)>)>,
    /// Metres, renderer world space.
    pub position: Vec3,
    /// Degrees about the world axes, applied `Rx * Ry * Rz`.
    pub rotation_degrees: Vec3,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    /// Degrees the mounting bracket is set to, positive nose-down. It tilts the whole fixture on
    /// the axis its yoke or clamp turns about, on top of the placement rotation.
    pub bracket_angle: f32,
    /// Degrees a fitted shaper or barn-door module is turned to, or `None` when none is fitted.
    /// A framing module the desk can rotate starts from here.
    pub shaper_angle: Option<f32>,
    /// Portable source/filter and static blade settings for this exact physical instance.
    pub installed_appearance: InstalledFixtureAppearance,
}

/// One logical fixture with its selected immutable profile revision.
#[derive(Clone, Debug)]
pub struct PatchedFixture {
    pub fixture_id: Uuid,
    pub name: String,
    pub number: Option<u32>,
    pub profile: Arc<FixtureProfile>,
    pub mode_id: Uuid,
    pub instances: Vec<PhysicalInstance>,
}

/// Everything one emitter needs from DMX.
#[derive(Clone, Debug, Default)]
pub struct EmitterBinding {
    pub intensity: Option<ChannelRef>,
    pub pan: Option<ChannelRef>,
    pub tilt: Option<ChannelRef>,
    pub zoom: Option<ChannelRef>,
    pub iris: Option<ChannelRef>,
    pub focus: Option<ChannelRef>,
    pub frost: Option<ChannelRef>,
    pub shutter: Option<ChannelRef>,
    pub strobe: Option<ChannelRef>,
    /// Gobo wheel position and its rotation, from the first gobo wheel the mode carries.
    pub gobo: Option<ChannelRef>,
    pub gobo_rotation: Option<ChannelRef>,
    pub prism: Option<ChannelRef>,
    pub prism_rotation: Option<ChannelRef>,
    /// Framing-shutter blade insertions, in the order the attribute registry numbers them.
    pub shaper_blades: [Option<ChannelRef>; 4],
    /// Framing-shutter blade angles, in physical degrees.
    pub shaper_blade_angles: [Option<ChannelRef>; 4],
    pub shaper_rotation: Option<ChannelRef>,
    pub fog: Option<ChannelRef>,
    pub colour: ColourBinding,
    /// Per-cell colour bindings for a multi-cell emitter. Empty means the emitter is uniform.
    pub cells: Vec<ColourBinding>,
    /// Logical universes this emitter reads, used to re-decode only what changed.
    pub universes: Vec<u16>,
    /// Applied after decoding, from the physical instance's axis-inversion policy.
    pub invert_pan: bool,
    pub invert_tilt: bool,
    /// The whole fixture's DMX window, present only on a laser.
    ///
    /// A laser's scan engine is handed its fixture's raw slots rather than decoded parameters,
    /// because the mapping from DMX to picture lives inside the script and no canonical attribute
    /// set describes it. Every other emitter reads named channels; this one reads the footprint.
    pub laser_window: Option<LaserWindow>,
    /// The whole raw fixture window supplied to an Effect program.
    pub effect_window: Option<EffectWindow>,
    /// Raw fixture footprint for the first physics body owned by this fixture instance.
    pub physics_window: Option<PhysicsWindow>,
}

/// The one fully patched virtual-camera fixture a dedicated external Visualizer may follow.
#[derive(Clone, Debug)]
pub struct ExternalCameraBinding {
    pub fixture_id: Uuid,
    pub instance_id: Uuid,
    pub label: String,
    pub x: ChannelRef,
    pub y: ChannelRef,
    pub z: ChannelRef,
    pub yaw: ChannelRef,
    pub pitch: ChannelRef,
    pub roll: ChannelRef,
    pub zoom: ChannelRef,
    pub universes: Vec<u16>,
}

/// A laser fixture's raw DMX footprint, resolved to absolute addresses.
#[derive(Clone, Debug)]
pub struct LaserWindow {
    pub logical_universe: u16,
    /// Absolute 1-based DMX addresses in patch order. Index `n` here is `input.dmx[n]` in a script.
    pub slots: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct EffectWindow {
    pub logical_universe: u16,
    pub slots: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct PhysicsWindow {
    pub body_index: usize,
    pub logical_universe: u16,
    pub slots: Vec<u16>,
}

/// Colour channels for one emitter or one cell.
#[derive(Clone, Debug, Default)]
pub struct ColourBinding {
    pub red: Option<ChannelRef>,
    pub green: Option<ChannelRef>,
    pub blue: Option<ChannelRef>,
    pub white: Option<ChannelRef>,
    pub amber: Option<ChannelRef>,
    pub ultraviolet: Option<ChannelRef>,
    pub cold_white: Option<ChannelRef>,
    pub warm_white: Option<ChannelRef>,
    pub cyan: Option<ChannelRef>,
    pub magenta: Option<ChannelRef>,
    pub yellow: Option<ChannelRef>,
    pub wheel: Option<ChannelRef>,
    /// Per-cell intensity, present on pixel modes that dim each cell.
    pub intensity: Option<ChannelRef>,
}

impl ColourBinding {
    pub fn is_empty(&self) -> bool {
        self.red.is_none()
            && self.green.is_none()
            && self.blue.is_none()
            && self.white.is_none()
            && self.amber.is_none()
            && self.ultraviolet.is_none()
            && self.cold_white.is_none()
            && self.warm_white.is_none()
            && self.cyan.is_none()
            && self.magenta.is_none()
            && self.yellow.is_none()
            && self.wheel.is_none()
    }
}

/// Where a fixture's beam starts and what it turns about, read off the body being drawn.
///
/// A profile that describes its own emitter geometry keeps it. Everything else — most of the
/// library, and everything imported from a patch sheet — has nothing to say, and the answer it
/// used to get was the fixture's origin. In the shipped model set that is the rigging point, so
/// the beam started at the clamp and a moving head's beam swung about the clamp too.
#[derive(Clone, Copy, Debug)]
struct EmitterMount {
    /// Emitter origin in fixture-local metres.
    origin: Vec3,
    /// What tilt turns it about, in the same space.
    pivot: Vec3,
    /// Which way the emitting face looks, in the same space.
    aim: Vec3,
    /// How big the model's own emitting face is, where the model has one.
    face: Option<glam::Vec2>,
}

impl Default for EmitterMount {
    fn default() -> Self {
        Self {
            face: None,
            origin: Vec3::ZERO,
            pivot: Vec3::ZERO,
            aim: Vec3::NEG_Y,
        }
    }
}

impl EmitterMount {
    /// Read the lens and the trunnions off a model, at the scale that model is drawn.
    fn from_model(model: &viz_scene::FixtureModel, body_size: Vec3) -> Self {
        let Some(anchor) = model.emitter_anchor else {
            return Self::default();
        };
        let scale = model.scale_to(body_size);
        Self {
            // The model is drawn at the fixture's size, so its lens scales with it.
            face: model.emitter_size.map(|size| size * scale),
            origin: anchor * scale,
            // Only a model with something that tilts has trunnions worth turning about.
            pivot: if model.has_head {
                model.head_pivot * scale
            } else {
                Vec3::ZERO
            },
            aim: model.emitter_axis.unwrap_or(Vec3::NEG_Y),
        }
    }

    /// The emitter's rest orientation, as the Euler degrees the scene stores.
    ///
    /// Every emitter aims along local `-Y`, so a body whose emitting face looks somewhere else is
    /// expressed as the turn from one to the other. A lantern's is the identity.
    fn rest_orientation(&self) -> Vec3 {
        if self.aim.abs_diff_eq(Vec3::NEG_Y, 1e-4) {
            return Vec3::ZERO;
        }
        let (x, y, z) = Quat::from_rotation_arc(Vec3::NEG_Y, self.aim.normalize_or(Vec3::NEG_Y))
            .to_euler(glam::EulerRot::XYZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }
}

/// A compiled scene and its bindings, parallel to `scene.emitters`.
pub struct ScenePlan {
    pub scene: Scene,
    pub bindings: Vec<EmitterBinding>,
    pub external_camera: Option<ExternalCameraBinding>,
    /// Actionable reason no DMX camera was selected when camera fixtures were ambiguous/invalid.
    pub external_camera_issue: Option<String>,
    pub warnings: Vec<String>,
}

/// The model a profile is drawn with, read once however many instances are patched from it.
///
/// A model that cannot be read leaves the fixture on its procedural proxy and says why, rather
/// than leaving a hole on the stage. A profile that names no model is the common case rather than
/// an error — most of the library and everything imported from a patch sheet arrives without one —
/// and gets the shipped body its type and channels imply, parsed once for every profile that
/// lands on the same one.
fn resolve_model(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    scene: &mut Scene,
    models: &mut std::collections::HashMap<light_core::FixtureId, Option<u32>>,
    defaults: &mut std::collections::HashMap<&'static str, u32>,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    match models.entry(fixture.profile.id) {
        std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            let resolved = match fixture.profile.model_asset.as_deref() {
                Some(asset) => match read_model_asset(asset) {
                    Ok(model) => {
                        scene.models.push(model);
                        Some(scene.models.len() as u32 - 1)
                    }
                    Err(reason) => {
                        warnings.push(format!(
                            "{} {}: {reason}; drawing the built-in body instead",
                            fixture.profile.manufacturer, fixture.profile.name
                        ));
                        None
                    }
                },
                None => {
                    let chosen = default_model::choose(&fixture.profile.fixture_type, traits(mode));
                    match defaults.entry(chosen.name) {
                        std::collections::hash_map::Entry::Occupied(cached) => Some(*cached.get()),
                        std::collections::hash_map::Entry::Vacant(cached) => {
                            match viz_scene::read_glb(chosen.bytes) {
                                Ok(model) => {
                                    scene.models.push(model);
                                    let index = scene.models.len() as u32 - 1;
                                    cached.insert(index);
                                    Some(index)
                                }
                                Err(reason) => {
                                    warnings
                                        .push(format!("shipped model {}: {reason}", chosen.name));
                                    None
                                }
                            }
                        }
                    }
                }
            };
            *entry.insert(resolved)
        }
    }
}

fn resolve_plan_artwork(
    fixture: &PatchedFixture,
    scene: &mut Scene,
    cache: &mut HashMap<light_core::FixtureId, [Option<u32>; 5]>,
    warnings: &mut Vec<String>,
) -> [Option<u32>; 5] {
    if let Some(indices) = cache.get(&fixture.profile.id) {
        return *indices;
    }
    let mut indices = [None; 5];
    if let Some(projections) = fixture.profile.projection_assets.as_ref() {
        for projection in &projections.views {
            match assets::read_plan_artwork(projection) {
                Ok(artwork) => {
                    let index = scene.plan_artwork.len() as u32;
                    indices[artwork.view.index()] = Some(index);
                    scene.plan_artwork.push(artwork);
                }
                Err(reason) => warnings.push(format!(
                    "{} {} {} projection: {reason}; using renderer fallback",
                    fixture.profile.manufacturer,
                    fixture.profile.name,
                    projection.view.wire()
                )),
            }
        }
    }
    cache.insert(fixture.profile.id, indices);
    indices
}

/// Compile the patch into a scene. Fixtures with `VisualOnly` policy become scenery elsewhere and
/// are skipped here.
pub fn compile(fixtures: &[PatchedFixture]) -> ScenePlan {
    let mut scene = Scene::default();
    let mut bindings = Vec::new();
    let mut external_camera = None;
    let mut external_camera_issue = None;
    let mut warnings = Vec::new();
    let mut models: std::collections::HashMap<light_core::FixtureId, Option<u32>> =
        std::collections::HashMap::new();
    // Shipped default bodies are shared by every profile that lands on the same one.
    let mut defaults: std::collections::HashMap<&'static str, u32> =
        std::collections::HashMap::new();
    // Gobo artwork, keyed by the asset it came from: a rig of twenty identical profile heads
    // decodes each piece of glass once.
    let mut artwork: std::collections::HashMap<String, Option<u32>> =
        std::collections::HashMap::new();
    let mut plan_artwork: HashMap<light_core::FixtureId, [Option<u32>; 5]> = HashMap::new();

    for fixture in fixtures {
        // Visual-only Venue objects are compiled by the desk's scenery path. Keeping them out of
        // the fixture plan prevents a curtain, pipe, or truss from acquiring a lamp icon merely
        // because it arrived through the portable fixture-package schema.
        if fixture.profile.patch_policy == PatchPolicy::VisualOnly {
            continue;
        }
        let Some((mode, primary_slots)) = selected_mode(fixture, &mut warnings) else {
            continue;
        };
        let class = fallback::classify(&fixture.profile.fixture_type);
        let motion = motion_axes(mode);
        let moving = fallback::is_moving(motion.pan.is_some(), motion.tilt.is_some());
        // A profile that carries a model gets it read once, however many instances are patched
        // from it. A model that cannot be read leaves the fixture on its procedural proxy and
        // says why, rather than leaving a hole on the stage.
        let model = resolve_model(
            fixture,
            mode,
            &mut scene,
            &mut models,
            &mut defaults,
            &mut warnings,
        );
        let plan_projections =
            resolve_plan_artwork(fixture, &mut scene, &mut plan_artwork, &mut warnings);
        let plan_fallback = if fallback::has_generic_plan_type(&fixture.profile.fixture_type) {
            PlanFallback::GenericType
        } else {
            PlanFallback::UnknownBox
        };
        scene.fixture_plan.push(FixturePlanBinding {
            fixture_id: fixture.fixture_id,
            artwork: plan_projections,
            fallback: plan_fallback,
        });

        let body_size = fallback::body_size(
            class,
            moving,
            fixture.profile.physical.width_millimetres,
            fixture.profile.physical.height_millimetres,
            fixture.profile.physical.depth_millimetres,
        );
        // What light out of this fixture looks like. It belongs to the profile, not to one
        // patched instance, so it is resolved once and shared by every instance of it.
        let mut optics = fallback::emitter_optics(
            class,
            body_size,
            fixture.profile.physical.luminous_output_lumens,
        );
        apply_declared_optics(&mut optics, &fixture.profile.optics);
        // A laser's scanner and its scan engine belong to the profile too, and the script is
        // compiled per patched fixture rather than per profile, so the source is shared by handle.
        let laser = class.is_laser().then(|| {
            let resolved = laser_optics(fixture.profile.laser.as_ref());
            if resolved.script.is_none() {
                warnings.push(format!(
                    "{} {} is a laser but its profile ships no scan script; it will not project",
                    fixture.profile.manufacturer, fixture.profile.name
                ));
            }
            resolved
        });
        let effect = class.is_effect().then(|| {
            let resolved = effect_program(fixture.profile.effect.as_ref());
            if resolved.script.is_none() {
                warnings.push(format!(
                    "{} {} is an Effect fixture but ships no effect script; it will remain off",
                    fixture.profile.manufacturer, fixture.profile.name
                ));
            }
            resolved
        });
        let physics = fixture.profile.physics.as_ref().map(physics_program);
        // The wheel this fixture turns, if its package carries one. Artwork is shared by handle:
        // one piece of glass declared by twenty fixtures is decoded once and lives in the scene
        // once.
        optics.gobo_wheel = gobo_wheel(&fixture.profile, &mut scene, &mut artwork, &mut warnings);
        // Where this fixture's light leaves it, taken from the body being drawn. Only used when
        // the profile does not describe its own optics, which is the common case.
        let mount = model
            .and_then(|index| scene.models.get(index as usize))
            .map(|body| EmitterMount::from_model(body, body_size))
            .unwrap_or_default();
        apply_model_source_face(&mut optics, fixture, mount, body_size);
        compile_instances(
            &mut scene,
            &mut bindings,
            &mut external_camera,
            &mut external_camera_issue,
            &mut warnings,
            fixture,
            mode,
            &primary_slots,
            class,
            &motion,
            body_size,
            moving,
            model,
            optics,
            mount,
            laser,
            effect,
            physics,
        );
    }
    scene.recompute_bounds();
    ScenePlan {
        scene,
        bindings,
        external_camera,
        external_camera_issue,
        warnings,
    }
}

fn apply_model_source_face(
    optics: &mut EmitterOptics,
    fixture: &PatchedFixture,
    mount: EmitterMount,
    body_size: Vec3,
) {
    /*
     * The lit face is the size of the lens it is drawn on.
     *
     * A profile that states its own light source is the authority and keeps it. Everything else
     * — which is most of the library — was getting a size from its class: one number per kind of
     * lantern, with no relation to the model it lands on. A 150 mm guess in the middle of a
     * 260 mm lens is why the thing projecting the light looked several times bigger than the
     * light leaving it.
     *
     * The model knows, because the lens is geometry in it. It already told us where that
     * geometry is and which way it looks; this is the third thing it can answer.
     */
    if fixture.profile.optics.light_source.is_none()
        && let Some(face) = mount.face
    {
        // Never wider than the lantern it is set into, which is the rule the help states and
        // the last guard against a model whose emitting geometry is modelled generously.
        let face = face.min(glam::Vec2::new(body_size.x, body_size.y.max(body_size.z)));
        match optics.source.form {
            // A round lens is one number, so a face measured a little off-square is taken at
            // its mean rather than being drawn as an oval nobody built.
            SourceForm::Round => {
                let diameter = (face.x + face.y) * 0.5;
                optics.source.width = diameter;
                optics.source.height = diameter;
            }
            SourceForm::Oval | SourceForm::Rectangular => {
                optics.source.width = face.x;
                optics.source.height = face.y;
            }
        }
    }
}

fn external_camera_binding(
    fixture: &PatchedFixture,
    instance: &PhysicalInstance,
    mode: &FixtureMode,
    channels: &HashMap<Uuid, ChannelRef>,
) -> Result<Option<ExternalCameraBinding>, String> {
    let mut found: HashMap<&str, ChannelRef> = HashMap::new();
    for channel in &mode.channels {
        let identity = [
            channel.attribute.0.as_str(),
            channel.fixture_attribute.0.as_str(),
        ]
        .into_iter()
        .find(|identity| identity.starts_with("camera."));
        let Some(identity) = identity else { continue };
        if let Some(reference) = channels.get(&channel.id) {
            found.entry(identity).or_insert_with(|| reference.clone());
        }
    }
    // A declared but unpatched virtual camera has no authority and leaves the last pose alone.
    if found.is_empty() {
        return Ok(None);
    }
    let label = format!("{} ({})", instance.name, fixture.profile.name);
    let mut take = |identity: &'static str, bytes: usize| -> Result<ChannelRef, String> {
        let reference = found
            .remove(identity)
            .ok_or_else(|| format!("{label} camera mode is missing {identity}"))?;
        if reference.slots.len() != bytes {
            return Err(format!(
                "{label} {identity} must use {bytes} DMX bytes, got {}",
                reference.slots.len()
            ));
        }
        Ok(reference)
    };
    let x = take("camera.position.x", 3)?;
    let y = take("camera.position.y", 3)?;
    let z = take("camera.position.z", 3)?;
    let yaw = take("camera.yaw", 2)?;
    let pitch = take("camera.pitch", 2)?;
    let roll = take("camera.roll", 2)?;
    let zoom = take("camera.zoom", 2)?;
    let mut universes = [&x, &y, &z, &yaw, &pitch, &roll, &zoom]
        .into_iter()
        .map(|reference| reference.logical_universe)
        .collect::<Vec<_>>();
    universes.sort_unstable();
    universes.dedup();
    Ok(Some(ExternalCameraBinding {
        fixture_id: fixture.fixture_id,
        instance_id: instance.instance_id,
        label,
        x,
        y,
        z,
        yaw,
        pitch,
        roll,
        zoom,
        universes,
    }))
}

/// Resolve the selected mode and its primary-slot map, recording a fixture-scoped warning when
/// either half of that immutable profile revision cannot be compiled.
fn selected_mode<'a>(
    fixture: &'a PatchedFixture,
    warnings: &mut Vec<String>,
) -> Option<(&'a FixtureMode, HashMap<Uuid, u16>)> {
    let Some(mode) = fixture
        .profile
        .modes
        .iter()
        .find(|mode| mode.id == fixture.mode_id)
    else {
        warnings.push(format!(
            "{}: selected mode is missing from profile revision {}",
            fixture.name, fixture.profile.revision
        ));
        return None;
    };
    match mode.primary_slots() {
        Ok(slots) => Some((mode, slots)),
        Err(error) => {
            warnings.push(format!("{}: {error}", fixture.name));
            None
        }
    }
}

/// Split number to `(logical universe, base address)` for one physical instance.
fn address_map(instance: &PhysicalInstance) -> HashMap<u16, (u16, u16)> {
    instance
        .split_patches
        .iter()
        .filter_map(|(split, address)| address.map(|address| (*split, address)))
        .collect()
}

/// Every channel of the mode resolved to absolute addresses, keyed by channel id.
fn compile_channels(
    mode: &FixtureMode,
    primary_slots: &HashMap<Uuid, u16>,
    addresses: &HashMap<u16, (u16, u16)>,
) -> HashMap<Uuid, ChannelRef> {
    let mut compiled = HashMap::with_capacity(mode.channels.len());
    for channel in &mode.channels {
        if channel.behavior == ChannelBehavior::Static {
            continue;
        }
        let Some((universe, base)) = addresses.get(&channel.split).copied() else {
            continue;
        };
        let Some(primary) = primary_slots.get(&channel.id).copied() else {
            continue;
        };
        let mut slots = Vec::with_capacity(channel.resolution.bytes());
        slots.push(base + primary - 1);
        for slot in &channel.secondary_slots {
            slots.push(base + slot - 1);
        }
        slots.truncate(channel.resolution.bytes());
        if slots.iter().any(|slot| *slot == 0 || *slot > 512) {
            continue;
        }
        compiled.insert(
            channel.id,
            ChannelRef {
                logical_universe: universe,
                slots,
                max_raw: channel.resolution.max_raw(),
                invert: channel.invert,
                physical_min: channel.physical_min.unwrap_or(0.0),
                physical_max: channel.physical_max.unwrap_or(1.0),
                snap: channel.snap,
                default_raw: channel.default_raw,
                functions: channel.functions.clone(),
            },
        );
    }
    compiled
}

#[derive(Default)]
struct MotionAxes {
    pan: Option<MotionAxis>,
    tilt: Option<MotionAxis>,
    /// Node id chain used to place an emitter relative to the moving head.
    tilt_node: Option<Uuid>,
}

/// What a mode has channels for, as the default-model rules want it.
fn traits(mode: &FixtureMode) -> FixtureTraits {
    let mut traits = FixtureTraits::default();
    for channel in &mode.channels {
        traits.observe(
            channel.attribute.0.as_str(),
            channel.attribute.is_intensity(),
        );
    }
    traits
}

/// Read pan and tilt travel from the geometry graph rather than assuming it.
fn motion_axes(mode: &FixtureMode) -> MotionAxes {
    let mut axes = MotionAxes::default();
    for node in &mode.geometry.nodes {
        let Some(motion) = &node.motion else { continue };
        if motion.kind != GeometryMotionKind::Rotation {
            continue;
        }
        let axis = MotionAxis {
            axis: vector(motion.axis),
            min_degrees: motion.physical_min,
            max_degrees: motion.physical_max,
        };
        match motion.attribute.0.as_str() {
            "pan" => axes.pan = Some(axis),
            "tilt" => {
                axes.tilt = Some(axis);
                axes.tilt_node = Some(node.id);
            }
            _ => {}
        }
    }
    axes
}

fn vector(value: Vector3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

/// Millimetres in the profile's own space to metres in renderer space.
pub(super) fn millimetres(value: Vector3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z) / 1000.0
}

#[allow(clippy::too_many_arguments)]
fn build_emitters(
    scene: &mut Scene,
    bindings: &mut Vec<EmitterBinding>,
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    class: OpticalClass,
    motion: &MotionAxes,
    instance: &PhysicalInstance,
    fixture_index: u32,
    channels: &HashMap<Uuid, ChannelRef>,
    optics: EmitterOptics,
    mount: EmitterMount,
    laser: Option<LaserOptics>,
    effect: Option<EffectProgram>,
) {
    // Resolved once for the fixture: every head of a laser reads the same footprint, because the
    // script is given the whole fixture rather than one head's channels.
    let laser_window = laser.as_ref().and_then(|_| laser_window(channels));
    let effect_window = effect.as_ref().and_then(|_| effect_window(channels));
    // A laser's scan engine is handed the fixture's whole window and answers with the deflection
    // of every point it draws, so its position channels are already in the figure. Letting the
    // desk swing the head on the same channels would apply them a second time, and through a pan
    // range a scanner does not have: a laser is aimed by the bracket it hangs in, not by the desk.
    let steered = !class.is_laser();
    if mode.geometry.emitters.is_empty() {
        build_fallback_emitters(
            scene,
            bindings,
            fixture,
            mode,
            class,
            motion,
            instance,
            fixture_index,
            channels,
            optics,
            mount,
            laser,
            effect,
            &laser_window,
            &effect_window,
            steered,
        );
        return;
    }

    let head_channels = group_by_head(mode, channels);
    for emitter in &mode.geometry.emitters {
        let owned = head_channels
            .get(&emitter.head_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut binding = build_binding(owned, instance, mode, emitter.head_id, channels);
        let head_index = mode
            .heads
            .iter()
            .position(|head| head.id == emitter.head_id)
            .unwrap_or(0) as u16;
        let cells = layout_cells(emitter);
        if cells.len() > 1 {
            binding.cells = cell_bindings(owned, cells.len());
        }
        let directional = emitter.directional && class.is_directional();
        // A laser's geometry describes where its window is, not a field it projects, so the class
        // decides the kind here exactly as it does for a head with no geometry at all.
        let kind = if class.is_laser() {
            EmitterKind::Laser
        } else if class.is_effect() {
            EmitterKind::Effect
        } else if class == OpticalClass::Atmosphere || binding.fog.is_some() {
            EmitterKind::Atmosphere
        } else if directional {
            EmitterKind::Beam
        } else {
            EmitterKind::Emissive
        };
        let (narrow, wide) = zoom_refined(
            emitter.beam_angle_degrees,
            emitter.field_angle_degrees,
            &binding,
        );
        scene.emitters.push(EmitterInstance {
            fixture_index,
            head_index,
            label: emitter.name.clone(),
            local_origin: millimetres(emitter.origin),
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: vector(emitter.orientation_degrees),
            pan: steered.then(|| pan_axis(motion, &binding)).flatten(),
            tilt: steered.then(|| tilt_axis(motion, &binding)).flatten(),
            beam_angle_degrees: narrow,
            field_angle_degrees: wide,
            // A profile that describes its own edge softness is describing this head's rim.
            optics: EmitterOptics {
                sharpness: if emitter.feather > 0.0 {
                    (1.0 - emitter.feather).clamp(0.0, 1.0)
                } else {
                    optics.sharpness
                },
                ..optics.clone()
            },
            kind,
            cells: EmitterLayoutCells { offsets: cells },
            laser: (kind == EmitterKind::Laser)
                .then(|| laser.clone())
                .flatten(),
            effect: (kind == EmitterKind::Effect)
                .then(|| effect.clone())
                .flatten(),
            live_shaper_angle_roles: std::array::from_fn(|index| {
                binding.shaper_blade_angles[index].is_some()
            }),
            shaper_roles: std::array::from_fn(|index| {
                binding.shaper_blades[index].is_some()
                    || binding.shaper_blade_angles[index].is_some()
            }),
            live_shaper_rotation_role: binding.shaper_rotation.is_some(),
        });
        binding.laser_window = laser_window.clone();
        binding.effect_window = effect_window.clone();
        bindings.push(binding);
    }
}

/// Heads that represent emitting hardware in an invented fallback layout.
///
/// A shared master is a control group for its children. It is retained only when it is the
/// profile's sole head, where there is no child geometry to stand in for it.
fn physical_head_indices(mode: &FixtureMode) -> Vec<usize> {
    let physical: Vec<usize> = mode
        .heads
        .iter()
        .enumerate()
        .filter_map(|(index, head)| (!head.master_shared).then_some(index))
        .collect();
    if physical.is_empty() {
        (0..mode.heads.len()).collect()
    } else {
        physical
    }
}

/// Build the deliberately invented emitter layout used when a profile has no emitter geometry.
#[allow(clippy::too_many_arguments)]
fn build_fallback_emitters(
    scene: &mut Scene,
    bindings: &mut Vec<EmitterBinding>,
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    class: OpticalClass,
    motion: &MotionAxes,
    instance: &PhysicalInstance,
    fixture_index: u32,
    channels: &HashMap<Uuid, ChannelRef>,
    optics: EmitterOptics,
    mount: EmitterMount,
    laser: Option<LaserOptics>,
    effect: Option<EffectProgram>,
    laser_window: &Option<LaserWindow>,
    effect_window: &Option<EffectWindow>,
    steered: bool,
) {
    let head_channels = group_by_head(mode, channels);
    // A shared master head is a control group, not another physical lamp. Profiles for pixel bars
    // commonly put it before the real cells; counting it in the fallback layout creates one extra
    // face and spreads the row past both ends of the body.
    let physical_heads = physical_head_indices(mode);
    let physical_head_count = physical_heads.len();
    let body_width = scene
        .fixtures
        .get(fixture_index as usize)
        .map_or_else(|| head_span(class), |fixture| fixture.body.size.x);
    // The layout below is invented, so the faces it places have to be trimmed to it: a row of
    // lamp lenses wider than the pitch they are spread at would merge into one smear.
    let head_optics = fitted_to_head_pitch(&optics, physical_head_count, body_width);
    // Fallback: one emitter per logical head, aimed along the head's rest direction.
    for (physical_index, head_index) in physical_heads.into_iter().enumerate() {
        let head = &mode.heads[head_index];
        // An unpatched head has no channels. It stays in the scene and stays visible; only
        // its DMX is suppressed until the fixture is patched again.
        let owned = head_channels
            .get(&head.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut binding = build_binding(owned, instance, mode, head.id, channels);
        let kind = emitter_kind(class, &binding);
        let (narrow, wide) = cone_angles(class, &binding);
        scene.emitters.push(EmitterInstance {
            fixture_index,
            head_index: head_index as u16,
            label: head.name.clone(),
            local_origin: mount.origin
                + head_offset(
                    physical_index,
                    physical_head_count,
                    head_optics.source.width,
                    body_width,
                ),
            tilt_pivot: mount.pivot,
            local_orientation_degrees: mount.rest_orientation(),
            pan: steered.then(|| pan_axis(motion, &binding)).flatten(),
            tilt: steered.then(|| tilt_axis(motion, &binding)).flatten(),
            beam_angle_degrees: narrow,
            field_angle_degrees: wide,
            optics: head_optics.clone(),
            kind,
            cells: EmitterLayoutCells::single(),
            laser: (kind == EmitterKind::Laser)
                .then(|| laser.clone())
                .flatten(),
            effect: (kind == EmitterKind::Effect)
                .then(|| effect.clone())
                .flatten(),
            live_shaper_angle_roles: std::array::from_fn(|index| {
                binding.shaper_blade_angles[index].is_some()
            }),
            shaper_roles: std::array::from_fn(|index| {
                binding.shaper_blades[index].is_some()
                    || binding.shaper_blade_angles[index].is_some()
            }),
            live_shaper_rotation_role: binding.shaper_rotation.is_some(),
        });
        binding.laser_window = laser_window.clone();
        binding.effect_window = effect_window.clone();
        bindings.push(binding);
    }
    if mode.heads.is_empty() {
        // A profile with no heads at all still gets one generic emitter so it is visible.
        headless_emitter(
            scene,
            bindings,
            fixture,
            class,
            instance,
            fixture_index,
            optics,
            mount,
            laser,
            effect,
            laser_window,
            effect_window,
        );
    }
}

/// A profile with no heads at all still gets one generic emitter, so a fixture that describes
/// nothing about itself is still on the stage rather than missing from it.
#[allow(clippy::too_many_arguments)]
fn headless_emitter(
    scene: &mut Scene,
    bindings: &mut Vec<EmitterBinding>,
    fixture: &PatchedFixture,
    class: OpticalClass,
    instance: &PhysicalInstance,
    fixture_index: u32,
    optics: EmitterOptics,
    mount: EmitterMount,
    laser: Option<LaserOptics>,
    effect: Option<EffectProgram>,
    laser_window: &Option<LaserWindow>,
    effect_window: &Option<EffectWindow>,
) {
    let mut binding = EmitterBinding {
        invert_pan: instance.invert_pan,
        invert_tilt: instance.invert_tilt,
        ..EmitterBinding::default()
    };
    let (narrow, wide) = class.cone_angles();
    scene.emitters.push(EmitterInstance {
        fixture_index,
        head_index: 0,
        label: fixture.name.clone(),
        local_origin: mount.origin,
        tilt_pivot: mount.pivot,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: narrow,
        field_angle_degrees: wide,
        optics: optics.clone(),
        kind: emitter_kind(class, &binding),
        cells: EmitterLayoutCells::single(),
        laser: laser.clone(),
        effect: effect.clone(),
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    binding.laser_window = laser_window.clone();
    binding.effect_window = effect_window.clone();
    bindings.push(binding);
}

/// The scanner a laser profile describes, with the gaps filled from what a show laser typically is.
///
/// Written to be usable from a profile that declares nothing at all: a package that names its
/// fixture type as a laser and ships a script gets a working projector without anyone having
/// measured a scan angle. What it cannot supply is the script — an invented pattern would be a
/// lie about what the fixture does, so a laser with no engine stays dark and is reported.
fn laser_optics(declared: Option<&ProfileLaser>) -> LaserOptics {
    let mut optics = LaserOptics::default();
    let Some(declared) = declared else {
        return optics;
    };
    if let Some(script) = declared.scan_script_asset.as_deref()
        && let Some(source) = decode_script(script)
    {
        optics.script_key = script_key(&source);
        optics.script = Some(source.into());
    }
    if let Some(degrees) = declared.scan_angle_degrees.filter(|value| *value > 0.0) {
        optics.scan_half_angle_x = degrees.clamp(1.0, 180.0).to_radians() * 0.5;
        optics.scan_half_angle_y = optics.scan_half_angle_x;
    }
    if let Some(degrees) = declared.scan_angle_y_degrees.filter(|value| *value > 0.0) {
        optics.scan_half_angle_y = degrees.clamp(1.0, 180.0).to_radians() * 0.5;
    }
    if let Some(rate) = declared.points_per_second.filter(|value| *value > 0.0) {
        optics.points_per_second = rate.clamp(100.0, 500_000.0);
    }
    if let Some(divergence) = declared
        .divergence_milliradians
        .filter(|value| *value > 0.0)
    {
        optics.divergence = divergence.clamp(0.05, 50.0) / 1000.0;
    }
    if let Some(aperture) = declared.aperture_millimetres.filter(|value| *value > 0.0) {
        optics.aperture_metres = aperture.clamp(0.2, 100.0) / 1000.0;
    }
    if let Some(power) = declared
        .optical_power_milliwatts
        .filter(|value| *value > 0.0)
    {
        optics.optical_power_watts = power.clamp(1.0, 100_000.0) / 1000.0;
    }
    optics
}

fn effect_program(declared: Option<&ProfileEffect>) -> EffectProgram {
    let mut program = EffectProgram {
        script: None,
        script_key: 0,
        result_version: 1,
    };
    let Some(declared) = declared else {
        return program;
    };
    program.result_version = declared.result_version;
    if let Some(script) = declared.effect_script_asset.as_deref()
        && let Some(source) = decode_script(script)
    {
        program.script_key = script_key(&source);
        program.script = Some(source.into());
    }
    program
}

fn physics_program(declared: &ProfilePhysics) -> PhysicsProgram {
    let mut program = PhysicsProgram {
        script: None,
        script_key: 0,
        result_version: declared.result_version,
    };
    if let Some(script) = declared.control_script_asset.as_deref()
        && let Some(source) = decode_script(script)
    {
        program.script_key = script_key(&source);
        program.script = Some(source.into());
    }
    program
}

/// The fixture's whole DMX footprint, in patch order.
///
/// Built from every channel of the mode rather than from one head's, because a script is handed
/// the fixture as the desk addresses it. Ordering by address is what makes `input.dmx[0]` the
/// fixture's first channel, which is the only thing a manufacturer's DMX chart lets a script
/// author rely on.
fn laser_window(channels: &HashMap<Uuid, ChannelRef>) -> Option<LaserWindow> {
    let mut universes: Vec<u16> = channels
        .values()
        .map(|channel| channel.logical_universe)
        .collect();
    universes.sort_unstable();
    universes.dedup();
    // A laser split across universes is not something a scan engine can be handed coherently;
    // the first universe is the one its footprint is quoted against.
    let logical_universe = *universes.first()?;
    let mut slots: Vec<u16> = channels
        .values()
        .filter(|channel| channel.logical_universe == logical_universe)
        .flat_map(|channel| channel.slots.iter().copied())
        .collect();
    slots.sort_unstable();
    slots.dedup();
    (!slots.is_empty()).then_some(LaserWindow {
        logical_universe,
        slots,
    })
}

fn effect_window(channels: &HashMap<Uuid, ChannelRef>) -> Option<EffectWindow> {
    laser_window(channels).map(|window| EffectWindow {
        logical_universe: window.logical_universe,
        slots: window.slots,
    })
}

/// Anything the profile says about its own light replaces what its type would have guessed.
///
/// The class fallback exists because most libraries describe a lantern's dimensions and nothing
/// about its optics. A profile that does describe them is the authority: it was measured, or read
/// off a manual, or set by the operator who owns the rig.
fn apply_declared_optics(optics: &mut EmitterOptics, declared: &ProfileOptics) {
    if let Some(output) = declared.output {
        optics.output = output.clamp(0.05, 8.0);
    }
    if let Some(sharpness) = declared.sharpness {
        optics.sharpness = sharpness.clamp(0.0, 1.0);
    }
    if let Some(uniformity) = declared.uniformity {
        optics.uniformity = uniformity.clamp(0.0, 1.0);
    }
    if let Some(source) = declared.light_source {
        // A declared lens is not second-guessed against the body: someone stated it.
        optics.source = LightSource {
            form: match source.form {
                LightSourceForm::Round => SourceForm::Round,
                LightSourceForm::Oval => SourceForm::Oval,
                LightSourceForm::Rectangular => SourceForm::Rectangular,
            },
            width: (source.width_millimetres / 1000.0).max(0.005),
            height: (source.height_millimetres / 1000.0).max(0.005),
        };
    }
}

fn emitter_kind(class: OpticalClass, binding: &EmitterBinding) -> EmitterKind {
    // A laser is decided by its class alone. Nothing in its channel set distinguishes it — the
    // slots that pick a pattern look exactly like the slots that pick a gobo.
    if class.is_laser() {
        return EmitterKind::Laser;
    }
    if class.is_effect() {
        return EmitterKind::Effect;
    }
    // An atmosphere machine contributes haze whether or not its output channel happens to be
    // named `fog`; the profile's own classification is the authority.
    if class == OpticalClass::Atmosphere || binding.fog.is_some() {
        return EmitterKind::Atmosphere;
    }
    if class.is_directional() {
        EmitterKind::Beam
    } else {
        EmitterKind::Emissive
    }
}

/// Beam and field angles, refined by a zoom channel that declares real degrees.
fn zoom_refined(narrow: f32, wide: f32, binding: &EmitterBinding) -> (f32, f32) {
    let Some(zoom) = &binding.zoom else {
        return (narrow.max(0.5), wide.max(narrow.max(0.5)));
    };
    let declared_degrees = zoom.physical_max > 1.5 && zoom.physical_max <= 180.0;
    if declared_degrees {
        let low = zoom.physical_min.min(zoom.physical_max).max(0.5);
        let high = zoom.physical_min.max(zoom.physical_max);
        return (low, high.max(low));
    }
    (narrow.max(0.5), wide.max(narrow.max(0.5)))
}

fn cone_angles(class: OpticalClass, binding: &EmitterBinding) -> (f32, f32) {
    let (narrow, wide) = class.cone_angles();
    zoom_refined(narrow, wide, binding)
}

/// Spread fallback heads along the body so a bar's heads do not stack on one point.
fn head_offset(head_index: usize, count: usize, face_width: f32, body_width: f32) -> Vec3 {
    if count <= 1 {
        return Vec3::ZERO;
    }
    let position = head_index as f32 / (count - 1) as f32 - 0.5;
    // Leave half a face at either end. Even a generously modelled merged `source-array` can no
    // longer push the first or last cell outside the fixture carrying it.
    Vec3::new(position * (body_width - face_width).max(0.0), 0.0, 0.0)
}

/// How far along the body the fallback layout spreads a fixture's heads, in metres.
fn head_span(class: OpticalClass) -> f32 {
    match class {
        OpticalClass::Emissive | OpticalClass::Blinder => 1.0,
        _ => 0.6,
    }
}

/// The optics one head of a fallback layout is given: the fixture's own, trimmed to the pitch the
/// heads are spread at.
///
/// No lamp face is wider than the distance to the next lamp — a ten-lamp bar a metre long has
/// hundred-millimetre lamps, whatever a class default says in the abstract. Without this a bank of
/// round lenses reads as one continuous glowing tube instead of as the row of lamps it is.
fn fitted_to_head_pitch(
    optics: &EmitterOptics,
    head_count: usize,
    body_width: f32,
) -> EmitterOptics {
    let mut fitted = optics.clone();
    if head_count < 2 {
        return fitted;
    }
    let pitch = body_width / head_count as f32;
    let bound = (pitch * 0.9).max(0.01);
    fitted.source.width = fitted.source.width.min(bound);
    fitted.source.height = fitted.source.height.min(bound);
    fitted
}

/// Pan travel, taken from the geometry graph when it declares motion and otherwise from the
/// documented fallback whenever the mode actually has a pan channel.
fn pan_axis(motion: &MotionAxes, binding: &EmitterBinding) -> Option<MotionAxis> {
    motion.pan.or_else(|| {
        binding.pan.as_ref().map(|_| MotionAxis {
            axis: Vec3::Y,
            min_degrees: -fallback::FALLBACK_PAN_DEGREES,
            max_degrees: fallback::FALLBACK_PAN_DEGREES,
        })
    })
}

fn tilt_axis(motion: &MotionAxes, binding: &EmitterBinding) -> Option<MotionAxis> {
    motion.tilt.or_else(|| {
        binding.tilt.as_ref().map(|_| MotionAxis {
            axis: Vec3::X,
            min_degrees: -fallback::FALLBACK_TILT_DEGREES,
            max_degrees: fallback::FALLBACK_TILT_DEGREES,
        })
    })
}

mod assets;
mod bindings;
mod compile_instances;

pub use assets::{GOBO_ARTWORK_EDGE, decode_gobo_artwork};
use assets::{decode_script, gobo_wheel, read_model_asset, script_key};
use bindings::{build_binding, cell_bindings, group_by_head, layout_cells};
use compile_instances::compile_instances;

#[cfg(test)]
mod model_tests;
