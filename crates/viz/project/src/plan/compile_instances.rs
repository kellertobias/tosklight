//! Compile every physical instance of one selected fixture into the scene and its bindings.

use super::*;
use viz_scene::{ChainRig, RiserFeet, SceneryDetail};

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_instances(
    scene: &mut Scene,
    bindings: &mut Vec<EmitterBinding>,
    external_camera: &mut Option<ExternalCameraBinding>,
    external_camera_issue: &mut Option<String>,
    position_points: &mut Vec<PositionPointBinding>,
    warnings: &mut Vec<String>,
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    primary_slots: &HashMap<Uuid, u16>,
    class: OpticalClass,
    motion: &MotionAxes,
    body_size: Vec3,
    moving: bool,
    model: Option<u32>,
    optics: EmitterOptics,
    mount: EmitterMount,
    laser: Option<LaserOptics>,
    effect: Option<EffectProgram>,
    physics: Option<PhysicsProgram>,
) {
    let geometry = fixture.profile.mode_geometry(mode);
    let point = is_position_point(mode);
    let shared_addresses = fixture
        .instances
        .iter()
        .map(address_map)
        .find(|addresses| !addresses.is_empty())
        .unwrap_or_default();
    for instance in &fixture.instances {
        let mut instance_optics = optics.clone();
        if let Some(output) = instance
            .installed_appearance
            .luminous_output_lumens
            .and_then(|lumens| fallback::output_for_lumens(class, lumens))
        {
            instance_optics.output = output;
        }
        let generated_scenery = generated_scenery(fixture, instance);
        let fixture_index = scene.fixtures.len() as u32;
        let size = body_size * instance.model_scale;
        // The hinge sits in the model at the scale the model is drawn, as the lens does.
        let bracket_hinge = model
            .and_then(|index| scene.models.get(index as usize))
            .and_then(|drawn| Some(drawn.bracket_hinge? * drawn.scale_to(size)));
        scene.fixtures.push(FixtureInstance {
            drawn_as_scenery: generated_scenery.is_some(),
            invisible: point,
            instance_id: instance.instance_id,
            fixture_id: fixture.fixture_id,
            name: instance.name.clone(),
            number: fixture.number,
            position: instance.position,
            rotation_degrees: instance.rotation_degrees,
            position_master: None,
            bracket_degrees: instance.bracket_angle,
            bracket_hinge,
            shaper_degrees: instance.shaper_angle,
            installed_colour: crate::installed_appearance_linear_rgb(
                &fixture.profile,
                &instance.installed_appearance,
            ),
            installed_shaper_angles_degrees: instance.installed_appearance.shaper_angles_degrees,
            // Scaled here, once, so the drawn model, picking and the plan symbol all read the
            // same size from the body.
            body: FixtureBody {
                size,
                kind: class.body_kind(moving),
            },
            patched: !shared_addresses.is_empty(),
            address: instance
                .split_patches
                .iter()
                .find_map(|(_, address)| *address)
                .or_else(|| shared_addresses.values().copied().min()),
            model,
            fallback: emitterless_fallback(fixture, &geometry, class),
        });
        // Its own address where it has one, the fixture's where it has not.
        let own = address_map(instance);
        let addresses = if own.is_empty() {
            shared_addresses.clone()
        } else {
            own
        };
        let channels = compile_channels(&fixture.profile, mode, primary_slots, &addresses);
        let physics_body_index = fixture
            .profile
            .physics
            .as_ref()
            .map(|declared| push_physics_scenery(scene, instance, declared, &physics));
        if let Some(object) = generated_scenery {
            scene.scenery.push(object);
        }
        if let Some(binding) = position_point_binding(fixture, instance, mode, &channels) {
            position_points.push(binding);
        }
        match external_camera_binding(fixture, instance, mode, &channels) {
            Ok(Some(candidate)) if external_camera.is_none() && external_camera_issue.is_none() => {
                *external_camera = Some(candidate);
            }
            Ok(Some(candidate)) => {
                let first = external_camera
                    .as_ref()
                    .map(|binding: &ExternalCameraBinding| binding.label.as_str())
                    .unwrap_or("another camera fixture");
                let detail = format!(
                    "{} and {} both request the dedicated external 3D Visualizer camera; only one is supported, so DMX camera routing is disabled",
                    first, candidate.label
                );
                *external_camera = None;
                *external_camera_issue = Some(detail.clone());
                warnings.push(detail);
            }
            Ok(None) => {}
            Err(detail) => {
                *external_camera = None;
                *external_camera_issue = Some(detail.clone());
                warnings.push(detail);
            }
        }
        // A 3D Point sends no light: the fallback projector a model-less profile gets would put a
        // beam where there is only a reference.
        if point {
            continue;
        }
        build_emitters(
            scene,
            bindings,
            fixture,
            mode,
            &geometry,
            class,
            motion,
            instance,
            fixture_index,
            &channels,
            instance_optics,
            mount.scaled(instance.model_scale),
            laser.clone(),
            effect.clone(),
        );
        if let Some(body_index) = physics_body_index
            && let Some(binding) = bindings.last_mut()
            && let Some(window) = laser_window(&channels)
        {
            binding.physics_window = Some(PhysicsWindow {
                body_index,
                logical_universe: window.logical_universe,
                slots: window.slots,
            });
        }
    }
}

