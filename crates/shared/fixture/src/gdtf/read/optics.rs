//! GDTF emitters, filters, colour-wheel slots and beam spectra as the physical colour model.
//!
//! Links resolve to the exact imported channel functions. Source data is nominal and unverified:
//! every imported item carries Unknown provenance, a linear response and a maximum level of 1.
//! A model that does not validate is dropped with a diagnostic instead of failing the import.

use super::{
    GdtfImportDiagnostic, channels::resolve_attribute, diagnostic, dmx, identity, xml::Node,
};
use crate::gdtf::physical::{band_from_name, derived_band, is_color, xyz};
use crate::{
    ChannelFunction, ChannelFunctionBehavior, ColorPhysicalModel, FilterSpectrum, FixtureMode,
    FixtureProfile, HeadOpticalPath, NativeColorBinding, OpticalEmitter, OpticalFilter,
    OpticalProvenance, OpticalSource, OpticalTransmission, SpectrumSample,
};
use std::collections::BTreeMap;
use uuid::Uuid;

/// The fixture-level nodes channel functions and beams link to.
pub(super) struct Descriptions<'a> {
    emitters: BTreeMap<&'a str, &'a Node>,
    filters: BTreeMap<&'a str, &'a Node>,
    wheels: BTreeMap<&'a str, &'a Node>,
    beams: Vec<&'a Node>,
}

fn named<'a>(parent: Option<&'a Node>, kind: &'static str) -> BTreeMap<&'a str, &'a Node> {
    parent
        .map(|parent| {
            parent
                .children_named(kind)
                .filter_map(|node| Some((node.attr("Name")?, node)))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn descriptions(fixture: &Node) -> Descriptions<'_> {
    let physical = fixture.child("PhysicalDescriptions");
    let mut beams = Vec::new();
    if let Some(geometries) = fixture.child("Geometries") {
        collect_beams(geometries, &mut beams);
    }
    Descriptions {
        emitters: named(physical.and_then(|node| node.child("Emitters")), "Emitter"),
        filters: named(physical.and_then(|node| node.child("Filters")), "Filter"),
        wheels: named(fixture.child("Wheels"), "Wheel"),
        beams,
    }
}

fn collect_beams<'a>(node: &'a Node, output: &mut Vec<&'a Node>) {
    for child in &node.children {
        if child.name == "Beam" {
            output.push(child);
        }
        collect_beams(child, output);
    }
}

/// Nominal beam angle, colour temperature and flux of the first beam that declares them.
pub(super) fn beam_optics(descriptions: &Descriptions<'_>, profile: &mut FixtureProfile) {
    let first = |key: &str| {
        descriptions.beams.iter().find_map(|beam| {
            beam.attr(key)?
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite() && *value > 0.0)
        })
    };
    let optics = &mut profile.optics;
    optics.beam_angle_degrees = first("BeamAngle");
    optics.color_temperature_kelvin = first("ColorTemperature");
    optics.luminous_output_lumens = first("LuminousFlux");
}

#[derive(Default)]
struct Draft {
    emitters: Vec<OpticalEmitter>,
    filters: Vec<OpticalFilter>,
    bound: Vec<Uuid>,
    fixed: Option<OpticalSource>,
}

/// Resolved context of one linked channel function.
struct Linked<'a> {
    geometry: &'a str,
    channel_id: Uuid,
    function: &'a ChannelFunction,
    semantic: &'a str,
}

