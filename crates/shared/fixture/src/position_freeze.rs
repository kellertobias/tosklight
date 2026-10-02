//! Portable accepted motor holds. Compatibility is checked on configuration changes,
//! never by reinterpreting stored raw output through a replacement fixture profile.
use crate::{
    FixtureError, FixtureProfile, FreezeFamily, GeometryBracket, GeometryGraph, PatchedFixture,
    PositionCalibrationContext, ProfileError, forward::PositionInstallation,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPositionOutput {
    pub version: u16,
    pub instances: Vec<FrozenPositionInstance>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPositionInstance {
    pub instance_id: Uuid,
    pub controls: Vec<FrozenPositionControl>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPositionControl {
    pub channel_id: Uuid,
    /// Versioned physical interpretation, independent of portable identity remapping.
    pub signature: String,
    /// Full native width, including fine bytes. Never a normalized encoder value.
    pub raw: u32,
}

/// Validate persisted shape, not compatibility with today's patch. Deleted copies,
/// replaced channels and old signatures are stale holds, not corrupt show files.
/// The engine checks signature and current raw width together before applying a hold.
/// `u32` deserialization bounds the original raw domain; comparing only against a
/// replacement channel's (possibly smaller) width would incorrectly reject old shows.
pub fn validate_position_freeze(fixture: &PatchedFixture) -> Result<(), FixtureError> {
    let invalid = |message: &str| FixtureError::Invalid(format!("Position Freeze: {message}"));
    let mut total = 0usize;
    let mut shared = HashMap::new();
    // Stable iteration makes the first malformed-record diagnostic deterministic.
    let mut targets = fixture.freeze.targets.iter().collect::<Vec<_>>();
    targets.sort_by_key(|(id, _)| id.0);
    for (owner, target) in targets {
        let Some(output) = &target.position_native else {
            continue;
        };
        if owner.0.is_nil() || (!target.full && !target.families.contains(&FreezeFamily::Position))
        {
            return Err(invalid(
                "native output requires a non-nil owner and Full or Position ownership",
            ));
        }
        if output.version != 1 || output.instances.is_empty() || output.instances.len() > 4096 {
            return Err(invalid("expected version 1 and 1–4096 physical instances"));
        }
        let mut instances = HashSet::new();
        for instance in &output.instances {
            if instance.instance_id.is_nil() || !instances.insert(instance.instance_id) {
                return Err(invalid(
                    "physical instance IDs must be non-nil and unique per owner",
                ));
            }
            if instance.controls.is_empty() || instance.controls.len() > 4096 {
                return Err(invalid("expected 1–4096 controls per physical instance"));
            }
            total += instance.controls.len();
            if total > 65_536 {
                return Err(invalid("more than 65536 stored controls in one fixture"));
            }
            let mut channels = HashSet::new();
            for control in &instance.controls {
                if control.channel_id.is_nil() || !channels.insert(control.channel_id) {
                    return Err(invalid(
                        "channel IDs must be non-nil and unique per instance",
                    ));
                }
                if control.signature.len() != 64
                    || !control
                        .signature
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                {
                    return Err(invalid(
                        "control signature must be 64 lowercase hexadecimal characters",
                    ));
                }
                let key = (instance.instance_id, control.channel_id);
                let value = (&control.signature, control.raw);
                if shared
                    .insert(key, value)
                    .is_some_and(|previous| previous != value)
                {
                    return Err(invalid(
                        "owners have conflicting holds for one shared physical control",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> ProfileError {
    ProfileError::Invalid(format!("Position Freeze signature: {}", message.into()))
}

fn sorted(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_cached_key(Value::to_string);
    rows
}

/// Cold structural identity for one native Position motor control. No profile,
/// mode, channel, function, node, emitter or head UUID is included in the digest.
/// Names, evidence, Color, lens opening/focus, address routing and world mounts
/// are deliberately excluded. Callers cache the result with their compiled profile.
pub fn position_freeze_control_signature(
    profile: &FixtureProfile,
    mode_id: Uuid,
    channel_id: Uuid,
    installed: PositionInstallation<'_>,
) -> Result<Option<String>, ProfileError> {
    let mode = profile
        .mode(mode_id)
        .ok_or_else(|| invalid("missing mode"))?;
    let Some(model) = &mode.position_physical else {
        return Ok(None);
    };
    let bindings = model
        .bindings
        .iter()
        .filter(|b| b.channel_id == channel_id)
        .collect::<Vec<_>>();
    if bindings.is_empty() {
        return Ok(None);
    }
    let graph = profile.mode_geometry(mode);
    graph.validate(&mode.heads.iter().map(|h| h.id).collect())?;
    mode.validate_position_physical(&graph)?;
    if !installed.bracket_degrees.is_finite() {
        return Err(invalid("nonfinite bracket angle"));
    }
    let context = PositionCalibrationContext::new(profile, mode_id)
        .map_err(invalid)?
        .ok_or_else(|| invalid("missing calibration context"))?;
    let mut calibration = installed.calibration.cloned().unwrap_or_default();
    calibration.validate().map_err(invalid)?;
    // Exactly the same stale-override fallback as CompiledPositionForward.
    if calibration
        .axis_overrides
        .as_ref()
        .is_some_and(|o| o.validate_for_context(&context).is_err())
    {
        calibration.axis_overrides = None;
    }
    let channel = mode
        .channels
        .iter()
        .find(|c| c.id == channel_id)
        .ok_or_else(|| invalid("missing channel"))?;
    let mut motors = Vec::new();
    for binding in bindings {
        let function = channel
            .functions
            .iter()
            .find(|f| f.id == binding.function_id)
            .ok_or_else(|| invalid("missing function"))?;
        let effective = calibration
            .effective_axis(
                &context,
                binding.node_id,
                binding.role,
                installed.invert_pan,
                installed.invert_tilt,
            )
            .map_err(invalid)?;
        let mut lenses = Vec::new();
        for emitter in &graph.emitters {
            let chain = ancestry(&graph, emitter.node_id)?;
            if chain.iter().any(|n| n.id == binding.node_id) {
                lenses.push(json!({
                    "path": structural_path(&graph, &chain, installed.bracket_degrees),
                    "origin": emitter.origin, "orientation": emitter.orientation_degrees,
                    "directional": emitter.directional,
                    "channel_head": emitter.head_id == Some(channel.head_id),
                    "has_head": emitter.head_id.is_some(),
                }));
            }
        }
        let chain = ancestry(&graph, binding.node_id)?;
        motors.push(json!({
            "path": structural_path(&graph, &chain, installed.bracket_degrees),
            "role": binding.role,
            "zero": effective.zero_degrees, "invert": effective.invert,
            "patch_invert": match binding.role {
                crate::PositionAxisRole::Pan => installed.invert_pan,
                crate::PositionAxisRole::Tilt => installed.invert_tilt,
            },
            "function": {
                "from": function.dmx_from, "to": function.dmx_to,
                "priority": function.priority, "behavior": function.behavior,
                "samples": function.physical_mapping.as_ref().map(|m| &m.samples),
                "angular": function.angular_motion,
            },
            "lenses": sorted(lenses),
        }));
    }
    let canonical = json!({
        "version": 1,
        "coordinates": graph.physical_contract.as_ref().map(|c| c.version),
        "resolution": channel.resolution, "invert": channel.invert,
        "transform": channel.canonical_transform,
        "shared_head": mode.heads.iter().find(|h| h.id == channel.head_id).is_some_and(|h| h.master_shared),
        "motors": sorted(motors),
    });
    let bytes = serde_json::to_vec(&canonical).map_err(|e| invalid(e.to_string()))?;
    Ok(Some(format!("{:x}", Sha256::digest(bytes))))
}

fn ancestry(graph: &GeometryGraph, node: Uuid) -> Result<Vec<&crate::GeometryNode>, ProfileError> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = Some(node);
    while let Some(id) = cursor {
        if !seen.insert(id) || chain.len() >= 4096 {
            return Err(invalid("cyclic or oversized ancestry"));
        }
        let n = graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| invalid("missing ancestor"))?;
        chain.push(n);
        cursor = n.parent_id;
    }
    chain.reverse();
    Ok(chain)
}

fn structural_path(
    graph: &GeometryGraph,
    chain: &[&crate::GeometryNode],
    bracket_degrees: f64,
) -> Vec<Value> {
    chain
        .iter()
        .map(|node| {
            let bracket = graph
                .physical_contract
                .as_ref()
                .and_then(|c| match &c.bracket {
                    GeometryBracket::Hinge {
                        node_id,
                        pivot,
                        axis,
                    } if *node_id == node.id => {
                        Some(json!({"pivot":pivot,"axis":axis,"degrees":bracket_degrees}))
                    }
                    _ => None,
                });
            json!({
                "transform": node.transform, "pivot": node.pivot, "bracket": bracket,
                "motion": node.motion.as_ref().map(|m| json!({
                    "kind":m.kind,"axis":m.axis,"min":m.physical_min,"max":m.physical_max,
                    "speed":m.max_speed_per_second,"acceleration":m.acceleration_per_second_squared,
                    "deceleration":m.deceleration_per_second_squared,
                })),
            })
        })
        .collect()
}
