//! Conversion of exact native function intervals into the GDTF writer's model.

use super::{attribute_key, native_function_attribute, unique};
use crate::gdtf::{ChannelSet, Function, Mode, gdtf_name};
use crate::{
    AngularMotionKind, ChannelFunctionBehavior, CompiledPhysicalMapping, FixtureChannel,
    ProfileError,
};
use std::collections::{HashMap, HashSet};

pub(super) fn from_channel(channel: &FixtureChannel) -> Result<Vec<Function>, ProfileError> {
    let mut source: Vec<_> = channel.functions.iter().collect();
    source.sort_by_key(|function| function.dmx_from);
    let mut output = Vec::new();
    let mut names = HashSet::new();
    let mut next = 0_u64;
    let maximum = channel.resolution.max_raw();
    for function in source {
        if function.dmx_from > function.dmx_to
            || function.dmx_to > maximum
            || u64::from(function.dmx_from) < next
        {
            return Err(ProfileError::Invalid(format!(
                "GDTF function {} has an invalid or overlapping raw interval",
                function.name
            )));
        }
        if u64::from(function.dmx_from) > next {
            output.push(gap(
                next as u32,
                function.dmx_from - 1,
                channel.default_raw,
                &mut names,
            ));
        }
        if let Some(calibration) = &function.physical_mapping {
            CompiledPhysicalMapping::compile(channel, function)?;
            if calibration.samples.len() > 2 {
                return Err(ProfileError::Invalid(format!(
                    "GDTF function {} has a piecewise physical curve that this exporter cannot preserve; export the native fixture package instead",
                    function.name
                )));
            }
        }
        let (physical, physical_unit) = match &function.behavior {
            ChannelFunctionBehavior::Continuous {
                physical_min,
                physical_max,
                unit,
            } => {
                if !physical_min.is_finite() || !physical_max.is_finite() {
                    return Err(ProfileError::Invalid(format!(
                        "GDTF function {} has non-finite physical endpoints",
                        function.name
                    )));
                }
                (
                    Some((*physical_min, *physical_max)),
                    physical_unit(unit.as_deref())?,
                )
            }
            _ => (None, None),
        };
        let native_attribute = native_function_attribute(channel, function.attribute.0.as_ref());
        let (mut attribute, feature) = attribute_key(native_attribute);
        if function
            .angular_motion
            .is_some_and(|motion| motion.kind == AngularMotionKind::AngularVelocity)
        {
            attribute = match attribute.as_str() {
                "Pan" => "PanRotate".into(),
                "Tilt" => "TiltRotate".into(),
                _ => attribute,
            };
        }
        let label = match &function.behavior {
            ChannelFunctionBehavior::Fixed { label, .. }
            | ChannelFunctionBehavior::Indexed { label, .. }
                if !label.trim().is_empty() =>
            {
                label.as_str()
            }
            _ => function.name.as_str(),
        };
        let default = if (function.dmx_from..=function.dmx_to).contains(&channel.default_raw) {
            channel.default_raw
        } else {
            match &function.behavior {
                ChannelFunctionBehavior::Fixed { raw_value, .. }
                | ChannelFunctionBehavior::Indexed { raw_value, .. } => {
                    if !(function.dmx_from..=function.dmx_to).contains(raw_value) {
                        return Err(ProfileError::Invalid(format!(
                            "GDTF function {} has a fixed value outside its raw interval",
                            function.name
                        )));
                    }
                    *raw_value
                }
                _ => function.dmx_from,
            }
        };
        output.push(Function {
            // A GDTF reader resolves `InitialFunction` links with `/` as a separator too.
            name: unique(
                &gdtf_name(&function.name).replace('/', "-"),
                "Function",
                " ",
                &mut names,
            ),
            attribute,
            main_attribute: None,
            original_attribute: native_attribute.to_owned(),
            feature: feature.to_owned(),
            physical_unit,
            from: function.dmx_from,
            to: function.dmx_to,
            default,
            physical,
            sets: vec![ChannelSet {
                name: label.to_owned(),
                from: function.dmx_from,
            }],
            emitter: None,
            filter: None,
            wheel: None,
        });
        next = u64::from(function.dmx_to) + 1;
    }
    if !output.is_empty() && next <= u64::from(maximum) {
        output.push(gap(next as u32, maximum, channel.default_raw, &mut names));
    }
    Ok(output)
}