/// Builds `mode.color_physical` from the links of its source DMX mode.
pub(super) fn attach(
    descriptions: &Descriptions<'_>,
    source: &Node,
    mode: &mut FixtureMode,
    attributes: &BTreeMap<String, &Node>,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) {
    let mut drafts = BTreeMap::<String, Draft>::new();
    let channels = source
        .child("DMXChannels")
        .map(|node| node.children_named("DMXChannel").collect::<Vec<_>>())
        .unwrap_or_default();
    for channel in channels {
        let (Some(geometry), Some(logical)) =
            (channel.attr("Geometry"), channel.child("LogicalChannel"))
        else {
            continue;
        };
        let Some(attribute) = logical.attr("Attribute") else {
            continue;
        };
        let channel_id = identity(mode.id, "channel", &format!("{geometry}_{attribute}"));
        let Some(imported) = mode
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
        else {
            continue;
        };
        for function in logical.children_named("ChannelFunction") {
            let Ok(semantic) =
                resolve_attribute(function.attr("Attribute").unwrap_or(""), attributes)
            else {
                continue;
            };
            let start = |node: &Node| {
                dmx::value(node.attr("DMXFrom").unwrap_or("0/1"), imported.resolution).ok()
            };
            let find = |raw: Option<u32>| {
                imported
                    .functions
                    .iter()
                    .find(|function| Some(function.dmx_from) == raw)
            };
            let draft = drafts.entry(geometry.to_owned()).or_default();
            if let Some(found) = find(start(function)) {
                let linked = Linked {
                    geometry,
                    channel_id,
                    function: found,
                    semantic,
                };
                if let Some(name) = function.attr("Emitter").filter(|name| !name.is_empty()) {
                    emitter(descriptions, mode.id, &linked, name, draft, diagnostics);
                }
                if let Some(name) = function.attr("Filter").filter(|name| !name.is_empty()) {
                    filter(
                        descriptions,
                        mode.id,
                        &linked,
                        name,
                        name,
                        draft,
                        diagnostics,
                    );
                }
            }
            let Some(wheel) = function.attr("Wheel").filter(|name| !name.is_empty()) else {
                continue;
            };
            if !is_color_wheel(semantic) {
                continue;
            }
            let Some(wheel) = descriptions.wheels.get(wheel) else {
                diagnostic(
                    diagnostics,
                    geometry,
                    format!("Wheel {wheel:?} does not resolve; no wheel filter is imported."),
                );
                continue;
            };
            let slots = wheel.children_named("Slot").collect::<Vec<_>>();
            for set in function.children_named("ChannelSet") {
                let index = set
                    .attr("WheelSlotIndex")
                    .and_then(|index| index.parse::<usize>().ok());
                let (Some(slot), Some(found)) = (
                    index.and_then(|index| slots.get(index.checked_sub(1)?)),
                    find(start(set)),
                ) else {
                    continue;
                };
                let linked = Linked {
                    geometry,
                    channel_id,
                    function: found,
                    semantic,
                };
                let name = slot.attr("Name").unwrap_or(&found.name);
                let link = slot.attr("Filter").filter(|name| !name.is_empty());
                filter(
                    descriptions,
                    mode.id,
                    &linked,
                    name,
                    link.unwrap_or(""),
                    draft,
                    diagnostics,
                );
            }
        }
    }
    beam_spectra(descriptions, mode, &mut drafts, diagnostics);
    drafts.retain(|_, draft| {
        !draft.emitters.is_empty() || !draft.filters.is_empty() || draft.fixed.is_some()
    });
    if drafts.is_empty() {
        return;
    }
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 0,
        paths: drafts
            .into_iter()
            .map(|(geometry, draft)| path(mode, &geometry, draft))
            .collect(),
    });
    if let Err(error) = mode.validate_color_physical() {
        mode.color_physical = None;
        diagnostic(
            diagnostics,
            format!("DMXMode.{}", mode.name),
            format!(
                "Linked emitters, filters and wheel slots do not form a valid physical colour model and are not imported: {error}"
            ),
        );
    } else {
        diagnostic(
            diagnostics,
            format!("DMXMode.{}", mode.name),
            "Linked emitters, filters and wheel slots are imported as a nominal physical colour model; their source data is unverified.",
        );
    }
}

fn is_color_wheel(semantic: &str) -> bool {
    semantic
        .strip_prefix("Color")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
}

fn imported(kind: &str, name: &str) -> OpticalProvenance {
    OpticalProvenance {
        source: Some(format!("GDTF {kind} {name}; source data unverified")),
        ..Default::default()
    }
}

fn path_id(mode: Uuid, geometry: &str) -> Uuid {
    identity(mode, "color-path", geometry)
}

fn path(mode: &FixtureMode, geometry: &str, draft: Draft) -> HeadOpticalPath {
    let head_id = identity(mode.id, "head", geometry);
    let controls = mode
        .channels
        .iter()
        .filter(|channel| {
            draft.bound.contains(&channel.id) || (channel.head_id == head_id && is_color(channel))
        })
        .map(|channel| channel.id)
        .collect();
    let source = if draft.emitters.is_empty() {
        draft.fixed.unwrap_or(OpticalSource::Unknown)
    } else {
        OpticalSource::Additive {
            emitters: draft.emitters,
        }
    };
    HeadOpticalPath {
        id: path_id(mode.id, geometry),
        head_id,
        controls,
        source,
        filters: draft.filters,
        measurements: Vec::new(),
    }
}

/// CIE `x,y,Y` text as XYZ; absent or malformed colour is unknown, never black.
fn color(node: &Node) -> Option<light_core::Xyz> {
    let values = node
        .attr("Color")?
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
        })
        .collect::<Option<Vec<_>>>()?;
    xyz(<[f64; 3]>::try_from(values).ok()?)
}

/// `(physical, interpolation is Step, spectrum)` of every measurement with a usable spectrum.
fn measurements(node: &Node) -> Vec<(f64, bool, Vec<SpectrumSample>)> {
    let mut output = node
        .children_named("Measurement")
        .filter_map(|measurement| {
            let physical = measurement.attr("Physical")?.parse::<f64>().ok()?;
            let spectrum = measurement
                .children_named("MeasurementPoint")
                .filter_map(|point| {
                    Some(SpectrumSample {
                        wavelength_nm: point.attr("WaveLength")?.parse().ok()?,
                        value: point.attr("Energy")?.parse().ok()?,
                    })
                })
                .collect::<Vec<_>>();
            (physical.is_finite() && spectrum.len() >= 2).then(|| {
                (
                    physical,
                    measurement.attr("InterpolationTo") == Some("Step"),
                    spectrum,
                )
            })
        })
        .collect::<Vec<_>>();
    output.sort_by(|a, b| a.0.total_cmp(&b.0));
    output
}

