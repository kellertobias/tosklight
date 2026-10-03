use super::{GdtfImportDiagnostic, diagnostic, dmx, functions, identity, invalid, xml::Node};
use crate::{
    CanonicalTransform, ChannelBehavior, ChannelResolution, FixtureChannel, FixtureHead,
    FixtureMode, FixtureSplit, ProfileError,
};
use light_core::AttributeKey;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub(super) fn mode(
    node: &Node,
    profile_id: Uuid,
    attributes: &BTreeMap<String, &Node>,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) -> Result<FixtureMode, ProfileError> {
    let name = node.required("Name")?;
    let id = identity(profile_id, "mode", name);
    for section in ["Relations", "FTMacros"] {
        if node
            .child(section)
            .is_some_and(|child| !child.children.is_empty())
        {
            return Err(invalid(format!(
                "mode {name:?}: {section} is not supported; retain the source and configure these relationships explicitly"
            )));
        }
    }
    let mut mode = empty_mode(id, name);
    let source = node
        .child("DMXChannels")
        .ok_or_else(|| invalid(format!("mode {name:?} has no DMXChannels")))?;
    let mut placed = BTreeMap::new();
    let mut used = HashSet::new();
    let mut footprints = BTreeMap::<u16, u16>::new();
    let mut source_names = HashSet::new();
    for channel in source.children_named("DMXChannel") {
        let logical = channel.children_named("LogicalChannel").collect::<Vec<_>>();
        if logical.len() != 1 {
            return Err(invalid(format!(
                "mode {name:?}: each channel currently requires one LogicalChannel; conditional channel ownership is not supported"
            )));
        }
        let logical = logical[0];
        let attribute_name = logical.required("Attribute")?;
        let geometry = channel.required("Geometry")?;
        let channel_name = format!("{geometry}_{attribute_name}");
        if !source_names.insert(channel_name.clone()) {
            return Err(invalid(format!(
                "mode {name:?}: duplicate channel reference {channel_name}"
            )));
        }
        let (split, offsets) = channel_placement(channel, &channel_name)?;
        let resolution = dmx::resolution(offsets.len())?;
        for slot in &offsets {
            if !used.insert((split, *slot)) {
                return Err(invalid(format!(
                    "mode {name:?}: break {split} slot {slot} is used more than once"
                )));
            }
            footprints
                .entry(split)
                .and_modify(|max| *max = (*max).max(*slot))
                .or_insert(*slot);
        }
        let head_id = identity(id, "head", geometry);
        if !mode.heads.iter().any(|head| head.id == head_id) {
            mode.heads.push(FixtureHead {
                id: head_id,
                name: geometry.to_owned(),
                master_shared: false,
            });
        }
        let channel_id = identity(id, "channel", &channel_name);
        let semantic_attribute = resolve_attribute(attribute_name, attributes)?;
        let (native, canonical, transform) = mapping(semantic_attribute);
        let (functions, default) = functions::read(
            channel,
            logical,
            channel_id,
            &channel_name,
            resolution,
            attributes,
            transform,
            diagnostics,
        )?;
        let highlight = match channel.attr("Highlight") {
            None | Some("None") => default,
            Some(value) => dmx::value(value, resolution)?,
        };
        let (snap, master) = logical_snap_and_master(logical, &channel_name, diagnostics)?;
        if canonical.0.starts_with("gdtf.") {
            diagnostic(
                diagnostics,
                &channel_name,
                format!(
                    "Custom attribute {semantic_attribute:?} retains its exact source identity and needs an explicit desk attribute mapping."
                ),
            );
        }
        let unit = physical_unit(attributes.get(attribute_name).copied());
        let parsed = FixtureChannel {
            id: channel_id,
            head_id,
            split,
            fixture_attribute: native,
            attribute: canonical,
            canonical_transform: transform,
            resolution,
            secondary_slots: offsets[1..].to_vec(),
            default_raw: default,
            highlight_raw: highlight,
            physical_min: None,
            physical_max: None,
            unit,
            invert: false,
            snap,
            reacts_to_virtual_intensity: false,
            virtual_intensity_inverted: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: master == "Group",
            reacts_to_grand_master: master == "Grand",
            behavior: channel_behavior(attribute_name, logical),
            functions,
        };
        placed.insert((split, offsets[0]), parsed);
    }
    let slots = SourceSlots {
        placed,
        used,
        footprints,
    };
    finish_slot_allocation(&mut mode, name, slots, diagnostics)?;
    Ok(mode)
}

