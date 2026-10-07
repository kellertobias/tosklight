//! Physical colour models as GDTF emitters, filters, colour wheels and beam spectra.
//!
//! Every binding becomes a link on the exact channel function it names. Data GDTF has no place
//! for is reported as an export diagnostic instead of being silently dropped or approximated.

use super::{GEOMETRY, GdtfExportDiagnostic, attribute_key, native_function_attribute, unique};
use crate::gdtf::physical::{
    BeamOptics, Emitter, Filter, Interpolation, Measurement, PhysicalDescriptions, Slot, Wheel,
    band_name, derived_band, is_color, xyy,
};
use crate::gdtf::{Function, gdtf_name};
use crate::{
    ChannelFunction, ChannelFunctionBehavior, FilterSpectrum, FixtureChannel, FixtureMode,
    FixtureProfile, HeadOpticalPath, NativeColorBinding, OpticalEmitter, OpticalFilter,
    OpticalProvenance, OpticalSource, OpticalTransmission, SpectrumSample,
};
use light_core::Xyz;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone)]
enum Link {
    Emitter(String),
    Filter(String),
    Slot(String, u16),
}

/// The GDTF physical descriptions of a profile and the function links into them.
pub(super) struct Optics {
    pub physical: PhysicalDescriptions,
    links: HashMap<(usize, Uuid, Uuid), Link>,
    pub diagnostics: Vec<GdtfExportDiagnostic>,
}

impl Optics {
    /// Sets the physical links of the exported functions of one profile channel.
    pub fn annotate(&self, mode: usize, channel: &FixtureChannel, functions: &mut [Function]) {
        for source in &channel.functions {
            let Some(link) = self.links.get(&(mode, channel.id, source.id)) else {
                continue;
            };
            let Some(function) = functions
                .iter_mut()
                .find(|function| function.from == source.dmx_from && function.to == source.dmx_to)
            else {
                continue;
            };
            match link.clone() {
                Link::Emitter(name) => function.emitter = Some(name),
                Link::Filter(name) => function.filter = Some(name),
                Link::Slot(wheel, slot) => function.wheel = Some((wheel, slot)),
            }
        }
    }
}

/// A fixed source waiting for the fixture-wide beam it can be linked from.
struct FixedSource {
    mode: usize,
    beam: String,
    geometry: String,
    node: String,
    emitter: Emitter,
}

/// A wheel-slot filter: raw start, function and filter name.
type WheelSlot = (u32, Uuid, String);

#[derive(Default)]
struct Builder {
    physical: PhysicalDescriptions,
    links: HashMap<(usize, Uuid, Uuid), Link>,
    diagnostics: Vec<GdtfExportDiagnostic>,
    keys: HashMap<String, String>,
    emitter_names: HashSet<String>,
    filter_names: HashSet<String>,
    wheel_names: HashSet<String>,
    /// Wheel-slot filters per mode and channel: `(raw start, function, filter name)`.
    slots: BTreeMap<(usize, Uuid), Vec<WheelSlot>>,
    fixed: Vec<FixedSource>,
}

pub(super) fn build(profile: &FixtureProfile, heads: &HashMap<Uuid, String>) -> Optics {
    let mut builder = Builder::default();
    let optics = &profile.optics;
    let positive = |value: Option<f32>| value.filter(|value| value.is_finite() && *value > 0.0);
    builder.physical.beam_optics = BeamOptics {
        beam_angle: positive(optics.beam_angle_degrees),
        color_temperature: positive(optics.color_temperature_kelvin),
        luminous_flux: positive(optics.luminous_output_lumens),
    };
    for (index, mode) in profile.modes.iter().enumerate() {
        let Some(model) = &mode.color_physical else {
            continue;
        };
        for path in &model.paths {
            builder.path(index, mode, path, heads);
        }
    }
    builder.wheels(profile);
    builder.fixed_sources(profile, heads);
    Optics {
        physical: builder.physical,
        links: builder.links,
        diagnostics: builder.diagnostics,
    }
}

/// The geometry the writer places a channel of `head` on.
fn geometry(head: Uuid, heads: &HashMap<Uuid, String>) -> String {
    heads
        .get(&head)
        .cloned()
        .unwrap_or_else(|| GEOMETRY.to_owned())
}

fn resolve(
    mode: &FixtureMode,
    binding: NativeColorBinding,
) -> Option<(&FixtureChannel, &ChannelFunction)> {
    let channel = mode
        .channels
        .iter()
        .find(|channel| channel.id == binding.channel_id)?;
    let function = channel
        .functions
        .iter()
        .find(|function| function.id == binding.function_id)?;
    Some((channel, function))
}