/// One DMX-driven scenic body for this instance, returning its index in the scene's physics list.
fn push_physics_scenery(
    scene: &mut Scene,
    instance: &PhysicalInstance,
    declared: &light_fixture::ProfilePhysics,
    physics: &Option<PhysicsProgram>,
) -> usize {
    let index = scene.physics_scenery.len();
    let kind = match declared.scenery_kind {
        ProfilePhysicsSceneryKind::Curtain => SceneryKind::Curtain,
        ProfilePhysicsSceneryKind::Prop => SceneryKind::Prop,
    };
    scene.physics_scenery.push(PhysicsSceneryObject {
        fixture_instance_id: instance.instance_id,
        scenery: SceneryObject {
            id: instance.instance_id,
            name: instance.name.clone(),
            position: instance.position,
            rotation_degrees: instance.rotation_degrees,
            size: Vec3::from_array(declared.size_metres),
            colour: [0.32, 0.08, 0.06],
            roughness: 0.86,
            kind,
            chords: 1,
            detail: Default::default(),
            position_master: None,
        },
        program: physics.clone().unwrap_or_default(),
        body: PhysicsBody {
            mass_kilograms: declared.mass_kilograms,
            gravity_metres_per_second_squared: declared.gravity_metres_per_second_squared,
        },
        constraints: PhysicsConstraints {
            floor_y_metres: declared.floor_y_metres,
            scenery_collision: declared.scenery_collision,
            self_collision: declared.self_collision,
        },
    });
    index
}

/// The badge the desk shows when a fixture has no emitter geometry of its own.
fn emitterless_fallback(
    fixture: &PatchedFixture,
    geometry: &light_fixture::GeometryGraph,
    class: OpticalClass,
) -> Option<FallbackReason> {
    geometry.emitters.is_empty().then(|| {
        FallbackReason::new(
            "fixture optics",
            format!(
                "{} {} has no emitter geometry; using the generic {:?} projector",
                fixture.profile.manufacturer, fixture.profile.name, class
            ),
        )
    })
}

/// What an operator may set, held inside what the object can actually be built at.
fn clamp_scenery_size(size: Vec3, declared: &light_fixture::ProfileScenery) -> Vec3 {
    let minimum = vector(declared.minimum_size_metres);
    let maximum = vector(declared.maximum_size_metres);
    let default = vector(declared.default_size_metres);
    let axis = |set: bool, value: f32, low: f32, high: f32, fallback: f32| {
        if set {
            value.clamp(low, high)
        } else {
            fallback
        }
    };
    Vec3::new(
        axis(
            declared.adjustable.width,
            size.x,
            minimum.x,
            maximum.x,
            default.x,
        ),
        axis(
            declared.adjustable.height,
            size.y,
            minimum.y,
            maximum.y,
            default.y,
        ),
        axis(
            declared.adjustable.depth,
            size.z,
            minimum.z,
            maximum.z,
            default.z,
        ),
    )
}

pub(super) fn vector(value: light_fixture::Vector3) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

impl EmitterMount {
    /// The same mount on a body drawn `scale` times its size: the lens and trunnions move with it.
    fn scaled(self, scale: f32) -> Self {
        Self {
            origin: self.origin * scale,
            pivot: self.pivot * scale,
            face: self.face.map(|face| face * scale),
            aim: self.aim,
        }
    }
}

