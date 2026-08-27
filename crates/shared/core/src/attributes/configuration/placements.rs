use super::super::*;

pub(super) fn recommended_builtin_placements() -> Vec<AttributePlacement> {
    canonical_placements()
        .into_iter()
        .chain(fixture_control_placements())
        .filter(|(id, _, _, _)| !built_in_attribute_is_retired(id))
        .map(|(id, group, page, slot)| AttributePlacement {
            attribute: AttributeKey(id.into()),
            encoder: EncoderPlacement::new(group, page, slot),
            push_turn_of: match id {
                "color.wheel.1.rotation" => Some(AttributeKey("color.wheel.1".into())),
                "color.wheel.2.rotation" => Some(AttributeKey("color.wheel.2".into())),
                "prism.1.rotation" => Some(AttributeKey("prism.1".into())),
                "prism.2.rotation" => Some(AttributeKey("prism.2".into())),
                "animation.1.rotation" => Some(AttributeKey("animation.1".into())),
                _ => None,
            },
        })
        .collect()
}

/// Where the desk's own attributes sit on the encoder deck.
fn canonical_placements() -> Vec<PlacedAttribute> {
    use EncoderGroup::{Beam, Color, Control, Focus, Intensity, Media, Position, Shapers};

    vec![
        ("intensity", Intensity, 1, 1),
        ("shutter", Intensity, 1, 2),
        ("strobe", Intensity, 1, 3),
        ("volume", Intensity, 1, 4),
        ("color.red", Color, 1, 1),
        ("color.green", Color, 1, 2),
        ("color.blue", Color, 1, 3),
        ("color.white", Color, 1, 4),
        ("color.amber", Color, 1, 5),
        ("color.uv", Color, 1, 6),
        ("color.lime", Color, 2, 1),
        ("color.indigo", Color, 2, 2),
        ("color.mint", Color, 2, 3),
        ("color.temperature", Color, 2, 4),
        ("color.wheel.1", Color, 2, 5),
        ("color.wheel.2", Color, 2, 6),
        ("color.wheel.1.rotation", Color, 3, 1),
        ("color.wheel.2.rotation", Color, 3, 2),
        ("pan", Position, 1, 1),
        ("tilt", Position, 1, 2),
        ("pan.time", Position, 1, 5),
        ("tilt.time", Position, 1, 6),
        ("position.movement", Position, 1, 3),
        ("position.rotation", Position, 1, 4),
        ("camera.position.x", Position, 2, 1),
        ("camera.position.y", Position, 2, 2),
        ("camera.position.z", Position, 2, 3),
        ("camera.yaw", Position, 2, 4),
        ("camera.pitch", Position, 2, 5),
        ("camera.roll", Position, 2, 6),
        ("gobo.1", Beam, 1, 1),
        ("gobo.1.rotation", Beam, 1, 2),
        ("gobo.2", Beam, 1, 3),
        ("gobo.2.rotation", Beam, 1, 4),
        ("prism.1", Beam, 1, 5),
        ("prism.2", Beam, 1, 6),
        ("animation.1", Beam, 2, 1),
        ("prism.1.rotation", Beam, 2, 2),
        ("prism.2.rotation", Beam, 2, 3),
        ("animation.1.rotation", Beam, 2, 4),
        ("beam.effect.1", Beam, 2, 5),
        ("beam.effect.2", Beam, 2, 6),
        ("beam", Beam, 3, 1),
        ("iris", Shapers, 1, 1),
        ("shaper.blade.1.position", Shapers, 1, 2),
        ("shaper.blade.1.angle", Shapers, 1, 3),
        ("shaper.blade.2.position", Shapers, 1, 4),
        ("shaper.blade.2.angle", Shapers, 1, 5),
        ("shaper.rotation", Shapers, 1, 6),
        ("shaper.blade.3.position", Shapers, 2, 1),
        ("shaper.blade.3.angle", Shapers, 2, 2),
        ("shaper.blade.4.position", Shapers, 2, 3),
        ("shaper.blade.4.angle", Shapers, 2, 4),
        ("shaper.keystone.x", Shapers, 2, 5),
        ("shaper.keystone.y", Shapers, 2, 6),
        ("focus", Focus, 1, 1),
        ("zoom", Focus, 1, 2),
        ("softness", Focus, 1, 3),
        // Camera Zoom stays with the six pose encoders so its linked activation group never
        // crosses control surfaces. It is still classified as Focus in the attribute registry.
        ("camera.zoom", Position, 3, 1),
        ("point.position.x", Position, 4, 1),
        ("point.position.y", Position, 4, 2),
        ("point.position.z", Position, 4, 3),
        ("point.rotation.x", Position, 4, 4),
        ("point.rotation.y", Position, 4, 5),
        ("point.rotation.z", Position, 4, 6),
        ("control", Control, 1, 1),
        ("media.play_mode", Control, 1, 2),
        ("media.playback_speed", Control, 1, 3),
        ("media.playback_bpm", Control, 1, 4),
        ("media.playback.blur", Control, 1, 6),
        ("media.scaling_mode", Control, 1, 5),
        ("media.folder", Media, 1, 1),
        ("media.file", Media, 1, 2),
        ("media.mask.folder", Media, 1, 3),
        ("media.mask.file", Media, 1, 4),
        ("audio.folder", Media, 2, 1),
        ("audio.file", Media, 2, 2),
        ("audio.transport", Media, 2, 3),
        ("audio.repeat", Media, 2, 4),
        // Audio Volume is a level, so it sits with the other levels on Intensity rather than
        // among the media addressing attributes, and shares the slot its canonical form uses.
        ("audio.volume", Intensity, 1, 5),
        ("media.position.x", Position, 1, 5),
        ("media.position.y", Position, 1, 6),
        ("media.scale.x", Media, 3, 3),
        ("media.scale.y", Media, 3, 4),
        ("media.mask.invert", Media, 1, 5),
        ("media.flip_mirror", Media, 1, 6),
        ("media.mask.opacity", Intensity, 1, 3),
        ("media.mask.scale.x", Media, 5, 1),
        ("media.mask.scale.y", Media, 5, 2),
        ("media.mask.position.x", Media, 5, 3),
        ("media.mask.position.y", Media, 5, 4),
        ("media.effect.1", Media, 4, 1),
        ("media.effect.2", Media, 4, 2),
        ("media.effect.3", Media, 4, 3),
        ("media.effect.4", Media, 4, 4),
    ]
}

