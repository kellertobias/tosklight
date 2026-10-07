//! Synthetic TL-558 destination profiles with authored Focus/Zoom function curves. Curves mirror
//! the TL-566 fitter references; they are not lamp calibrations.
use light_core::{AttributeKey, OpeningConvention};
use light_fixture::*;
use uuid::Uuid;

pub(in crate::runtime) use super::super::color::profiles::patched;

/// One continuous optics function over `from..=to`, optionally with a calibration.
#[derive(Clone)]
pub(in crate::runtime) struct Curve {
    pub from: u32,
    pub to: u32,
    /// Physical value at `from` and at `to` (descending = reversed curve).
    pub physical: (f32, f32),
    pub unit: Option<&'static str>,
    /// Interior samples (endpoints are added); None = no authored calibration at all.
    pub samples: Option<Vec<(u32, f32)>>,
    pub quality: PhysicalDataQuality,
    pub convention: Option<OpeningConvention>,
}

impl Curve {
    pub fn zoom(from: u32, to: u32, physical: (f32, f32), interior: &[(u32, f32)]) -> Self {
        Self {
            from,
            to,
            physical,
            unit: Some("deg"),
            samples: Some(interior.to_vec()),
            quality: PhysicalDataQuality::Measured,
            convention: Some(OpeningConvention::Field),
        }
    }

    pub fn focus_percent(from: u32, to: u32, physical: (f32, f32)) -> Self {
        Self {
            from,
            to,
            physical,
            unit: Some("%"),
            samples: Some(vec![]),
            quality: PhysicalDataQuality::Measured,
            convention: None,
        }
    }

    /// Native travel only: no unit and no calibration.
    pub fn nominal(from: u32, to: u32) -> Self {
        Self {
            from,
            to,
            physical: (0., 1.),
            unit: None,
            samples: None,
            quality: PhysicalDataQuality::Unknown,
            convention: None,
        }
    }

    fn function(&self, attribute: &AttributeKey) -> ChannelFunction {
        let mut function =
            ChannelFunction::continuous(attribute.0.to_string(), attribute.clone(), self.to);
        function.dmx_from = self.from;
        function.dmx_to = self.to;
        function.behavior = ChannelFunctionBehavior::Continuous {
            physical_min: self.physical.0,
            physical_max: self.physical.1,
            unit: self.unit.map(Into::into),
        };
        function.physical_mapping = self.samples.as_ref().map(|interior| {
            let mut points = vec![PhysicalMappingPoint {
                raw: self.from,
                physical: self.physical.0,
            }];
            points.extend(
                interior
                    .iter()
                    .map(|&(raw, physical)| PhysicalMappingPoint { raw, physical }),
            );
            points.push(PhysicalMappingPoint {
                raw: self.to,
                physical: self.physical.1,
            });
            PhysicalMappingCalibration {
                quality: self.quality,
                source: Some("Synthetic TL-558 reference".into()),
                opening_convention: self.convention,
                samples: if interior.is_empty() { vec![] } else { points },
                ..Default::default()
            }
        });
        function
    }
}

fn channel(
    head: Uuid,
    attribute: &str,
    resolution: ChannelResolution,
    slot: u16,
) -> FixtureChannel {
    let max = resolution.max_raw();
    let attribute = AttributeKey(attribute.into());
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute: attribute.clone(),
        canonical_transform: CanonicalTransform::Identity,
        resolution,
        secondary_slots: (1..resolution.bytes() as u16).map(|i| slot + i).collect(),
        default_raw: 0,
        highlight_raw: max,
        physical_min: Some(0.0),
        physical_max: Some(1.0),
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            attribute.0.to_string(),
            attribute,
            max,
        )],
    }
}

/// Channel 0 is an Intensity channel that optics fitting must never touch.
pub(in crate::runtime) struct OpticsBuilder {
    profile: FixtureProfile,
    slots: u16,
}

impl OpticsBuilder {
    pub fn new(name: &str) -> Self {
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = name.into();
        profile.revision = 1;
        let head = profile.modes[0].heads[0].id;
        profile.modes[0].channels = vec![channel(head, "intensity", ChannelResolution::U8, 1)];
        Self { profile, slots: 1 }
    }

    /// A channel whose first function follows `curve`; extra functions are appended as given.
    pub fn optics(
        mut self,
        attribute: &str,
        resolution: ChannelResolution,
        curve: Curve,
        extra: Vec<ChannelFunction>,
    ) -> Self {
        let head = self.profile.modes[0].heads[0].id;
        let mut value = channel(head, attribute, resolution, self.slots + 1);
        value.functions = vec![curve.function(&value.attribute)];
        value.functions.extend(extra);
        self.slots += resolution.bytes() as u16;
        self.profile.modes[0].channels.push(value);
        self
    }

    pub fn zoom(self, resolution: ChannelResolution, curve: Curve) -> Self {
        self.optics("zoom", resolution, curve, vec![])
    }

    pub fn focus(self, resolution: ChannelResolution, curve: Curve) -> Self {
        self.optics("focus", resolution, curve, vec![])
    }