fn spectrum_points(spectrum: &[SpectrumSample]) -> Vec<(f32, f32)> {
    spectrum
        .iter()
        .map(|sample| (sample.wavelength_nm, sample.value))
        .collect()
}

impl Builder {
    fn report(&mut self, node: &str, message: impl Into<String>) {
        self.diagnostics.push(GdtfExportDiagnostic {
            node: node.to_owned(),
            message: message.into(),
        });
    }

    fn provenance(&mut self, node: &str, provenance: &OpticalProvenance) {
        if *provenance != OpticalProvenance::default() {
            self.report(
                node,
                "Optical provenance (quality, source and revision) has no GDTF field and is not exported; a GDTF import marks this data unverified.",
            );
        }
    }

    /// Interns a description by its exact content, so identical data across modes is shared.
    fn intern(&mut self, key: String, base: &str, fallback: &str, kind: u8) -> (String, bool) {
        let key = format!("{kind}:{key}");
        if let Some(name) = self.keys.get(&key) {
            return (name.clone(), false);
        }
        let names = match kind {
            0 => &mut self.emitter_names,
            1 => &mut self.filter_names,
            _ => &mut self.wheel_names,
        };
        let name = unique(&gdtf_name(base), fallback, " ", names);
        self.keys.insert(key, name.clone());
        (name, true)
    }

    fn emitter(&mut self, mut emitter: Emitter) -> String {
        let key = format!(
            "{:?}",
            (
                &emitter.name,
                emitter.color,
                &emitter.diode_part,
                &emitter.measurements
            )
        );
        let (name, new) = self.intern(key, &emitter.name.clone(), "Emitter", 0);
        if new {
            emitter.name = name.clone();
            self.physical.emitters.push(emitter);
        }
        name
    }

    fn filter(&mut self, mut filter: Filter) -> String {
        let key = format!("{:?}", (&filter.name, &filter.measurements));
        let (name, new) = self.intern(key, &filter.name.clone(), "Filter", 1);
        if new {
            filter.name = name.clone();
            self.physical.filters.push(filter);
        }
        name
    }

    fn path(
        &mut self,
        index: usize,
        mode: &FixtureMode,
        path: &HeadOpticalPath,
        heads: &HashMap<Uuid, String>,
    ) {
        let geometry = geometry(path.head_id, heads);
        let node = format!("{}.{geometry}", mode.name);
        if !path.measurements.is_empty() {
            self.report(
                &node,
                format!(
                    "{} whole-path colour measurement(s) have no GDTF representation and are not exported.",
                    path.measurements.len()
                ),
            );
        }
        let mut linked = HashSet::new();
        let mut exported = false;
        match &path.source {
            OpticalSource::Unknown => {}
            OpticalSource::Fixed {
                xyz,
                spectrum,
                provenance,
            } => {
                self.provenance(&node, provenance);
                exported |= self.fixed(index, mode, path, heads, (*xyz, spectrum), &node);
            }
            OpticalSource::Additive { emitters } => {
                for emitter in emitters {
                    if self.additive(index, mode, emitter, &geometry, heads) {
                        linked.insert(emitter.binding.channel_id);
                        exported = true;
                    }
                }
            }
        }
        for filter in &path.filters {
            if self.optical_filter(index, mode, filter, &geometry, heads) {
                linked.insert(filter.binding.channel_id);
                exported = true;
            }
        }
        for control in &path.controls {
            let Some(channel) = mode.channels.iter().find(|channel| channel.id == *control) else {
                continue;
            };
            let owned = self::geometry(channel.head_id, heads) == geometry && is_color(channel);
            if !linked.contains(control) && !owned {
                self.report(
                    &node,
                    format!(
                        "Colour control {} is neither linked to an emitter or filter nor a Color channel of this geometry; GDTF cannot record that ownership.",
                        channel.attribute.0
                    ),
                );
            }
        }
        if !exported {
            self.report(
                &node,
                "This optical path has no emitter, filter or source data GDTF can carry; a GDTF import has no physical colour model for it.",
            );
        }
    }

