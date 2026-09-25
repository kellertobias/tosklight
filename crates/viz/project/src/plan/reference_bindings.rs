//! Bindings for the reference objects a renderer follows rather than lights: the one external
//! camera a dedicated Visualizer may take its view from, and every patched 3D Point whose pose
//! other placements are slaved to. Both are read off the wire like a lantern's level.

use super::*;

/// One patched 3D Point's axes, so a renderer on the network can read the point's pose out of
/// the universes exactly as it reads a lantern's level.
///
/// A point is a reference object other placements are slaved to. It carries no light, and its
/// pose is not something a renderer may invent: an unpatched point reports nothing here and its
/// slaves stay where the rig put them until the desk states the pose another way.
#[derive(Clone, Debug)]
pub struct PositionPointBinding {
    pub fixture_id: Uuid,
    pub instance_id: Uuid,
    /// Where the point itself was rigged, in renderer world metres. Its offset is measured from
    /// here and its slaves turn about it.
    pub origin: Vec3,
    /// Offset along each renderer world axis, in the desk's `x`, `y`, `z` order — across the
    /// stage, upstage and up — before the axes are turned into the renderer's.
    pub position: [Option<ChannelRef>; 3],
    /// Turn about each axis, in the same desk order.
    pub rotation: [Option<ChannelRef>; 3],
    pub universes: Vec<u16>,
}

/// The DMX axes of a 3D Point, when this instance has an address for them.
///
/// Only the axes the mode carries are read: a position-only mode leaves the rotation channels
/// absent and the point keeps no turn. A point with no address at all reports nothing, which is
/// not a fault — an operator may keep a point purely as a programmer object.
pub(super) fn position_point_binding(
    fixture: &PatchedFixture,
    instance: &PhysicalInstance,
    mode: &FixtureMode,
    channels: &HashMap<Uuid, ChannelRef>,
) -> Option<PositionPointBinding> {
    let axis = |identity: &str| -> Option<ChannelRef> {
        mode.channels
            .iter()
            .find(|channel| {
                &*channel.attribute.0 == identity || &*channel.fixture_attribute.0 == identity
            })
            .and_then(|channel| channels.get(&channel.id).cloned())
    };
    let position = [
        axis("point.position.x"),
        axis("point.position.y"),
        axis("point.position.z"),
    ];
    let rotation = [
        axis("point.rotation.x"),
        axis("point.rotation.y"),
        axis("point.rotation.z"),
    ];
    if position.iter().chain(&rotation).all(Option::is_none) {
        return None;
    }
    let mut universes = position
        .iter()
        .chain(&rotation)
        .flatten()
        .map(|reference| reference.logical_universe)
        .collect::<Vec<_>>();
    universes.sort_unstable();
    universes.dedup();
    Some(PositionPointBinding {
        fixture_id: fixture.fixture_id,
        instance_id: instance.instance_id,
        origin: instance.position,
        position,
        rotation,
        universes,
    })
}

pub(super) fn external_camera_binding(
    fixture: &PatchedFixture,
    instance: &PhysicalInstance,
    mode: &FixtureMode,
    channels: &HashMap<Uuid, ChannelRef>,
) -> Result<Option<ExternalCameraBinding>, String> {
    let mut found: HashMap<&str, ChannelRef> = HashMap::new();
    for channel in &mode.channels {
        let identity = [&*channel.attribute.0, &*channel.fixture_attribute.0]
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
