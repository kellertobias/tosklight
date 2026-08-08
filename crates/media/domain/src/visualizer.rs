//! Generated visualizers.
//!
//! A visualizer is a *source*, not a layer effect: it produces a complete output-sized texture
//! and then receives the same transform, tint, dimmer, mask, and effect processing as a video or
//! a still. That is why nothing here knows about layers.
//!
//! Two identifiers are deliberately separate. A [`VisualizerKind`] carries a stable internal type
//! id that never changes — configuration files and saved shows refer to it. A DMX address is
//! where a desk currently reaches a configured visualizer, and that is assignable, because file
//! `0` and file `255` are blank sentinels and a bank has 254 usable slots for however many
//! configurations an operator makes.

use serde::{Deserialize, Serialize};

use crate::address::{AddressClass, MediaAddress};
use crate::color::Tint;

/// The bank generated visualizers are assigned into by default.
///
/// The whole `220..=255` range belongs to generated sources; shipping one populated bank is a
/// starting point, not a reservation of the rest.
pub const DEFAULT_BANK: u8 = 220;

/// Every visualizer this product ships.
///
/// The discriminants are the stable internal type ids and are grouped by family — spectrum,
/// geometry, particles, light, texture, and dimensional — with gaps left inside each family so a
/// later addition lands beside its relatives instead of at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[repr(u16)]
pub enum VisualizerKind {
    EqualizerBars = 0,
    WaveformOscilloscope = 1,
    CircularSpectrum = 2,
    WaveTerrain = 3,
    PulsingCircles = 10,
    MorphingPolygon = 11,
    MinimalistShapes = 12,
    Kaleidoscope = 13,
    BeatExplosions = 20,
    DancingSwarm = 21,
    Starfield = 22,
    LightningTendrils = 23,
    RadiatingRays = 30,
    StrobeFlash = 31,
    ColorCycling = 32,
    CrossingLines = 33,
    DigitalGlitch = 40,
    CrtScanline = 41,
    RotatingShape = 50,
    FractalMorph = 51,
}

/// Every kind, in the order the shipped catalog assigns them.
pub const ALL_KINDS: [VisualizerKind; 20] = [
    VisualizerKind::EqualizerBars,
    VisualizerKind::WaveformOscilloscope,
    VisualizerKind::CircularSpectrum,
    VisualizerKind::WaveTerrain,
    VisualizerKind::PulsingCircles,
    VisualizerKind::MorphingPolygon,
    VisualizerKind::MinimalistShapes,
    VisualizerKind::Kaleidoscope,
    VisualizerKind::BeatExplosions,
    VisualizerKind::DancingSwarm,
    VisualizerKind::Starfield,
    VisualizerKind::LightningTendrils,
    VisualizerKind::RadiatingRays,
    VisualizerKind::StrobeFlash,
    VisualizerKind::ColorCycling,
    VisualizerKind::CrossingLines,
    VisualizerKind::DigitalGlitch,
    VisualizerKind::CrtScanline,
    VisualizerKind::RotatingShape,
    VisualizerKind::FractalMorph,
];

impl VisualizerKind {
    pub const fn type_id(self) -> u16 {
        self as u16
    }

    pub fn from_type_id(id: u16) -> Option<Self> {
        ALL_KINDS.into_iter().find(|kind| kind.type_id() == id)
    }