fn scenery_kind(kind: light_fixture::ProfileSceneryKind) -> SceneryKind {
    match kind {
        light_fixture::ProfileSceneryKind::Riser => SceneryKind::Riser,
        light_fixture::ProfileSceneryKind::Stairs => SceneryKind::Stairs,
        light_fixture::ProfileSceneryKind::Truss => SceneryKind::Truss,
        light_fixture::ProfileSceneryKind::Curtain => SceneryKind::Curtain,
        light_fixture::ProfileSceneryKind::Railing => SceneryKind::Railing,
        light_fixture::ProfileSceneryKind::MirrorBall => SceneryKind::MirrorBall,
        light_fixture::ProfileSceneryKind::Chain => SceneryKind::Chain,
        light_fixture::ProfileSceneryKind::Box => SceneryKind::Box,
        light_fixture::ProfileSceneryKind::Cylinder => SceneryKind::Cylinder,
        light_fixture::ProfileSceneryKind::Sphere => SceneryKind::Sphere,
        light_fixture::ProfileSceneryKind::FlightRack => SceneryKind::FlightRack,
        light_fixture::ProfileSceneryKind::PaTop => SceneryKind::PaTop,
        light_fixture::ProfileSceneryKind::LineArray => SceneryKind::LineArray,
        light_fixture::ProfileSceneryKind::Prop => SceneryKind::Prop,
    }
}

/// The same materials the desk's own scenery uses, so a patched truss and a legacy one match.
fn scenery_colour(kind: light_fixture::ProfileSceneryKind) -> [f32; 3] {
    match kind {
        // Mill-finish aluminium: a light, slightly cool grey.
        light_fixture::ProfileSceneryKind::Truss => [0.33, 0.335, 0.345],
        // Stage drape is black wool serge, not the generic prop grey.
        light_fixture::ProfileSceneryKind::Curtain => [0.008, 0.008, 0.01],
        // Galvanised rigging chain.
        light_fixture::ProfileSceneryKind::Chain => [0.32, 0.32, 0.33],
        // Staging is film-faced multiplex: the dark brown phenolic face of a deck.
        light_fixture::ProfileSceneryKind::Riser | light_fixture::ProfileSceneryKind::Stairs => {
            [0.036, 0.019, 0.009]
        }
        // A primitive is a stand-in shape an operator colours; neutral grey until they do.
        light_fixture::ProfileSceneryKind::Box
        | light_fixture::ProfileSceneryKind::Cylinder
        | light_fixture::ProfileSceneryKind::Sphere => [0.3, 0.3, 0.3],
        // Black road-case board and black speaker cabinets.
        light_fixture::ProfileSceneryKind::FlightRack
        | light_fixture::ProfileSceneryKind::PaTop
        | light_fixture::ProfileSceneryKind::LineArray => [0.03, 0.03, 0.035],
        _ => [0.14, 0.14, 0.15],
    }
}

fn scenery_roughness(kind: light_fixture::ProfileSceneryKind) -> f32 {
    match kind {
        light_fixture::ProfileSceneryKind::Curtain => 0.96,
        // Mill-finish aluminium: a soft, broad highlight rather than a polished one.
        light_fixture::ProfileSceneryKind::Truss => 0.6,
        light_fixture::ProfileSceneryKind::Railing | light_fixture::ProfileSceneryKind::Chain => {
            0.45
        }
        _ => 0.8,
    }
}

/// What the profile and the placement say beyond the kind: a truss's bracing, a chain's ends.
///
/// A chain placed before its ends could be chosen hangs from a hoist at its top, which is how a
/// chain is most often rigged.
fn scenery_detail(
    declared: &light_fixture::ProfileScenery,
    options: &light_fixture::SceneryOptions,
) -> SceneryDetail {
    use light_fixture::ProfileSceneryKind as Kind;
    let chain = match declared.kind {
        Kind::Chain => match options.chain_mode() {
            light_fixture::ChainMode::Plain => ChainRig::Plain,
            light_fixture::ChainMode::MotorTop => ChainRig::MotorTop,
            light_fixture::ChainMode::MotorBottom => ChainRig::MotorBottom,
        },
        _ => ChainRig::Plain,
    };
    SceneryDetail {
        deco: declared.kind == Kind::Truss && declared.pattern == light_fixture::TrussPattern::Deco,
        chain,
        // A flight of stairs stands on its own steps and is raised by nothing, so only a deck has
        // feet. Stairs are their own kind now; a package that still declares them as a riser is
        // read by its name, which is how every stair in an older show was told apart.
        feet: match declared.kind {
            Kind::Riser => match declared.feet {
                light_fixture::RiserFeet::Scissor => RiserFeet::Scissor,
                light_fixture::RiserFeet::Fixed => RiserFeet::Fixed,
            },
            _ => RiserFeet::None,
        },
        // The placement chooses the sides; one placed before it could keeps what its profile has.
        handrails: {
            let sides = options.stair_handrails(declared.handrails);
            viz_scene::StairRails {
                left: sides.left(),
                right: sides.right(),
            }
        },
    }
}

