use super::spectrum::{self, SAMPLES, Spectrum};
use crate::{
    ColorRecipeMeasurement, FixtureMode, FixtureProfile, HeadOpticalPath,
    InstalledColorCalibration, InstalledColorCalibrationStatus, NativeColorBinding,
    OpticalEmitterBand, OpticalProvenance, OpticalSource, OpticalTransmission, PhysicalDataQuality,
    ProfileError,
};
use light_core::Xyz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

mod fitting;
pub use fitting::*;

/// Passive prediction status, not operator errors or notification requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorForwardFlags(pub u16);
impl ColorForwardFlags {
    pub const UNKNOWN_SOURCE: Self = Self(1);
    pub const UNKNOWN_EMITTER: Self = Self(2);
    pub const UNKNOWN_FILTER: Self = Self(4);
    pub const FILTER_SAMPLE_GAP: Self = Self(8);
    pub const SPECTRAL_COVERAGE: Self = Self(16);
    pub const UNMODELED_CONTROL: Self = Self(32);
    pub const STALE_CALIBRATION: Self = Self(64);
    pub const NATIVE_OVER_LIMIT: Self = Self(128);
    pub const INCONSISTENT_SOURCE: Self = Self(256);
    pub const NUMERIC_OVERFLOW: Self = Self(512);
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    fn add(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForwardUvDrive {
    pub emitter_id: Uuid,
    /// Actual normalized native drive, before external shutter/dimmer gating; not UV watts.
    pub drive: f64,
    pub above_limit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForwardPortableUv {
    /// Common normalized native drive, independent of visible output and optical response.
    /// This is a control estimate, not UV radiant power; above-limit drives are preserved.
    pub amount: f64,
    pub quality: PhysicalDataQuality,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorForwardResult {
    pub head_id: Uuid,
    /// Only modeled POST-filter contributions. Incomplete does not mean physically black.
    pub known_xyz: Xyz,
    pub visible_complete: bool,
    pub data_quality: PhysicalDataQuality,
    pub flags: ColorForwardFlags,
    /// None means unknown or unequal independent UV banks. Some(0) means known zero.
    /// Visible measurements cannot establish a missing native UV control model.
    pub portable_uv: Option<ForwardPortableUv>,
    pub uv_drive_max: f64,
    /// Reused storage allocated by create_output, never resized during evaluation.
    pub uv_emitters: Box<[ForwardUvDrive]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorForwardInputError {
    ChannelCount,
    RawOutOfRange,
    OutputLayout,
}

#[derive(Clone, Debug)]
pub struct CompiledColorForward {
    raw_maxima: Box<[u32]>,
    paths: Box<[Path]>,
    stale_calibration: bool,
    installation_unknown: ColorForwardFlags,
}
#[derive(Clone, Debug)]
struct Binding {
    channel: usize,
    from: u32,
    to: u32,
}
impl Binding {
    fn active(&self, raw: &[u32]) -> bool {
        (self.from..=self.to).contains(&raw[self.channel])
    }
    fn fraction(&self, raw: &[u32], reversed: bool) -> f64 {
        let (numerator, denominator) = self.drive_ratio(raw, reversed);
        f64::from(numerator) / f64::from(denominator)
    }
    fn drive_ratio(&self, raw: &[u32], reversed: bool) -> (u32, u32) {
        let numerator = if reversed {
            self.to - raw[self.channel]
        } else {
            raw[self.channel] - self.from
        };
        (numerator, self.to - self.from)
    }
}
#[derive(Clone, Debug)]
struct Appearance {
    xyz: Option<[f64; 3]>,
    spectrum: Option<Box<Spectrum>>,
    quality: PhysicalDataQuality,
    incomplete_spectrum: bool,
    inconsistent: bool,
}
impl Appearance {
    fn compile(
        xyz: Option<Xyz>,
        samples: &[crate::SpectrumSample],
        evidence: &OpticalProvenance,
    ) -> Self {
        let spectral = spectrum::resample(samples);
        let authored = xyz.map(|v| [f64::from(v.x), f64::from(v.y), f64::from(v.z)]);
        let integrated = spectral.as_ref().map(|s| spectrum::integrate(s, None));
        let inconsistent = authored.zip(integrated).is_some_and(|(a, b)| {
            let scale = a.into_iter().chain(b).fold(1e-9_f64, f64::max);
            (0..3).any(|i| (a[i] - b[i]).abs() > scale * 0.01)
        });
        Self {
            xyz: integrated.or(authored),
            spectrum: spectral,
            quality: evidence.quality,
            incomplete_spectrum: !samples.is_empty() && integrated.is_none(),
            inconsistent,
        }
    }
    fn contribution(
        &self,
        amount: f64,
        filters: bool,
        filter_known: bool,
        transmission: &Spectrum,
        flags: &mut ColorForwardFlags,
    ) -> Option<[f64; 3]> {
        if amount == 0.0 {
            return Some([0.0; 3]);
        }
        if self.inconsistent {
            flags.add(ColorForwardFlags::INCONSISTENT_SOURCE);
            return None;
        }
        if self.xyz == Some([0.0; 3]) {
            return Some([0.0; 3]);
        }
        if filters {
            if !filter_known {
                return None;
            }
            let Some(source) = &self.spectrum else {
                flags.add(ColorForwardFlags::SPECTRAL_COVERAGE);
                return None;
            };
            Some(spectrum::integrate(source, Some(transmission)).map(|v| v * amount))
        } else {
            if self.incomplete_spectrum && self.xyz.is_none() {
                flags.add(ColorForwardFlags::SPECTRAL_COVERAGE);
            }
            self.xyz.map(|v| v.map(|v| v * amount))
        }
    }
}
#[derive(Clone, Debug)]
struct Emitter {
    id: Uuid,
    binding: Binding,
    appearance: Appearance,
    reversed: bool,
    maximum: f64,
    exponent: f64,
    gain: f64,
    uv_index: Option<usize>,
}
#[derive(Clone, Debug)]
enum Source {
    Unknown,
    Fixed(Appearance),
    Additive(Box<[Emitter]>),
}
#[derive(Clone, Debug)]
struct FilterSample {
    from: u32,
    to: u32,
    transmission: Option<Box<Spectrum>>,
}
impl FilterSample {
    /// A known unit transmission at every wavelength filters nothing: the path's output is
    /// exactly the unfiltered output, so no source spectrum is needed (a control parked open).
    pub(super) fn identity(&self) -> bool {
        self.transmission
            .as_ref()
            .is_some_and(|t| t.iter().all(|v| *v >= 1.0))
    }
}
#[derive(Clone, Debug)]
struct Filter {
    binding: Binding,
    samples: Option<Box<[FilterSample]>>,
    quality: PhysicalDataQuality,
}
#[derive(Clone, Debug)]
struct Measurement {
    recipe: Box<[(usize, u32)]>,
    xyz: Xyz,
    quality: PhysicalDataQuality,
}
#[derive(Clone, Debug)]
struct Path {
    head: Uuid,
    controls: Box<[usize]>,
    modeled: Box<[Box<[Binding]>]>,
    source: Source,
    /// Alternative UV functions on one native channel form one independently driven bank.
    /// Entries index Source::Additive emitters; allocated once during compilation.
    uv_banks: Box<[Box<[usize]>]>,
    filters: Box<[Filter]>,
    measurements: HashMap<u64, Vec<Measurement>>,
}

fn invalid(message: &str) -> ProfileError {
    ProfileError::Invalid(format!("Color forward model: {message}"))
}
fn binding(mode: &FixtureMode, ids: &HashMap<Uuid, usize>, native: NativeColorBinding) -> Binding {
    let channel = ids[&native.channel_id];
    let function = mode.channels[channel]
        .functions
        .iter()
        .find(|f| f.id == native.function_id)
        .unwrap();
    Binding {
        channel,
        from: function.dmx_from,
        to: function.dmx_to,
    }
}
pub(super) fn quality_min(a: PhysicalDataQuality, b: PhysicalDataQuality) -> PhysicalDataQuality {
    let rank = |q| match q {
        PhysicalDataQuality::Unknown => 0,
        PhysicalDataQuality::Estimated => 1,
        PhysicalDataQuality::Manufacturer => 2,
        PhysicalDataQuality::Measured => 3,
    };
    if rank(a) < rank(b) { a } else { b }
}
fn recipe_hash(values: impl Iterator<Item = (usize, u32)>) -> u64 {
    values.fold(0xcbf29ce484222325, |hash, (channel, raw)| {
        (hash ^ channel as u64)
            .wrapping_mul(0x100000001b3)
            .wrapping_add(u64::from(raw))
            .wrapping_mul(0x100000001b3)
    })
}

impl CompiledColorForward {
    /// Called for a validated profile when configuration changes, not once per output frame.
    pub fn compile(
        profile: &FixtureProfile,
        mode_id: Uuid,
        installed: Option<&InstalledColorCalibration>,
    ) -> Result<Option<Self>, ProfileError> {
        Self::compile_with_context(profile, mode_id, installed, None)
    }
    /// Context must originate from the immutable source profile, before runtime compaction.
    pub fn compile_with_context(
        profile: &FixtureProfile,
        mode_id: Uuid,
        installed: Option<&InstalledColorCalibration>,
        context: Option<&crate::ColorCalibrationContext>,
    ) -> Result<Option<Self>, ProfileError> {
        if let Some(context) = context {
            context
                .validate_runtime_profile(profile, mode_id)
                .map_err(|e| invalid(&e))?;
        }
        let mode = profile
            .mode(mode_id)
            .ok_or_else(|| invalid("missing mode"))?;
        let Some(model) = &mode.color_physical else {
            return Ok(None);
        };
        // Bound compiled storage before resampling. This is a model-support limit, not a show edit.
        let work = model.paths.iter().fold(0usize, |sum, p| {
            let emitters = match &p.source {
                OpticalSource::Additive { emitters } => emitters.len(),
                _ => 1,
            };
            sum.saturating_add(p.controls.len())
                .saturating_add(emitters)
                .saturating_add(p.measurements.iter().map(|m| m.recipe.len()).sum::<usize>())
                .saturating_add(
                    p.filters
                        .iter()
                        .map(|f| match &f.transmission {
                            OpticalTransmission::Unknown => 1,
                            OpticalTransmission::Spectral { samples } => samples.len(),
                        })
                        .sum::<usize>(),
                )
        });
        if mode.channels.len() > 4096 || work > 16_384 {
            return Err(invalid("model exceeds bounded forward capacity"));
        }
        mode.validate_color_physical()?;
        super::validate_native_domains(mode)?;
        let stale_calibration = installed.is_some_and(|c| {
            if let Some(context) = context {
                c.validate_for_context(context).is_err()
            } else {
                !matches!(
                    c.status(profile, mode_id),
                    InstalledColorCalibrationStatus::Current
                )
            }
        });
        let current = installed.filter(|_| !stale_calibration);
        let installed_work = current.map_or(0, |c| {
            c.paths.iter().fold(0usize, |sum, p| {
                p.measurements
                    .iter()
                    .fold(sum.saturating_add(p.emitters.len()), |sum, m| {
                        sum.saturating_add(m.recipe.len())
                    })
            })
        });
        if work.saturating_add(installed_work) > 16_384 {
            return Err(invalid(
                "model and calibration exceed bounded forward capacity",
            ));
        }
        let ids = mode
            .channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect::<HashMap<_, _>>();
        let paths = model
            .paths
            .iter()
            .map(|path| Path::compile(mode, path, current, &ids))
            .collect::<Result<_, _>>()?;
        spectrum::observer(); // Initialize the pinned observer during compilation only.
        Ok(Some(Self {
            raw_maxima: mode
                .channels
                .iter()
                .map(|c| c.resolution.max_raw())
                .collect(),
            paths,
            stale_calibration,
            installation_unknown: ColorForwardFlags::default(),
        }))
    }
    /// Legacy source labels/CCT and gel swatches cannot reconstruct a changed optical path.
    /// Keep native activity but do not present the old source or unfiltered XYZ as achieved.
    pub fn with_installed_appearance(
        mut self,
        appearance: &crate::InstalledFixtureAppearance,
    ) -> Self {
        if appearance.light_source != crate::InstalledLightSource::ProfileDefault
            || appearance.color_temperature_kelvin.is_some()
        {
            self.installation_unknown
                .add(ColorForwardFlags::UNKNOWN_SOURCE);
        }
        if appearance.gel != crate::GelAssignment::OpenWhite {
            self.installation_unknown
                .add(ColorForwardFlags::UNKNOWN_FILTER);
        }
        self
    }
    /// Mode-channel indices of one head's Color path in fitting order: exactly the complete
    /// Color-owned footprint `CompiledColorFitting::controls` reports for that head, without
    /// building its fitting tables. `None` for a head without a Color path.
    pub fn head_controls(&self, head: Uuid) -> Option<&[usize]> {
        self.paths
            .iter()
            .find(|path| path.head == head)
            .map(|path| path.controls.as_ref())
    }
    /// Missing unrelated mode channels must not hide a head whose optical inputs are known.
    pub fn inputs_available(&self, path: usize, available: &[bool]) -> bool {
        self.paths
            .get(path)
            .is_some_and(|p| p.controls.iter().all(|&i| available.get(i) == Some(&true)))
    }
    /// A Direct recipe owns one head. Keep its original native indices but avoid evaluating
    /// every unrelated cell of a multi-head profile for each individual recipe.
    pub(crate) fn for_head(&self, head: Uuid) -> Option<Self> {
        let path = self.paths.iter().find(|p| p.head == head)?.clone();
        Some(Self {
            raw_maxima: self.raw_maxima.clone(),
            paths: vec![path].into_boxed_slice(),
            stale_calibration: self.stale_calibration,
            installation_unknown: self.installation_unknown,
        })
    }
    pub fn create_output(&self) -> Vec<ColorForwardResult> {
        self.paths
            .iter()
            .map(|p| {
                let uv_emitters: Box<[ForwardUvDrive]> = match &p.source {
                    Source::Additive(emitters) => emitters
                        .iter()
                        .filter(|e| e.uv_index.is_some())
                        .map(|e| ForwardUvDrive {
                            emitter_id: e.id,
                            drive: 0.0,
                            above_limit: false,
                        })
                        .collect(),
                    _ => Box::new([]),
                };
                ColorForwardResult {
                    head_id: p.head,
                    known_xyz: Xyz {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    visible_complete: false,
                    data_quality: PhysicalDataQuality::Unknown,
                    flags: ColorForwardFlags::default(),
                    portable_uv: None,
                    uv_drive_max: 0.0,
                    uv_emitters,
                }
            })
            .collect()
    }
    /// Reads FINAL native values. No canonical transform, channel inversion, intent fallback,
    /// drive clipping, profile lookup, hashing of source assets or allocation occurs here.
    pub fn evaluate(
        &self,
        raw: &[u32],
        output: &mut [ColorForwardResult],
    ) -> Result<(), ColorForwardInputError> {
        if raw.len() != self.raw_maxima.len() {
            return Err(ColorForwardInputError::ChannelCount);
        }
        if raw.iter().zip(&self.raw_maxima).any(|(raw, max)| raw > max) {
            return Err(ColorForwardInputError::RawOutOfRange);
        }
        if output.len() != self.paths.len()
            || output.iter().zip(&self.paths).any(|(out, path)| {
                let expected = match &path.source {
                    Source::Additive(e) => {
                        let emitters = e.iter().filter(|e| e.uv_index.is_some());
                        if emitters
                            .clone()
                            .zip(out.uv_emitters.iter())
                            .any(|(a, b)| a.id != b.emitter_id)
                        {
                            return true;
                        }
                        emitters.count()
                    }
                    _ => 0,
                };
                out.head_id != path.head || out.uv_emitters.len() != expected
            })
        {
            return Err(ColorForwardInputError::OutputLayout);
        }
        for (path, out) in self.paths.iter().zip(output) {
            path.evaluate(raw, self.stale_calibration, out);
            if self.installation_unknown.0 != 0 {
                out.flags.add(self.installation_unknown);
                out.known_xyz = Xyz {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                };
                out.visible_complete = false;
                out.data_quality = PhysicalDataQuality::Unknown;
            }
        }
        Ok(())
    }
}

impl Path {
    fn compile(
        mode: &FixtureMode,
        path: &HeadOpticalPath,
        installed: Option<&InstalledColorCalibration>,
        ids: &HashMap<Uuid, usize>,
    ) -> Result<Self, ProfileError> {
        let calibration = installed.and_then(|c| {
            c.paths
                .iter()
                .find(|p| p.source_identity.path_id == path.id)
        });
        let mut modeled = Vec::new();
        let mut uv_count = 0;
        let source = match &path.source {
            OpticalSource::Unknown => Source::Unknown,
            OpticalSource::Fixed {
                xyz,
                spectrum,
                provenance,
            } => Source::Fixed(Appearance::compile(*xyz, spectrum, provenance)),
            OpticalSource::Additive { emitters } => Source::Additive(
                emitters
                    .iter()
                    .map(|e| {
                        let native = binding(mode, ids, e.binding);
                        modeled.push(native.clone());
                        let correction = calibration
                            .and_then(|c| c.emitters.iter().find(|g| g.emitter_id == e.id));
                        let mut appearance = Appearance::compile(e.xyz, &e.spectrum, &e.provenance);
                        if let Some(gain) = correction {
                            appearance.quality =
                                quality_min(appearance.quality, gain.provenance.quality);
                        }
                        let uv_index = (e.band == OpticalEmitterBand::Ultraviolet).then(|| {
                            let i = uv_count;
                            uv_count += 1;
                            i
                        });
                        Emitter {
                            id: e.id,
                            binding: native,
                            appearance,
                            reversed: e.native_reversed,
                            maximum: f64::from(e.maximum_level),
                            exponent: f64::from(e.response_exponent),
                            gain: correction.map_or(1.0, |g| f64::from(g.output_gain)),
                            uv_index,
                        }
                    })
                    .collect(),
            ),
        };
        let filters = path
            .filters
            .iter()
            .map(|f| {
                let native = binding(mode, ids, f.binding);
                modeled.push(native.clone());
                Filter {
                    binding: native,
                    quality: f.provenance.quality,
                    samples: match &f.transmission {
                        OpticalTransmission::Unknown => None,
                        OpticalTransmission::Spectral { samples } => Some(
                            samples
                                .iter()
                                .map(|s| FilterSample {
                                    from: s.raw_from,
                                    to: s.raw_to,
                                    transmission: spectrum::resample(&s.spectrum),
                                })
                                .collect(),
                        ),
                    },
                }
            })
            .collect();
        let mut controls = path.controls.iter().map(|id| ids[id]).collect::<Vec<_>>();
        controls.sort_unstable();
        let uv_banks: Box<[Box<[usize]>]> = match &source {
            Source::Additive(emitters) => {
                let mut banks = std::collections::BTreeMap::<usize, Vec<usize>>::new();
                for (index, emitter) in emitters.iter().enumerate() {
                    if emitter.uv_index.is_some() {
                        banks
                            .entry(emitter.binding.channel)
                            .or_default()
                            .push(index);
                    }
                }
                banks.into_values().map(Vec::into_boxed_slice).collect()
            }
            _ => Box::new([]),
        };
        let mut result = Self {
            head: path.head_id,
            modeled: {
                let mut by_channel: HashMap<usize, Vec<Binding>> = HashMap::new();
                for b in modeled {
                    by_channel.entry(b.channel).or_default().push(b);
                }
                controls
                    .iter()
                    .map(|c| by_channel.remove(c).unwrap_or_default().into_boxed_slice())
                    .collect()
            },
            controls: controls.into_boxed_slice(),
            source,
            uv_banks,
            filters,
            measurements: HashMap::new(),
        };
        // Changed emitter gains cannot be applied to a whole-path profile observation without
        // decomposing that measurement. Current installed whole-path observations take precedence.
        if !calibration.is_some_and(|c| c.emitters.iter().any(|g| g.output_gain != 1.0)) {
            for m in &path.measurements {
                result.add_measurement(m, ids, false)?;
            }
        }
        if let Some(c) = calibration {
            for m in &c.measurements {
                result.add_measurement(m, ids, true)?;
            }
        }
        Ok(result)
    }
    fn add_measurement(
        &mut self,
        m: &ColorRecipeMeasurement,
        ids: &HashMap<Uuid, usize>,
        replace: bool,
    ) -> Result<(), ProfileError> {
        let mut recipe = m
            .recipe
            .iter()
            .map(|v| (ids[&v.channel_id], v.raw))
            .collect::<Vec<_>>();
        recipe.sort_unstable();
        let hash = recipe_hash(recipe.iter().copied());
        let bucket = self.measurements.entry(hash).or_default();
        if let Some(previous) = bucket
            .iter_mut()
            .find(|p| p.recipe.as_ref() == recipe.as_slice())
        {
            if !replace && (previous.xyz != m.xyz || previous.quality != m.provenance.quality) {
                return Err(invalid("conflicting complete-recipe measurements"));
            }
            if replace {
                previous.xyz = m.xyz;
                previous.quality = m.provenance.quality;
            }
        } else {
            bucket.push(Measurement {
                recipe: recipe.into_boxed_slice(),
                xyz: m.xyz,
                quality: m.provenance.quality,
            });
        }
        Ok(())
    }
    fn portable_uv(&self, raw: &[u32], unmodeled: bool) -> Option<ForwardPortableUv> {
        if unmodeled || matches!(&self.source, Source::Unknown) {
            return None;
        }
        let mut common: Option<(u32, u32)> = None;
        if let Source::Additive(emitters) = &self.source {
            for bank in &self.uv_banks {
                // A modeled non-UV alternative on this channel explicitly leaves this bank off.
                // An unmodeled alternative was rejected above, so absence cannot invent zero.
                let drive = bank
                    .iter()
                    .map(|&index| &emitters[index])
                    .find(|emitter| emitter.binding.active(raw))
                    .map_or((0, 1), |emitter| {
                        emitter.binding.drive_ratio(raw, emitter.reversed)
                    });
                if let Some(previous) = common {
                    // Exact comparison avoids both U32 low-byte loss and arbitrary tolerances
                    // that would silently aggregate independently controlled unequal emitters.
                    if u64::from(previous.0) * u64::from(drive.1)
                        != u64::from(drive.0) * u64::from(previous.1)
                    {
                        return None;
                    }
                } else {
                    common = Some(drive);
                }
            }
        }
        let (numerator, denominator) = common.unwrap_or((0, 1));
        Some(ForwardPortableUv {
            amount: f64::from(numerator) / f64::from(denominator),
            quality: PhysicalDataQuality::Estimated,
        })
    }
    /// Combined transmission of the active filters: whether any filters, whether all are known,
    /// and their data quality. A unit transmission (a control parked open) filters nothing.
    fn filtered(
        &self,
        raw: &[u32],
        flags: &mut ColorForwardFlags,
    ) -> (Spectrum, bool, bool, PhysicalDataQuality) {
        let mut transmission = [1.0; SAMPLES];
        let mut active_filters = false;
        let mut filter_known = true;
        let mut quality = PhysicalDataQuality::Measured;
        for f in self.filters.iter().filter(|f| f.binding.active(raw)) {
            quality = quality_min(quality, f.quality);
            let sample = f.samples.as_ref().map(|samples| {
                samples
                    .iter()
                    .find(|s| (s.from..=s.to).contains(&raw[f.binding.channel]))
            });
            if sample.flatten().is_some_and(FilterSample::identity) {
                continue;
            }
            active_filters = true;
            match sample {
                None => {
                    filter_known = false;
                    flags.add(ColorForwardFlags::UNKNOWN_FILTER);
                }
                Some(sample) => match sample {
                    None => {
                        filter_known = false;
                        flags.add(ColorForwardFlags::FILTER_SAMPLE_GAP);
                    }
                    Some(sample) => match &sample.transmission {
                        None => {
                            filter_known = false;
                            flags.add(ColorForwardFlags::SPECTRAL_COVERAGE);
                        }
                        Some(spectrum) => {
                            for i in 0..SAMPLES {
                                transmission[i] *= spectrum[i];
                            }
                        }
                    },
                },
            }
        }
        (transmission, active_filters, filter_known, quality)
    }

    fn evaluate(&self, raw: &[u32], stale: bool, out: &mut ColorForwardResult) {
        let mut flags = ColorForwardFlags::default();
        if stale {
            flags.add(ColorForwardFlags::STALE_CALIBRATION);
        }
        let unmodeled = self
            .modeled
            .iter()
            .any(|bindings| !bindings.iter().any(|b| b.active(raw)));
        if unmodeled {
            flags.add(ColorForwardFlags::UNMODELED_CONTROL);
        }
        // Keep native-control completeness separate from visible appearance. A complete XYZ
        // observation below may clear optical flags, but cannot infer an unknown UV amount.
        out.portable_uv = self.portable_uv(raw, unmodeled);
        let mut complete = !unmodeled;
        let (transmission, active_filters, filter_known, mut quality) =
            self.filtered(raw, &mut flags);
        let mut xyz = [0.0; 3];
        out.uv_drive_max = 0.0;
        for uv in &mut out.uv_emitters {
            uv.drive = 0.0;
            uv.above_limit = false;
        }
        match &self.source {
            Source::Unknown => {
                complete = false;
                flags.add(ColorForwardFlags::UNKNOWN_SOURCE);
                quality = PhysicalDataQuality::Unknown;
            }
            Source::Fixed(a) => {
                quality = quality_min(quality, a.quality);
                if let Some(value) =
                    a.contribution(1.0, active_filters, filter_known, &transmission, &mut flags)
                {
                    xyz = value;
                } else {
                    complete = false;
                    flags.add(ColorForwardFlags::UNKNOWN_SOURCE);
                }
            }
            Source::Additive(emitters) => {
                let mut any_active = false;
                for e in emitters.iter().filter(|e| e.binding.active(raw)) {
                    let drive = e.binding.fraction(raw, e.reversed);
                    let above_limit = drive > e.maximum;
                    if above_limit {
                        flags.add(ColorForwardFlags::NATIVE_OVER_LIMIT);
                    }
                    if let Some(i) = e.uv_index {
                        out.uv_emitters[i] = ForwardUvDrive {
                            emitter_id: e.id,
                            drive,
                            above_limit,
                        };
                        out.uv_drive_max = out.uv_drive_max.max(drive);
                    }
                    let amount = drive.powf(e.exponent) * e.gain;
                    if amount > 0.0 {
                        quality = quality_min(quality, e.appearance.quality);
                        any_active = true;
                    }
                    if let Some(value) = e.appearance.contribution(
                        amount,
                        active_filters,
                        filter_known,
                        &transmission,
                        &mut flags,
                    ) {
                        for i in 0..3 {
                            xyz[i] += value[i];
                        }
                    } else {
                        complete = false;
                        flags.add(ColorForwardFlags::UNKNOWN_EMITTER);
                    }
                }
                if !any_active {
                    quality = quality_min(quality, PhysicalDataQuality::Estimated);
                }
            }
        }
        // An unknown control role can alter the entire path (CTC, macro or a wheel gap).
        // Only explicitly modeled additive emitters may retain independent known contributions.
        if unmodeled {
            xyz = [0.0; 3];
        }
        let hash = recipe_hash(self.controls.iter().map(|c| (*c, raw[*c])));
        if let Some(measurement) = self.measurements.get(&hash).and_then(|bucket| {
            bucket
                .iter()
                .find(|m| m.recipe.iter().all(|(i, v)| raw[*i] == *v))
        }) {
            xyz = [
                f64::from(measurement.xyz.x),
                f64::from(measurement.xyz.y),
                f64::from(measurement.xyz.z),
            ];
            complete = true;
            quality = measurement.quality;
            flags.0 &=
                ColorForwardFlags::STALE_CALIBRATION.0 | ColorForwardFlags::NATIVE_OVER_LIMIT.0;
        }
        if xyz
            .iter()
            .any(|v| !v.is_finite() || *v > f64::from(f32::MAX))
        {
            complete = false;
            flags.add(ColorForwardFlags::NUMERIC_OVERFLOW);
            xyz = [0.0; 3];
        }
        out.known_xyz = Xyz {
            x: xyz[0] as f32,
            y: xyz[1] as f32,
            z: xyz[2] as f32,
        };
        out.visible_complete = complete;
        out.data_quality = quality;
        out.flags = flags;
    }
}