    /// The operator-visible name. Stable, because it appears on a cue sheet.
    pub const fn label(self) -> &'static str {
        match self {
            Self::EqualizerBars => "Equalizer Bars",
            Self::WaveformOscilloscope => "Waveform Oscilloscope",
            Self::CircularSpectrum => "Circular Spectrum",
            Self::WaveTerrain => "Wave Terrain",
            Self::PulsingCircles => "Pulsing Circles",
            Self::MorphingPolygon => "Morphing Polygon",
            Self::MinimalistShapes => "Minimalist Shapes",
            Self::Kaleidoscope => "Kaleidoscope",
            Self::BeatExplosions => "Beat Explosions",
            Self::DancingSwarm => "Dancing Swarm",
            Self::Starfield => "Starfield",
            Self::LightningTendrils => "Lightning Tendrils",
            Self::RadiatingRays => "Radiating Rays",
            Self::StrobeFlash => "Strobe Flash",
            Self::ColorCycling => "Color Cycling",
            Self::CrossingLines => "Crossing Lines",
            Self::DigitalGlitch => "Digital Glitch",
            Self::CrtScanline => "CRT Scanline",
            Self::RotatingShape => "Rotating 3D Shape",
            Self::FractalMorph => "Fractal Morph",
        }
    }

    /// Which parameters this kind actually reads.
    ///
    /// The parameter block is one shape for every visualizer so the GPU side stays uniform; this
    /// is what stops an editor offering an operator a control that does nothing.
    pub const fn parameters(self) -> &'static [Parameter] {
        use Parameter::{
            Amount, Count, Curvature, Decay, Filled, Gravity, Iterations, Lifetime, Mirror, Mode,
            Primary, Radius, Reactivity, Secondary, Size, Smoothing, Speed, Thickness, Threshold,
            Wireframe, Zoom,
        };
        match self {
            Self::EqualizerBars => &[Count, Size, Primary, Secondary, Amount, Smoothing, Mirror],
            Self::WaveformOscilloscope => &[Thickness, Amount, Primary, Filled, Smoothing],
            Self::CircularSpectrum => &[Radius, Count, Size, Primary, Mirror],
            Self::WaveTerrain => &[Speed, Size, Zoom, Primary, Wireframe],
            Self::PulsingCircles => &[Count, Size, Primary, Filled, Reactivity, Decay],
            Self::MorphingPolygon => &[Count, Radius, Thickness, Primary, Filled, Amount],
            Self::MinimalistShapes => &[Count, Size, Speed, Primary, Mode],
            Self::Kaleidoscope => &[Count, Speed, Zoom, Primary],
            Self::BeatExplosions => &[Count, Speed, Lifetime, Primary, Gravity],
            Self::DancingSwarm => &[Count, Speed, Radius, Primary, Size],
            Self::Starfield => &[Count, Speed, Primary],
            Self::LightningTendrils => &[Count, Size, Primary, Threshold],
            Self::RadiatingRays => &[Count, Size, Thickness, Speed, Primary],
            Self::StrobeFlash => &[Primary, Threshold, Decay, Mirror],
            Self::ColorCycling => &[Speed, Amount],
            Self::CrossingLines => &[Count, Speed, Primary, Secondary, Mode],
            Self::DigitalGlitch => &[Amount, Speed],
            Self::CrtScanline => &[Count, Curvature],
            Self::RotatingShape => &[Mode, Speed, Size, Primary, Wireframe],
            Self::FractalMorph => &[Zoom, Iterations],
        }
    }
}

/// One named control in the shared parameter block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Parameter {
    Count,
    Size,
    Speed,
    Amount,
    Radius,
    Thickness,
    Reactivity,
    Decay,
    Zoom,
    Iterations,
    Threshold,
    Smoothing,
    Gravity,
    Lifetime,
    Curvature,
    Primary,
    Secondary,
    Mirror,
    Filled,
    Wireframe,
    Mode,
}

/// The one parameter block every visualizer shares.
///
/// A single shape means one uniform layout, one editor, one migration. Which fields a given kind
/// reads is [`VisualizerKind::parameters`]; the rest keep their defaults and are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerParameters {
    /// How many of the repeated thing — bars, rings, vertices, particles, stars, lines.
    pub count: u32,
    /// The size of one of them, as a fraction of the output's smaller dimension.
    pub size: f32,
    /// Animation rate, where `1.0` is the designed speed.
    pub speed: f32,
    /// How strongly the effect is applied, `0.0..=1.0`.
    pub amount: f32,
    pub radius: f32,
    pub thickness: f32,
    /// How much audio moves the effect, where `1.0` is the designed response.
    pub reactivity: f32,
    /// How quickly a triggered response falls back, `0.0..=1.0` per frame.
    pub decay: f32,
    pub zoom: f32,
    pub iterations: u32,
    /// The energy a trigger needs, `0.0..=1.0`.
    pub threshold: f32,
    /// How much a value is carried between frames, `0.0..=1.0`.
    pub smoothing: f32,
    pub gravity: f32,
    /// Seconds a spawned thing survives.
    pub lifetime: f32,
    pub curvature: f32,
    pub primary: Tint,
    pub secondary: Tint,
    pub mirror: bool,
    pub filled: bool,
    pub wireframe: bool,
    /// A kind-specific variant selector — shape type, reaction mode.
    pub mode: u8,
}

impl Default for VisualizerParameters {
    fn default() -> Self {
        Self {
            count: 32,
            size: 0.05,
            speed: 1.0,
            amount: 1.0,
            radius: 0.3,
            thickness: 0.01,
            reactivity: 1.0,
            decay: 0.1,
            zoom: 1.0,
            iterations: 64,
            threshold: 0.5,
            smoothing: 0.5,
            gravity: 0.5,
            lifetime: 2.0,
            curvature: 0.2,
            primary: Tint::new(0.1, 0.84, 0.93),
            secondary: Tint::new(1.0, 0.7, 0.06),
            mirror: false,
            filled: false,
            wireframe: false,
            mode: 0,
        }
    }
}

