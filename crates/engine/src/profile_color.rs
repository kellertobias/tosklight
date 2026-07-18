use light_core::Xyz;
use light_fixture::{ColorSystem, FixtureMode, srgb_to_xyz};
use std::collections::HashMap;

pub(crate) fn channel_visual_level(
    mode: &FixtureMode,
    channels: &HashMap<uuid::Uuid, u32>,
    channel_id: uuid::Uuid,
) -> Option<f32> {
    let channel = mode
        .channels
        .iter()
        .find(|channel| channel.id == channel_id)?;
    let level = channels.get(&channel_id).copied()? as f32 / channel.resolution.max_raw() as f32;
    Some(if channel.invert { 1.0 - level } else { level }.clamp(0.0, 1.0))
}

pub(crate) fn profile_visual_color(
    mode: &FixtureMode,
    head_id: uuid::Uuid,
    channels: &HashMap<uuid::Uuid, u32>,
    fallback: Option<Xyz>,
) -> Option<Xyz> {
    let system = mode
        .color_systems
        .iter()
        .find(|system| system.head_id == head_id);
    match system.map(|system| &system.system) {
        Some(ColorSystem::Additive { emitters }) => {
            Some(emitters.iter().filter(|emitter| emitter.visible).fold(
                Xyz {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                |sum, emitter| {
                    let emitted = channel_visual_level(mode, channels, emitter.channel_id)
                        .unwrap_or(0.0)
                        .powf(emitter.response_curve);
                    Xyz {
                        x: sum.x + emitter.xyz.x * emitted,
                        y: sum.y + emitter.xyz.y * emitted,
                        z: sum.z + emitter.xyz.z * emitted,
                    }
                },
            ))
        }
        Some(ColorSystem::Subtractive {
            cyan_channel_id,
            magenta_channel_id,
            yellow_channel_id,
        }) => Some(srgb_to_xyz(
            1.0 - channel_visual_level(mode, channels, *cyan_channel_id).unwrap_or(0.0),
            1.0 - channel_visual_level(mode, channels, *magenta_channel_id).unwrap_or(0.0),
            1.0 - channel_visual_level(mode, channels, *yellow_channel_id).unwrap_or(0.0),
        )),
        Some(ColorSystem::DiscreteWheel { channel_id, slots }) => {
            let raw = channels.get(channel_id).copied()?;
            slots
                .iter()
                .find(|slot| raw >= slot.dmx_from && raw <= slot.dmx_to)
                .and_then(|slot| slot.measured_xyz)
                .or(fallback)
        }
        None => rgb_fallback(mode, head_id, channels).or(fallback),
    }
}

fn rgb_fallback(
    mode: &FixtureMode,
    head_id: uuid::Uuid,
    channels: &HashMap<uuid::Uuid, u32>,
) -> Option<Xyz> {
    let level = |attribute: &str| {
        mode.channels
            .iter()
            .find(|channel| channel.head_id == head_id && channel.attribute.0 == attribute)
            .and_then(|channel| channel_visual_level(mode, channels, channel.id))
    };
    match (
        level("color.red"),
        level("color.green"),
        level("color.blue"),
    ) {
        (Some(red), Some(green), Some(blue)) => Some(srgb_to_xyz(red, green, blue)),
        _ => None,
    }
}
