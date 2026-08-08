//! Compile a patched show into a renderer scene plus the channel bindings that decode it.
//!
//! This runs once per patch revision. A DMX frame never re-enters this code.

use crate::binding::ChannelRef;
use crate::default_model::{self, FixtureTraits};
use crate::fallback::{self, OpticalClass};
use glam::{Quat, Vec3};
use light_fixture::{
    ChannelBehavior, FixtureMode, FixtureProfile, GeometryMotionKind, InstalledFixtureAppearance,
    LightSourceForm, PatchPolicy, ProfileLaser, ProfileOptics, Vector3,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use viz_scene::{
    EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FallbackReason, FixtureBody,
    FixtureInstance, LaserOptics, LightSource, MotionAxis, Scene, SourceForm,
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
}

/// A laser fixture's raw DMX footprint, resolved to absolute addresses.
#[derive(Clone, Debug)]
pub struct LaserWindow {
    pub logical_universe: u16,
    /// Absolute 1-based DMX addresses in patch order. Index `n` here is `input.dmx[n]` in a script.
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
            face: model
                .emitter_size
                .map(|size| size * scale),
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
    pub warnings: Vec<String>,
}

/// Compile the patch into a scene. Fixtures with `VisualOnly` policy become scenery elsewhere and
/// are skipped here.
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

pub fn compile(fixtures: &[PatchedFixture]) -> ScenePlan {
    let mut scene = Scene::default();
    let mut bindings = Vec::new();
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

    for fixture in fixtures {
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
        /*
         * Where this fixture is addressed, for the instances that carry no address themselves.
         *
         * A multi-patch instance is the same logical fixture standing somewhere else: it has its
         * own position, its own inversions and its own installed colour, and it shares the
         * programming — which means it shares the DMX. The patch says so by giving it no address
         * of its own.
         *
         * Read as an independent patch that happens to be unaddressed, such an instance decodes
         * no channels and is drawn dark for ever, so a bank of four ACLs on one address lit one
         * lamp and left three cold. It is not unpatched; it is patched to the same place.
         */
        let shared_addresses = fixture
            .instances
            .iter()
            .map(address_map)
            .find(|addresses| !addresses.is_empty())
            .unwrap_or_default();
        for instance in &fixture.instances {
            let fixture_index = scene.fixtures.len() as u32;
            let missing_optics = mode.geometry.emitters.is_empty();
            scene.fixtures.push(FixtureInstance {
                instance_id: instance.instance_id,
                fixture_id: fixture.fixture_id,
                name: instance.name.clone(),
                number: fixture.number,
                position: instance.position,
                rotation_degrees: instance.rotation_degrees,
                bracket_degrees: instance.bracket_angle,
                shaper_degrees: instance.shaper_angle,
                installed_colour: crate::installed_appearance_linear_rgb(
                    &fixture.profile,
                    &instance.installed_appearance,
                ),
                installed_shaper_angles_degrees: instance
                    .installed_appearance
                    .shaper_angles_degrees,
                body: FixtureBody {
                    size: body_size,
                    kind: class.body_kind(moving),
                },
                patched: !shared_addresses.is_empty(),
                address: instance
                    .split_patches
                    .iter()
                    .find_map(|(_, address)| *address)
                    .or_else(|| shared_addresses.values().copied().min()),
                model,
                fallback: missing_optics.then(|| {
                    FallbackReason::new(
                        "fixture optics",
                        format!(
                            "{} {} has no emitter geometry; using the generic {:?} projector",
                            fixture.profile.manufacturer, fixture.profile.name, class
                        ),
                    )
                }),
            });
            if fixture.profile.patch_policy == PatchPolicy::VisualOnly {
                continue;
            }
            // Its own address where it has one, the fixture's where it has not.
            // Its own address where it has one, the fixture's where it has not.
            let own = address_map(instance);
            let addresses = if own.is_empty() {
                shared_addresses.clone()
            } else {
                own
            };
            let channels = compile_channels(mode, &primary_slots, &addresses);
            build_emitters(
                &mut scene,
                &mut bindings,
                fixture,
                mode,
                class,
                &motion,
                instance,
                fixture_index,
                &channels,
                optics.clone(),
                mount,
                laser.clone(),
            );
        }
    }
    scene.recompute_bounds();
    ScenePlan {
        scene,
        bindings,
        warnings,
    }
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

/// Read pan and tilt travel from the geometry graph rather than assuming it.
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
) {
    // Resolved once for the fixture: every head of a laser reads the same footprint, because the
    // script is given the whole fixture rather than one head's channels.
    let laser_window = laser.as_ref().and_then(|_| laser_window(channels));
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
            &laser_window,
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
        bindings.push(binding);
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
    laser_window: &Option<LaserWindow>,
    steered: bool,
) {
    let head_channels = group_by_head(mode, channels);
    // The layout below is invented, so the faces it places have to be trimmed to it: a row of
    // lamp lenses wider than the pitch they are spread at would merge into one smear.
    let head_optics = fitted_to_head_pitch(&optics, mode, class);
    // Fallback: one emitter per logical head, aimed along the head's rest direction.
    for (head_index, head) in mode.heads.iter().enumerate() {
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
            local_origin: mount.origin + head_offset(mode, head_index, class),
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
            laser_window,
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
    laser_window: &Option<LaserWindow>,
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
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    });
    binding.laser_window = laser_window.clone();
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
fn head_offset(mode: &FixtureMode, head_index: usize, class: OpticalClass) -> Vec3 {
    let count = mode.heads.len();
    if count <= 1 {
        return Vec3::ZERO;
    }
    let position = head_index as f32 / (count - 1) as f32 - 0.5;
    Vec3::new(position * head_span(class), 0.0, 0.0)
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
    mode: &FixtureMode,
    class: OpticalClass,
) -> EmitterOptics {
    let mut fitted = optics.clone();
    if mode.heads.len() < 2 {
        return fitted;
    }
    let pitch = head_span(class) / (mode.heads.len() - 1) as f32;
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

pub use assets::{GOBO_ARTWORK_EDGE, decode_gobo_artwork};
use assets::{decode_script, gobo_wheel, read_model_asset, script_key};
use bindings::{build_binding, cell_bindings, group_by_head, layout_cells};

#[cfg(test)]
mod model_tests {
    use super::*;
    use light_core::AttributeKey;
    use light_fixture::{
        CanonicalTransform, ChannelBehavior, ChannelResolution, FixtureChannel, GelAssignment,
        ProfileLightSource,
    };

    /// One patched fixture of a named type, for the optics questions below.
    fn patched(fixture_type: &str, optics: ProfileOptics) -> PatchedFixture {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Generic".into();
        profile.name = "Test".into();
        profile.fixture_type = fixture_type.into();
        profile.optics = optics;
        let mode_id = profile.modes[0].id;
        PatchedFixture {
            fixture_id: Uuid::new_v4(),
            name: "Test".into(),
            number: Some(1),
            profile: Arc::new(profile),
            mode_id,
            instances: vec![PhysicalInstance {
                instance_id: Uuid::new_v4(),
                name: "Test".into(),
                split_patches: vec![(1, Some((1, 1)))],
                position: Vec3::new(0.0, 5.0, 0.0),
                rotation_degrees: Vec3::ZERO,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 0.0,
                shaper_angle: None,
                installed_appearance: InstalledFixtureAppearance::default(),
            }],
        }
    }

    /// A bank of lamps on one address is a bank of lamps, not one lamp and three dark ones.
    ///
    /// Multi-patch is how an operator says "the same fixture, standing over there as well". The
    /// instance has its own position and its own inversions and shares the programming, which the
    /// patch expresses by giving it no address of its own. Read as an independent patch that
    /// happens to be unaddressed, it decodes nothing and is drawn dark for ever — so four ACLs on
    /// one address lit one and left three cold.
    #[test]
    fn a_multipatch_instance_reads_the_fixture_it_shares_its_programming_with() {
        let mut fixture = patched("par", ProfileOptics::default());
        // A dimmer, so there is something to read. Without a channel every binding is empty and
        // the question this test asks cannot be answered either way.
        let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 1;
        let head_id = mode.heads[0].id;
        mode.channels = vec![FixtureChannel {
            id: Uuid::new_v4(),
            head_id,
            split: 1,
            fixture_attribute: AttributeKey("intensity".into()),
            attribute: AttributeKey("intensity".into()),
            canonical_transform: CanonicalTransform::Identity,
            resolution: ChannelResolution::U8,
            secondary_slots: Vec::new(),
            default_raw: 0,
            highlight_raw: 255,
            physical_min: Some(0.0),
            physical_max: Some(1.0),
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: Vec::new(),
        }];
        let root_addresses = fixture.instances[0].split_patches.clone();
        let mut standing_elsewhere = fixture.instances[0].clone();
        standing_elsewhere.instance_id = Uuid::new_v4();
        standing_elsewhere.name = "Second".into();
        standing_elsewhere.position = Vec3::new(3.0, 5.0, 0.0);
        // What the desk actually sends for a multi-patch instance: a split with no address in it.
        standing_elsewhere.split_patches = vec![(1, None)];
        fixture.instances.push(standing_elsewhere);

        let plan = compile(&[fixture]);

        assert_eq!(plan.scene.fixtures.len(), 2, "both instances are drawn");
        assert!(
            plan.scene.fixtures.iter().all(|fixture| fixture.patched),
            "an instance sharing the fixture's address is patched, not unpatched"
        );
        assert_eq!(plan.bindings.len(), plan.scene.emitters.len());
        assert!(
            plan.bindings
                .iter()
                .all(|binding| binding.universes == vec![1]),
            "every instance reads the universe the fixture is addressed in, got {:?}",
            plan.bindings
                .iter()
                .map(|binding| binding.universes.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            plan.bindings
                .iter()
                .all(|binding| binding.intensity.is_some()),
            "and decodes its dimmer"
        );
        // And the root's own address is untouched by any of this.
        assert_eq!(root_addresses, vec![(1, Some((1, 1)))]);
    }

    /// The mechanical angles are patch facts, so the projection has to carry them into the scene.
    ///
    /// A rig where every clamp is set at 35 degrees and half the lanterns wear barn doors is drawn
    /// hanging straight and square if this seam drops them.
    #[test]
    fn root_and_multipatch_keep_independent_bracket_and_installed_shaper_angles() {
        let mut fixture = patched("profile", ProfileOptics::default());
        fixture.instances[0].bracket_angle = -35.0;
        fixture.instances[0].shaper_angle = Some(22.5);
        fixture.instances[0]
            .installed_appearance
            .shaper_angles_degrees = [10.0, 20.0, 30.0, 40.0];
        let mut copy = fixture.instances[0].clone();
        copy.instance_id = Uuid::new_v4();
        copy.bracket_angle = 17.0;
        copy.shaper_angle = Some(-12.5);
        copy.installed_appearance.shaper_angles_degrees = [-11.0, -22.0, -33.0, -44.0];
        fixture.instances.push(copy);
        let plan = compile(&[fixture]);
        assert_eq!(plan.scene.fixtures.len(), 2);
        assert_eq!(plan.scene.fixtures[0].bracket_degrees, -35.0);
        assert_eq!(plan.scene.fixtures[0].shaper_degrees, Some(22.5));
        assert_eq!(
            plan.scene.fixtures[0].installed_shaper_angles_degrees,
            [10.0, 20.0, 30.0, 40.0]
        );
        assert_eq!(plan.scene.fixtures[1].bracket_degrees, 17.0);
        assert_eq!(plan.scene.fixtures[1].shaper_degrees, Some(-12.5));
        assert_eq!(
            plan.scene.fixtures[1].installed_shaper_angles_degrees,
            [-11.0, -22.0, -33.0, -44.0]
        );
    }

    #[test]
    fn root_and_multipatch_keep_independent_installed_colours() {
        let mut fixture = patched("profile", ProfileOptics::default());
        fixture.instances[0]
            .installed_appearance
            .color_temperature_kelvin = Some(3_200);
        let mut copy = fixture.instances[0].clone();
        copy.instance_id = Uuid::new_v4();
        copy.installed_appearance.color_temperature_kelvin = Some(10_000);
        copy.installed_appearance.gel = GelAssignment::Custom {
            name: "Red".into(),
            color_srgb: "#FF0000".into(),
            note: None,
        };
        fixture.instances.push(copy);

        let plan = compile(&[fixture]);
        assert_eq!(plan.scene.fixtures.len(), 2);
        assert_ne!(
            plan.scene.fixtures[0].installed_colour,
            plan.scene.fixtures[1].installed_colour
        );
        assert_eq!(plan.scene.fixtures[1].installed_colour[1], 0.0);
        assert_eq!(plan.scene.fixtures[1].installed_colour[2], 0.0);
    }

    #[test]
    fn canonical_cct_identity_aliases_bind_each_physical_channel_once() {
        let mut fixture = patched("wash", ProfileOptics::default());
        let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 2;
        let head_id = mode.heads[0].id;
        mode.channels = [
            ("color.cold_white", "color.white"),
            ("color.warm_white", "color.amber"),
        ]
        .into_iter()
        .map(|(fixture_attribute, attribute)| FixtureChannel {
            id: Uuid::new_v4(),
            head_id,
            split: 1,
            fixture_attribute: AttributeKey(fixture_attribute.into()),
            attribute: AttributeKey(attribute.into()),
            canonical_transform: CanonicalTransform::Identity,
            resolution: ChannelResolution::U8,
            secondary_slots: Vec::new(),
            default_raw: 0,
            highlight_raw: 255,
            physical_min: Some(0.0),
            physical_max: Some(1.0),
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: Vec::new(),
        })
        .collect();

        let plan = compile(&[fixture]);
        let colour = &plan.bindings[0].colour;
        assert!(colour.white.is_some());
        assert!(colour.amber.is_some());
        assert!(colour.cold_white.is_none());
        assert!(colour.warm_white.is_none());
    }

    #[test]
    fn canonical_softness_alias_binds_the_physical_frost_channel_once() {
        let mut fixture = patched("profile", ProfileOptics::default());
        let profile = Arc::get_mut(&mut fixture.profile).expect("sole owner");
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 1;
        let head_id = mode.heads[0].id;
        mode.channels = vec![FixtureChannel {
            id: Uuid::new_v4(),
            head_id,
            split: 1,
            fixture_attribute: AttributeKey("frost".into()),
            attribute: AttributeKey("softness".into()),
            canonical_transform: CanonicalTransform::Identity,
            resolution: ChannelResolution::U8,
            secondary_slots: Vec::new(),
            default_raw: 0,
            highlight_raw: 0,
            physical_min: Some(0.0),
            physical_max: Some(1.0),
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: false,
            reacts_to_grand_master: false,
            behavior: ChannelBehavior::Controlled,
            functions: Vec::new(),
        }];

        let plan = compile(&[fixture]);
        assert!(plan.bindings[0].frost.is_some());
    }

    fn optics_of(fixture: PatchedFixture) -> viz_scene::EmitterOptics {
        let plan = compile(&[fixture]);
        plan.scene
            .emitters
            .first()
            .expect("one emitter")
            .optics
            .clone()
    }

    /// A library that says nothing about its optics still has to render as the right sort of
    /// lantern: the declared fixture type decides, and a profile and a flood differ.
    #[test]
    fn a_profile_that_declares_no_optics_falls_back_to_its_type() {
        let spot = optics_of(patched("profile", ProfileOptics::default()));
        let flood = optics_of(patched("cyc flood", ProfileOptics::default()));
        assert!(
            spot.sharpness > flood.sharpness + 0.4,
            "a profile cuts and a flood does not: {} against {}",
            spot.sharpness,
            flood.sharpness
        );
        assert!(spot.uniformity > 0.5 && flood.source.form == SourceForm::Rectangular);
    }

    /// And what a profile does declare wins. This is the point of the block: the library is the
    /// authority on the fixture, not the renderer's guess from a type name.
    #[test]
    fn declared_optics_replace_the_fallback_for_that_fixture() {
        let optics = optics_of(patched(
            // A wash by name, deliberately declared as something else.
            "wash",
            ProfileOptics {
                output: Some(2.5),
                sharpness: Some(0.95),
                uniformity: Some(0.1),
                light_source: Some(ProfileLightSource {
                    form: LightSourceForm::Rectangular,
                    width_millimetres: 240.0,
                    height_millimetres: 60.0,
                }),
            },
        ));
        assert_eq!(optics.output, 2.5);
        assert_eq!(optics.sharpness, 0.95);
        assert_eq!(optics.uniformity, 0.1);
        assert_eq!(optics.source.form, SourceForm::Rectangular);
        assert!((optics.source.width - 0.24).abs() < 1e-6);
        assert!((optics.source.height - 0.06).abs() < 1e-6);
    }

    /// A declared lens is stated, not guessed, so it is not second-guessed against the body — but
    /// a lens the renderer had to invent is kept inside the lantern that carries it.
    #[test]
    fn an_invented_lens_stays_inside_the_body_and_a_declared_one_is_taken_as_read() {
        let mut small = patched("wash", ProfileOptics::default());
        let profile = Arc::get_mut(&mut small.profile).expect("sole owner");
        profile.physical.width_millimetres = Some(90.0);
        profile.physical.height_millimetres = Some(90.0);
        profile.physical.depth_millimetres = Some(90.0);
        let invented = optics_of(small.clone());
        assert!(
            invented.source.width <= 0.09,
            "an invented lens cannot be wider than the lantern: {}",
            invented.source.width
        );

        let profile = Arc::get_mut(&mut small.profile).expect("sole owner");
        profile.optics.light_source = Some(ProfileLightSource {
            form: LightSourceForm::Round,
            width_millimetres: 300.0,
            height_millimetres: 300.0,
        });
        assert!((optics_of(small).source.width - 0.3).abs() < 1e-6);
    }
}