impl VisualizerParameters {
    /// Brings an out-of-range value back into a range the renderer can use.
    ///
    /// Configuration arrives from a file an operator may have edited, so this clamps rather than
    /// rejects: a visualizer with an absurd bar count should draw something sensible, not refuse
    /// to load in the middle of a show.
    pub fn clamped(self) -> Self {
        Self {
            count: self.count.clamp(1, 512),
            size: clamp_unit(self.size, 0.001, 1.0),
            speed: clamp_unit(self.speed, 0.0, 8.0),
            amount: clamp_unit(self.amount, 0.0, 1.0),
            radius: clamp_unit(self.radius, 0.0, 1.0),
            thickness: clamp_unit(self.thickness, 0.0005, 0.5),
            reactivity: clamp_unit(self.reactivity, 0.0, 8.0),
            decay: clamp_unit(self.decay, 0.0, 1.0),
            zoom: clamp_unit(self.zoom, 0.05, 16.0),
            iterations: self.iterations.clamp(1, 256),
            threshold: clamp_unit(self.threshold, 0.0, 1.0),
            smoothing: clamp_unit(self.smoothing, 0.0, 1.0),
            gravity: clamp_unit(self.gravity, -4.0, 4.0),
            lifetime: clamp_unit(self.lifetime, 0.05, 60.0),
            curvature: clamp_unit(self.curvature, 0.0, 1.0),
            ..self
        }
    }
}

fn clamp_unit(value: f32, low: f32, high: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        low
    }
}

/// One configured visualizer: a kind, a name, and the parameters it was tuned with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerConfiguration {
    pub kind: VisualizerKind,
    /// What an operator calls this configuration. Defaults to the kind's label.
    pub name: String,
    pub parameters: VisualizerParameters,
}

impl VisualizerConfiguration {
    pub fn new(kind: VisualizerKind) -> Self {
        Self {
            kind,
            name: kind.label().to_owned(),
            parameters: VisualizerParameters::default(),
        }
    }
}

/// One address a configured visualizer answers at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedEntry {
    pub address: MediaAddress,
    pub configuration: VisualizerConfiguration,
}

/// Why a generated-source catalog edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GeneratedCatalogError {
    #[error("that address is not in the generated-source range")]
    NotGeneratedSpace,
    #[error("another visualizer already answers at that address")]
    AddressTaken,
}

/// Which visualizer a desk reaches at which address.
///
/// Versioned, because the assignment is data an operator's show depends on: moving a visualizer
/// must be a deliberate migration, never a side effect of shipping a new build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCatalog {
    pub version: u32,
    pub entries: Vec<GeneratedEntry>,
}

pub const CATALOG_VERSION: u32 = 1;

impl Default for GeneratedCatalog {
    /// Every shipped visualizer, in family order, in the first bank.
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            entries: ALL_KINDS
                .into_iter()
                .enumerate()
                .map(|(index, kind)| GeneratedEntry {
                    // Files start at one: file zero is blank in every bank.
                    address: MediaAddress::new(DEFAULT_BANK, index as u8 + 1),
                    configuration: VisualizerConfiguration::new(kind),
                })
                .collect(),
        }
    }
}

impl GeneratedCatalog {
    pub fn resolve(&self, address: MediaAddress) -> Option<&VisualizerConfiguration> {
        self.entries
            .iter()
            .find(|entry| entry.address == address)
            .map(|entry| &entry.configuration)
    }

    pub fn address_of(&self, kind: VisualizerKind) -> Option<MediaAddress> {
        self.entries
            .iter()
            .find(|entry| entry.configuration.kind == kind)
            .map(|entry| entry.address)
    }

    /// Adds a configuration at an address, or says why it cannot.
    pub fn assign(
        &mut self,
        address: MediaAddress,
        configuration: VisualizerConfiguration,
    ) -> Result<(), GeneratedCatalogError> {
        if address.classify() != AddressClass::GeneratedVisualizer {
            return Err(GeneratedCatalogError::NotGeneratedSpace);
        }
        if self.resolve(address).is_some() {
            return Err(GeneratedCatalogError::AddressTaken);
        }
        self.entries.push(GeneratedEntry {
            address,
            configuration,
        });
        self.entries.sort_by_key(|entry| entry.address_key());
        Ok(())
    }

