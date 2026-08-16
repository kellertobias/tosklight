//! Compile every physical instance of one selected fixture into the scene and its bindings.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_instances(
    scene: &mut Scene,
    bindings: &mut Vec<EmitterBinding>,
    external_camera: &mut Option<ExternalCameraBinding>,
    external_camera_issue: &mut Option<String>,
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
            installed_shaper_angles_degrees: instance.installed_appearance.shaper_angles_degrees,
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
        // Its own address where it has one, the fixture's where it has not.
        let own = address_map(instance);
        let addresses = if own.is_empty() {
            shared_addresses.clone()
        } else {
            own
        };
        let channels = compile_channels(&fixture.profile, mode, primary_slots, &addresses);
        let physics_body_index = fixture.profile.physics.as_ref().map(|declared| {
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
        });
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
        build_emitters(
            scene,
            bindings,
            fixture,
            mode,
            class,
            motion,
            instance,
            fixture_index,
            &channels,
            instance_optics,
            mount,
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