/// Read the DMX break and the significance-ordered slot offsets of one source channel.
fn channel_placement(channel: &Node, channel_name: &str) -> Result<(u16, Vec<u16>), ProfileError> {
    let split = channel.attr("DMXBreak").unwrap_or("1").parse::<u16>().map_err(|_| invalid(format!("channel {channel_name}: DMXBreak must be an explicit positive break; geometry-reference overrides are unsupported")))?;
    if split == 0 {
        return Err(invalid("DMXBreak must be positive"));
    }
    let offsets = channel
        .required("Offset")?
        .split(',')
        .map(|offset| {
            offset
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|slot| (1..=512).contains(slot))
                .ok_or_else(|| {
                    invalid(format!(
                        "channel {channel_name}: offsets must be slots 1–512"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((split, offsets))
}

/// Read the LogicalChannel Snap flag and Master binding, reporting the desk-master scope.
fn logical_snap_and_master<'a>(
    logical: &'a Node,
    channel_name: &str,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) -> Result<(bool, &'a str), ProfileError> {
    let snap = match logical.attr("Snap").unwrap_or("No") {
        "Yes" | "On" => true,
        "No" | "Off" => false,
        value => {
            return Err(invalid(format!(
                "channel {channel_name}: invalid Snap {value:?}"
            )));
        }
    };
    let master = logical.attr("Master").unwrap_or("None");
    if !matches!(master, "None" | "Grand" | "Group") {
        return Err(invalid(format!(
            "channel {channel_name}: invalid Master {master:?}"
        )));
    }
    if master != "None" {
        diagnostic(
            diagnostics,
            channel_name,
            format!(
                "GDTF Master={master} enables the corresponding desk master only; sequence-master policy must be configured separately."
            ),
        );
    }
    Ok((snap, master))
}

/// Source slot bookkeeping collected while reading a mode's DMXChannels.
struct SourceSlots {
    placed: BTreeMap<(u16, u16), FixtureChannel>,
    used: HashSet<(u16, u16)>,
    footprints: BTreeMap<u16, u16>,
}

/// Fill sparse source offsets with static gap rows, derive splits and verify that canonical
/// row-order allocation reproduces every declared source offset.
fn finish_slot_allocation(
    mode: &mut FixtureMode,
    name: &str,
    slots: SourceSlots,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) -> Result<(), ProfileError> {
    let SourceSlots {
        mut placed,
        used,
        footprints,
    } = slots;
    let id = mode.id;
    if mode.heads.is_empty() {
        return Err(invalid(format!(
            "mode {name:?} has no supported physical channels"
        )));
    }
    // The canonical table derives primary addresses from row order. Explicit static gap rows
    // preserve sparse source offsets; finer bytes remain in their declared significance order.
    let gap_head = mode.heads[0].id;
    for (&split, &footprint) in &footprints {
        for slot in 1..=footprint {
            if !used.contains(&(split, slot)) {
                let channel = gap(id, gap_head, split, slot);
                placed.insert((split, slot), channel);
            }
        }
        mode.splits.push(FixtureSplit {
            number: split,
            footprint,
        });
    }
    let expected = placed
        .iter()
        .map(|((_, slot), channel)| (channel.id, *slot))
        .collect::<BTreeMap<_, _>>();
    mode.channels = placed.into_values().collect();
    bind_native_hue_saturation(mode, diagnostics)?;
    let actual = mode.primary_slots()?;
    if expected
        .iter()
        .any(|(id, slot)| actual.get(id) != Some(slot))
    {
        return Err(invalid(format!(
            "mode {name:?}: canonical slot allocation did not preserve source offsets"
        )));
    }
    diagnostic(
        diagnostics,
        format!("DMXMode.{name}"),
        "Geometry names identify source channel groups only; shared-head dependencies require physical geometry configuration.",
    );
    Ok(())
}

fn empty_mode(id: Uuid, name: &str) -> FixtureMode {
    FixtureMode {
        id,
        name: name.to_owned(),
        notes: String::new(),
        splits: Vec::new(),
        heads: Vec::new(),
        channels: Vec::new(),
        color_systems: Vec::new(),
        color_physical: None,
        position_physical: None,
        control_actions: Vec::new(),
        geometry: Default::default(),
        emitter_heads: Vec::new(),
        motion_attributes: Vec::new(),
    }
}

/// A channel whose every function is NoFeature is static; everything else is controlled.
fn channel_behavior(attribute_name: &str, logical: &Node) -> ChannelBehavior {
    if attribute_name == "NoFeature"
        && logical
            .children_named("ChannelFunction")
            .all(|function| function.attr("Attribute") == Some("NoFeature"))
    {
        ChannelBehavior::Static
    } else {
        ChannelBehavior::Controlled
    }
}

fn gap(mode_id: Uuid, head_id: Uuid, split: u16, slot: u16) -> FixtureChannel {
    FixtureChannel {
        id: identity(mode_id, "unused-slot", &format!("{split}:{slot}")),
        head_id,
        split,
        fixture_attribute: AttributeKey("fixture.control".into()),
        attribute: AttributeKey("fixture.control".into()),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: 0,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: true,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Static,
        functions: Vec::new(),
    }
}

pub(super) fn mapping(source: &str) -> (AttributeKey, AttributeKey, CanonicalTransform) {
    let native = match source {
        "Dimmer" => "intensity",
        "Pan" | "PanRotate" => "pan",
        "Tilt" | "TiltRotate" => "tilt",
        "ColorAdd_R" => "color.red",
        "ColorAdd_G" => "color.green",
        "ColorAdd_B" => "color.blue",
        "ColorAdd_W" => "color.white",
        "ColorAdd_WW" => "color.warm_white",
        "ColorAdd_CW" => "color.cold_white",
        "ColorAdd_A" => "color.amber",
        "ColorAdd_GY" => "color.lime",
        "ColorAdd_UV" => "color.uv",
        "ColorHSB_Hue" => "color.hue",
        "ColorHSB_Saturation" => "color.saturation",
        "ColorHSB_Brightness" => "color.brightness",
        "ColorSub_C" => "color.cyan",
        "ColorSub_M" => "color.magenta",
        "ColorSub_Y" => "color.yellow",
        "CTC" => "color.temperature",
        "Zoom" => "zoom",
        "Focus1" => "focus",
        "Iris" => "iris",
        "Shutter1" => "shutter",
        "Shutter1Strobe" => "strobe",
        "Frost1" => "softness",
        "NoFeature" => "fixture.control",
        _ => "",
    };
    let native = if !native.is_empty() {
        native.to_owned()
    } else if let Some(wheel) = numbered(source, "Color", "color.wheel.") {
        wheel
    } else if let Some(wheel) = numbered(source, "Gobo", "gobo.") {
        wheel
    } else {
        // AttributeKey is a persisted string identity. Do not lowercase or sanitize it:
        // distinct valid source names (Custom-A / Custom_A) must never merge.
        format!("gdtf.{source}")
    };
    let native = AttributeKey(native.into());
    let (canonical, transform) = light_core::canonical_attribute_migration(&native)
        .map(|(canonical, transform)| {
            (
                canonical,
                match transform {
                    light_core::CanonicalAttributeTransform::Identity => {
                        CanonicalTransform::Identity
                    }
                    light_core::CanonicalAttributeTransform::InvertNormalized => {
                        CanonicalTransform::InvertNormalized
                    }
                },
            )
        })
        .unwrap_or((native.clone(), CanonicalTransform::Identity));
    (native, canonical, transform)
}

fn numbered(source: &str, prefix: &str, target: &str) -> Option<String> {
    let suffix = source.strip_prefix(prefix)?;
    (!suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| format!("{target}{suffix}"))
}

pub(super) fn physical_unit(attribute: Option<&Node>) -> Option<String> {
    let unit = attribute?.attr("PhysicalUnit")?;
    Some(
        match unit {
            "Angle" => "degrees",
            "AngularSpeed" => "degrees/s",
            "Percent" => "percent",
            "Length" => "metres",
            "Frequency" => "Hz",
            "Temperature" => "kelvin",
            "Time" => "seconds",
            "None" => "normalized",
            other => other,
        }
        .to_owned(),
    )
}

/// MainAttribute carries semantic aliases; units remain on the referenced original definition.
/// Never infer aliases by stripping a suffix from an arbitrary manufacturer's name.
pub(super) fn resolve_attribute<'a>(
    source: &'a str,
    attributes: &'a BTreeMap<String, &Node>,
) -> Result<&'a str, ProfileError> {
    let mut current = source;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(invalid(format!(
                "cyclic MainAttribute reference for {source:?}"
            )));
        }
        let node = attributes
            .get(current)
            .ok_or_else(|| invalid(format!("Attribute reference {current:?} does not resolve")))?;
        match node.attr("MainAttribute").filter(|name| !name.is_empty()) {
            Some(main) => current = main,
            None => return Ok(current),
        }
    }
}