    pub fn build(self) -> FixtureProfile {
        let mut profile = self.profile;
        profile.modes[0].splits[0].footprint = self.slots;
        profile.validate().unwrap();
        profile
    }
}

/// Wash A: reversed nonlinear U16 Field zoom 50° → 20° → 5°, measured reversed U8 Focus
/// 100% → 0% on 10..=200 (the TL-566 reference layout).
pub(in crate::runtime) fn wash_a() -> FixtureProfile {
    OpticsBuilder::new("TL-558 wash A")
        .zoom(
            ChannelResolution::U16,
            Curve::zoom(0, 65535, (50., 5.), &[(32768, 20.)]),
        )
        .focus(
            ChannelResolution::U8,
            Curve::focus_percent(10, 200, (100., 0.)),
        )
        .build()
}

/// Spot B: ascending nonlinear U8 Field zoom 8° → 20° → 40°, nominal U16 Focus travel.
pub(in crate::runtime) fn spot_b() -> FixtureProfile {
    OpticsBuilder::new("TL-558 spot B")
        .zoom(
            ChannelResolution::U8,
            Curve::zoom(0, 255, (8., 40.), &[(128, 20.)]),
        )
        .focus(ChannelResolution::U16, Curve::nominal(0, 65535))
        .build()
}

/// Profile of one zoom channel at `resolution`: 60° → 20° → 8° (descending) or the reverse,
/// inside `3..=max-5`, Beam convention.
pub(in crate::runtime) fn swept_zoom(
    resolution: ChannelResolution,
    descending: bool,
) -> (FixtureProfile, (u32, u32)) {
    let (from, to) = (3, resolution.max_raw() - 5);
    let third = from + (to - from) / 3;
    let (a, b, c) = if descending {
        (60., 20., 8.)
    } else {
        (8., 20., 60.)
    };
    let mut curve = Curve::zoom(from, to, (a, c), &[(third, b)]);
    curve.quality = PhysicalDataQuality::Manufacturer;
    curve.convention = Some(OpeningConvention::Beam);
    let profile = OpticsBuilder::new(&format!("TL-558 zoom {resolution:?} {descending}"))
        .zoom(resolution, curve)
        .build();
    (profile, (from, to))
}

/// U8 zoom: Field 40° → 10° on 0..=99, a fixed macro on 100..=127, Field 20° → 60° on 128..=255.
pub(in crate::runtime) fn multi_function_zoom() -> FixtureProfile {
    let attribute = AttributeKey("zoom".into());
    let mut macro_fn = ChannelFunction::continuous("Macro", attribute.clone(), 127);
    macro_fn.dmx_from = 100;
    macro_fn.behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "zoom_macro".into(),
        label: "Macro".into(),
        raw_value: 100,
    };
    let wide = Curve::zoom(128, 255, (20., 60.), &[]).function(&attribute);
    OpticsBuilder::new("TL-558 multi-function zoom")
        .optics(
            "zoom",
            ChannelResolution::U8,
            Curve::zoom(0, 99, (40., 10.), &[]),
            vec![macro_fn, wide],
        )
        .focus(
            ChannelResolution::U8,
            Curve::focus_percent(0, 255, (0., 100.)),
        )
        .build()
}

/// One U8 channel carrying Focus on 0..=127 and Zoom on 128..=255: writing either family would
/// move the other, so neither is independently drivable.
pub(in crate::runtime) fn shared_focus_zoom_channel() -> FixtureProfile {
    let zoom = Curve::zoom(128, 255, (10., 40.), &[]).function(&AttributeKey("zoom".into()));
    OpticsBuilder::new("TL-558 shared channel")
        .optics(
            "focus",
            ChannelResolution::U8,
            Curve::focus_percent(0, 127, (0., 100.)),
            vec![zoom],
        )
        .build()
}

/// Wash A plus a second, non-master head with its own U8 Focus. The second head has no Zoom
/// control of its own, so it inherits the master-shared head's U16 Zoom: one native control,
/// two programming targets.
pub(in crate::runtime) fn two_head_shared_zoom() -> FixtureProfile {
    let mut profile = OpticsBuilder::new("TL-558 two heads, shared zoom")
        .zoom(
            ChannelResolution::U16,
            Curve::zoom(0, 65535, (50., 5.), &[(32768, 20.)]),
        )
        .focus(
            ChannelResolution::U8,
            Curve::focus_percent(10, 200, (100., 0.)),
        )
        .build();
    let mode = &mut profile.modes[0];
    let second = FixtureHead {
        id: Uuid::new_v4(),
        name: "Second".into(),
        master_shared: false,
    };
    let mut focus = mode.channels[2].clone();
    focus.id = Uuid::new_v4();
    focus.functions[0].id = Uuid::new_v4();
    focus.head_id = second.id;
    focus.secondary_slots.clear();
    mode.heads.push(second);
    mode.channels.push(focus);
    mode.splits[0].footprint += 1;
    profile.validate().unwrap();
    profile
}
