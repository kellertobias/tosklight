//! Wiring one head's channels to the parameters the renderer decodes.
//!
//! A binding is the seam between a fixture profile and a live DMX frame: which address carries an
//! intensity, which pair carries a 16-bit pan, which three carry a colour. Everything here is
//! about finding those, and about the cells of a multi-head fixture keeping their own.

use super::{ChannelRef, ColourBinding, EmitterBinding, PhysicalInstance, millimetres};
use glam::Vec3;
use light_core::AttributeKey;
use light_fixture::{
    CanonicalTransform, EmitterLayout, FixtureChannel, FixtureMode, GeometryEmitter,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Channels grouped by the head that owns them, keeping fixture-level channels available to all.
pub(super) fn group_by_head<'a>(
    mode: &'a FixtureMode,
    channels: &'a HashMap<Uuid, ChannelRef>,
) -> HashMap<Uuid, Vec<(&'a FixtureChannel, &'a ChannelRef)>> {
    let mut grouped: HashMap<Uuid, Vec<(&FixtureChannel, &ChannelRef)>> = HashMap::new();
    for channel in &mode.channels {
        let Some(reference) = channels.get(&channel.id) else {
            continue;
        };
        grouped
            .entry(channel.head_id)
            .or_default()
            .push((channel, reference));
    }
    // A shared head contributes its channels to every other head, which is how a shared dimmer or
    // shared pan arm reaches each logical head.
    let shared: Vec<(&FixtureChannel, &ChannelRef)> = mode
        .heads
        .iter()
        .filter(|head| head.master_shared)
        .filter_map(|head| grouped.get(&head.id).cloned())
        .flatten()
        .collect();
    if !shared.is_empty() && mode.heads.len() > 1 {
        for head in &mode.heads {
            if head.master_shared {
                continue;
            }
            let entry = grouped.entry(head.id).or_default();
            for (channel, reference) in &shared {
                if !entry
                    .iter()
                    .any(|(existing, _)| existing.attribute == channel.attribute)
                {
                    entry.push((channel, reference));
                }
            }
        }
    }
    grouped
}

pub(super) fn build_binding(
    owned: &[(&FixtureChannel, &ChannelRef)],
    instance: &PhysicalInstance,
    _mode: &FixtureMode,
    _head_id: Uuid,
    _channels: &HashMap<Uuid, ChannelRef>,
) -> EmitterBinding {
    let mut binding = EmitterBinding {
        invert_pan: instance.invert_pan,
        invert_tilt: instance.invert_tilt,
        ..EmitterBinding::default()
    };
    for (channel, reference) in owned {
        // A channel carries two names: the canonical engine attribute and the manufacturer's own.
        // A hazer, for example, exposes its fog output as canonical `intensity` while keeping
        // `fog` as its fixture attribute, so both have to be consulted.
        assign(&mut binding, &channel.attribute, reference);
        let canonical_identity_alias = channel.canonical_transform == CanonicalTransform::Identity
            && light_core::canonical_attribute_migration(&channel.fixture_attribute).is_some_and(
                |(canonical, transform)| {
                    canonical == channel.attribute
                        && transform == light_core::CanonicalAttributeTransform::Identity
                },
            );
        if channel.fixture_attribute != channel.attribute && !canonical_identity_alias {
            assign(&mut binding, &channel.fixture_attribute, reference);
        }
    }
    let mut universes: Vec<u16> = owned
        .iter()
        .map(|(_, reference)| reference.logical_universe)
        .collect();
    universes.sort_unstable();
    universes.dedup();
    binding.universes = universes;
    binding
}