    /// Checks that a binding lands on the geometry a GDTF reader groups into this path.
    fn bound<'a>(
        &mut self,
        mode: &'a FixtureMode,
        binding: NativeColorBinding,
        geometry: &str,
        heads: &HashMap<Uuid, String>,
        node: &str,
    ) -> Option<(&'a FixtureChannel, &'a ChannelFunction)> {
        let (channel, function) = resolve(mode, binding)?;
        if self::geometry(channel.head_id, heads) != geometry {
            self.report(
                node,
                format!(
                    "A control on another head is bound to this path; GDTF links carry no path, so the binding on {} is not exported.",
                    channel.attribute.0
                ),
            );
            return None;
        }
        Some((channel, function))
    }

    fn additive(
        &mut self,
        index: usize,
        mode: &FixtureMode,
        emitter: &OpticalEmitter,
        geometry: &str,
        heads: &HashMap<Uuid, String>,
    ) -> bool {
        let node = format!("{}.{geometry}.{}", mode.name, emitter.name);
        let Some((channel, function)) = self.bound(mode, emitter.binding, geometry, heads, &node)
        else {
            return false;
        };
        let ChannelFunctionBehavior::Continuous {
            physical_min,
            physical_max,
            ..
        } = function.behavior
        else {
            return false;
        };
        if emitter.native_reversed != (physical_min > physical_max) {
            self.report(
                &node,
                "GDTF derives emitter direction from the function's PhysicalFrom/PhysicalTo; the authored reversal differs and is not exported.",
            );
        }
        if emitter.maximum_level != 1.0 || emitter.response_exponent != 1.0 {
            self.report(
                &node,
                "Emitter maximum level and response exponent have no GDTF field; a GDTF import uses 1 and a linear response.",
            );
        }
        self.provenance(&node, &emitter.provenance);
        let color = emitter.xyz.and_then(|xyz| {
            let color = xyy(xyz);
            if color.is_none() {
                self.report(
                    &node,
                    "Visible XYZ with zero luminance cannot be written as CIE xyY; the emitter colour is not exported.",
                );
            }
            color
        });
        let attribute = attribute_key(native_function_attribute(
            channel,
            function.attribute.0.as_ref(),
        ))
        .0;
        let diode_part =
            (emitter.band != derived_band(&attribute)).then(|| band_name(emitter.band).to_owned());
        let measurements = (!emitter.spectrum.is_empty())
            .then(|| Measurement {
                physical: 100.0,
                interpolation: Interpolation::Linear,
                points: spectrum_points(&emitter.spectrum),
            })
            .into_iter()
            .collect();
        let name = self.emitter(Emitter {
            name: emitter.name.clone(),
            color,
            diode_part,
            measurements,
        });
        self.links
            .insert((index, channel.id, function.id), Link::Emitter(name));
        true
    }

    fn optical_filter(
        &mut self,
        index: usize,
        mode: &FixtureMode,
        filter: &OpticalFilter,
        geometry: &str,
        heads: &HashMap<Uuid, String>,
    ) -> bool {
        let node = format!("{}.{geometry}.{}", mode.name, filter.name);
        let Some((channel, function)) = self.bound(mode, filter.binding, geometry, heads, &node)
        else {
            return false;
        };
        self.provenance(&node, &filter.provenance);
        let measurements = match &filter.transmission {
            OpticalTransmission::Unknown => Vec::new(),
            OpticalTransmission::Spectral { samples } => {
                measurements(function, samples).unwrap_or_else(|| {
                    self.report(
                        &node,
                        "Spectral samples that neither cover one raw value nor run to the next sample cannot be expressed as GDTF measurements; the filter transmission is not exported.",
                    );
                    Vec::new()
                })
            }
        };
        let name = self.filter(Filter {
            name: filter.name.clone(),
            measurements,
        });
        if matches!(
            function.behavior,
            ChannelFunctionBehavior::Continuous { .. }
        ) {
            self.links
                .insert((index, channel.id, function.id), Link::Filter(name));
        } else {
            self.slots.entry((index, channel.id)).or_default().push((
                function.dmx_from,
                function.id,
                name,
            ));
        }
        true
    }

    /// One wheel per channel and mode, its slots in raw order; identical wheels are shared.
    fn wheels(&mut self, profile: &FixtureProfile) {
        for ((index, channel_id), mut slots) in std::mem::take(&mut self.slots) {
            let Some(channel) = profile.modes[index]
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
            else {
                continue;
            };
            slots.sort_by_key(|(from, _, _)| *from);
            let wheel_slots = slots
                .iter()
                .map(|(_, _, filter)| Slot {
                    name: filter.clone(),
                    filter: Some(filter.clone()),
                })
                .collect::<Vec<_>>();
            let base = attribute_key(native_function_attribute(
                channel,
                channel.attribute.0.as_ref(),
            ))
            .0;
            let (name, new) = self.intern(format!("{wheel_slots:?}"), &base, "Wheel", 2);
            if new {
                self.physical.wheels.push(Wheel {
                    name: name.clone(),
                    slots: wheel_slots,
                });
            }
            for (slot, (_, function, _)) in slots.iter().enumerate() {
                self.links.insert(
                    (index, channel_id, *function),
                    Link::Slot(name.clone(), slot as u16 + 1),
                );
            }
        }
    }

    fn fixed(
        &mut self,
        index: usize,
        mode: &FixtureMode,
        path: &HeadOpticalPath,
        heads: &HashMap<Uuid, String>,
        (xyz, spectrum): (Option<Xyz>, &[SpectrumSample]),
        node: &str,
    ) -> bool {
        let shared = mode
            .heads
            .iter()
            .any(|head| head.id == path.head_id && head.master_shared);
        let beam = match heads.get(&path.head_id) {
            Some(beam) => beam.clone(),
            None if shared && heads.is_empty() => "Beam".to_owned(),
            None => {
                self.report(
                    node,
                    "A fixed source on a shared head of a multi-beam fixture has no single GDTF beam; it is not exported.",
                );
                return false;
            }
        };
        let color = xyz.and_then(xyy);
        if xyz.is_some() && color.is_none() {
            self.report(
                node,
                "Visible XYZ with zero luminance cannot be written as CIE xyY; the source colour is not exported.",
            );
        }
        let measurements = (!spectrum.is_empty())
            .then(|| Measurement {
                physical: 100.0,
                interpolation: Interpolation::Linear,
                points: spectrum_points(spectrum),
            })
            .into_iter()
            .collect();
        self.fixed.push(FixedSource {
            mode: index,
            geometry: geometry(path.head_id, heads),
            emitter: Emitter {
                name: format!("{beam} source"),
                color,
                diode_part: None,
                measurements,
            },
            beam,
            node: node.to_owned(),
        });
        true
    }

    /// Beam spectra are fixture-wide: link one only when every mode drawing on it agrees.
    fn fixed_sources(&mut self, profile: &FixtureProfile, heads: &HashMap<Uuid, String>) {
        let sources = std::mem::take(&mut self.fixed);
        let beams = sources
            .iter()
            .map(|source| source.beam.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for beam in beams {
            let group = sources
                .iter()
                .filter(|source| source.beam == beam)
                .collect::<Vec<_>>();
            let geometry = &group[0].geometry;
            let drawing = profile
                .modes
                .iter()
                .enumerate()
                .filter(|(_, mode)| {
                    mode.channels
                        .iter()
                        .any(|channel| self::geometry(channel.head_id, heads) == *geometry)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let agrees = group
                .iter()
                .all(|source| source.emitter == group[0].emitter)
                && drawing
                    .iter()
                    .all(|index| group.iter().any(|source| source.mode == *index))
                && group.iter().all(|source| drawing.contains(&source.mode));
            if !agrees {
                for source in &group {
                    self.report(
                        &source.node,
                        "GDTF links a fixed source spectrum to a fixture-wide beam; the modes drawing on this beam disagree, so the source is not exported.",
                    );
                }
                continue;
            }
            let name = self.emitter(group[0].emitter.clone());
            self.physical.beam_spectra.push((beam, name));
        }
    }
}

/// GDTF measurements for spectral filter samples, or `None` when a range cannot be expressed.
///
/// A one-value sample is a `Linear` point; a sample that runs to the next sample (or the end of the
/// function) is a `Step`. `Physical` is the raw position within the function as 0–100.
fn measurements(
    function: &ChannelFunction,
    samples: &[FilterSpectrum],
) -> Option<Vec<Measurement>> {
    let (from, to) = (function.dmx_from, function.dmx_to);
    let percent = |raw: u32| {
        if to == from {
            0.0
        } else {
            100.0 * f64::from(raw - from) / f64::from(to - from)
        }
    };
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let end = samples
                .get(index + 1)
                .map_or(Some(to), |next| next.raw_from.checked_sub(1));
            let interpolation = if sample.raw_from == sample.raw_to {
                Interpolation::Linear
            } else if Some(sample.raw_to) == end {
                Interpolation::Step
            } else {
                return None;
            };
            Some(Measurement {
                physical: percent(sample.raw_from),
                interpolation,
                points: spectrum_points(&sample.spectrum),
            })
        })
        .collect()
}
