//! GDTF `Wheels` and `PhysicalDescriptions`: emitters, filters and colour-wheel slots.
//!
//! Values are written as shortest round-tripping decimals so a reader recovers the exact numbers
//! the writer was given. What a model cannot carry is reported by the caller, never invented here.

use super::{escape, gdtf_name};
use crate::{FixtureChannel, OpticalEmitterBand};
use light_core::Xyz;

/// CIE xyY written for a black source; any chromaticity reproduces XYZ 0 at luminance 0.
const BLACK_XYY: [f64; 3] = [0.3127, 0.329, 0.0];

#[cfg(test)]
mod optical_scale_tests {
    use super::*;

    #[test]
    fn optical_white_hundred_is_runtime_reference_white_one() {
        let white = xyz([0.3127, 0.329, 100.0]).unwrap();
        assert_eq!(white.y, 1.0);
        let exported = xyy(white).unwrap();
        assert_eq!(exported[2], 100.0);
        assert!((exported[0] - 0.3127).abs() < 1e-7);
    }
}

/// Optical CIE xyY: GDTF white has Y=100; runtime reference white has Y=1.
/// This is not the GDTF ColorSpace representation, whose white already has Y=1.
pub(crate) fn xyy(xyz: Xyz) -> Option<[f64; 3]> {
    let [x, y, z] = [xyz.x, xyz.y, xyz.z].map(f64::from);
    let sum = x + y + z;
    if sum <= 0.0 {
        return Some(BLACK_XYY);
    }
    (y > 0.0).then(|| [x / sum, y / sum, y * 100.0])
}

/// XYZ of a CIE xyY triple; `None` when a luminous colour has no chromaticity `y`.
pub(crate) fn xyz(values: [f64; 3]) -> Option<Xyz> {
    xyz_with_optical_scale(values, 0.01)
}

/// Legacy scale is used only to verify exact old imported models against retained source.
pub(crate) fn xyz_with_optical_scale([x, y, luminance]: [f64; 3], scale: f64) -> Option<Xyz> {
    let luminance = luminance * scale;
    if luminance == 0.0 {
        return Some(Xyz {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    }
    (y > 0.0 && luminance > 0.0).then(|| Xyz {
        x: (x * luminance / y) as f32,
        y: luminance as f32,
        z: ((1.0 - x - y) * luminance / y) as f32,
    })
}

/// The emitter band a GDTF reader derives from the function's semantic attribute alone.
pub(crate) fn derived_band(attribute: &str) -> OpticalEmitterBand {
    if attribute == "ColorAdd_UV" {
        OpticalEmitterBand::Ultraviolet
    } else {
        OpticalEmitterBand::Visible
    }
}

const BANDS: [(OpticalEmitterBand, &str); 4] = [
    (OpticalEmitterBand::Visible, "Visible"),
    (OpticalEmitterBand::Ultraviolet, "Ultraviolet"),
    (OpticalEmitterBand::Infrared, "Infrared"),
    (OpticalEmitterBand::OtherNonVisible, "Other non-visible"),
];

/// `DiodePart` text recording a band the attribute alone does not imply.
pub(crate) fn band_name(band: OpticalEmitterBand) -> &'static str {
    BANDS
        .iter()
        .find(|(known, _)| *known == band)
        .map_or("Visible", |(_, name)| name)
}

pub(crate) fn band_from_name(name: &str) -> Option<OpticalEmitterBand> {
    BANDS
        .iter()
        .find(|(_, known)| *known == name)
        .map(|(band, _)| *band)
}

/// A channel the physical colour model must own: any Color attribute or function.
pub(crate) fn is_color(channel: &FixtureChannel) -> bool {
    let color = |key: &str| key == "color" || key.starts_with("color.");
    color(&channel.fixture_attribute.0)
        || color(&channel.attribute.0)
        || channel
            .functions
            .iter()
            .any(|function| color(&function.attribute.0))
}

/// How a measurement continues towards the next one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interpolation {
    Linear,
    Step,
}

/// One spectral measurement. `physical` is the channel function's physical value, 0–100.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub physical: f64,
    pub interpolation: Interpolation,
    /// `(wavelength nm, energy)` points in ascending wavelength.
    pub points: Vec<(f32, f32)>,
}

/// A light source referenced by `Emitter=` or by a beam's `EmitterSpectrum`.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    pub name: String,
    /// CIE 1931 `x, y, Y`.
    pub color: Option<[f64; 3]>,
    pub diode_part: Option<String>,
    pub measurements: Vec<Measurement>,
}

/// A colour filter referenced by `Filter=` or by a wheel slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub name: String,
    pub measurements: Vec<Measurement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    pub filter: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wheel {
    pub name: String,
    pub slots: Vec<Slot>,
}