fn assign(binding: &mut EmitterBinding, attribute: &AttributeKey, reference: &ChannelRef) {
    let key = attribute.0.as_str();
    let base = key.rsplit('.').next().unwrap_or(key);
    let slot = |target: &mut Option<ChannelRef>| {
        if target.is_none() {
            *target = Some(reference.clone());
        }
    };
    if attribute.is_intensity() {
        slot(&mut binding.intensity);
        return;
    }
    match key {
        "pan" => slot(&mut binding.pan),
        "tilt" => slot(&mut binding.tilt),
        "zoom" => slot(&mut binding.zoom),
        "iris" => slot(&mut binding.iris),
        "focus" => slot(&mut binding.focus),
        "frost" | "softness" => slot(&mut binding.frost),
        "shutter" => slot(&mut binding.shutter),
        "strobe" => slot(&mut binding.strobe),
        // Only the first wheel of each kind drives the picture. A fixture with two gobo wheels
        // still projects one pattern at a time, and inventing a combination of both would be
        // guessing at optics the profile does not describe.
        "gobo.1" => slot(&mut binding.gobo),
        "gobo.1.rotation" => slot(&mut binding.gobo_rotation),
        "prism.1" => slot(&mut binding.prism),
        "prism.1.rotation" => slot(&mut binding.prism_rotation),
        "shaper.blade.1.position" => slot(&mut binding.shaper_blades[0]),
        "shaper.blade.1.angle" => slot(&mut binding.shaper_blade_angles[0]),
        "shaper.blade.2.position" => slot(&mut binding.shaper_blades[1]),
        "shaper.blade.2.angle" => slot(&mut binding.shaper_blade_angles[1]),
        "shaper.blade.3.position" => slot(&mut binding.shaper_blades[2]),
        "shaper.blade.3.angle" => slot(&mut binding.shaper_blade_angles[2]),
        "shaper.blade.4.position" => slot(&mut binding.shaper_blades[3]),
        "shaper.blade.4.angle" => slot(&mut binding.shaper_blade_angles[3]),
        "shaper.rotation" => slot(&mut binding.shaper_rotation),
        "fog" | "haze" | "smoke" => slot(&mut binding.fog),
        _ => {}
    }
    if key.starts_with("color") {
        let colour = &mut binding.colour;
        match base {
            "red" => slot(&mut colour.red),
            "green" => slot(&mut colour.green),
            "blue" => slot(&mut colour.blue),
            "white" => slot(&mut colour.white),
            "amber" => slot(&mut colour.amber),
            "uv" | "ultraviolet" => slot(&mut colour.ultraviolet),
            "cold_white" => slot(&mut colour.cold_white),
            "warm_white" => slot(&mut colour.warm_white),
            "cyan" => slot(&mut colour.cyan),
            "magenta" => slot(&mut colour.magenta),
            "yellow" => slot(&mut colour.yellow),
            _ => {
                if key.starts_with("color.wheel") {
                    slot(&mut colour.wheel);
                }
            }
        }
    }
}

/// Cell offsets in metres for one emitter layout.
pub(super) fn layout_cells(emitter: &GeometryEmitter) -> Vec<Vec3> {
    match &emitter.layout {
        EmitterLayout::Point => vec![Vec3::ZERO],
        EmitterLayout::ExplicitPixels { positions } => {
            positions.iter().copied().map(millimetres).collect()
        }
        EmitterLayout::Strip {
            count,
            spacing_millimetres,
        } => (0..*count)
            .map(|index| {
                let offset = (index as f32 - (*count as f32 - 1.0) / 2.0) * spacing_millimetres;
                Vec3::new(offset / 1000.0, 0.0, 0.0)
            })
            .collect(),
        EmitterLayout::Ring {
            count,
            radius_millimetres,
        } => (0..*count)
            .map(|index| {
                let angle = index as f32 / (*count).max(1) as f32 * std::f32::consts::TAU;
                Vec3::new(
                    angle.cos() * radius_millimetres / 1000.0,
                    0.0,
                    angle.sin() * radius_millimetres / 1000.0,
                )
            })
            .collect(),
        EmitterLayout::Matrix {
            columns,
            rows,
            spacing,
        } => {
            let mut offsets = Vec::with_capacity(usize::from(*columns) * usize::from(*rows));
            for row in 0..*rows {
                for column in 0..*columns {
                    offsets.push(Vec3::new(
                        (column as f32 - (*columns as f32 - 1.0) / 2.0) * spacing.x / 1000.0,
                        (row as f32 - (*rows as f32 - 1.0) / 2.0) * spacing.y / 1000.0,
                        0.0,
                    ));
                }
            }
            offsets
        }
    }
}

/// Split a head's colour channels across its cells when the mode repeats them per pixel.
pub(super) fn cell_bindings(
    owned: &[(&FixtureChannel, &ChannelRef)],
    cells: usize,
) -> Vec<ColourBinding> {
    let mut per_attribute: HashMap<&str, Vec<&ChannelRef>> = HashMap::new();
    for (channel, reference) in owned {
        let key = channel.attribute.0.as_str();
        if key.starts_with("color") || channel.attribute.is_intensity() {
            per_attribute.entry(key).or_default().push(reference);
        }
    }
    // Only treat the mode as per-cell when at least one attribute repeats once per cell.
    if !per_attribute
        .values()
        .any(|references| references.len() >= cells)
    {
        return Vec::new();
    }
    (0..cells)
        .map(|cell| {
            let mut binding = ColourBinding::default();
            for (key, references) in &per_attribute {
                let Some(reference) = references.get(cell) else {
                    continue;
                };
                let base = key.rsplit('.').next().unwrap_or(key);
                let target = match base {
                    "red" => &mut binding.red,
                    "green" => &mut binding.green,
                    "blue" => &mut binding.blue,
                    "white" => &mut binding.white,
                    "amber" => &mut binding.amber,
                    "uv" | "ultraviolet" => &mut binding.ultraviolet,
                    "cyan" => &mut binding.cyan,
                    "magenta" => &mut binding.magenta,
                    "yellow" => &mut binding.yellow,
                    "intensity" => &mut binding.intensity,
                    _ => continue,
                };
                *target = Some((*reference).clone());
            }
            binding
        })
        .collect()
}
