#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnImageParameters {
    pub strength: f32,
    pub line_detail: f32,
}

impl DrawnImageParameters {
    pub const IDS: [&'static str; 2] = ["drawn-strength", "drawn-line-detail"];
    pub const LABELS: [&'static str; 2] = ["Stylization strength", "Line detail"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32| match value {
            Some(value) if value.is_finite() => value.clamp(0.0, 1.0),
            _ => fallback,
        };
        Self {
            strength: bounded(values.first().copied(), defaults.strength),
            line_detail: bounded(values.get(1).copied(), defaults.line_detail),
        }
    }

    pub const fn as_array(self) -> [f32; 2] {
        [self.strength, self.line_detail]
    }
}

impl Default for DrawnImageParameters {
    fn default() -> Self {
        Self {
            strength: 0.8,
            line_detail: 0.55,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatFormFlashParameters {
    pub enlargement: f32,
    pub lifetime_seconds: f32,
    pub density: u8,
    pub variation: f32,
}

impl BeatFormFlashParameters {
    pub const IDS: [&'static str; 4] = [
        "beat-form-enlargement",
        "beat-form-lifetime",
        "beat-form-density",
        "beat-form-variation",
    ];
    pub const LABELS: [&'static str; 4] = ["Start size", "Lifetime", "Forms per beat", "Variation"];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32, low: f32, high: f32| match value {
            Some(value) if value.is_finite() => value.clamp(low, high),
            _ => fallback,
        };
        Self {
            enlargement: bounded(values.first().copied(), defaults.enlargement, 1.0, 4.0),
            lifetime_seconds: bounded(values.get(1).copied(), defaults.lifetime_seconds, 0.1, 5.0),
            density: bounded(
                values.get(2).copied(),
                f32::from(defaults.density),
                1.0,
                4.0,
            )
            .round() as u8,
            variation: bounded(values.get(3).copied(), defaults.variation, 0.0, 1.0),
        }
    }

    pub const fn as_array(self) -> [f32; 4] {
        [
            self.enlargement,
            self.lifetime_seconds,
            self.density as f32,
            self.variation,
        ]
    }
}

impl Default for BeatFormFlashParameters {
    fn default() -> Self {
        Self {
            enlargement: 1.6,
            lifetime_seconds: 0.9,
            density: 1,
            variation: 0.35,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BeatGridWaveOrigin {
    #[default]
    Centre,
    Top,
    Right,
    Bottom,
    Left,
}

impl BeatGridWaveOrigin {
    pub const ALL: [Self; 5] = [
        Self::Centre,
        Self::Top,
        Self::Right,
        Self::Bottom,
        Self::Left,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Centre => "centre",
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|origin| origin.wire_name() == value)
    }

    pub const fn parameter(self) -> f32 {
        match self {
            Self::Centre => 0.0,
            Self::Top => 1.0,
            Self::Right => 2.0,
            Self::Bottom => 3.0,
            Self::Left => 4.0,
        }
    }

    pub fn from_parameter(value: f32) -> Self {
        match value.round() as i32 {
            1 => Self::Top,
            2 => Self::Right,
            3 => Self::Bottom,
            4 => Self::Left,
            _ => Self::Centre,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGridWaveParameters {
    pub density: f32,
    pub height: f32,
    pub duration_seconds: f32,
    pub origin: BeatGridWaveOrigin,
    pub hue_degrees: f32,
    pub brightness: f32,
}

impl BeatGridWaveParameters {
    pub const IDS: [&'static str; 6] = [
        "beat-grid-density",
        "beat-grid-height",
        "beat-grid-duration",
        "beat-grid-origin",
        "beat-grid-hue",
        "beat-grid-brightness",
    ];
    pub const LABELS: [&'static str; 6] = [
        "Grid density",
        "Wave height",
        "Travel time",
        "Wave origin",
        "Grid hue",
        "Brightness",
    ];

    pub fn from_parameters(values: &[f32]) -> Self {
        let defaults = Self::default();
        let bounded = |value: Option<f32>, fallback: f32, low: f32, high: f32| match value {
            Some(value) if value.is_finite() => value.clamp(low, high),
            _ => fallback,
        };
        Self {
            density: bounded(values.first().copied(), defaults.density, 6.0, 64.0),
            height: bounded(values.get(1).copied(), defaults.height, 0.0, 1.0),
            duration_seconds: bounded(values.get(2).copied(), defaults.duration_seconds, 0.2, 4.0),
            origin: BeatGridWaveOrigin::from_parameter(values.get(3).copied().unwrap_or_default()),
            hue_degrees: bounded(values.get(4).copied(), defaults.hue_degrees, 0.0, 360.0),
            brightness: bounded(values.get(5).copied(), defaults.brightness, 0.1, 2.0),
        }
    }

    pub const fn as_array(self) -> [f32; 6] {
        [
            self.density,
            self.height,
            self.duration_seconds,
            self.origin.parameter(),
            self.hue_degrees,
            self.brightness,
        ]
    }
}

impl Default for BeatGridWaveParameters {
    fn default() -> Self {
        Self {
            density: 24.0,
            height: 0.5,
            duration_seconds: 1.2,
            origin: BeatGridWaveOrigin::Centre,
            hue_degrees: 190.0,
            brightness: 1.0,
        }
    }
}