fn gap(from: u32, to: u32, default: u32, names: &mut HashSet<String>) -> Function {
    Function {
        name: unique("Unused", "Unused", " ", names),
        attribute: "NoFeature".into(),
        main_attribute: None,
        original_attribute: String::new(),
        feature: "Control.Control".into(),
        physical_unit: None,
        from,
        to,
        default: default.clamp(from, to),
        physical: None,
        sets: Vec::new(),
        emitter: None,
        filter: None,
        wheel: None,
    }
}

/// Only units with the same numeric convention are mapped. No guessed conversion or
/// relabelling (for example rpm as degree/s or millimetres as metres) is permitted.
pub(super) fn physical_unit(unit: Option<&str>) -> Result<Option<String>, ProfileError> {
    let Some(unit) = unit.map(str::trim).filter(|unit| !unit.is_empty()) else {
        return Ok(None);
    };
    let gdtf = match unit.to_ascii_lowercase().as_str() {
        "none" | "normalized" | "normalised" | "0..1" => "None",
        "deg" | "degree" | "degrees" | "°" | "angle" => "Angle",
        "deg/s" | "degree/s" | "degrees/s" | "degrees per second" | "°/s" | "angularspeed" => {
            "AngularSpeed"
        }
        "%" | "percent" | "percentage" => "Percent",
        "m" | "metre" | "metres" | "meter" | "meters" | "length" => "Length",
        "hz" | "hertz" | "frequency" => "Frequency",
        "k" | "kelvin" | "temperature" => "Temperature",
        "s" | "second" | "seconds" | "time" => "Time",
        "colorcomponent" => "ColorComponent",
        _ => {
            return Err(ProfileError::Invalid(format!(
                "GDTF export cannot preserve physical unit {unit:?}; export the native fixture package instead"
            )));
        }
    };
    Ok(Some(gdtf.into()))
}

/// GDTF units live on global Attribute definitions. Two modes may use the same canonical
/// attribute with different units; give those definitions distinct names, never keep the
/// first unit and silently reinterpret later functions.
pub(super) fn separate_attribute_units(modes: &mut [Mode]) {
    let mut definitions = HashMap::<(String, Option<String>), String>::new();
    let mut names = HashSet::new();
    for channel in modes.iter_mut().flat_map(|mode| &mut mode.channels) {
        register(
            &mut channel.attribute,
            &mut channel.main_attribute,
            &channel.physical_unit,
            &mut definitions,
            &mut names,
        );
        for function in &mut channel.functions {
            register(
                &mut function.attribute,
                &mut function.main_attribute,
                &function.physical_unit,
                &mut definitions,
                &mut names,
            );
        }
    }
}

fn register(
    attribute: &mut String,
    main_attribute: &mut Option<String>,
    unit: &Option<String>,
    definitions: &mut HashMap<(String, Option<String>), String>,
    names: &mut HashSet<String>,
) {
    let key = (attribute.clone(), unit.clone());
    if let Some(name) = definitions.get(&key) {
        if name != attribute && main_attribute.is_none() {
            *main_attribute = Some(attribute.clone());
        }
        *attribute = name.clone();
        return;
    }
    let name = if names.insert(attribute.to_ascii_lowercase()) {
        attribute.clone()
    } else {
        unique(
            &format!("{}_{}", attribute, unit.as_deref().unwrap_or("Unknown")),
            "Attribute",
            "_",
            names,
        )
    };
    if name != *attribute && main_attribute.is_none() {
        *main_attribute = Some(attribute.clone());
    }
    definitions.insert(key, name.clone());
    *attribute = name;
}
