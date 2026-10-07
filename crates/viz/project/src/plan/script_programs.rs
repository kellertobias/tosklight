//! Script programs and DMX windows for laser, effect and physics emitters.

use super::assets::{decode_script, script_key};
use super::{ChannelRef, EffectWindow, LaserWindow};
use light_fixture::{ProfileEffect, ProfileLaser, ProfilePhysics};
use std::collections::HashMap;
use uuid::Uuid;
use viz_scene::{EffectProgram, LaserOptics, PhysicsProgram};

/// The scanner a laser profile describes, with the gaps filled from what a show laser typically is.
///
/// Written to be usable from a profile that declares nothing at all: a package that names its
/// fixture type as a laser and ships a script gets a working projector without anyone having
/// measured a scan angle. What it cannot supply is the script — an invented pattern would be a
/// lie about what the fixture does, so a laser with no engine stays dark and is reported.
pub(super) fn laser_optics(declared: Option<&ProfileLaser>) -> LaserOptics {
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

pub(super) fn effect_program(declared: Option<&ProfileEffect>) -> EffectProgram {
    let mut program = EffectProgram {
        script: None,
        script_key: 0,
        result_version: 1,
    };
    let Some(declared) = declared else {
        return program;
    };
    program.result_version = declared.result_version;
    if let Some(script) = declared.effect_script_asset.as_deref()
        && let Some(source) = decode_script(script)
    {
        program.script_key = script_key(&source);
        program.script = Some(source.into());
    }
    program
}

pub(super) fn physics_program(declared: &ProfilePhysics) -> PhysicsProgram {
    let mut program = PhysicsProgram {
        script: None,
        script_key: 0,
        result_version: declared.result_version,
    };
    if let Some(script) = declared.control_script_asset.as_deref()
        && let Some(source) = decode_script(script)
    {
        program.script_key = script_key(&source);
        program.script = Some(source.into());
    }
    program
}

/// The fixture's whole DMX footprint, in patch order.
///
/// Built from every channel of the mode rather than from one head's, because a script is handed
/// the fixture as the desk addresses it. Ordering by address is what makes `input.dmx[0]` the
/// fixture's first channel, which is the only thing a manufacturer's DMX chart lets a script
/// author rely on.
pub(super) fn laser_window(channels: &HashMap<Uuid, ChannelRef>) -> Option<LaserWindow> {
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

pub(super) fn effect_window(channels: &HashMap<Uuid, ChannelRef>) -> Option<EffectWindow> {
    laser_window(channels).map(|window| EffectWindow {
        logical_universe: window.logical_universe,
        slots: window.slots,
    })
}
