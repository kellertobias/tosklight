//! Manual barn doors are installation hardware, never a second DMX authority.
use super::*;

pub(super) struct Shapers {
    pub turn: f32,
    pub angles: [f32; 4],
    pub blades: [f32; 4],
    pub params: [f32; 4],
}

pub(super) fn resolve(
    fixture: &FixtureInstance,
    emitter: &EmitterInstance,
    value: &EmitterValues,
) -> Shapers {
    let live_hardware =
        emitter.shaper_roles.iter().any(|role| *role) || emitter.live_shaper_rotation_role;
    let manual = !live_hardware
        && fixture
            .installed_shaper_blades
            .iter()
            .any(|blade| *blade > 0.0);
    let turn = if emitter.live_shaper_rotation_role {
        value.shaper_rotation_degrees
    } else if live_hardware || manual {
        fixture.shaper_degrees.unwrap_or(0.0)
    } else {
        0.0
    };
    let angles = std::array::from_fn(|index| {
        if emitter.live_shaper_angle_roles[index] {
            value.shaper_blade_angles_degrees[index]
        } else if emitter.shaper_roles[index] || manual {
            fixture.installed_shaper_angles_degrees[index]
        } else {
            0.0
        }
    })
    .map(f32::to_radians);
    Shapers {
        turn: turn.to_radians(),
        angles,
        // The old live-DMX shader stroke reaches the centre at1. Manual full-stroke fractions
        // explicitly run from one field edge to the opposite edge, without changing old cues.
        blades: if manual {
            fixture
                .installed_shaper_blades
                .map(|blade| blade.clamp(0.0, 1.0) * 2.0)
        } else {
            value.shaper_blades
        },
        // Nominal geometric penumbra, independent of the Fresnel's much softer radial field.
        params: if manual {
            [0.04, 0.0, 0.0, 0.0]
        } else {
            [0.0; 4]
        },
    }
}