    pub fn remove(&mut self, address: MediaAddress) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.address != address);
        self.entries.len() != before
    }
}

impl GeneratedEntry {
    const fn address_key(&self) -> (u8, u8) {
        (self.address.folder, self.address.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_visualizer_is_addressable_and_none_share_an_address() {
        let catalog = GeneratedCatalog::default();
        assert_eq!(catalog.entries.len(), ALL_KINDS.len());

        let mut seen = std::collections::HashSet::new();
        for entry in &catalog.entries {
            assert_eq!(
                entry.address.classify(),
                AddressClass::GeneratedVisualizer,
                "{} landed outside the generated-source range",
                entry.configuration.kind.label()
            );
            assert!(
                seen.insert(entry.address),
                "two visualizers answer at {}",
                entry.address
            );
        }
    }

    #[test]
    fn no_shipped_visualizer_lands_on_a_blank_sentinel() {
        for entry in GeneratedCatalog::default().entries {
            assert!(
                !entry.address.is_blank(),
                "{} is unreachable: {} is a blank sentinel",
                entry.configuration.kind.label(),
                entry.address
            );
        }
    }

    #[test]
    fn internal_type_ids_are_stable_and_round_trip() {
        for kind in ALL_KINDS {
            assert_eq!(VisualizerKind::from_type_id(kind.type_id()), Some(kind));
        }
        // The documented ids, spot-checked at each family boundary.
        assert_eq!(VisualizerKind::EqualizerBars.type_id(), 0);
        assert_eq!(VisualizerKind::PulsingCircles.type_id(), 10);
        assert_eq!(VisualizerKind::BeatExplosions.type_id(), 20);
        assert_eq!(VisualizerKind::RadiatingRays.type_id(), 30);
        assert_eq!(VisualizerKind::DigitalGlitch.type_id(), 40);
        assert_eq!(VisualizerKind::FractalMorph.type_id(), 51);
        assert_eq!(VisualizerKind::from_type_id(999), None);
    }

    #[test]
    fn every_kind_declares_at_least_one_parameter_and_names_no_duplicate() {
        for kind in ALL_KINDS {
            let parameters = kind.parameters();
            assert!(
                !parameters.is_empty(),
                "{} offers an operator nothing to change",
                kind.label()
            );
            let unique: std::collections::HashSet<_> = parameters.iter().collect();
            assert_eq!(
                unique.len(),
                parameters.len(),
                "{} repeats a parameter",
                kind.label()
            );
        }
    }

    #[test]
    fn an_edited_configuration_file_cannot_produce_an_unusable_visualizer() {
        let absurd = VisualizerParameters {
            count: 100_000,
            size: -3.0,
            zoom: f32::NAN,
            iterations: 0,
            ..Default::default()
        }
        .clamped();

        assert_eq!(absurd.count, 512);
        assert!(absurd.size > 0.0);
        assert!(absurd.zoom.is_finite());
        assert_eq!(absurd.iterations, 1);
    }

    #[test]
    fn a_visualizer_may_only_be_assigned_inside_the_generated_range() {
        let mut catalog = GeneratedCatalog::default();
        let configuration = VisualizerConfiguration::new(VisualizerKind::Starfield);

        assert_eq!(
            catalog.assign(MediaAddress::new(1, 5), configuration.clone()),
            Err(GeneratedCatalogError::NotGeneratedSpace),
            "a library address is not generated space"
        );
        assert_eq!(
            catalog.assign(MediaAddress::new(200, 5), configuration.clone()),
            Err(GeneratedCatalogError::NotGeneratedSpace),
            "the text bank is not generated space"
        );
        assert_eq!(
            catalog.assign(MediaAddress::new(DEFAULT_BANK, 1), configuration.clone()),
            Err(GeneratedCatalogError::AddressTaken)
        );

        let free = MediaAddress::new(221, 7);
        assert_eq!(catalog.assign(free, configuration), Ok(()));
        assert_eq!(
            catalog.resolve(free).map(|found| found.kind),
            Some(VisualizerKind::Starfield)
        );
    }

    #[test]
    fn removing_an_assignment_makes_the_address_answer_nothing() {
        let mut catalog = GeneratedCatalog::default();
        let address = catalog
            .address_of(VisualizerKind::Kaleidoscope)
            .expect("shipped");

        assert!(catalog.remove(address));
        assert_eq!(catalog.resolve(address), None);
        assert!(!catalog.remove(address), "removing twice is not a change");
    }
}