/// One attribute's recommended encoder: its name, then group, page and slot.
type PlacedAttribute = (&'static str, EncoderGroup, u16, u8);

/// Where the fixture-specific control channels sit, on pages of their own after the canonical
/// ones.
///
/// Each is in the group its family names, so a Robe's pan/tilt speed is reachable from Position
/// and a Sharpy's effect wheel from Beam, rather than all of them landing together in Custom.
fn fixture_control_placements() -> Vec<PlacedAttribute> {
    use EncoderGroup::{Beam, Color, Control, Intensity, Position, Shapers};

    vec![
        ("fixture.plate_pixel_master", Intensity, 2, 1),
        ("fixture.plate_background_master", Intensity, 2, 2),
        ("fixture.pan_tilt_speed", Position, 5, 1),
        ("fixture.pan_tilt_speed_time", Position, 5, 2),
        ("fixture.pan_tilt_time", Position, 5, 3),
        ("fixture.mspeed", Position, 5, 4),
        ("fixture.blackout_move", Position, 5, 5),
        ("fixture.colour_macros", Color, 4, 1),
        ("fixture.colour_mix_control", Color, 4, 2),
        ("fixture.tint", Color, 4, 3),
        ("fixture.blade_1", Shapers, 3, 1),
        ("fixture.blade_2", Shapers, 3, 2),
        ("fixture.blade_3", Shapers, 3, 3),
        ("fixture.blade_4", Shapers, 3, 4),
        ("fixture.framing_macro", Shapers, 3, 5),
        ("fixture.framing_macro_speed", Shapers, 3, 6),
        ("fixture.barndoor_macros", Shapers, 4, 1),
        ("fixture.barndoor_macro_speed", Shapers, 4, 2),
        ("fixture.barndoor_module_rotation", Shapers, 4, 3),
        ("fixture.effect_animations", Beam, 4, 1),
        ("fixture.effect_wheel_position", Beam, 4, 2),
        ("fixture.effect_wheel_rotation", Beam, 4, 3),
        ("fixture.effects_movement", Beam, 4, 4),
        ("fixture.effects_speed", Beam, 4, 5),
        ("fixture.fx_crossfade", Beam, 4, 6),
        ("fixture.beam_fx_select", Beam, 5, 1),
        ("fixture.beam_fx_movement", Beam, 5, 2),
        ("fixture.beam_rate", Beam, 5, 3),
        ("fixture.beam_duration", Beam, 5, 4),
        ("fixture.beam_time", Beam, 5, 5),
        ("fixture.plate_fx_select", Beam, 5, 6),
        ("fixture.plate_fx_movement", Beam, 6, 1),
        ("fixture.plate_flash_rate", Beam, 6, 2),
        ("fixture.plate_flash_duration", Beam, 6, 3),
        ("fixture.control", Control, 2, 1),
        ("fixture.special_control", Control, 2, 2),
        ("fixture.function", Control, 2, 3),
        ("fixture.plus_7_control", Control, 2, 4),
        ("fixture.programs", Control, 2, 5),
        ("fixture.fan_control", Control, 2, 6),
        ("fixture.auto_speed", Control, 3, 1),
        ("fixture.lamp_control", Control, 3, 2),
        ("fixture.power_special_functions", Control, 3, 3),
        ("fixture.reset", Control, 3, 4),
        ("fixture.unused_4", Control, 3, 5),
        ("fixture.unused_7", Control, 3, 6),
        ("fixture.unused_8", Control, 4, 1),
    ]
}