/// Nominal optics a beam geometry carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeamOptics {
    pub beam_angle: Option<f32>,
    pub color_temperature: Option<f32>,
    pub luminous_flux: Option<f32>,
}

/// Everything below `Wheels` and `PhysicalDescriptions`, plus the beam links into it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicalDescriptions {
    pub emitters: Vec<Emitter>,
    pub filters: Vec<Filter>,
    pub wheels: Vec<Wheel>,
    /// `(beam name, emitter name)`: the white-light spectrum of a fixed source.
    pub beam_spectra: Vec<(String, String)>,
    /// Applied to every beam geometry.
    pub beam_optics: BeamOptics,
}

impl PhysicalDescriptions {
    /// Extra attributes for one `<Beam>` element.
    pub(super) fn beam_attributes(&self, beam: &str) -> String {
        let mut attributes = String::new();
        let optics = &self.beam_optics;
        for (key, value) in [
            ("BeamAngle", optics.beam_angle),
            ("ColorTemperature", optics.color_temperature),
            ("LuminousFlux", optics.luminous_flux),
        ] {
            if let Some(value) = value {
                attributes.push_str(&format!(" {key}=\"{value}\""));
            }
        }
        if let Some((_, emitter)) = self.beam_spectra.iter().find(|(name, _)| name == beam) {
            attributes.push_str(&format!(
                " EmitterSpectrum=\"{}\"",
                escape(&gdtf_name(emitter))
            ));
        }
        attributes
    }
}

pub(super) fn push_wheels(xml: &mut String, physical: &PhysicalDescriptions) {
    if physical.wheels.is_empty() {
        xml.push_str("    <Wheels/>\n");
        return;
    }
    xml.push_str("    <Wheels>\n");
    for wheel in &physical.wheels {
        xml.push_str(&format!(
            "      <Wheel Name=\"{}\">\n",
            escape(&gdtf_name(&wheel.name))
        ));
        for slot in &wheel.slots {
            let filter = slot.filter.as_ref().map_or_else(String::new, |filter| {
                format!(" Filter=\"{}\"", escape(&gdtf_name(filter)))
            });
            xml.push_str(&format!(
                "        <Slot Name=\"{}\"{filter}/>\n",
                escape(&gdtf_name(&slot.name))
            ));
        }
        xml.push_str("      </Wheel>\n");
    }
    xml.push_str("    </Wheels>\n");
}

pub(super) fn push_physical_descriptions(xml: &mut String, physical: &PhysicalDescriptions) {
    if physical.emitters.is_empty() && physical.filters.is_empty() {
        xml.push_str("    <PhysicalDescriptions/>\n");
        return;
    }
    xml.push_str("    <PhysicalDescriptions>\n      <Emitters>\n");
    for emitter in &physical.emitters {
        let color = emitter.color.map_or_else(String::new, |[x, y, luminance]| {
            format!(" Color=\"{x},{y},{luminance}\"")
        });
        let diode = emitter
            .diode_part
            .as_ref()
            .map_or_else(String::new, |part| {
                format!(" DiodePart=\"{}\"", escape(part))
            });
        push_measured(
            xml,
            "Emitter",
            &format!(
                "Name=\"{}\"{color}{diode}",
                escape(&gdtf_name(&emitter.name))
            ),
            &emitter.measurements,
        );
    }
    xml.push_str("      </Emitters>\n      <Filters>\n");
    for filter in &physical.filters {
        push_measured(
            xml,
            "Filter",
            &format!("Name=\"{}\"", escape(&gdtf_name(&filter.name))),
            &filter.measurements,
        );
    }
    xml.push_str("      </Filters>\n    </PhysicalDescriptions>\n");
}

fn push_measured(xml: &mut String, element: &str, attributes: &str, measurements: &[Measurement]) {
    if measurements.is_empty() {
        xml.push_str(&format!("        <{element} {attributes}/>\n"));
        return;
    }
    xml.push_str(&format!("        <{element} {attributes}>\n"));
    for measurement in measurements {
        let interpolation = match measurement.interpolation {
            Interpolation::Linear => "Linear",
            Interpolation::Step => "Step",
        };
        xml.push_str(&format!(
            "          <Measurement Physical=\"{}\" InterpolationTo=\"{interpolation}\">\n",
            measurement.physical
        ));
        for (wavelength, energy) in &measurement.points {
            xml.push_str(&format!(
                "            <MeasurementPoint WaveLength=\"{wavelength}\" Energy=\"{energy}\"/>\n"
            ));
        }
        xml.push_str("          </Measurement>\n");
    }
    xml.push_str(&format!("        </{element}>\n"));
}
