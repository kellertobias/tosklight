use super::{ChannelFunction, ChannelFunctionBehavior, FixtureChannel, FixtureProfile};
use light_core::AttributeKey;

/// Repair known malformed shipped revisions in a transient runtime projection.
///
/// The immutable profile bytes and digest stored in a show remain untouched. Callers apply this
/// only after identity and digest verification, or after reading the already-verified patch API.
/// Keeping the rules here gives the desk engine and every Stage consumer the same interpretation.
pub fn apply_runtime_profile_compatibility(profile: &mut FixtureProfile) {
    apply_jbled_a7_shutter_compatibility(profile);
}

fn apply_jbled_a7_shutter_compatibility(profile: &mut FixtureProfile) {
    const JBLED_A7_PROFILE_ID: &str = "3ddb091b-9c99-90a1-59cc-a0ba0c7cbd7e";
    if profile.id.0.to_string() != JBLED_A7_PROFILE_ID || profile.revision != 1 {
        return;
    }
    for mode in &mut profile.modes {
        let Some(channel) = mode
            .channels
            .iter_mut()
            .find(|channel| malformed_whole_range_shutter(channel))
        else {
            continue;
        };
        channel.default_raw = 16;
        channel.functions = jbled_a7_shutter_functions(channel);
    }
}

fn malformed_whole_range_shutter(channel: &FixtureChannel) -> bool {
    *channel.attribute.0 == *"shutter"
        && channel.functions.len() == 1
        && channel.functions[0].name == "Shutter / Strobe"
        && channel.functions[0].dmx_from == 0
        && channel.functions[0].dmx_to == 255
}

fn jbled_a7_shutter_functions(channel: &FixtureChannel) -> Vec<ChannelFunction> {
    let original_id = channel.functions[0].id.as_u128();
    let fixed = |band: u128, name: &str, from, to, semantic: &str, raw| ChannelFunction {
        id: uuid::Uuid::from_u128(original_id ^ band),
        name: name.into(),
        dmx_from: from,
        dmx_to: to,
        attribute: AttributeKey("shutter".into()),
        priority: 0,
        angular_motion: None,
        behavior: ChannelFunctionBehavior::Fixed {
            semantic_id: semantic.into(),
            label: name.into(),
            raw_value: raw,
        },
    };
    let effect = |band: u128, name: &str, from, to| ChannelFunction {
        id: uuid::Uuid::from_u128(original_id ^ band),
        name: name.into(),
        dmx_from: from,
        dmx_to: to,
        attribute: AttributeKey("shutter".into()),
        priority: 0,
        angular_motion: None,
        behavior: ChannelFunctionBehavior::Continuous {
            physical_min: 0.6,
            physical_max: 4.8,
            unit: Some("seconds".into()),
        },
    };
    vec![
        fixed(1, "Shutter closed", 0, 15, "closed", 0),
        fixed(2, "Shutter open", 16, 95, "open", 16),
        effect(3, "Shutter pulse opening", 96, 110),
        fixed(4, "Shutter open", 111, 111, "open", 111),
        effect(5, "Fade effect with dimmer", 112, 125),
        fixed(6, "Shutter open", 126, 126, "open", 126),
        fixed(7, "Shutter closed", 127, 127, "closed", 127),
        effect(8, "Shutter pulse opening", 128, 142),
        fixed(9, "Shutter open", 143, 143, "open", 143),
        effect(10, "Shutter pulse closing", 144, 158),
        fixed(11, "Shutter closed", 159, 159, "closed", 159),
        effect(12, "Shutter fade 0%", 160, 174),
        fixed(13, "Shutter open", 175, 175, "open", 175),
        effect(14, "Shutter fade 100%", 176, 190),
        fixed(15, "Shutter closed", 191, 191, "closed", 191),
        effect(16, "Shutter random 100%", 192, 206),
        fixed(17, "Shutter open", 207, 207, "open", 207),
        effect(18, "Shutter random 0%", 208, 222),
        fixed(19, "Shutter closed", 223, 223, "closed", 223),
        effect(20, "Shutter random fade 0%", 224, 238),
        fixed(21, "Shutter open", 239, 239, "open", 239),
        effect(22, "Shutter random fade 100%", 240, 254),
        fixed(23, "Shutter open", 255, 255, "open", 255),
    ]
}
