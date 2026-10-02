use super::{
    GdtfImportDiagnostic,
    channels::{mapping, physical_unit, resolve_attribute},
    diagnostic, dmx, identity, invalid,
    xml::Node,
};
use crate::{
    AngularMotion, AngularMotionKind, CanonicalTransform, ChannelFunction, ChannelFunctionBehavior,
    ChannelResolution, PhysicalDataQuality, PhysicalMappingCalibration, ProfileError,
};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub(super) fn read(
    channel: &Node,
    logical: &Node,
    channel_id: Uuid,
    channel_name: &str,
    resolution: ChannelResolution,
    attributes: &BTreeMap<String, &Node>,
    transform: CanonicalTransform,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) -> Result<(Vec<ChannelFunction>, u32), ProfileError> {
    let source = logical
        .children_named("ChannelFunction")
        .collect::<Vec<_>>();
    if source.is_empty() {
        return Err(invalid(format!(
            "channel {channel_name}: missing ChannelFunction"
        )));
    }
    let mut names = HashSet::new();
    let starts = source
        .iter()
        .map(|node| dmx::value(node.attr("DMXFrom").unwrap_or("0/1"), resolution))
        .collect::<Result<Vec<_>, _>>()?;
    if starts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid(format!(
            "channel {channel_name}: function starts must strictly increase in source order"
        )));
    }
    let mut functions = Vec::new();
    let mut defaults = BTreeMap::new();
    for (index, node) in source.iter().enumerate() {
        let name = node.required("Name")?;
        if !names.insert(name) {
            return Err(invalid(format!(
                "channel {channel_name}: duplicate function {name:?}"
            )));
        }
        let path = format!("{channel_name}.{}.{name}", logical.required("Attribute")?);
        let from = starts[index];
        let to = starts
            .get(index + 1)
            .map_or(resolution.max_raw(), |next| next - 1);
        let default = dmx::value(node.attr("Default").unwrap_or("0/1"), resolution)?;
        if !(from..=to).contains(&default) {
            return Err(invalid(format!(
                "{path}: function default lies outside its raw interval"
            )));
        }
        defaults.insert(path.clone(), (default, from, to));
        for reference in ["ModeMaster", "DMXProfile"] {
            if node.attr(reference).is_some_and(|value| !value.is_empty()) {
                return Err(invalid(format!(
                    "{path}: {reference} is not supported; importing it as an unconditional linear function would lose its meaning"
                )));
            }
        }
        if node.child("SubChannelSet").is_some() {
            return Err(invalid(format!("{path}: SubChannelSet is not supported")));
        }
        let source_attribute = node.required("Attribute")?;
        let semantic_attribute = resolve_attribute(source_attribute, attributes)?;
        let (_, attribute, function_transform) = mapping(semantic_attribute);
        if function_transform != transform {
            return Err(invalid(format!(
                "{path}: functions with different canonical transforms on one channel are not supported"
            )));
        }
        let unit = physical_unit(attributes.get(source_attribute).copied());
        let physical_from = physical(node, "PhysicalFrom", 0.0)?;
        let physical_to = physical(node, "PhysicalTo", 1.0)?;
        let sets = node.children_named("ChannelSet").collect::<Vec<_>>();
        let function_id = identity(channel_id, "function", name);
        for reference in ["Emitter", "Filter", "Wheel", "ColorSpace", "Gamut"] {
            if let Some(value) = node.attr(reference).filter(|value| !value.is_empty()) {
                diagnostic(
                    diagnostics,
                    &path,
                    format!(
                        "{reference} reference {value:?} is retained in source only; optical calibration remains unknown."
                    ),
                );
            }
        }
        let scalar = matches!(
            semantic_attribute,
            "Dimmer"
                | "Pan"
                | "Tilt"
                | "PanRotate"
                | "TiltRotate"
                | "Zoom"
                | "Focus1"
                | "CTC"
                | "Iris"
                | "Frost1"
                | "Shutter1Strobe"
        ) || semantic_attribute.starts_with("ColorAdd_")
            || semantic_attribute.starts_with("ColorSub_")
            || semantic_attribute.starts_with("ColorHSB_");
        if scalar && !sets.is_empty() {
            // ChannelSets label physical subranges; their presence alone does not make a
            // continuous axis discrete. The profile exporter labels every continuous function.
            let mut previous = None;
            for set in &sets {
                let start = dmx::value(set.attr("DMXFrom").unwrap_or("0/1"), resolution)?;
                if start < from || start > to || previous.is_some_and(|previous| start <= previous)
                {
                    return Err(invalid(format!(
                        "{path}: channel-set starts must increase within the function"
                    )));
                }
                previous = Some(start);
                for (key, parent) in [("PhysicalFrom", physical_from), ("PhysicalTo", physical_to)]
                {
                    if set.attr(key).is_some() && physical(set, key, parent)? != parent {
                        return Err(invalid(format!(
                            "{path}: varying ChannelSet physical overrides require a richer mapping and cannot be flattened"
                        )));
                    }
                }
            }
            diagnostic(
                diagnostics,
                &path,
                "ChannelSet labels remain in source; the continuous function's physical mapping is preserved.",
            );
        }
        if sets.is_empty() || scalar {
            let constant = physical_from == physical_to || from == to;
            let behavior = if constant {
                diagnostic(
                    diagnostics,
                    &path,
                    "Constant physical output is retained in source; no artificial physical range has been invented.",
                );
                ChannelFunctionBehavior::Fixed {
                    semantic_id: function_id.to_string(),
                    label: name.to_owned(),
                    raw_value: default,
                }
            } else {
                ChannelFunctionBehavior::Continuous {
                    physical_min: physical_from,
                    physical_max: physical_to,
                    unit: unit.clone(),
                }
            };
            let angular_motion = (!constant)
                .then(|| match semantic_attribute {
                    "Pan" | "Tilt" => Some(AngularMotionKind::AbsolutePosition),
                    "PanRotate" | "TiltRotate" => Some(AngularMotionKind::AngularVelocity),
                    _ => None,
                })
                .flatten()
                .map(|kind| AngularMotion {
                    kind,
                    max_speed_degrees_per_second: None,
                    acceleration_degrees_per_second_squared: None,
                    deceleration_degrees_per_second_squared: None,
                });
            functions.push(ChannelFunction {
                id: function_id,
                name: name.to_owned(),
                dmx_from: from,
                dmx_to: to,
                attribute,
                priority: 0,
                angular_motion,
                physical_mapping: (!constant).then(|| PhysicalMappingCalibration {
                    quality: PhysicalDataQuality::Unknown,
                    source: Some(format!("GDTF {path}; source calibration unverified")),
                    ..Default::default()
                }),
                behavior,
            });
        } else {
            let set_starts = sets
                .iter()
                .map(|set| dmx::value(set.attr("DMXFrom").unwrap_or("0/1"), resolution))
                .collect::<Result<Vec<_>, _>>()?;
            if set_starts.first() != Some(&from)
                || set_starts.iter().any(|start| *start > to)
                || set_starts.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid(format!(
                    "{path}: channel sets must start at the function boundary and increase within it"
                )));
            }
            let mut set_names = HashSet::new();
            for (index, set) in sets.iter().enumerate() {
                let set_from = set_starts[index];
                let set_to = set_starts.get(index + 1).map_or(to, |next| next - 1);
                let name = set
                    .attr("Name")
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Set {set_from}"));
                if !set_names.insert(name.clone()) {
                    return Err(invalid(format!(
                        "{path}: duplicate ChannelSet name {name:?}"
                    )));
                }
                let id = identity(function_id, "set", &name);
                functions.push(ChannelFunction {
                    id,
                    name: name.clone(),
                    dmx_from: set_from,
                    dmx_to: set_to,
                    attribute: attribute.clone(),
                    priority: 0,
                    angular_motion: None,
                    physical_mapping: None,
                    behavior: ChannelFunctionBehavior::Indexed {
                        semantic_id: id.to_string(),
                        label: name,
                        raw_value: default.clamp(set_from, set_to),
                    },
                });
            }
            diagnostic(
                diagnostics,
                &path,
                "Named channel sets retain exact native ranges; physical subranges and wheel-slot optical references remain in source and are not treated as a calibrated continuous mapping.",
            );
        }
    }
    let initial = channel
        .attr("InitialFunction")
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('/', "."));
    let (default, from, to) = if let Some(initial) = initial {
        *defaults.get(&initial).ok_or_else(|| {
            invalid(format!(
                "channel {channel_name}: InitialFunction {initial:?} does not resolve"
            ))
        })?
    } else {
        let first = source[0].required("Name")?;
        defaults[&format!("{channel_name}.{}.{first}", logical.required("Attribute")?)]
    };
    let default = channel
        .attr("Default")
        .map(|text| dmx::value(text, resolution))
        .transpose()?
        .unwrap_or(default);
    if !(from..=to).contains(&default) {
        return Err(invalid(format!(
            "channel {channel_name}: default lies outside the initial function"
        )));
    }
    Ok((functions, default))
}

fn physical(node: &Node, key: &str, default: f32) -> Result<f32, ProfileError> {
    match node.attr(key) {
        None => Ok(default),
        Some(text) => text
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{} {key} must be finite", node.name))),
    }
}
