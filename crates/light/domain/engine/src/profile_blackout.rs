use light_fixture::{FixtureChannel, FixtureMode};

pub(crate) fn blackout_raw(mode: &FixtureMode, channel: &FixtureChannel, raw: u32) -> u32 {
    if channel.attribute.is_intensity() {
        return if channel.invert {
            channel.resolution.max_raw()
        } else {
            0
        };
    }
    for system in &mode.color_systems {
        match &system.system {
            light_fixture::ColorSystem::Additive { emitters }
                if emitters
                    .iter()
                    .any(|emitter| emitter.channel_id == channel.id) =>
            {
                return if channel.invert {
                    channel.resolution.max_raw()
                } else {
                    0
                };
            }
            light_fixture::ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
            } if [cyan_channel_id, magenta_channel_id, yellow_channel_id]
                .contains(&&channel.id) =>
            {
                return if channel.invert {
                    0
                } else {
                    channel.resolution.max_raw()
                };
            }
            _ => {}
        }
    }
    raw
}