fn emitter(
    descriptions: &Descriptions<'_>,
    mode: Uuid,
    linked: &Linked<'_>,
    name: &str,
    draft: &mut Draft,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) {
    let Some(node) = descriptions.emitters.get(name) else {
        diagnostic(
            diagnostics,
            linked.geometry,
            format!("Emitter {name:?} does not resolve; it is not imported."),
        );
        return;
    };
    let reversed = matches!(
        linked.function.behavior,
        ChannelFunctionBehavior::Continuous { physical_min, physical_max, .. } if physical_min > physical_max
    );
    let band = node
        .attr("DiodePart")
        .and_then(band_from_name)
        .unwrap_or_else(|| derived_band(linked.semantic));
    let id = identity(
        path_id(mode, linked.geometry),
        "emitter",
        &linked.function.id.to_string(),
    );
    draft.emitters.push(OpticalEmitter {
        id,
        name: name.to_owned(),
        binding: NativeColorBinding {
            channel_id: linked.channel_id,
            function_id: linked.function.id,
        },
        xyz: color(node),
        spectrum: measurements(node)
            .pop()
            .map(|(_, _, spectrum)| spectrum)
            .unwrap_or_default(),
        band,
        native_reversed: reversed,
        maximum_level: 1.0,
        response_exponent: 1.0,
        provenance: imported("Emitter", name),
    });
    draft.bound.push(linked.channel_id);
}

/// A filter on a function or a wheel slot. `link` is empty when the slot names no filter.
fn filter(
    descriptions: &Descriptions<'_>,
    mode: Uuid,
    linked: &Linked<'_>,
    name: &str,
    link: &str,
    draft: &mut Draft,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) {
    let node = descriptions.filters.get(link);
    if !link.is_empty() && node.is_none() {
        diagnostic(
            diagnostics,
            linked.geometry,
            format!("Filter {link:?} does not resolve; its transmission is unknown."),
        );
    }
    let function = linked.function;
    let (from, to) = (function.dmx_from, function.dmx_to);
    let raw = |physical: f64| {
        let span = f64::from(to - from);
        from + (physical.clamp(0.0, 100.0) / 100.0 * span).round() as u32
    };
    let measured = node.map(|node| measurements(node)).unwrap_or_default();
    let samples = measured
        .iter()
        .enumerate()
        .map(|(index, (physical, step, spectrum))| {
            let raw_from = raw(*physical);
            let raw_to = if *step {
                measured
                    .get(index + 1)
                    .map_or(to, |next| raw(next.0).saturating_sub(1).max(raw_from))
            } else {
                raw_from
            };
            FilterSpectrum {
                raw_from,
                raw_to,
                spectrum: spectrum.clone(),
            }
        })
        .collect::<Vec<_>>();
    let transmission = if samples.is_empty() {
        OpticalTransmission::Unknown
    } else {
        OpticalTransmission::Spectral { samples }
    };
    let id = identity(
        path_id(mode, linked.geometry),
        "filter",
        &function.id.to_string(),
    );
    draft.filters.push(OpticalFilter {
        id,
        name: name.to_owned(),
        binding: NativeColorBinding {
            channel_id: linked.channel_id,
            function_id: function.id,
        },
        transmission,
        provenance: imported("Filter", name),
    });
    draft.bound.push(linked.channel_id);
}

/// A beam's `EmitterSpectrum` is the fixed white source of the head it belongs to: the head of the
/// same name, or the body when the fixture has a single beam.
fn beam_spectra(
    descriptions: &Descriptions<'_>,
    mode: &FixtureMode,
    drafts: &mut BTreeMap<String, Draft>,
    diagnostics: &mut Vec<GdtfImportDiagnostic>,
) {
    for beam in &descriptions.beams {
        let (Some(beam_name), Some(link)) = (beam.attr("Name"), beam.attr("EmitterSpectrum"))
        else {
            continue;
        };
        let has = |name: &str| mode.heads.iter().any(|head| head.name == name);
        let geometry = if has(beam_name) {
            beam_name
        } else if descriptions.beams.len() == 1 && has(crate::gdtf::GEOMETRY) {
            crate::gdtf::GEOMETRY
        } else {
            continue;
        };
        let Some(node) = descriptions.emitters.get(link) else {
            diagnostic(
                diagnostics,
                beam_name,
                format!("EmitterSpectrum {link:?} does not resolve; the source stays unknown."),
            );
            continue;
        };
        drafts.entry(geometry.to_owned()).or_default().fixed = Some(OpticalSource::Fixed {
            xyz: color(node),
            spectrum: measurements(node)
                .pop()
                .map(|(_, _, spectrum)| spectrum)
                .unwrap_or_default(),
            provenance: imported("Emitter", link),
        });
    }
}