/// The scenery as the plan reads it, with a flight of stairs from an older show read as stairs.
///
/// Stairs are their own kind now. A show written before that holds a profile snapshot declaring
/// itself a riser, with nothing but its name to tell it from a deck — which is exactly how the
/// plan told them apart then — so a riser that calls itself stairs is still built as stairs. A
/// package that declares the kind says so for itself and is never read by its name.
fn declared_kind(
    declared: &light_fixture::ProfileScenery,
    profile_name: &str,
) -> light_fixture::ProfileScenery {
    use light_fixture::ProfileSceneryKind as Kind;
    let named_stairs =
        declared.kind == Kind::Riser && profile_name.to_lowercase().contains("stair");
    light_fixture::ProfileScenery {
        kind: if named_stairs {
            Kind::Stairs
        } else {
            declared.kind
        },
        ..*declared
    }
}

/// Where a generated object's box is centred, from where it was placed.
///
/// A stage element — a deck on a scissor lift, a deck on regular feet, or a flight of stairs — is
/// placed by its feet: the placement is the middle of its footprint on the floor it stands on. Its
/// height then raises or lowers the deck while the feet stay where they are. Every other generated
/// kind is placed by the centre of its box.
pub(super) fn scenery_centre(
    instance: &PhysicalInstance,
    kind: light_fixture::ProfileSceneryKind,
    size: Vec3,
) -> Vec3 {
    match kind {
        // A deck, a flight of stairs and a handrail all stand on the floor they are placed on, and
        // so do a flight rack and a PA top, whose height grows upwards from where they stand.
        light_fixture::ProfileSceneryKind::Riser
        | light_fixture::ProfileSceneryKind::Stairs
        | light_fixture::ProfileSceneryKind::Railing
        | light_fixture::ProfileSceneryKind::FlightRack
        | light_fixture::ProfileSceneryKind::PaTop => {
            let up = viz_scene::euler_degrees(instance.rotation_degrees) * Vec3::Y;
            instance.position + up * (size.y * 0.5)
        }
        _ => instance.position,
    }
}

/// A Venue object that declares its shape, built at the size it was placed rather than drawn from
/// a model made for one size.
fn generated_scenery(
    fixture: &PatchedFixture,
    instance: &PhysicalInstance,
) -> Option<SceneryObject> {
    let declared = fixture.profile.scenery.as_ref()?;
    let declared = &declared_kind(declared, &fixture.profile.name);
    let size = instance
        .scenery_size_metres
        .unwrap_or_else(|| vector(declared.default_size_metres));
    // Held to what the object can be built at first, then drawn at its model scale.
    let size = clamp_scenery_size(size, declared) * instance.model_scale;
    Some(SceneryObject {
        id: instance.instance_id,
        name: instance.name.clone(),
        position: scenery_centre(instance, declared.kind, size),
        rotation_degrees: instance.rotation_degrees,
        size,
        // The colour the operator chose for this one, when they chose one; the kind's own
        // material otherwise.
        colour: instance
            .scenery_options
            .colour_linear()
            .unwrap_or_else(|| scenery_colour(declared.kind)),
        roughness: scenery_roughness(declared.kind),
        kind: scenery_kind(declared.kind),
        chords: declared.chords,
        detail: scenery_detail(declared, &instance.scenery_options),
        position_master: None,
    })
}

/// Whether the mode is a 3D Point's: it carries the point's position, the same test the desk and
/// the Architect apply.
fn is_position_point(mode: &FixtureMode) -> bool {
    mode.channels.iter().any(|channel| {
        &*channel.attribute.0 == "point.position.x"
            || &*channel.fixture_attribute.0 == "point.position.x"
    })
}