fn bind_native_hue_saturation(
    mode: &mut FixtureMode,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) -> Result<(), ProfileError> {
    for head in &mode.heads {
        let channels = mode
            .channels
            .iter()
            .filter(|channel| channel.head_id == head.id)
            .collect::<Vec<_>>();
        let coordinate = |name: &str| -> Result<Option<Uuid>, ProfileError> {
            let found = channels
                .iter()
                .filter(|channel| channel.attribute.0.as_ref() == name)
                .map(|channel| channel.id)
                .collect::<Vec<_>>();
            if found.len() > 1 {
                return Err(invalid(format!(
                    "head {} has ambiguous native {name} controls",
                    head.name
                )));
            }
            Ok(found.first().copied())
        };
        let hue = coordinate("color.hue")?;
        let saturation = coordinate("color.saturation")?;
        let brightness = coordinate("color.brightness")?;
        if hue.is_none() && saturation.is_none() && brightness.is_none() {
            continue;
        }
        let (Some(hue_channel_id), Some(saturation_channel_id)) = (hue, saturation) else {
            return Err(invalid(format!(
                "head {}: native HSB requires both Hue and Saturation channels",
                head.name
            )));
        };
        mode.color_systems.push(crate::HeadColorSystem {
            head_id: head.id,
            correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            calibration: Default::default(),
            system: crate::ColorSystem::HueSaturation {
                hue_channel_id,
                saturation_channel_id,
                intensity_channel_id: brightness,
            },
        });
        diagnostic(
            diagnostics,
            format!("{}.{}", mode.name, head.name),
            "Native hue/saturation coordinates are linked; their optical response remains nominal and unmeasured.",
        );
    }
    Ok(())
}
